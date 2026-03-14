//! Executor registration, shared state, and the [`TaskContext`] passed to each task.
//!
//! Register one [`TaskExecutor`] per task type via
//! [`SchedulerBuilder::executor`](crate::SchedulerBuilder::executor) or
//! [`typed_executor`](crate::SchedulerBuilder::typed_executor). At dispatch
//! time the scheduler looks up the executor by name and calls
//! [`execute`](TaskExecutor::execute) with a [`TaskContext`] containing the
//! persisted record, a cancellation token, a progress reporter, and any
//! shared application state registered via
//! [`SchedulerBuilder::app_state`](crate::SchedulerBuilder::app_state).

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::scheduler::{ProgressReporter, Scheduler};
use crate::store::{StoreError, TaskStore};
use crate::task::{SubmitOutcome, TaskError, TaskRecord, TaskSubmission, TypedTask};

// ── State Map ────────────────────────────────────────────────────────

/// Type-keyed map of shared application state.
///
/// Multiple state types can be registered (one value per concrete type).
/// Executors retrieve them via [`TaskContext::state::<T>()`]. This is the
/// same pattern used by Axum `Extensions` and Tauri `State`.
///
/// The map supports post-build insertion via [`Scheduler::register_state`]
/// so that library consumers (e.g. shoebox inside a Tauri app) can inject
/// state after the scheduler has been constructed by the parent.
#[derive(Default)]
pub(crate) struct StateMap {
    inner: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl StateMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a `StateMap` from pre-collected entries.
    pub(crate) fn from_entries(entries: Vec<(TypeId, Arc<dyn Any + Send + Sync>)>) -> Self {
        Self {
            inner: RwLock::new(entries.into_iter().collect()),
        }
    }

    /// Insert a state value. Overwrites any previous value of the same type.
    pub async fn insert<T: Send + Sync + 'static>(&self, value: Arc<T>) {
        self.inner.write().await.insert(TypeId::of::<T>(), value);
    }
}

/// Snapshot of state for passing into a [`TaskContext`].
///
/// Created by cloning the inner map under the lock once, then used
/// lock-free for the lifetime of the task execution.
#[derive(Clone, Default)]
pub(crate) struct StateSnapshot {
    entries: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl StateSnapshot {
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.entries
            .get(&TypeId::of::<T>())
            .and_then(|arc| arc.downcast_ref::<T>())
    }
}

impl StateMap {
    /// Take a lock-free snapshot for use inside a task context.
    pub(crate) async fn snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            entries: self.inner.read().await.clone(),
        }
    }
}

// ── Child Spawner ────────────────────────────────────────────────

/// Handle for spawning child tasks from within an executor.
///
/// Wraps a [`TaskStore`] reference and the parent task ID so that
/// child submissions automatically inherit the parent relationship.
/// Holds a `Notify` reference to wake the scheduler run loop after
/// spawning, so children are dispatched promptly.
#[derive(Clone)]
pub(crate) struct ChildSpawner {
    store: TaskStore,
    parent_id: i64,
    work_notify: Arc<tokio::sync::Notify>,
}

impl ChildSpawner {
    pub(crate) fn new(
        store: TaskStore,
        parent_id: i64,
        work_notify: Arc<tokio::sync::Notify>,
    ) -> Self {
        Self {
            store,
            parent_id,
            work_notify,
        }
    }

    /// Submit a single child task. Sets `parent_id` automatically.
    pub async fn spawn(&self, mut sub: TaskSubmission) -> Result<SubmitOutcome, StoreError> {
        sub.parent_id = Some(self.parent_id);
        let outcome = self.store.submit(&sub).await?;
        self.work_notify.notify_one();
        Ok(outcome)
    }

    /// Submit multiple child tasks in a single transaction.
    pub async fn spawn_batch(
        &self,
        submissions: &mut [TaskSubmission],
    ) -> Result<Vec<SubmitOutcome>, StoreError> {
        for sub in submissions.iter_mut() {
            sub.parent_id = Some(self.parent_id);
        }
        let outcomes = self.store.submit_batch(submissions).await?;
        self.work_notify.notify_one();
        Ok(outcomes)
    }
}

// ── IO Tracker ────────────────────────────────────────────────────

/// Accumulated IO metrics reported by the executor during execution.
///
/// Accessible via [`TaskContext::record_read_bytes`],
/// [`TaskContext::record_write_bytes`], etc. The scheduler reads the
/// final snapshot after the executor returns.
pub(crate) struct IoTracker {
    pub read_bytes: AtomicI64,
    pub write_bytes: AtomicI64,
}

impl IoTracker {
    pub fn new() -> Self {
        Self {
            read_bytes: AtomicI64::new(0),
            write_bytes: AtomicI64::new(0),
        }
    }

    pub fn snapshot(&self) -> crate::task::TaskMetrics {
        crate::task::TaskMetrics {
            read_bytes: self.read_bytes.load(Ordering::Relaxed),
            write_bytes: self.write_bytes.load(Ordering::Relaxed),
        }
    }
}

// ── Task Context ─────────────────────────────────────────────────────

/// Execution context passed to a [`TaskExecutor`].
///
/// Bundles the task record, cancellation token, progress reporter, and
/// optional application state into a single value. This keeps the executor
/// signature stable when new contextual data is added in the future.
pub struct TaskContext {
    /// The full task record including payload, priority, and IO estimates.
    pub record: TaskRecord,
    /// Cancelled when the task is preempted. Check `token.is_cancelled()`
    /// at natural yield points and return early if set.
    pub token: CancellationToken,
    /// Report progress back to the scheduler (0.0–1.0).
    pub progress: ProgressReporter,
    /// Handle to the scheduler that dispatched this task. Allows executors to
    /// submit continuation tasks, look up other tasks, etc. without needing
    /// a separate `OnceLock` or `Arc<Scheduler>` in application state.
    pub scheduler: Scheduler,
    /// Shared application state set via [`SchedulerBuilder::app_state`](crate::SchedulerBuilder::app_state).
    pub(crate) app_state: StateSnapshot,
    /// Spawner for creating child tasks via [`spawn_child`](Self::spawn_child)
    /// and [`spawn_children`](Self::spawn_children). Present for all tasks
    /// dispatched by the scheduler — the parent relationship is set automatically
    /// when children are spawned.
    pub(crate) child_spawner: Option<ChildSpawner>,
    /// IO bytes accumulator fed by [`record_read_bytes`](Self::record_read_bytes)
    /// and [`record_write_bytes`](Self::record_write_bytes). The scheduler reads
    /// the final totals after the executor returns and stores them in history
    /// for future IO budget estimation.
    pub(crate) io: Arc<IoTracker>,
}

impl TaskContext {
    /// Deserialize the payload as a [`TypedTask`].
    ///
    /// Returns an error if the payload is missing or deserialization fails.
    /// This is the primary way to extract a typed task inside an executor.
    ///
    /// # Example
    ///
    /// ```ignore
    /// async fn execute(&self, ctx: &TaskContext) -> Result<(), TaskError> {
    ///     let task: MyTask = ctx.payload()?;
    ///     // ... do work ...
    ///     Ok(())
    /// }
    /// ```
    pub fn payload<T: TypedTask>(&self) -> Result<T, TaskError> {
        self.record
            .deserialize_payload()
            .map_err(TaskError::from)?
            .ok_or_else(|| TaskError::new("missing payload"))
    }

    /// Deserialize the payload as a [`TypedTask`].
    ///
    /// Convenience wrapper around [`TaskRecord::deserialize_payload`] that
    /// mirrors the typed submission API.
    #[deprecated(since = "2.0.0", note = "use `ctx.payload::<T>()` instead")]
    pub fn deserialize_typed<T: TypedTask>(&self) -> Result<Option<T>, serde_json::Error> {
        self.record.deserialize_payload()
    }

    /// Retrieve shared application state registered via
    /// [`SchedulerBuilder::app_state`](crate::SchedulerBuilder::app_state) or
    /// [`Scheduler::register_state`](crate::Scheduler::register_state).
    ///
    /// Returns `None` if the type was never registered. Multiple types can
    /// coexist — each is keyed by its concrete `TypeId`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// struct MyServices { db: DatabasePool, http: reqwest::Client }
    ///
    /// // In the executor:
    /// let svc = ctx.state::<MyServices>().expect("app state not set");
    /// svc.db.query("...").await?;
    /// ```
    pub fn state<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.app_state.get::<T>()
    }

    /// Record actual bytes read during this task's execution.
    ///
    /// Can be called multiple times — values are accumulated. The scheduler
    /// reads the total after the executor returns.
    pub fn record_read_bytes(&self, bytes: i64) {
        self.io.read_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record actual bytes written during this task's execution.
    ///
    /// Can be called multiple times — values are accumulated. The scheduler
    /// reads the total after the executor returns.
    pub fn record_write_bytes(&self, bytes: i64) {
        self.io.write_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Spawn a child task that will be tracked under this task as parent.
    ///
    /// The child's `parent_id` is set automatically. Returns the submit
    /// outcome, or `None` if this context was not created with hierarchy
    /// support (should not happen in normal scheduler operation).
    pub async fn spawn_child(&self, sub: TaskSubmission) -> Result<SubmitOutcome, StoreError> {
        let spawner = self
            .child_spawner
            .as_ref()
            .expect("spawn_child called on a context without ChildSpawner");
        spawner.spawn(sub).await
    }

    /// Spawn multiple child tasks in a single transaction.
    pub async fn spawn_children(
        &self,
        mut submissions: Vec<TaskSubmission>,
    ) -> Result<Vec<SubmitOutcome>, StoreError> {
        let spawner = self
            .child_spawner
            .as_ref()
            .expect("spawn_children called on a context without ChildSpawner");
        spawner.spawn_batch(&mut submissions).await
    }
}

/// Executes tasks of a registered type.
///
/// Each executor is associated with a named task type (e.g. `"scan-l3"`, `"exif"`).
/// When the scheduler pops a task, it looks up the executor by `task_type` and
/// calls `execute` with a [`TaskContext`] containing the persisted record,
/// a cancellation token, and a progress reporter.
///
/// Implementors deserialize the task's `payload` blob themselves — taskmill
/// treats it as opaque bytes.
///
/// # Example
///
/// ```ignore
/// use taskmill::{TaskExecutor, TaskContext, TaskError};
///
/// struct MyExecutor;
///
/// impl TaskExecutor for MyExecutor {
///     async fn execute<'a>(
///         &'a self,
///         ctx: &'a TaskContext,
///     ) -> Result<(), TaskError> {
///         ctx.progress.report(0.5, Some("halfway".into()));
///         Ok(())
///     }
/// }
/// ```
pub trait TaskExecutor: Send + Sync + 'static {
    /// Execute a task.
    ///
    /// - `ctx`: Execution context with the task record, cancellation token,
    ///   and progress reporter.
    ///
    /// On success, return `Ok(())`. Use [`TaskContext::record_read_bytes`]
    /// and [`TaskContext::record_write_bytes`] to report IO during execution.
    /// On failure, return a [`TaskError`] indicating whether retry is appropriate.
    fn execute<'a>(
        &'a self,
        ctx: &'a TaskContext,
    ) -> impl Future<Output = Result<(), TaskError>> + Send + 'a;

    /// Called after all children of a parent task have completed.
    ///
    /// Only invoked for tasks that spawned children via
    /// [`TaskContext::spawn_child`]. The default implementation is a no-op.
    ///
    /// Use this for cleanup or assembly work (e.g. calling
    /// `CompleteMultipartUpload` after all parts have been uploaded).
    fn finalize<'a>(
        &'a self,
        _ctx: &'a TaskContext,
    ) -> impl Future<Output = Result<(), TaskError>> + Send + 'a {
        async { Ok(()) }
    }
}

/// Registry mapping task type names to their executors.
///
/// Built during application startup before the scheduler begins popping tasks.
/// After construction, the registry is immutable (shared via `Arc`).
pub struct TaskTypeRegistry {
    types: HashMap<String, Arc<dyn ErasedExecutor>>,
}

/// Object-safe wrapper around [`TaskExecutor`] for dynamic dispatch in the registry.
///
/// This trait exists because RPITIT (`impl Future`) in `TaskExecutor` is not
/// object-safe. The blanket impl below automatically wraps any `TaskExecutor`
/// so callers never interact with `ErasedExecutor` directly.
pub(crate) trait ErasedExecutor: Send + Sync + 'static {
    fn execute_erased<'a>(
        &'a self,
        ctx: &'a TaskContext,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), TaskError>> + Send + 'a>>;

    fn finalize_erased<'a>(
        &'a self,
        ctx: &'a TaskContext,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), TaskError>> + Send + 'a>>;
}

impl<T: TaskExecutor> ErasedExecutor for T {
    fn execute_erased<'a>(
        &'a self,
        ctx: &'a TaskContext,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), TaskError>> + Send + 'a>> {
        Box::pin(self.execute(ctx))
    }

    fn finalize_erased<'a>(
        &'a self,
        ctx: &'a TaskContext,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), TaskError>> + Send + 'a>> {
        Box::pin(self.finalize(ctx))
    }
}

impl TaskTypeRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            types: HashMap::new(),
        }
    }

    /// Register an executor for a named task type.
    ///
    /// Panics if the name is already registered (catch configuration errors
    /// at startup, not at runtime).
    pub fn register<E: TaskExecutor>(&mut self, name: &str, executor: Arc<E>) {
        if self.types.contains_key(name) {
            panic!("task type '{name}' already registered");
        }
        self.types
            .insert(name.to_string(), executor as Arc<dyn ErasedExecutor>);
    }

    /// Look up the executor for a task type.
    pub(crate) fn get(&self, name: &str) -> Option<&Arc<dyn ErasedExecutor>> {
        self.types.get(name)
    }

    /// All registered type names.
    pub fn type_names(&self) -> Vec<&str> {
        self.types.keys().map(|s| s.as_str()).collect()
    }

    /// Number of registered types.
    pub fn len(&self) -> usize {
        self.types.len()
    }

    /// Returns `true` if no executors have been registered.
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Register a pre-erased executor. Used by the builder which already holds
    /// `Arc<dyn ErasedExecutor>`.
    pub(crate) fn register_erased(&mut self, name: &str, executor: Arc<dyn ErasedExecutor>) {
        if self.types.contains_key(name) {
            panic!("task type '{name}' already registered");
        }
        self.types.insert(name.to_string(), executor);
    }
}

impl Default for TaskTypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopExecutor;

    impl TaskExecutor for NoopExecutor {
        async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
            Ok(())
        }
    }

    #[test]
    fn register_and_lookup() {
        let mut reg = TaskTypeRegistry::new();
        reg.register("test-type", Arc::new(NoopExecutor));

        assert!(reg.get("test-type").is_some());
        assert!(reg.get("unknown").is_none());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    #[should_panic(expected = "already registered")]
    fn duplicate_registration_panics() {
        let mut reg = TaskTypeRegistry::new();
        reg.register("dup", Arc::new(NoopExecutor));
        reg.register("dup", Arc::new(NoopExecutor));
    }
}
