//! Task spawning, active-task tracking, preemption, and parent-child resolution.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;

use crate::priority::Priority;
use crate::registry::{ChildSpawner, IoTracker, ParentContext, TaskContext};
use crate::store::TaskStore;
use crate::task::{IoBudget, ParentResolution, TaskRecord};

use super::progress::ProgressReporter;
use super::SchedulerEvent;

// ── Active Task ────────────────────────────────────────────────────

/// Handle to a running task for preemption and progress tracking.
pub(crate) struct ActiveTask {
    pub record: TaskRecord,
    pub token: CancellationToken,
    /// Last reported progress from the executor (0.0 to 1.0).
    pub reported_progress: Option<f32>,
    /// When the last progress report was received.
    pub reported_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Handle to the spawned tokio task, set after spawn.
    pub handle: Option<tokio::task::JoinHandle<()>>,
    /// Shared IO tracker for byte-level progress reporting.
    pub io: Arc<IoTracker>,
    /// When this task started executing.
    pub started_at: std::time::Instant,
}

/// Snapshot of byte-level progress for a single active task.
pub(crate) type ByteProgressSnapshot = (
    i64,
    String,
    String,
    String,
    u64,
    Option<u64>,
    Option<i64>,
    std::time::Instant,
);

// ── Active Task Map ────────────────────────────────────────────────

/// Thread-safe map of currently running tasks.
///
/// Wraps the active-task bookkeeping that was previously inlined in
/// `Scheduler`, making preemption and progress queries independently
/// testable.
///
/// Uses `std::sync::Mutex` rather than `tokio::Mutex` because most
/// operations do trivial `HashMap` work under the lock with no `.await`.
/// Methods that need async I/O (`preempt_below`, `pause_all`) collect
/// data under the lock and release it before awaiting.
#[derive(Clone)]
pub(crate) struct ActiveTaskMap {
    inner: Arc<Mutex<HashMap<i64, ActiveTask>>>,
    /// IDs of parent tasks ready for finalization. Populated by
    /// `handle_parent_resolution` when all children complete. Uses a
    /// `HashSet` to deduplicate — two children completing simultaneously
    /// may both resolve the parent, but we only finalize once.
    pub(crate) pending_finalizers: Arc<Mutex<HashSet<i64>>>,
}

impl ActiveTaskMap {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            pending_finalizers: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    pub fn insert(&self, id: i64, task: ActiveTask) {
        self.inner.lock().unwrap().insert(id, task);
    }

    pub fn remove(&self, id: i64) -> Option<ActiveTask> {
        self.inner.lock().unwrap().remove(&id)
    }

    /// Snapshot of all active task records.
    pub fn records(&self) -> Vec<TaskRecord> {
        self.inner
            .lock()
            .unwrap()
            .values()
            .map(|at| at.record.clone())
            .collect()
    }

    /// Snapshot of progress data for all active tasks.
    pub fn progress_snapshots(
        &self,
    ) -> Vec<(
        TaskRecord,
        Option<f32>,
        Option<chrono::DateTime<chrono::Utc>>,
    )> {
        self.inner
            .lock()
            .unwrap()
            .values()
            .map(|at| (at.record.clone(), at.reported_progress, at.reported_at))
            .collect()
    }

    /// Snapshot of byte-level progress for all active tasks.
    ///
    /// Returns `(task_id, task_type, key, label, bytes_completed, bytes_total, parent_id, started_at)`.
    /// Single lock acquisition — reads atomic counters and copies scalar fields only.
    pub fn byte_progress_snapshots(&self) -> Vec<ByteProgressSnapshot> {
        let map = self.inner.lock().unwrap();
        map.values()
            .map(|at| {
                let (completed, total) = at.io.progress_snapshot();
                (
                    at.record.id,
                    at.record.task_type.clone(),
                    at.record.key.clone(),
                    at.record.label.clone(),
                    completed,
                    total,
                    at.record.parent_id,
                    at.started_at,
                )
            })
            .collect()
    }

    /// Update reported progress for a specific task.
    pub fn update_progress(&self, task_id: i64, percent: f32) {
        let mut map = self.inner.lock().unwrap();
        if let Some(at) = map.get_mut(&task_id) {
            at.reported_progress = Some(percent);
            at.reported_at = Some(chrono::Utc::now());
        }
    }

    /// Preempt active tasks with priority lower than the incoming priority.
    ///
    /// Cancels their tokens, pauses them in the store, and emits
    /// `SchedulerEvent::Preempted`. Returns the IDs of preempted tasks.
    ///
    /// Collects tasks to preempt under the sync lock, then releases the
    /// lock before performing async store writes.
    pub async fn preempt_below(
        &self,
        incoming_priority: Priority,
        store: &TaskStore,
        event_tx: &tokio::sync::broadcast::Sender<SchedulerEvent>,
    ) -> Vec<i64> {
        // Phase 1: collect + remove under sync lock.
        let to_preempt: Vec<(i64, ActiveTask)> = {
            let mut active = self.inner.lock().unwrap();
            let ids: Vec<i64> = active
                .iter()
                .filter(|(_, at)| at.record.priority.value() > incoming_priority.value())
                .map(|(id, _)| *id)
                .collect();
            ids.into_iter()
                .filter_map(|id| active.remove(&id).map(|at| (id, at)))
                .collect()
        };

        // Phase 2: async work without the lock held.
        let mut preempted = Vec::new();
        for (id, at) in to_preempt {
            tracing::info!(
                task_id = id,
                task_type = at.record.task_type,
                "preempting task for higher-priority work"
            );
            at.token.cancel();
            let _ = store.pause(id).await;
            let _ = event_tx.send(SchedulerEvent::Preempted(at.record.event_header()));
            preempted.push(id);
        }

        preempted
    }

    /// Check whether any active task would preempt work at the given priority.
    pub fn has_preemptors_for(&self, priority: Priority, preempt_threshold: Priority) -> bool {
        let active = self.inner.lock().unwrap();
        active.values().any(|at| {
            at.record.priority.value() <= preempt_threshold.value()
                && at.record.priority.value() < priority.value()
        })
    }

    /// Store the `JoinHandle` for a task that was just spawned.
    pub fn set_handle(&self, id: i64, handle: tokio::task::JoinHandle<()>) {
        let mut map = self.inner.lock().unwrap();
        if let Some(at) = map.get_mut(&id) {
            at.handle = Some(handle);
        }
    }

    /// Cancel all active tasks and abort their handles (hard shutdown).
    pub fn cancel_all(&self) {
        let mut active = self.inner.lock().unwrap();
        for (_, at) in active.drain() {
            at.token.cancel();
            if let Some(h) = at.handle {
                h.abort();
            }
        }
    }

    /// Cancel all tokens, drain the map, and return the join handles
    /// for graceful shutdown (caller can join with a timeout).
    pub fn cancel_and_drain_handles(&self) -> Vec<tokio::task::JoinHandle<()>> {
        let mut active = self.inner.lock().unwrap();
        let mut handles = Vec::with_capacity(active.len());
        for (_, at) in active.drain() {
            at.token.cancel();
            if let Some(h) = at.handle {
                handles.push(h);
            }
        }
        handles
    }

    /// Pause all active tasks: cancel their tokens and move them to paused
    /// state in the store. Returns the number of tasks paused.
    ///
    /// Drains the map under the sync lock, then releases the lock before
    /// performing async store writes.
    pub async fn pause_all(
        &self,
        store: &TaskStore,
        event_tx: &tokio::sync::broadcast::Sender<SchedulerEvent>,
    ) -> usize {
        // Drain under sync lock.
        let drained: Vec<(i64, ActiveTask)> = { self.inner.lock().unwrap().drain().collect() };
        let count = drained.len();
        // Async work without the lock held.
        for (id, at) in drained {
            at.token.cancel();
            let _ = store.pause(id).await;
            let _ = event_tx.send(SchedulerEvent::Preempted(at.record.event_header()));
        }
        count
    }
}

// ── Spawn ──────────────────────────────────────────────────────────

/// Whether to call `execute` or `finalize` on the executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionPhase {
    Execute,
    Finalize,
}

/// Shared scheduler resources passed to each spawned task.
pub(crate) struct SpawnContext {
    pub store: TaskStore,
    pub active: ActiveTaskMap,
    pub event_tx: tokio::sync::broadcast::Sender<SchedulerEvent>,
    pub max_retries: i32,
    pub app_state: crate::registry::StateSnapshot,
    pub work_notify: Arc<tokio::sync::Notify>,
    pub scheduler: super::WeakScheduler,
    #[allow(dead_code)]
    pub cancel_hook_timeout: tokio::time::Duration,
}

/// Spawn a task executor and wire up completion/failure handling.
///
/// Inserts the task into the active map, starts a progress listener,
/// and spawns the executor on a new tokio task.
pub(crate) async fn spawn_task(
    task: TaskRecord,
    executor: Arc<dyn crate::registry::ErasedExecutor>,
    ctx: SpawnContext,
    phase: ExecutionPhase,
) {
    let SpawnContext {
        store,
        active,
        event_tx,
        max_retries,
        app_state,
        work_notify,
        scheduler,
        cancel_hook_timeout: _,
    } = ctx;
    let child_token = CancellationToken::new();

    // Build execution context.
    let child_spawner = ChildSpawner::new(
        store.clone(),
        task.id,
        work_notify.clone(),
        ParentContext {
            created_at: task.created_at,
            ttl_seconds: task.ttl_seconds,
            ttl_from: task.ttl_from,
            started_at: task.started_at,
            tags: task.tags.clone(),
        },
    );
    let io = Arc::new(IoTracker::new());

    // Insert into active map before spawning to avoid races.
    active.insert(
        task.id,
        ActiveTask {
            record: task.clone(),
            token: child_token.clone(),
            reported_progress: None,
            reported_at: None,
            handle: None,
            io: io.clone(),
            started_at: std::time::Instant::now(),
        },
    );

    let ctx = TaskContext {
        record: task.clone(),
        token: child_token.clone(),
        progress: ProgressReporter::new(
            task.event_header(),
            event_tx.clone(),
            active.clone(),
            io.clone(),
        ),
        scheduler,
        app_state,
        child_spawner: Some(child_spawner),
        io: io.clone(),
    };

    // Emit dispatched event.
    let _ = event_tx.send(SchedulerEvent::Dispatched(task.event_header()));

    // Spawn executor.
    let task_id_for_handle = task.id;
    let active_for_handle = active.clone();
    let token_for_spawn = child_token.clone();
    let handle = tokio::spawn(async move {
        let task_id = task.id;
        let result = match phase {
            ExecutionPhase::Execute => executor.execute_erased(&ctx).await,
            ExecutionPhase::Finalize => executor.finalize_erased(&ctx).await,
        };

        // Read IO bytes from the context tracker.
        let metrics = io.snapshot();

        // Drop the context (and its progress reporter) — executor is done.
        drop(ctx);

        match result {
            Ok(()) => {
                // For the execute phase, check if the task spawned children.
                // If so, transition to waiting instead of completing.
                if phase == ExecutionPhase::Execute {
                    match store.active_children_count(task_id).await {
                        Ok(count) if count > 0 => {
                            if let Err(e) = store.set_waiting(task_id).await {
                                tracing::error!(task_id, error = %e, "failed to set task to waiting");
                            }
                            active.remove(task_id);
                            let _ = event_tx.send(SchedulerEvent::Waiting {
                                task_id,
                                children_count: count,
                            });
                            // Children may have completed before we set waiting.
                            // Re-check to avoid a missed finalization.
                            handle_parent_resolution(
                                task_id,
                                &store,
                                &active,
                                &event_tx,
                                max_retries,
                                &work_notify,
                            )
                            .await;
                            // Wake the scheduler to dispatch children (or finalizer).
                            work_notify.notify_one();
                            return;
                        }
                        Err(e) => {
                            tracing::error!(task_id, error = %e, "failed to check children count");
                            // Fall through to normal completion.
                        }
                        _ => {
                            // No children — complete normally.
                        }
                    }
                }

                match store.complete_with_record(&task, &metrics).await {
                    Ok(recurring_info) => {
                        // Emit recurring event if this was a recurring task.
                        if task.recurring_interval_secs.is_some() {
                            let (next_run, exec_count) = match recurring_info {
                                Some((next, count)) => (Some(next), count),
                                None => (None, task.recurring_execution_count + 1),
                            };
                            let _ = event_tx.send(SchedulerEvent::RecurringCompleted {
                                header: task.event_header(),
                                execution_count: exec_count,
                                next_run,
                            });
                        }
                    }
                    Err(e) => {
                        tracing::error!(task_id, error = %e, "failed to record task completion");
                    }
                }
                // Remove from active tracking AFTER the store write completes.
                active.remove(task_id);
                let _ = event_tx.send(SchedulerEvent::Completed(task.event_header()));

                // Resolve dependency edges: unblock tasks waiting on this one.
                match store.resolve_dependents(task_id).await {
                    Ok(unblocked) => {
                        for uid in &unblocked {
                            let _ = event_tx.send(SchedulerEvent::TaskUnblocked { task_id: *uid });
                        }
                    }
                    Err(e) => {
                        tracing::error!(task_id, error = %e, "failed to resolve dependents");
                    }
                }

                work_notify.notify_one();

                // If this was a child task, check if parent is ready.
                if let Some(parent_id) = task.parent_id {
                    handle_parent_resolution(
                        parent_id,
                        &store,
                        &active,
                        &event_tx,
                        max_retries,
                        &work_notify,
                    )
                    .await;
                }
            }
            Err(te) => {
                // If cancelled (preempted), the scheduler already paused it.
                if token_for_spawn.is_cancelled() {
                    active.remove(task_id);
                    return;
                }
                let will_retry = te.retryable && task.retry_count < max_retries;
                tracing::warn!(
                    task_id,
                    task_type = task.task_type,
                    error = %te.message,
                    retryable = te.retryable,
                    will_retry,
                    "task failed"
                );
                if let Err(e) = store
                    .fail_with_record(&task, &te.message, te.retryable, max_retries, &metrics)
                    .await
                {
                    tracing::error!(task_id, error = %e, "failed to record task failure");
                }
                // Remove from active tracking AFTER the store write completes.
                active.remove(task_id);
                let _ = event_tx.send(SchedulerEvent::Failed {
                    header: task.event_header(),
                    error: te.message.clone(),
                    will_retry,
                });
                work_notify.notify_one();

                // If permanent failure, propagate to dependency chain.
                if !will_retry {
                    match store.fail_dependents(task_id).await {
                        Ok((failed_ids, unblocked_ids)) => {
                            for fid in &failed_ids {
                                let _ = event_tx.send(SchedulerEvent::DependencyFailed {
                                    task_id: *fid,
                                    failed_dependency: task_id,
                                });
                            }
                            for uid in &unblocked_ids {
                                let _ =
                                    event_tx.send(SchedulerEvent::TaskUnblocked { task_id: *uid });
                            }
                            if !unblocked_ids.is_empty() {
                                work_notify.notify_one();
                            }
                        }
                        Err(e) => {
                            tracing::error!(task_id, error = %e, "failed to propagate failure to dependents");
                        }
                    }

                    if let Some(parent_id) = task.parent_id {
                        // Check if parent uses fail_fast.
                        if let Ok(Some(parent)) = store.task_by_id(parent_id).await {
                            if parent.fail_fast {
                                // Cancel remaining siblings.
                                if let Ok(running_ids) = store.cancel_children(parent_id).await {
                                    for rid in &running_ids {
                                        if let Some(at) = active.remove(*rid) {
                                            at.token.cancel();
                                            let _ = store.delete(*rid).await;
                                            let _ = event_tx.send(SchedulerEvent::Cancelled(
                                                at.record.event_header(),
                                            ));
                                        }
                                    }
                                }
                                // Fail the parent.
                                let msg = format!("child task {task_id} failed: {}", te.message);
                                if let Err(e) = store
                                    .fail_with_record(&parent, &msg, false, 0, &IoBudget::default())
                                    .await
                                {
                                    tracing::error!(
                                        parent_id,
                                        error = %e,
                                        "failed to record parent failure"
                                    );
                                }
                                let _ = event_tx.send(SchedulerEvent::Failed {
                                    header: parent.event_header(),
                                    error: msg,
                                    will_retry: false,
                                });
                            } else {
                                // Not fail_fast — check if all children done.
                                handle_parent_resolution(
                                    parent_id,
                                    &store,
                                    &active,
                                    &event_tx,
                                    max_retries,
                                    &work_notify,
                                )
                                .await;
                            }
                        }
                    }
                }
            }
        }
    });

    // Store the handle so shutdown can join it.
    active_for_handle.set_handle(task_id_for_handle, handle);
}

/// Check if a waiting parent is ready for finalization or has failed,
/// and dispatch the finalize phase if ready.
async fn handle_parent_resolution(
    parent_id: i64,
    store: &TaskStore,
    active: &ActiveTaskMap,
    event_tx: &tokio::sync::broadcast::Sender<SchedulerEvent>,
    _max_retries: i32,
    work_notify: &Arc<tokio::sync::Notify>,
) {
    match store.try_resolve_parent(parent_id).await {
        Ok(Some(ParentResolution::ReadyToFinalize)) => {
            // Enqueue parent for finalize dispatch.
            active.pending_finalizers.lock().unwrap().insert(parent_id);
            // Wake the scheduler to dispatch the finalize phase.
            work_notify.notify_one();
        }
        Ok(Some(ParentResolution::Failed(reason))) => {
            // All children done but some failed — fail the parent.
            if let Ok(Some(parent)) = store.task_by_id(parent_id).await {
                if let Err(e) = store
                    .fail_with_record(&parent, &reason, false, 0, &IoBudget::default())
                    .await
                {
                    tracing::error!(parent_id, error = %e, "failed to record parent failure");
                }
                let _ = event_tx.send(SchedulerEvent::Failed {
                    header: parent.event_header(),
                    error: reason,
                    will_retry: false,
                });
            }
        }
        Ok(Some(ParentResolution::StillWaiting)) | Ok(None) => {
            // Children still active or parent not found — nothing to do.
        }
        Err(e) => {
            tracing::error!(parent_id, error = %e, "failed to resolve parent");
        }
    }
}
