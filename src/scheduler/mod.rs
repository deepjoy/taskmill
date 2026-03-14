//! The scheduler: configuration, event stream, and the main run loop.
//!
//! [`Scheduler`] coordinates task execution — popping from the
//! [`TaskStore`], applying [backpressure](crate::backpressure),
//! IO-budget checks, and [group concurrency](crate::GroupLimits) limits,
//! preempting lower-priority work, and emitting [`SchedulerEvent`]s for UI
//! integration. Use [`SchedulerBuilder`] for ergonomic construction.
//!
//! See the [crate-level docs](crate) for a full walkthrough of the task
//! lifecycle, common patterns, and how the dispatch loop works.

mod builder;
pub(crate) mod dispatch;
pub(crate) mod event;
pub(crate) mod gate;
pub mod progress;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Arc;

use tokio::sync::{Mutex, Notify};
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::backpressure::{CompositePressure, ThrottlePolicy};
use crate::priority::Priority;
use crate::registry::TaskTypeRegistry;
use crate::resource::ResourceReader;
use crate::store::{StoreError, TaskStore};
use crate::task::{
    generate_dedup_key, SubmitOutcome, TaskLookup, TaskMetrics, TaskSubmission, TypedTask,
};

use dispatch::ActiveTaskMap;
use gate::GateContext;

pub use builder::SchedulerBuilder;
pub use event::{SchedulerConfig, SchedulerEvent, SchedulerSnapshot, ShutdownMode};
pub use gate::GroupLimits;
pub use progress::{EstimatedProgress, ProgressReporter};

// ── Scheduler ───────────────────────────────────────────────────────

/// Shared inner state behind `Arc` so `Scheduler` can be `Clone`.
#[allow(dead_code)]
struct SchedulerInner {
    store: TaskStore,
    max_concurrency: AtomicUsize,
    max_retries: i32,
    preempt_priority: Priority,
    poll_interval: Duration,
    throughput_sample_size: i32,
    shutdown_mode: ShutdownMode,
    registry: Arc<TaskTypeRegistry>,
    gate: Box<dyn gate::DispatchGate>,
    resource_reader: Mutex<Option<Arc<dyn ResourceReader>>>,
    /// In-memory tracking of active tasks and their cancellation tokens.
    active: ActiveTaskMap,
    /// Broadcast channel for lifecycle events.
    event_tx: tokio::sync::broadcast::Sender<SchedulerEvent>,
    /// Token to cancel the background resource sampler (if started).
    sampler_token: CancellationToken,
    /// Type-keyed application state passed to every executor via [`TaskContext::state`].
    app_state: Arc<crate::registry::StateMap>,
    /// Global pause flag — when `true`, the run loop skips dispatching.
    paused: AtomicBool,
    /// Wakes the run loop when new work is submitted or the scheduler is resumed.
    work_notify: Arc<Notify>,
    /// Per-group concurrency limits.
    group_limits: GroupLimits,
}

/// IO-aware priority scheduler.
///
/// Coordinates task execution by:
/// 1. Popping highest-priority pending tasks from the SQLite store
/// 2. Checking IO budget against running task estimates and system capacity
/// 3. Applying backpressure throttling based on external pressure sources
/// 4. Preempting lower-priority tasks when high-priority work arrives
/// 5. Managing retries and failure recording
/// 6. Emitting lifecycle events for UI integration
///
/// `Scheduler` is `Clone` — each clone shares the same underlying state.
/// This makes it easy to hold in `tauri::State<Scheduler>` or share across
/// async tasks.
#[derive(Clone)]
pub struct Scheduler {
    inner: Arc<SchedulerInner>,
}

/// Weak handle to a [`Scheduler`] that does not prevent shutdown.
///
/// Used inside [`TaskContext`](crate::TaskContext) to avoid keeping the
/// scheduler alive via a strong `Arc` cycle. Upgrade to a full `Scheduler`
/// before use — the upgrade fails if the scheduler has already been dropped.
#[derive(Clone)]
pub(crate) struct WeakScheduler {
    inner: std::sync::Weak<SchedulerInner>,
}

impl WeakScheduler {
    /// Attempt to upgrade to a full [`Scheduler`].
    ///
    /// Returns `None` if the scheduler has been dropped.
    pub fn upgrade(&self) -> Option<Scheduler> {
        self.inner.upgrade().map(|inner| Scheduler { inner })
    }
}

impl Scheduler {
    /// Create a weak handle that does not prevent scheduler shutdown.
    pub(crate) fn downgrade(&self) -> WeakScheduler {
        WeakScheduler {
            inner: Arc::downgrade(&self.inner),
        }
    }

    pub fn new(
        store: TaskStore,
        config: SchedulerConfig,
        registry: Arc<TaskTypeRegistry>,
        pressure: CompositePressure,
        policy: ThrottlePolicy,
    ) -> Self {
        let gate = Box::new(gate::DefaultDispatchGate::new(pressure, policy));
        Self::with_gate(
            store,
            config,
            registry,
            gate,
            Arc::new(crate::registry::StateMap::new()),
        )
    }

    /// Create a scheduler with a custom dispatch gate.
    pub(crate) fn with_gate(
        store: TaskStore,
        config: SchedulerConfig,
        registry: Arc<TaskTypeRegistry>,
        gate: Box<dyn gate::DispatchGate>,
        app_state: Arc<crate::registry::StateMap>,
    ) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(256);
        Self {
            inner: Arc::new(SchedulerInner {
                store,
                max_concurrency: AtomicUsize::new(config.max_concurrency),
                max_retries: config.max_retries,
                preempt_priority: config.preempt_priority,
                poll_interval: config.poll_interval,
                throughput_sample_size: config.throughput_sample_size,
                shutdown_mode: config.shutdown_mode,
                registry,
                gate,
                resource_reader: Mutex::new(None),
                active: ActiveTaskMap::new(),
                event_tx,
                sampler_token: CancellationToken::new(),
                app_state,
                paused: AtomicBool::new(false),
                work_notify: Arc::new(Notify::new()),
                group_limits: GroupLimits::new(),
            }),
        }
    }

    /// Create a [`SchedulerBuilder`] for ergonomic construction.
    pub fn builder() -> SchedulerBuilder {
        SchedulerBuilder::new()
    }

    /// Subscribe to scheduler lifecycle events.
    ///
    /// Returns a broadcast receiver. Events are emitted on task dispatch,
    /// completion, failure, preemption, cancellation, and progress. Useful for
    /// bridging to a Tauri frontend or updating UI state.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<SchedulerEvent> {
        self.inner.event_tx.subscribe()
    }

    /// Set the resource reader for IO-aware scheduling.
    pub async fn set_resource_reader(&self, reader: Arc<dyn ResourceReader>) {
        *self.inner.resource_reader.lock().await = Some(reader);
    }

    /// Get a reference to the underlying store for direct queries.
    pub fn store(&self) -> &TaskStore {
        &self.inner.store
    }

    /// Register shared application state after the scheduler has been built.
    ///
    /// This is useful when library code (e.g. shoebox) needs to inject its
    /// own state into a scheduler that was constructed by a parent
    /// application. Multiple types can coexist — each is keyed by `TypeId`.
    pub async fn register_state<T: Send + Sync + 'static>(&self, state: Arc<T>) {
        self.inner.app_state.insert(state).await;
    }

    /// Submit a task.
    ///
    /// If the task's priority meets the preemption threshold, running tasks
    /// with lower priority are preempted (their cancellation tokens are cancelled
    /// and they are paused in the store).
    pub async fn submit(&self, sub: &TaskSubmission) -> Result<SubmitOutcome, StoreError> {
        let outcome = self.inner.store.submit(sub).await?;

        if !matches!(outcome, SubmitOutcome::Duplicate) {
            // Preempt if this is a high-priority task.
            if sub.priority.value() <= self.inner.preempt_priority.value() {
                self.inner
                    .active
                    .preempt_below(sub.priority, &self.inner.store, &self.inner.event_tx)
                    .await;
            }

            // Wake the scheduler loop so it picks up the new/upgraded task.
            self.inner.work_notify.notify_one();
        }

        Ok(outcome)
    }

    /// Submit multiple tasks in a single SQLite transaction.
    ///
    /// Preemption is triggered once at the end if any inserted or upgraded
    /// task has high enough priority.
    pub async fn submit_batch(
        &self,
        submissions: &[TaskSubmission],
    ) -> Result<Vec<SubmitOutcome>, StoreError> {
        let results = self.inner.store.submit_batch(submissions).await?;

        // Find the highest (lowest numeric value) priority among tasks that
        // were inserted or had their priority upgraded.
        let best_priority = submissions
            .iter()
            .zip(results.iter())
            .filter(|(_, outcome)| !matches!(outcome, SubmitOutcome::Duplicate))
            .map(|(sub, _)| sub.priority)
            .min_by_key(|p| p.value());

        let any_changed = results
            .iter()
            .any(|o| !matches!(o, SubmitOutcome::Duplicate));

        if let Some(priority) = best_priority {
            if priority.value() <= self.inner.preempt_priority.value() {
                self.inner
                    .active
                    .preempt_below(priority, &self.inner.store, &self.inner.event_tx)
                    .await;
            }
        }

        if any_changed {
            self.inner.work_notify.notify_one();
        }

        Ok(results)
    }

    /// Submit a [`TypedTask`], handling serialization automatically.
    ///
    /// Uses the priority from [`TypedTask::priority()`].
    pub async fn submit_typed<T: TypedTask>(&self, task: &T) -> Result<SubmitOutcome, StoreError> {
        let sub = TaskSubmission::from_typed(task)?;
        self.submit(&sub).await
    }

    /// Submit a [`TypedTask`] with an explicit priority override.
    ///
    /// The provided `priority` replaces whatever [`TypedTask::priority()`]
    /// would return, keeping priority out of the serialized payload.
    pub async fn submit_typed_at<T: TypedTask>(
        &self,
        task: &T,
        priority: Priority,
    ) -> Result<SubmitOutcome, StoreError> {
        let mut sub = TaskSubmission::from_typed(task)?;
        sub.priority = priority;
        self.submit(&sub).await
    }

    /// Look up a task by the same inputs used during submission.
    ///
    /// Computes the dedup key from `task_type` and `dedup_input` (the
    /// explicit key string or payload bytes — whichever was used when
    /// submitting), then checks the active queue and history in one call.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Using an explicit key (same as TaskSubmission.key = Some("my-file.jpg"))
    /// let result = scheduler.task_lookup("thumbnail", Some(b"my-file.jpg")).await?;
    ///
    /// // Using payload-based dedup (same as TaskSubmission.key = None, payload = ...)
    /// let result = scheduler.task_lookup("ingest", Some(&payload_bytes)).await?;
    /// ```
    pub async fn task_lookup(
        &self,
        task_type: &str,
        dedup_input: Option<&[u8]>,
    ) -> Result<TaskLookup, StoreError> {
        let key = generate_dedup_key(task_type, dedup_input);
        self.inner.store.task_lookup(&key).await
    }

    /// Look up a [`TypedTask`] by value, using its serialized form as the
    /// dedup input.
    ///
    /// This mirrors [`submit_typed`](Self::submit_typed) — pass the same
    /// struct you would submit and get back its current status.
    pub async fn lookup_typed<T: TypedTask>(&self, task: &T) -> Result<TaskLookup, StoreError> {
        let payload = serde_json::to_vec(task)?;
        let key = generate_dedup_key(T::TASK_TYPE, Some(&payload));
        self.inner.store.task_lookup(&key).await
    }

    /// Cancel a task by id.
    ///
    /// If the task is currently running, its cancellation token is triggered
    /// and it is removed from the active map. If it is pending or paused,
    /// it is deleted from the store. Returns `true` if the task was found
    /// and cancelled.
    pub async fn cancel(&self, task_id: i64) -> Result<bool, StoreError> {
        // Cancel children first (cascade).
        let running_child_ids = self.inner.store.cancel_children(task_id).await?;
        for child_id in &running_child_ids {
            if let Some(at) = self.inner.active.remove(*child_id) {
                at.token.cancel();
                let _ = self.inner.store.delete(*child_id).await;
                let _ = self.inner.event_tx.send(SchedulerEvent::Cancelled {
                    task_id: *child_id,
                    task_type: at.record.task_type.clone(),
                    key: at.record.key.clone(),
                    label: at.record.label.clone(),
                });
            }
        }

        // Check if it's an active (running) task first.
        if let Some(at) = self.inner.active.remove(task_id) {
            at.token.cancel();
            self.inner.store.delete(task_id).await?;
            let _ = self.inner.event_tx.send(SchedulerEvent::Cancelled {
                task_id,
                task_type: at.record.task_type.clone(),
                key: at.record.key.clone(),
                label: at.record.label.clone(),
            });
            return Ok(true);
        }

        // Not active — try to delete from the queue (pending/paused/waiting).
        let deleted = self.inner.store.delete(task_id).await?;
        Ok(deleted)
    }

    /// Try to pop and execute the next task.
    ///
    /// Returns `true` if a task was dispatched, `false` if no work was available
    /// (empty queue, concurrency limit, IO budget exhausted, or throttled).
    pub async fn try_dispatch(&self) -> Result<bool, StoreError> {
        // Check concurrency limit.
        let active_count = self.inner.active.count();
        let max = self.inner.max_concurrency.load(AtomicOrdering::Relaxed);
        if active_count >= max {
            return Ok(false);
        }

        // Peek at the next candidate without changing its status.
        let Some(candidate) = self.inner.store.peek_next().await? else {
            return Ok(false);
        };

        // Build gate context from current state.
        let reader_guard = self.inner.resource_reader.lock().await;
        let gate_ctx = GateContext {
            store: &self.inner.store,
            resource_reader: reader_guard.as_ref(),
            group_limits: Some(&self.inner.group_limits),
        };

        // Admission check while the task is still pending — no running
        // window if the gate rejects.
        if !self.inner.gate.admit(&candidate, &gate_ctx).await? {
            drop(reader_guard);
            return Ok(false);
        }
        drop(reader_guard);

        // Atomically claim the task. Returns None if another dispatcher
        // claimed it (or it was cancelled) between peek and now.
        let Some(task) = self.inner.store.pop_by_id(candidate.id).await? else {
            return Ok(false);
        };

        // Look up executor.
        let Some(executor) = self.inner.registry.get(&task.task_type) else {
            tracing::error!(
                task_type = task.task_type,
                "no executor registered — failing task"
            );
            self.inner
                .store
                .fail(
                    task.id,
                    &format!("no executor registered for type '{}'", task.task_type),
                    false,
                    0,
                    &TaskMetrics::default(),
                )
                .await?;
            return Ok(true);
        };
        let executor = Arc::clone(executor);

        // Spawn the task — this inserts into the active map, builds the
        // context, emits Dispatched, and wires up completion handling.
        dispatch::spawn_task(
            task,
            executor,
            dispatch::SpawnContext {
                store: self.inner.store.clone(),
                active: self.inner.active.clone(),
                event_tx: self.inner.event_tx.clone(),
                max_retries: self.inner.max_retries,
                app_state: self.inner.app_state.snapshot().await,
                work_notify: Arc::clone(&self.inner.work_notify),
                scheduler: self.downgrade(),
            },
            dispatch::ExecutionPhase::Execute,
        )
        .await;

        Ok(true)
    }

    /// Try to dispatch a parent task for its finalize phase.
    ///
    /// Returns `true` if a finalizer was dispatched.
    async fn try_dispatch_finalizer(&self) -> Result<bool, StoreError> {
        // Pop the next pending finalizer.
        let parent_id = {
            let mut finalizers = self.inner.active.pending_finalizers.lock().unwrap();
            let Some(&id) = finalizers.iter().next() else {
                return Ok(false);
            };
            finalizers.remove(&id);
            id
        };

        // Transition the parent from waiting to running for finalize.
        self.inner.store.set_running_for_finalize(parent_id).await?;

        // Fetch the parent record (now running).
        let Some(task) = self.inner.store.task_by_id(parent_id).await? else {
            return Ok(false);
        };

        // Look up executor.
        let Some(executor) = self.inner.registry.get(&task.task_type) else {
            tracing::error!(
                task_type = task.task_type,
                "no executor registered for finalize — failing parent"
            );
            self.inner
                .store
                .fail(
                    parent_id,
                    "no executor for finalize",
                    false,
                    0,
                    &TaskMetrics::default(),
                )
                .await?;
            return Ok(true);
        };
        let executor = Arc::clone(executor);

        dispatch::spawn_task(
            task,
            executor,
            dispatch::SpawnContext {
                store: self.inner.store.clone(),
                active: self.inner.active.clone(),
                event_tx: self.inner.event_tx.clone(),
                max_retries: self.inner.max_retries,
                app_state: self.inner.app_state.snapshot().await,
                work_notify: Arc::clone(&self.inner.work_notify),
                scheduler: self.downgrade(),
            },
            dispatch::ExecutionPhase::Finalize,
        )
        .await;

        Ok(true)
    }

    /// Run the scheduler loop until the cancellation token is triggered.
    ///
    /// This is the main entry point. The loop wakes on three conditions:
    /// 1. Cancellation — triggers shutdown.
    /// 2. Notification — a task was submitted or the scheduler was resumed.
    /// 3. Poll interval — periodic housekeeping (e.g. resuming paused tasks).
    ///
    /// On mobile targets (iOS/Android), the notify-based wake avoids the
    /// constant 500ms polling that would otherwise prevent the CPU from sleeping.
    pub async fn run(&self, token: CancellationToken) {
        tracing::info!(
            max_concurrency = self.inner.max_concurrency.load(AtomicOrdering::Relaxed),
            "taskmill scheduler started"
        );

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    tracing::info!("taskmill scheduler shutting down");
                    self.shutdown().await;
                    break;
                }
                _ = self.inner.work_notify.notified() => {
                    self.poll_and_dispatch().await;
                }
                _ = tokio::time::sleep(self.inner.poll_interval) => {
                    self.poll_and_dispatch().await;
                }
            }
        }
    }

    /// Resume paused tasks, dispatch finalizers, and dispatch pending work.
    async fn poll_and_dispatch(&self) {
        if self.is_paused() {
            return;
        }

        // Resume paused tasks only if no active preemptors exist.
        if let Ok(paused) = self.inner.store.paused_tasks().await {
            for task in paused {
                if !self
                    .inner
                    .active
                    .has_preemptors_for(task.priority, self.inner.preempt_priority)
                {
                    let _ = self.inner.store.resume(task.id).await;
                }
            }
        }

        // Dispatch any pending finalizers (parent tasks ready for finalize phase).
        loop {
            match self.try_dispatch_finalizer().await {
                Ok(true) => continue,
                Ok(false) => break,
                Err(e) => {
                    tracing::error!(error = %e, "scheduler finalizer dispatch error");
                    break;
                }
            }
        }

        // Try to dispatch tasks until we can't.
        loop {
            match self.try_dispatch().await {
                Ok(true) => continue,
                Ok(false) => break,
                Err(e) => {
                    tracing::error!(error = %e, "scheduler dispatch error");
                    break;
                }
            }
        }
    }

    /// Perform shutdown according to the configured `ShutdownMode`.
    async fn shutdown(&self) {
        // Stop the resource sampler.
        self.inner.sampler_token.cancel();

        match self.inner.shutdown_mode {
            ShutdownMode::Hard => {
                self.inner.active.cancel_all();
            }
            ShutdownMode::Graceful(timeout) => {
                tracing::info!(
                    timeout_ms = timeout.as_millis() as u64,
                    "graceful shutdown — waiting for running tasks"
                );

                // Cancel all tokens and collect handles for joining.
                let handles = self.inner.active.cancel_and_drain_handles();
                let deadline = tokio::time::Instant::now() + timeout;

                for handle in handles {
                    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                    if remaining.is_zero() {
                        tracing::warn!("graceful shutdown timeout — aborting remaining tasks");
                        handle.abort();
                        continue;
                    }
                    if tokio::time::timeout(remaining, handle).await.is_err() {
                        tracing::warn!("task did not finish within graceful shutdown timeout");
                    }
                }

                tracing::info!("graceful shutdown complete");
            }
        }

        // Flush WAL and close the database.
        self.inner.store.close().await;
    }

    /// Snapshot of currently active (in-memory) tasks.
    pub async fn active_tasks(&self) -> Vec<crate::task::TaskRecord> {
        self.inner.active.records()
    }

    /// Get estimated progress for all running tasks.
    ///
    /// Combines executor-reported progress with throughput-based extrapolation
    /// using historical average duration for each task type.
    pub async fn estimated_progress(&self) -> Vec<EstimatedProgress> {
        let snapshots: Vec<_> = self.inner.active.progress_snapshots();
        let mut results = Vec::with_capacity(snapshots.len());
        for (record, reported, reported_at) in snapshots {
            results.push(
                progress::extrapolate(&record, reported, reported_at, &self.inner.store).await,
            );
        }
        results
    }

    /// Capture a single status snapshot for dashboard UIs.
    ///
    /// Gathers running tasks, queue depths, progress estimates, and
    /// backpressure in one call — exactly what a Tauri command would
    /// return to the frontend.
    pub async fn snapshot(&self) -> Result<SchedulerSnapshot, StoreError> {
        let running = self.inner.active.records();
        let pending_count = self.inner.store.pending_count().await?;
        let paused_count = self.inner.store.paused_count().await?;
        let waiting_count = self.inner.store.waiting_count().await?;
        let progress = self.estimated_progress().await;
        let pressure = self.inner.gate.pressure().await;
        let pressure_breakdown = self.inner.gate.pressure_breakdown().await;
        let max_concurrency = self.max_concurrency();

        Ok(SchedulerSnapshot {
            running,
            pending_count,
            paused_count,
            waiting_count,
            progress,
            pressure,
            pressure_breakdown,
            max_concurrency,
            is_paused: self.is_paused(),
        })
    }

    /// Update max concurrency at runtime (e.g., from adaptive controller or
    /// in response to battery/thermal state).
    pub fn set_max_concurrency(&self, limit: usize) {
        self.inner
            .max_concurrency
            .store(limit, AtomicOrdering::Relaxed);
        tracing::info!(new_limit = limit, "concurrency limit updated");
    }

    /// Read current max concurrency setting.
    pub fn max_concurrency(&self) -> usize {
        self.inner.max_concurrency.load(AtomicOrdering::Relaxed)
    }

    /// Pause the entire scheduler.
    ///
    /// Stops the run loop from dispatching new tasks and pauses all
    /// currently running tasks (their cancellation tokens are triggered
    /// and they are moved back to the `paused` state in the store so
    /// they will be re-dispatched on resume).
    ///
    /// Useful when the app is backgrounded, the laptop goes to sleep,
    /// or the user clicks "pause all" in the UI.
    pub async fn pause_all(&self) {
        self.inner.paused.store(true, AtomicOrdering::Release);
        let count = self
            .inner
            .active
            .pause_all(&self.inner.store, &self.inner.event_tx)
            .await;
        let _ = self.inner.event_tx.send(SchedulerEvent::Paused);
        tracing::info!(paused_tasks = count, "scheduler paused");
    }

    /// Resume the scheduler after a [`pause_all`](Self::pause_all).
    ///
    /// Clears the pause flag so the run loop will resume dispatching on
    /// its next poll tick. Tasks that were paused in the store will be
    /// picked up automatically.
    pub async fn resume_all(&self) {
        self.inner.paused.store(false, AtomicOrdering::Release);
        self.inner.work_notify.notify_one();
        let _ = self.inner.event_tx.send(SchedulerEvent::Resumed);
        tracing::info!("scheduler resumed");
    }

    /// Returns `true` if the scheduler is globally paused.
    pub fn is_paused(&self) -> bool {
        self.inner.paused.load(AtomicOrdering::Acquire)
    }

    /// Set the concurrency limit for a specific task group.
    ///
    /// Tasks with a matching `group_key` will be throttled so that at most
    /// `limit` run concurrently, independent of the global concurrency cap.
    pub fn set_group_limit(&self, group: impl Into<String>, limit: usize) {
        self.inner.group_limits.set_limit(group.into(), limit);
    }

    /// Remove a per-group concurrency override, falling back to the default.
    pub fn remove_group_limit(&self, group: &str) {
        self.inner.group_limits.remove_limit(group);
    }

    /// Set the default concurrency limit for any grouped task without a
    /// specific override. `0` means unlimited.
    pub fn set_default_group_concurrency(&self, limit: usize) {
        self.inner.group_limits.set_default(limit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{TaskContext, TaskExecutor};
    use crate::task::TaskError;

    struct InstantExecutor;

    impl TaskExecutor for InstantExecutor {
        async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
            ctx.record_read_bytes(100);
            ctx.record_write_bytes(50);
            Ok(())
        }
    }

    struct SlowExecutor;

    impl TaskExecutor for SlowExecutor {
        async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
            tokio::select! {
                _ = ctx.token().cancelled() => {
                    Err(TaskError::new("cancelled"))
                }
                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                    ctx.record_read_bytes(100);
                    ctx.record_write_bytes(50);
                    Ok(())
                }
            }
        }
    }

    #[allow(dead_code)]
    struct FailingExecutor;

    impl TaskExecutor for FailingExecutor {
        async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
            Err(TaskError::retryable("boom"))
        }
    }

    async fn setup(executor: Arc<dyn crate::registry::ErasedExecutor>) -> Scheduler {
        let store = TaskStore::open_memory().await.unwrap();
        let mut registry = TaskTypeRegistry::new();
        registry.register_erased("test", executor);

        Scheduler::new(
            store,
            SchedulerConfig::default(),
            Arc::new(registry),
            CompositePressure::new(),
            ThrottlePolicy::default_three_tier(),
        )
    }

    fn arc_erased<E: TaskExecutor>(e: E) -> Arc<dyn crate::registry::ErasedExecutor> {
        Arc::new(e) as Arc<dyn crate::registry::ErasedExecutor>
    }

    #[tokio::test]
    async fn dispatch_executes_task() {
        let sched = setup(arc_erased(InstantExecutor)).await;

        sched
            .submit(&TaskSubmission::new("test").key("k1"))
            .await
            .unwrap();

        let dispatched = sched.try_dispatch().await.unwrap();
        assert!(dispatched);

        // Give spawned task time to complete.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Task should be completed and in history.
        let k1 = crate::task::generate_dedup_key("test", Some(b"k1"));
        assert!(sched.store().task_by_key(&k1).await.unwrap().is_none());
        let hist = sched.store().history_by_key(&k1).await.unwrap();
        assert_eq!(hist.len(), 1);
    }

    #[tokio::test]
    async fn dispatch_returns_false_when_empty() {
        let sched = setup(arc_erased(InstantExecutor)).await;
        let dispatched = sched.try_dispatch().await.unwrap();
        assert!(!dispatched);
    }

    #[tokio::test]
    async fn unregistered_type_fails_task() {
        let store = TaskStore::open_memory().await.unwrap();
        let registry = TaskTypeRegistry::new(); // empty — no executors

        let sched = Scheduler::new(
            store,
            SchedulerConfig::default(),
            Arc::new(registry),
            CompositePressure::new(),
            ThrottlePolicy::default_three_tier(),
        );

        sched
            .submit(&TaskSubmission::new("unknown").key("k"))
            .await
            .unwrap();

        sched.try_dispatch().await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        let failed = sched.store().failed_tasks(10).await.unwrap();
        assert_eq!(failed.len(), 1);
    }

    #[tokio::test]
    async fn dedup_via_scheduler() {
        let sched = setup(arc_erased(InstantExecutor)).await;

        let sub = TaskSubmission::new("test").key("dup");

        let first = sched.submit(&sub).await.unwrap();
        let second = sched.submit(&sub).await.unwrap();
        assert!(first.is_inserted());
        assert_eq!(second, SubmitOutcome::Duplicate);
    }

    #[tokio::test]
    async fn set_max_concurrency_works() {
        let sched = setup(arc_erased(InstantExecutor)).await;
        assert_eq!(sched.max_concurrency(), 4);
        sched.set_max_concurrency(8);
        assert_eq!(sched.max_concurrency(), 8);
    }

    #[tokio::test]
    async fn cancel_pending_task() {
        let sched = setup(arc_erased(InstantExecutor)).await;

        let id = sched
            .submit(&TaskSubmission::new("test").key("cancel-me"))
            .await
            .unwrap()
            .id()
            .unwrap();

        let cancelled = sched.cancel(id).await.unwrap();
        assert!(cancelled);

        // Task should be gone.
        let cancel_key = crate::task::generate_dedup_key("test", Some(b"cancel-me"));
        assert!(sched
            .store()
            .task_by_key(&cancel_key)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn cancel_running_task() {
        let sched = setup(arc_erased(SlowExecutor)).await;

        let id = sched
            .submit(&TaskSubmission::new("test").key("cancel-running"))
            .await
            .unwrap()
            .id()
            .unwrap();

        // Dispatch it so it's running.
        sched.try_dispatch().await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        let cancelled = sched.cancel(id).await.unwrap();
        assert!(cancelled);
    }

    #[tokio::test]
    async fn event_emitted_on_complete() {
        let sched = setup(arc_erased(InstantExecutor)).await;
        let mut rx = sched.subscribe();

        sched
            .submit(&TaskSubmission::new("test").key("evt"))
            .await
            .unwrap();

        sched.try_dispatch().await.unwrap();

        // Should get Dispatched event.
        let evt = rx.recv().await.unwrap();
        assert!(matches!(evt, SchedulerEvent::Dispatched { .. }));

        // Wait for completion.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let evt = rx.recv().await.unwrap();
        assert!(matches!(evt, SchedulerEvent::Completed { .. }));
    }

    #[tokio::test]
    async fn scheduler_is_clone() {
        let sched = setup(arc_erased(InstantExecutor)).await;
        let sched2 = sched.clone();

        // Both should share the same store.
        sched
            .submit(&TaskSubmission::new("test").key("shared"))
            .await
            .unwrap();

        // The clone can see the task.
        let shared_key = crate::task::generate_dedup_key("test", Some(b"shared"));
        let task = sched2.store().task_by_key(&shared_key).await.unwrap();
        assert!(task.is_some());
    }

    #[tokio::test]
    async fn submit_typed_enqueues_task() {
        use serde::{Deserialize as De, Serialize as Ser};

        #[derive(Ser, De, Debug, PartialEq)]
        struct Thumb {
            path: String,
        }

        impl crate::task::TypedTask for Thumb {
            const TASK_TYPE: &'static str = "test";

            fn expected_read_bytes(&self) -> i64 {
                4096
            }

            fn expected_write_bytes(&self) -> i64 {
                512
            }
        }

        let sched = setup(arc_erased(InstantExecutor)).await;

        let task = Thumb {
            path: "/a.jpg".into(),
        };
        let outcome = sched.submit_typed(&task).await.unwrap();
        assert!(outcome.is_inserted());

        // Verify the stored record has correct metadata.
        let record = sched
            .store()
            .task_by_id(outcome.id().unwrap())
            .await
            .unwrap()
            .expect("task should exist");
        assert_eq!(record.task_type, "test");
        assert_eq!(record.expected_read_bytes, 4096);
        assert_eq!(record.expected_write_bytes, 512);

        // Payload round-trips.
        let recovered: Thumb = record.deserialize_payload().unwrap().unwrap();
        assert_eq!(recovered, task);
    }

    #[tokio::test]
    async fn snapshot_returns_dashboard_state() {
        let sched = setup(arc_erased(SlowExecutor)).await;

        // Submit two tasks.
        for key in &["snap-a", "snap-b"] {
            sched
                .submit(&TaskSubmission::new("test").key(*key))
                .await
                .unwrap();
        }

        // Dispatch one so it becomes running.
        sched.try_dispatch().await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        let snap = sched.snapshot().await.unwrap();

        assert_eq!(snap.running.len(), 1);
        assert_eq!(snap.pending_count, 1);
        assert_eq!(snap.paused_count, 0);
        assert_eq!(snap.progress.len(), 1);
        assert_eq!(snap.pressure, 0.0); // no pressure sources
        assert!(snap.pressure_breakdown.is_empty());
        assert_eq!(snap.max_concurrency, 4);
    }

    #[tokio::test]
    async fn pause_all_stops_dispatching() {
        let sched = setup(arc_erased(SlowExecutor)).await;

        // Submit two tasks.
        for key in &["pa-1", "pa-2"] {
            sched
                .submit(&TaskSubmission::new("test").key(*key))
                .await
                .unwrap();
        }

        // Dispatch one so it's running.
        sched.try_dispatch().await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(sched.active_tasks().await.len(), 1);

        // Pause — running task should be cancelled and moved to paused in store.
        sched.pause_all().await;
        assert!(sched.is_paused());
        assert_eq!(sched.active_tasks().await.len(), 0);

        // try_dispatch should still work at the store level (it doesn't check
        // the pause flag itself — the run loop does), but we can verify that
        // the snapshot shows is_paused.
        let snap = sched.snapshot().await.unwrap();
        assert!(snap.is_paused);

        // Resume — flag should clear.
        sched.resume_all().await;
        assert!(!sched.is_paused());
        let snap = sched.snapshot().await.unwrap();
        assert!(!snap.is_paused);
    }

    #[tokio::test]
    async fn pause_resume_events_emitted() {
        let sched = setup(arc_erased(InstantExecutor)).await;
        let mut rx = sched.subscribe();

        sched.pause_all().await;
        let evt = rx.recv().await.unwrap();
        assert!(matches!(evt, SchedulerEvent::Paused));

        sched.resume_all().await;
        let evt = rx.recv().await.unwrap();
        assert!(matches!(evt, SchedulerEvent::Resumed));
    }

    #[tokio::test]
    async fn app_state_accessible_from_executor() {
        use std::sync::atomic::{AtomicBool, Ordering};

        struct MyState {
            flag: Arc<AtomicBool>,
        }

        struct StateCheckExecutor;

        impl TaskExecutor for StateCheckExecutor {
            async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
                let state = ctx.state::<MyState>().expect("state should be set");
                state.flag.store(true, Ordering::SeqCst);
                Ok(())
            }
        }

        let flag = Arc::new(AtomicBool::new(false));

        let sched = Scheduler::builder()
            .store(TaskStore::open_memory().await.unwrap())
            .executor("test", Arc::new(StateCheckExecutor))
            .app_state(MyState { flag: flag.clone() })
            .build()
            .await
            .unwrap();

        sched
            .submit(&TaskSubmission::new("test").key("state-test"))
            .await
            .unwrap();

        sched.try_dispatch().await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn task_lookup_pending() {
        let sched = setup(arc_erased(InstantExecutor)).await;

        sched
            .submit(&TaskSubmission::new("test").key("lookup-1"))
            .await
            .unwrap();

        let result = sched.task_lookup("test", Some(b"lookup-1")).await.unwrap();
        assert!(matches!(
            result,
            crate::task::TaskLookup::Active(ref r) if r.status == crate::task::TaskStatus::Pending
        ));
    }

    #[tokio::test]
    async fn task_lookup_completed() {
        let sched = setup(arc_erased(InstantExecutor)).await;

        sched
            .submit(&TaskSubmission::new("test").key("lookup-done"))
            .await
            .unwrap();

        sched.try_dispatch().await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        let result = sched
            .task_lookup("test", Some(b"lookup-done"))
            .await
            .unwrap();
        assert!(matches!(result, crate::task::TaskLookup::History(_)));
    }

    #[tokio::test]
    async fn task_lookup_not_found() {
        let sched = setup(arc_erased(InstantExecutor)).await;
        let result = sched
            .task_lookup("test", Some(b"does-not-exist"))
            .await
            .unwrap();
        assert!(matches!(result, crate::task::TaskLookup::NotFound));
    }

    #[tokio::test]
    async fn lookup_typed_works() {
        use serde::{Deserialize as De, Serialize as Ser};

        #[derive(Ser, De, Debug, PartialEq)]
        struct Thumb {
            path: String,
        }

        impl crate::task::TypedTask for Thumb {
            const TASK_TYPE: &'static str = "test";
        }

        let sched = setup(arc_erased(InstantExecutor)).await;

        let task = Thumb {
            path: "/a.jpg".into(),
        };
        sched.submit_typed(&task).await.unwrap();

        let result = sched.lookup_typed(&task).await.unwrap();
        assert!(matches!(result, crate::task::TaskLookup::Active(_)));
    }

    // ── Hierarchy tests ─────────────────────────────────────────────

    /// An executor that spawns N child tasks during execution.
    struct SpawningExecutor {
        num_children: usize,
    }

    impl TaskExecutor for SpawningExecutor {
        async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
            for i in 0..self.num_children {
                let sub = TaskSubmission::new("child")
                    .key(format!("child-{i}"))
                    .priority(ctx.record().priority);
                ctx.spawn_child(sub).await?;
            }
            Ok(())
        }
    }

    /// An executor that records whether finalize was called.
    struct FinalizeTrackingExecutor {
        children: usize,
        finalized: Arc<std::sync::atomic::AtomicBool>,
    }

    impl TaskExecutor for FinalizeTrackingExecutor {
        async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
            for i in 0..self.children {
                let sub = TaskSubmission::new("child")
                    .key(format!("ft-child-{i}"))
                    .priority(ctx.record().priority);
                ctx.spawn_child(sub).await?;
            }
            Ok(())
        }

        async fn finalize<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
            self.finalized
                .store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn parent_enters_waiting_when_children_spawned() {
        let store = TaskStore::open_memory().await.unwrap();
        let mut registry = TaskTypeRegistry::new();
        registry.register_erased("parent", arc_erased(SpawningExecutor { num_children: 2 }));
        registry.register_erased("child", arc_erased(InstantExecutor));

        let sched = Scheduler::new(
            store,
            SchedulerConfig::default(),
            Arc::new(registry),
            CompositePressure::new(),
            ThrottlePolicy::default_three_tier(),
        );
        let mut rx = sched.subscribe();

        // Submit parent task.
        sched
            .submit(&TaskSubmission::new("parent").key("p1"))
            .await
            .unwrap();

        // Dispatch parent.
        sched.try_dispatch().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should get Dispatched, then Waiting events for the parent.
        let mut saw_waiting = false;
        for _ in 0..10 {
            if let Ok(evt) = rx.try_recv() {
                if matches!(evt, SchedulerEvent::Waiting { .. }) {
                    saw_waiting = true;
                    break;
                }
            }
        }
        assert!(saw_waiting, "expected Waiting event for parent");

        // Parent should be in waiting status in the store.
        let parent_key = crate::task::generate_dedup_key("parent", Some(b"p1"));
        let parent = sched
            .store()
            .task_by_key(&parent_key)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(parent.status, crate::task::TaskStatus::Waiting);

        // Two children should be pending.
        assert_eq!(sched.store().pending_count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn parent_auto_completes_after_children_finish() {
        let store = TaskStore::open_memory().await.unwrap();
        let mut registry = TaskTypeRegistry::new();
        registry.register_erased("parent", arc_erased(SpawningExecutor { num_children: 2 }));
        registry.register_erased("child", arc_erased(InstantExecutor));

        let sched = Scheduler::new(
            store,
            SchedulerConfig::default(),
            Arc::new(registry),
            CompositePressure::new(),
            ThrottlePolicy::default_three_tier(),
        );
        let mut rx = sched.subscribe();

        sched
            .submit(&TaskSubmission::new("parent").key("p-complete"))
            .await
            .unwrap();

        // Run scheduler loop.
        let token = CancellationToken::new();
        let sched_clone = sched.clone();
        let token_clone = token.clone();
        let handle = tokio::spawn(async move {
            sched_clone.run(token_clone).await;
        });

        // Wait for parent Completed event.
        let parent_key = crate::task::generate_dedup_key("parent", Some(b"p-complete"));
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut parent_completed = false;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
                Ok(Ok(SchedulerEvent::Completed { task_type, .. })) if task_type == "parent" => {
                    parent_completed = true;
                    break;
                }
                _ => {}
            }
        }

        // Check before shutdown closes the pool.
        let lookup = sched.store().task_lookup(&parent_key).await.unwrap();

        token.cancel();
        let _ = handle.await;

        assert!(parent_completed, "expected parent Completed event");
        assert!(
            matches!(lookup, crate::task::TaskLookup::History(ref h) if h.status == crate::task::HistoryStatus::Completed),
            "expected parent in history as completed, got: {lookup:?}"
        );
    }

    #[tokio::test]
    async fn finalize_called_after_children_complete() {
        let finalized = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let store = TaskStore::open_memory().await.unwrap();
        let mut registry = TaskTypeRegistry::new();
        registry.register_erased(
            "parent",
            arc_erased(FinalizeTrackingExecutor {
                children: 1,
                finalized: finalized.clone(),
            }),
        );
        registry.register_erased("child", arc_erased(InstantExecutor));

        let sched = Scheduler::new(
            store,
            SchedulerConfig::default(),
            Arc::new(registry),
            CompositePressure::new(),
            ThrottlePolicy::default_three_tier(),
        );
        let mut rx = sched.subscribe();

        sched
            .submit(&TaskSubmission::new("parent").key("p-finalize"))
            .await
            .unwrap();

        let token = CancellationToken::new();
        let sched_clone = sched.clone();
        let token_clone = token.clone();
        let handle = tokio::spawn(async move {
            sched_clone.run(token_clone).await;
        });

        // Wait for parent Completed event rather than a fixed sleep.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
                Ok(Ok(SchedulerEvent::Completed { task_type, .. })) if task_type == "parent" => {
                    break;
                }
                _ => {}
            }
        }

        token.cancel();
        let _ = handle.await;

        assert!(
            finalized.load(std::sync::atomic::Ordering::SeqCst),
            "finalize() should have been called"
        );
    }

    #[tokio::test]
    async fn cancel_parent_cascades_to_children() {
        let store = TaskStore::open_memory().await.unwrap();
        let mut registry = TaskTypeRegistry::new();
        registry.register_erased("parent", arc_erased(SpawningExecutor { num_children: 3 }));
        registry.register_erased("child", arc_erased(SlowExecutor));

        let sched = Scheduler::new(
            store,
            SchedulerConfig::default(),
            Arc::new(registry),
            CompositePressure::new(),
            ThrottlePolicy::default_three_tier(),
        );

        let parent_id = sched
            .submit(&TaskSubmission::new("parent").key("p-cancel"))
            .await
            .unwrap()
            .id()
            .unwrap();

        // Dispatch parent (which spawns children).
        sched.try_dispatch().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Cancel parent — should cascade to children.
        let cancelled = sched.cancel(parent_id).await.unwrap();
        assert!(cancelled);

        // All children should be gone.
        assert_eq!(sched.store().pending_count().await.unwrap(), 0);
        assert_eq!(sched.store().running_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn no_children_completes_normally() {
        // Task without children should complete as before (backward compat).
        let sched = setup(arc_erased(InstantExecutor)).await;

        sched
            .submit(&TaskSubmission::new("test").key("no-kids"))
            .await
            .unwrap();

        sched.try_dispatch().await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        let key = crate::task::generate_dedup_key("test", Some(b"no-kids"));
        let lookup = sched.store().task_lookup(&key).await.unwrap();
        assert!(matches!(lookup, crate::task::TaskLookup::History(_)));
    }
}
