//! Module definition — a self-contained bundle of task executors, defaults,
//! and resource policy.
//!
//! Define a [`Module`] on the library side, then register it with
//! [`SchedulerBuilder::module`](crate::SchedulerBuilder::module) on the
//! application side. All executor task types are automatically prefixed with
//! `"{name}::"` at registration time.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::Arc;
use std::time::Duration;

use crate::priority::Priority;
use crate::registry::{ErasedExecutor, TaskExecutor};
use crate::scheduler::progress::{EstimatedProgress, TaskProgress};
use crate::store::StoreError;
use crate::task::retry::RetryPolicy;
use crate::task::submit_builder::TypedTaskDefaults;
use crate::task::{ModuleSubmitDefaults, SubmitBuilder};
use crate::task::{
    SubmitOutcome, TaskHistoryRecord, TaskRecord, TaskStatus, TaskSubmission, TypedTask,
};

/// Per-executor options for task type registration within a module.
#[derive(Default, Clone)]
pub struct ExecutorOptions {
    /// Per-type default TTL. Overrides module-level `default_ttl` for this
    /// task type only.
    pub ttl: Option<Duration>,
    /// Per-type retry policy. Overrides module-level `default_retry_policy`
    /// for this task type only.
    pub retry_policy: Option<RetryPolicy>,
}

/// Internal storage for a registered executor within a module.
#[allow(dead_code)]
pub(crate) struct ModuleExecutor {
    /// Unprefixed task type name (e.g. `"thumbnail"`). Prefixed to
    /// `"media::thumbnail"` when the module is registered with the scheduler.
    pub task_type: String,
    pub executor: Arc<dyn ErasedExecutor>,
    pub options: ExecutorOptions,
}

/// A self-contained bundle of task executors, defaults, and resource policy.
///
/// A module is the unit of composition in taskmill. It collects all the task
/// types owned by one feature or crate, together with module-wide defaults
/// (priority, retry policy, group, TTL, tags) and an optional concurrency cap.
///
/// # Example
///
/// ```ignore
/// use std::sync::Arc;
/// use taskmill::{Module, RetryPolicy, BackoffStrategy, Priority};
/// use std::time::Duration;
///
/// pub fn media_module() -> Module {
///     Module::new("media")
///         .typed_executor::<Thumbnail>(Arc::new(ThumbnailExec))
///         .typed_executor::<Transcode>(Arc::new(TranscodeExec))
///         .default_priority(Priority::NORMAL)
///         .default_retry_policy(RetryPolicy {
///             strategy: BackoffStrategy::Exponential {
///                 initial: Duration::from_secs(1),
///                 max: Duration::from_secs(120),
///                 multiplier: 2.0,
///             },
///             max_retries: 5,
///         })
///         .default_group("media-pipeline")
///         .default_ttl(Duration::from_secs(3600))
///         .max_concurrency(4)
///         .app_state(MediaConfig { cdn_url: "...".into() })
/// }
/// ```
pub struct Module {
    pub(crate) name: String,
    pub(crate) executors: Vec<ModuleExecutor>,
    pub(crate) default_priority: Option<Priority>,
    pub(crate) default_retry_policy: Option<RetryPolicy>,
    pub(crate) default_group: Option<String>,
    pub(crate) default_ttl: Option<Duration>,
    pub(crate) default_tags: HashMap<String, String>,
    pub(crate) max_concurrency: Option<usize>,
    pub(crate) app_state_entries: Vec<(TypeId, Arc<dyn Any + Send + Sync>)>,
}

#[allow(dead_code)]
impl Module {
    /// Create a new module with the given name.
    ///
    /// # Panics
    ///
    /// Panics if `name` is empty or contains `"::"` (the reserved
    /// module/task-type separator).
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        assert!(!name.is_empty(), "module name must not be empty");
        assert!(
            !name.contains("::"),
            "module name must not contain '::' (reserved separator)"
        );
        Self {
            name,
            executors: Vec::new(),
            default_priority: None,
            default_retry_policy: None,
            default_group: None,
            default_ttl: None,
            default_tags: HashMap::new(),
            max_concurrency: None,
            app_state_entries: Vec::new(),
        }
    }

    /// Register a typed executor. The task type name is taken from
    /// `T::TASK_TYPE` and stored unprefixed (e.g. `"thumbnail"`).
    ///
    /// At [`SchedulerBuilder::build`](crate::SchedulerBuilder::build) time the
    /// name is prefixed with `"{module_name}::"` (e.g. `"media::thumbnail"`).
    pub fn typed_executor<T: TypedTask, E: TaskExecutor>(mut self, executor: Arc<E>) -> Self {
        self.executors.push(ModuleExecutor {
            task_type: T::TASK_TYPE.to_string(),
            executor: executor as Arc<dyn ErasedExecutor>,
            options: ExecutorOptions::default(),
        });
        self
    }

    /// Register a named executor.
    ///
    /// Prefer [`typed_executor`](Self::typed_executor) for type-safe
    /// registration.
    pub fn executor<E: TaskExecutor>(
        mut self,
        task_type: impl Into<String>,
        executor: Arc<E>,
    ) -> Self {
        self.executors.push(ModuleExecutor {
            task_type: task_type.into(),
            executor: executor as Arc<dyn ErasedExecutor>,
            options: ExecutorOptions::default(),
        });
        self
    }

    /// Register a named executor with both a per-type TTL and a retry policy.
    pub fn executor_with_options<E: TaskExecutor>(
        mut self,
        task_type: impl Into<String>,
        executor: Arc<E>,
        ttl: Option<Duration>,
        retry_policy: Option<RetryPolicy>,
    ) -> Self {
        self.executors.push(ModuleExecutor {
            task_type: task_type.into(),
            executor: executor as Arc<dyn ErasedExecutor>,
            options: ExecutorOptions { ttl, retry_policy },
        });
        self
    }

    /// Register a named executor with a per-type default TTL.
    pub fn executor_with_ttl<E: TaskExecutor>(
        self,
        task_type: impl Into<String>,
        executor: Arc<E>,
        ttl: Duration,
    ) -> Self {
        self.executor_with_options(task_type, executor, Some(ttl), None)
    }

    /// Register a named executor with a per-type retry policy.
    pub fn executor_with_retry_policy<E: TaskExecutor>(
        self,
        task_type: impl Into<String>,
        executor: Arc<E>,
        retry_policy: RetryPolicy,
    ) -> Self {
        self.executor_with_options(task_type, executor, None, Some(retry_policy))
    }

    /// Set the module-wide default priority applied to all tasks submitted
    /// through this module's handle (unless overridden per-submission).
    pub fn default_priority(mut self, priority: Priority) -> Self {
        self.default_priority = Some(priority);
        self
    }

    /// Set the module-wide default retry policy.
    pub fn default_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.default_retry_policy = Some(policy);
        self
    }

    /// Set the module-wide default group key for per-group concurrency
    /// limiting.
    pub fn default_group(mut self, group: impl Into<String>) -> Self {
        self.default_group = Some(group.into());
        self
    }

    /// Set the module-wide default TTL.
    pub fn default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// Add a default tag applied to all tasks submitted through this module's
    /// handle. Later calls with the same key overwrite the previous value.
    pub fn default_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.default_tags.insert(key.into(), value.into());
        self
    }

    /// Merge multiple default tags at once.
    pub fn default_tags(
        mut self,
        tags: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        self.default_tags
            .extend(tags.into_iter().map(|(k, v)| (k.into(), v.into())));
        self
    }

    /// Set the module-level concurrency cap (independent of the global
    /// `max_concurrency`).
    pub fn max_concurrency(mut self, limit: usize) -> Self {
        self.max_concurrency = Some(limit);
        self
    }

    /// Register module-scoped application state.
    ///
    /// This state is accessible to executors within this module via
    /// [`TaskContext::state::<T>()`](crate::TaskContext::state).
    pub fn app_state<T: Send + Sync + 'static>(self, state: T) -> Self {
        self.app_state_arc(Arc::new(state))
    }

    /// Register module-scoped state from a pre-existing `Arc`.
    pub fn app_state_arc<T: Send + Sync + 'static>(mut self, state: Arc<T>) -> Self {
        self.app_state_entries.push((TypeId::of::<T>(), state));
        self
    }

    /// The module name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The task type prefix used to namespace all task types in this module,
    /// e.g. `"media::"` for a module named `"media"`.
    pub fn prefix(&self) -> String {
        format!("{}::", self.name)
    }
}

// ── ModuleRegistry ───────────────────────────────────────────────────

/// Metadata for a single registered module, stored inside [`ModuleRegistry`].
pub struct ModuleEntry {
    /// Module name (e.g. `"media"`).
    pub name: String,
    /// Task type prefix (e.g. `"media::"`).
    pub prefix: String,
    pub default_priority: Option<Priority>,
    pub default_retry_policy: Option<RetryPolicy>,
    pub default_group: Option<String>,
    pub default_ttl: Option<Duration>,
    pub default_tags: HashMap<String, String>,
    pub max_concurrency: Option<usize>,
}

/// Registry of all modules registered with the scheduler.
///
/// Stored in [`SchedulerInner`](crate::scheduler::Scheduler) and used by
/// future steps to implement scoped handles, concurrency gating, and
/// module-aware dispatch.
pub struct ModuleRegistry {
    entries: Vec<ModuleEntry>,
}

impl ModuleRegistry {
    /// Create an empty registry (used for schedulers built without the module API).
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub(crate) fn new(entries: Vec<ModuleEntry>) -> Self {
        Self { entries }
    }

    /// Look up a module by name.
    pub fn get(&self, name: &str) -> Option<&ModuleEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// All registered module entries.
    pub fn entries(&self) -> &[ModuleEntry] {
        &self.entries
    }
}

// ── ModuleSnapshot ────────────────────────────────────────────────

/// Status snapshot for a single module — a scoped subset of [`SchedulerSnapshot`].
#[derive(Debug, Clone)]
pub struct ModuleSnapshot {
    /// Tasks from this module that are currently running.
    pub running: Vec<TaskRecord>,
    /// Number of this module's tasks in `pending` status.
    pub pending_count: i64,
    /// Number of this module's tasks in `paused` status.
    pub paused_count: i64,
    /// Estimated progress for each running task in this module.
    pub progress: Vec<EstimatedProgress>,
    /// Byte-level progress for tasks in this module reporting transfer progress.
    pub byte_progress: Vec<TaskProgress>,
    /// Whether this module is currently paused via [`ModuleHandle::pause`].
    pub is_paused: bool,
}

// ── ModuleReceiver ────────────────────────────────────────────────

/// Filtered broadcast receiver that only surfaces events belonging to one module.
///
/// Wraps a global [`broadcast::Receiver`](tokio::sync::broadcast::Receiver) and
/// filters in `recv()` — no background forwarder is spawned. Events for other
/// modules are silently dropped. Module-global events (`Paused`, `Resumed`) are
/// always forwarded.
pub struct ModuleReceiver<E> {
    inner: tokio::sync::broadcast::Receiver<E>,
    /// Module name, e.g. `"media"`. Used for event filtering on [`SchedulerEvent`].
    name: Arc<str>,
    /// Task-type prefix, e.g. `"media::"`. Used for progress event filtering.
    prefix: Arc<str>,
}

impl ModuleReceiver<crate::scheduler::SchedulerEvent> {
    /// Receive the next event for this module, blocking until one arrives.
    ///
    /// Events belonging to other modules are discarded. `Paused` and `Resumed`
    /// global events are always forwarded. Returns [`RecvError`] on channel
    /// closure or lag.
    pub async fn recv(
        &mut self,
    ) -> Result<crate::scheduler::SchedulerEvent, tokio::sync::broadcast::error::RecvError> {
        loop {
            let event = self.inner.recv().await?;
            if event
                .header()
                .is_some_and(|h| h.module == self.name.as_ref())
            {
                return Ok(event);
            }
            if matches!(
                event,
                crate::scheduler::SchedulerEvent::Paused
                    | crate::scheduler::SchedulerEvent::Resumed
            ) {
                return Ok(event);
            }
        }
    }
}

impl ModuleReceiver<TaskProgress> {
    /// Receive the next progress event for this module, discarding others.
    pub async fn recv(&mut self) -> Result<TaskProgress, tokio::sync::broadcast::error::RecvError> {
        loop {
            let event = self.inner.recv().await?;
            if event.task_type.starts_with(self.prefix.as_ref()) {
                return Ok(event);
            }
        }
    }
}

// ── ModuleHandle ──────────────────────────────────────────────────

/// Scoped handle to a registered module.
///
/// Obtained via [`Scheduler::module`](crate::Scheduler::module) or
/// [`Scheduler::try_module`](crate::Scheduler::try_module).
///
/// All submission methods auto-prefix the task type with `"{name}::"`, merge
/// module defaults, and inject a `_module` tag. All query and control methods
/// are scoped to this module's tasks using `task_type LIKE '{prefix}%'` at the
/// SQL level.
#[derive(Clone)]
pub struct ModuleHandle {
    pub(crate) scheduler: crate::scheduler::Scheduler,
    /// Module name, e.g. `"media"`.
    name: Arc<str>,
    /// Task type prefix, e.g. `"media::"`.
    prefix: Arc<str>,
    /// Module-level submission defaults applied to every `SubmitBuilder`.
    defaults: ModuleSubmitDefaults,
}

impl ModuleHandle {
    pub(crate) fn new(scheduler: crate::scheduler::Scheduler, entry: &ModuleEntry) -> Self {
        let mut defaults = ModuleSubmitDefaults {
            priority: entry.default_priority,
            group: entry.default_group.clone(),
            ttl: entry.default_ttl,
            tags: entry.default_tags.clone(),
        };
        // Ensure the `_module` tag is always present in the module defaults.
        defaults
            .tags
            .entry("_module".to_string())
            .or_insert_with(|| entry.name.clone());
        Self {
            scheduler,
            name: entry.name.as_str().into(),
            prefix: entry.prefix.as_str().into(),
            defaults,
        }
    }

    /// The module name (e.g. `"media"`).
    pub fn name(&self) -> &str {
        &self.name
    }

    // ── Submission ────────────────────────────────────────────────

    /// Submit a raw [`TaskSubmission`].
    ///
    /// The task type is **not yet prefixed** — prefixing happens inside
    /// [`SubmitBuilder`] at resolve time. Bare `.await` submits with all
    /// module defaults applied.
    pub fn submit(&self, sub: TaskSubmission) -> SubmitBuilder {
        SubmitBuilder::new(
            sub,
            self.scheduler.clone(),
            self.name.as_ref(),
            self.defaults.clone(),
        )
    }

    /// Submit a [`TypedTask`].
    ///
    /// Serializes the task and wraps it in a [`SubmitBuilder`]. Bare `.await`
    /// applies module defaults; chain override methods for per-call overrides.
    ///
    /// Uses the 5-layer precedence chain: module defaults override TypedTask
    /// values, and SubmitBuilder per-call overrides trump everything.
    #[allow(dead_code)]
    pub(crate) fn submit_typed<T: TypedTask>(&self, task: &T) -> SubmitBuilder {
        let config = T::config();
        let mut sub = TaskSubmission::new(T::TASK_TYPE).payload_json(task);
        if let Some(io) = config.expected_io {
            sub = sub.expected_io(io);
        }
        if let Some(od) = config.on_duplicate {
            sub = sub.on_duplicate(od);
        }
        if let Some(tf) = config.ttl_from {
            sub = sub.ttl_from(tf);
        }
        if let Some(k) = task.key() {
            sub = sub.key(k);
        }
        if let Some(l) = task.label() {
            sub = sub.label(l);
        }
        if let Some(delay) = config.run_after {
            sub = sub.run_after(delay);
        }
        if let Some(sched) = config.recurring {
            sub = sub.recurring_schedule(sched);
        }
        let typed_defaults = TypedTaskDefaults {
            priority: config.priority.unwrap_or(Priority::NORMAL),
            group: config.group_key,
            ttl: config.ttl,
            tags: task.tags(),
        };
        self.submit(sub).with_typed_defaults(typed_defaults)
    }

    // ── Single-task operations ────────────────────────────────────

    /// Cancel a task by ID.
    ///
    /// Returns `Ok(false)` if the task does not exist or does not belong to
    /// this module (validated by checking `task_type` prefix).
    pub async fn cancel(&self, task_id: i64) -> Result<bool, StoreError> {
        if !self.task_belongs(task_id).await? {
            return Ok(false);
        }
        self.scheduler.cancel(task_id).await
    }

    /// Re-submit a dead-lettered task belonging to this module.
    ///
    /// Returns `Err(StoreError::InvalidState)` if the history record exists
    /// but belongs to a different module.
    pub async fn retry_dead_letter(&self, history_id: i64) -> Result<SubmitOutcome, StoreError> {
        let record = self
            .scheduler
            .inner
            .store
            .history_by_id(history_id)
            .await?
            .ok_or_else(|| StoreError::NotFound(format!("history record {history_id}")))?;
        if !record.task_type.starts_with(self.prefix.as_ref()) {
            return Err(StoreError::InvalidState(format!(
                "history record {history_id} belongs to a different module (task_type = '{}')",
                record.task_type
            )));
        }
        self.scheduler.retry_dead_letter(history_id).await
    }

    // ── Bulk cancellation ─────────────────────────────────────────

    /// Cancel all tasks belonging to this module.
    ///
    /// Queries all active tasks with `task_type LIKE '{prefix}%'` and cancels
    /// each one. Returns the IDs that were successfully cancelled.
    ///
    /// **Behavior by task status:**
    /// - **Pending** — moved to history as `Cancelled`.
    /// - **Running** — cancellation token is triggered; the task is moved to
    ///   history as `Cancelled` (the executor should check `ctx.token()` and
    ///   exit cleanly).
    /// - **Paused** — cancelled immediately (no executor is running).
    /// - **Waiting** (parent with live children) — parent is cancelled, and
    ///   its children are cascade-cancelled regardless of which module owns them.
    ///
    /// Children in other modules are cancelled if they were linked via
    /// `.parent()` from a task in this module.
    pub async fn cancel_all(&self) -> Result<Vec<i64>, StoreError> {
        let tasks = self
            .scheduler
            .inner
            .store
            .tasks_by_type_prefix(&self.prefix)
            .await?;
        let mut cancelled = Vec::new();
        for task in &tasks {
            if self.scheduler.cancel(task.id).await? {
                cancelled.push(task.id);
            }
        }
        Ok(cancelled)
    }

    /// Cancel module tasks matching a predicate.
    ///
    /// Queries with `task_type LIKE '{prefix}%'` first, then applies the
    /// predicate client-side. Does not load all active tasks globally.
    pub async fn cancel_where(
        &self,
        predicate: impl Fn(&TaskRecord) -> bool,
    ) -> Result<Vec<i64>, StoreError> {
        let tasks = self
            .scheduler
            .inner
            .store
            .tasks_by_type_prefix(&self.prefix)
            .await?;
        let mut cancelled = Vec::new();
        for task in &tasks {
            if predicate(task) && self.scheduler.cancel(task.id).await? {
                cancelled.push(task.id);
            }
        }
        Ok(cancelled)
    }

    // ── Pause / resume ────────────────────────────────────────────

    /// Pause all tasks in this module.
    ///
    /// - Sets the per-module `is_paused` flag.
    /// - Pauses pending tasks in the database (status → `paused`).
    /// - Cancels running tasks' tokens and moves them to `paused` in the DB.
    ///
    /// Returns the total number of tasks paused (pending + running).
    pub async fn pause(&self) -> Result<usize, StoreError> {
        // Mark the module as paused.
        if let Some(flag) = self.scheduler.inner.module_paused.get(self.name.as_ref()) {
            flag.store(true, AtomicOrdering::Release);
        }

        // Pause running tasks (cancel their tokens, move to paused in DB).
        let running_paused = self
            .scheduler
            .inner
            .active
            .pause_module(
                &self.prefix,
                &self.scheduler.inner.store,
                &self.scheduler.inner.event_tx,
            )
            .await;

        // Pause pending tasks in the database.
        let pending_paused = self
            .scheduler
            .inner
            .store
            .pause_pending_by_type_prefix(&self.prefix)
            .await? as usize;

        Ok(running_paused + pending_paused)
    }

    /// Resume all paused tasks in this module.
    ///
    /// Clears the per-module `is_paused` flag. If the global scheduler is
    /// also paused, the database tasks remain `paused` — they will be
    /// picked up when the global scheduler is resumed.
    ///
    /// Returns the number of tasks moved back to `pending`.
    pub async fn resume(&self) -> Result<usize, StoreError> {
        // Clear the module pause flag.
        if let Some(flag) = self.scheduler.inner.module_paused.get(self.name.as_ref()) {
            flag.store(false, AtomicOrdering::Release);
        }

        // Do not resume DB tasks if the global scheduler is paused.
        if self.scheduler.is_paused() {
            return Ok(0);
        }

        let count = self
            .scheduler
            .inner
            .store
            .resume_paused_by_type_prefix(&self.prefix)
            .await? as usize;

        if count > 0 {
            self.scheduler.inner.work_notify.notify_one();
        }

        Ok(count)
    }

    /// Returns `true` if this module has been explicitly paused via
    /// [`pause`](Self::pause).
    ///
    /// **Note:** this reflects only module-level pauses. Individual tasks
    /// paused by other means (e.g. preemption) are not reflected here.
    pub fn is_paused(&self) -> bool {
        self.scheduler
            .inner
            .module_paused
            .get(self.name.as_ref())
            .is_some_and(|f| f.load(AtomicOrdering::Acquire))
    }

    // ── Module concurrency ────────────────────────────────────────

    /// Set the maximum number of tasks from this module that may run concurrently.
    ///
    /// Overwrites any cap set at build time. A value of `0` removes the cap
    /// (unlimited concurrency for this module). Takes effect on the next
    /// dispatch cycle.
    pub fn set_max_concurrency(&self, n: usize) {
        let mut caps = self.scheduler.inner.module_caps.write().unwrap();
        if n == 0 {
            caps.remove(self.name.as_ref());
        } else {
            caps.insert(self.name.to_string(), n);
        }
    }

    /// Read the current concurrency cap for this module.
    ///
    /// Returns `0` if no cap is configured (unlimited).
    pub fn max_concurrency(&self) -> usize {
        self.scheduler
            .inner
            .module_caps
            .read()
            .unwrap()
            .get(self.name.as_ref())
            .copied()
            .unwrap_or(0)
    }

    // ── Scoped queries ────────────────────────────────────────────

    /// All active tasks in this module (any status).
    pub fn active_tasks(&self) -> Vec<TaskRecord> {
        self.scheduler.inner.active.records(Some(&self.prefix))
    }

    /// Dead-lettered tasks in this module, newest first.
    pub async fn dead_letter_tasks(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<TaskHistoryRecord>, StoreError> {
        self.scheduler
            .inner
            .store
            .dead_letter_tasks_by_prefix(&self.prefix, limit, offset)
            .await
    }

    /// Find module tasks matching all specified tag filters (AND semantics).
    pub async fn tasks_by_tags(
        &self,
        filters: &[(&str, &str)],
        status: Option<TaskStatus>,
    ) -> Result<Vec<TaskRecord>, StoreError> {
        self.scheduler
            .inner
            .store
            .tasks_by_tags_with_prefix(&self.prefix, filters, status)
            .await
    }

    /// Estimated progress for all running tasks in this module.
    pub async fn estimated_progress(&self) -> Vec<EstimatedProgress> {
        let snapshots = self
            .scheduler
            .inner
            .active
            .progress_snapshots(Some(&self.prefix));
        let mut results = Vec::with_capacity(snapshots.len());
        for (record, reported, reported_at) in snapshots {
            results.push(
                crate::scheduler::progress::extrapolate(
                    &record,
                    reported,
                    reported_at,
                    &self.scheduler.inner.store,
                )
                .await,
            );
        }
        results
    }

    /// Byte-level progress for module tasks reporting transfer progress.
    pub fn byte_progress(&self) -> Vec<TaskProgress> {
        let snapshots = self
            .scheduler
            .inner
            .active
            .byte_progress_snapshots(Some(&self.prefix));
        snapshots
            .into_iter()
            .filter(|(_, _, _, _, completed, _, _, _)| *completed > 0)
            .map(
                |(
                    task_id,
                    task_type,
                    key,
                    label,
                    bytes_completed,
                    bytes_total,
                    _parent_id,
                    started_at,
                )| {
                    TaskProgress {
                        task_id,
                        task_type,
                        key,
                        label,
                        bytes_completed,
                        bytes_total,
                        throughput_bps: 0.0,
                        elapsed: started_at.elapsed(),
                        eta: None,
                    }
                },
            )
            .collect()
    }

    /// Capture a status snapshot for this module.
    pub async fn snapshot(&self) -> Result<ModuleSnapshot, StoreError> {
        let running = self.active_tasks();
        let pending_count = self
            .scheduler
            .inner
            .store
            .pending_count_by_prefix(&self.prefix)
            .await?;
        let paused_count = self
            .scheduler
            .inner
            .store
            .paused_count_by_prefix(&self.prefix)
            .await?;
        let progress = self.estimated_progress().await;
        let byte_progress = self.byte_progress();
        Ok(ModuleSnapshot {
            running,
            pending_count,
            paused_count,
            progress,
            byte_progress,
            is_paused: self.is_paused(),
        })
    }

    // ── Event subscription ────────────────────────────────────────

    /// Subscribe to scheduler lifecycle events for this module.
    ///
    /// Returns a [`ModuleReceiver`] that filters the global event stream,
    /// surfacing only events whose `task_type` belongs to this module.
    /// `Paused` / `Resumed` global events are always forwarded.
    pub fn subscribe(&self) -> ModuleReceiver<crate::scheduler::SchedulerEvent> {
        ModuleReceiver {
            inner: self.scheduler.inner.event_tx.subscribe(),
            name: self.name.clone(),
            prefix: self.prefix.clone(),
        }
    }

    /// Subscribe to byte-level progress events for this module.
    ///
    /// Returns a [`ModuleReceiver`] that filters the global progress stream.
    pub fn subscribe_progress(&self) -> ModuleReceiver<TaskProgress> {
        ModuleReceiver {
            inner: self.scheduler.inner.progress_tx.subscribe(),
            name: self.name.clone(),
            prefix: self.prefix.clone(),
        }
    }

    // ── Recurring task control ────────────────────────────────────

    /// Pause a recurring schedule. Validates the task belongs to this module.
    pub async fn pause_recurring(&self, task_id: i64) -> Result<(), StoreError> {
        self.validate_ownership(task_id).await?;
        self.scheduler.pause_recurring(task_id).await
    }

    /// Resume a paused recurring schedule. Validates the task belongs to this module.
    pub async fn resume_recurring(&self, task_id: i64) -> Result<(), StoreError> {
        self.validate_ownership(task_id).await?;
        self.scheduler.resume_recurring(task_id).await
    }

    /// Cancel a recurring schedule entirely. Validates the task belongs to this module.
    pub async fn cancel_recurring(&self, task_id: i64) -> Result<bool, StoreError> {
        if !self.task_belongs(task_id).await? {
            return Ok(false);
        }
        self.scheduler.cancel_recurring(task_id).await
    }

    // ── Private helpers ───────────────────────────────────────────

    /// Returns `true` if the active task with `task_id` has a `task_type`
    /// that starts with this module's prefix. Returns `false` if the task
    /// doesn't exist or belongs to a different module.
    async fn task_belongs(&self, task_id: i64) -> Result<bool, StoreError> {
        // Fast path: check the in-memory active map first.
        let records = self.scheduler.inner.active.records(None);
        if let Some(r) = records.iter().find(|r| r.id == task_id) {
            return Ok(r.task_type.starts_with(self.prefix.as_ref()));
        }
        // Fall back to DB (pending / paused tasks not in the active map).
        match self.scheduler.inner.store.task_by_id(task_id).await? {
            Some(r) => Ok(r.task_type.starts_with(self.prefix.as_ref())),
            None => Ok(false),
        }
    }

    /// Validate that a task belongs to this module, returning an error otherwise.
    async fn validate_ownership(&self, task_id: i64) -> Result<(), StoreError> {
        if self.task_belongs(task_id).await? {
            Ok(())
        } else {
            Err(StoreError::InvalidState(format!(
                "task {task_id} does not belong to module '{}'",
                self.name
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::domain::{Domain, DomainKey, TypedExecutor};
    use crate::priority::Priority;
    use crate::registry::TaskContext;
    use crate::task::retry::{BackoffStrategy, RetryPolicy};
    use crate::task::{TaskError, TypedTask};

    use super::*;

    struct NoopExecutor;

    impl<T: TypedTask> TypedExecutor<T> for NoopExecutor {
        async fn execute<'a>(
            &'a self,
            _payload: T,
            _ctx: &'a TaskContext,
        ) -> Result<(), TaskError> {
            Ok(())
        }
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct ThumbTask {
        path: String,
    }

    struct MediaDomain;
    impl DomainKey for MediaDomain {
        const NAME: &'static str = "media";
    }

    impl TypedTask for ThumbTask {
        type Domain = MediaDomain;
        const TASK_TYPE: &'static str = "thumbnail";
    }

    #[test]
    fn new_stores_name_and_typed_executor_reads_task_type() {
        let domain = Domain::<MediaDomain>::new().task::<ThumbTask>(NoopExecutor);
        let module = domain.into_module();

        assert_eq!(module.name(), "media");
        assert_eq!(module.prefix(), "media::");
        assert_eq!(module.executors.len(), 1);
        assert_eq!(module.executors[0].task_type, "thumbnail");
    }

    #[test]
    fn default_setters_populate_fields_and_tags_merge() {
        let policy = RetryPolicy {
            strategy: BackoffStrategy::Constant {
                delay: Duration::from_secs(1),
            },
            max_retries: 5,
        };
        let module = Module::new("sync")
            .default_priority(Priority::BACKGROUND)
            .default_retry_policy(policy)
            .default_group("remote-api")
            .default_ttl(Duration::from_secs(3600))
            .default_tag("env", "prod")
            .default_tag("team", "backend")
            .max_concurrency(4);

        assert_eq!(module.default_priority, Some(Priority::BACKGROUND));
        let rp = module.default_retry_policy.as_ref().unwrap();
        assert_eq!(rp.max_retries, 5);
        assert_eq!(module.default_group.as_deref(), Some("remote-api"));
        assert_eq!(module.default_ttl, Some(Duration::from_secs(3600)));
        assert_eq!(
            module.default_tags.get("env").map(|s| s.as_str()),
            Some("prod")
        );
        assert_eq!(
            module.default_tags.get("team").map(|s| s.as_str()),
            Some("backend")
        );
        assert_eq!(module.max_concurrency, Some(4));
    }

    #[test]
    fn default_tags_merges_multiple_calls() {
        let module = Module::new("analytics")
            .default_tag("env", "staging")
            .default_tag("env", "prod") // overwrites
            .default_tag("region", "us-east-1");

        assert_eq!(
            module.default_tags.get("env").map(|s| s.as_str()),
            Some("prod")
        );
        assert_eq!(
            module.default_tags.get("region").map(|s| s.as_str()),
            Some("us-east-1")
        );
        assert_eq!(module.default_tags.len(), 2);
    }

    #[test]
    #[should_panic(expected = "must not be empty")]
    fn new_empty_name_panics() {
        Module::new("");
    }

    #[test]
    #[should_panic(expected = "must not contain '::'")]
    fn new_name_with_separator_panics() {
        Module::new("a::b");
    }
}
