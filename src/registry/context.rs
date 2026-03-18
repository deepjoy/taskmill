//! [`TaskContext`] — the execution context passed to each task executor.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::scheduler::{ProgressReporter, WeakScheduler};
use crate::store::StoreError;
use crate::task::{SubmitOutcome, TaskError, TaskRecord, TaskSubmission, TypedTask};

use super::child_spawner::ChildSpawner;
use super::io_tracker::IoTracker;
use super::state::StateSnapshot;

/// Execution context passed to a [`TaskExecutor`](super::TaskExecutor).
///
/// Provides access to the task record, cancellation token, progress reporter,
/// shared application state, and scoped task submission. Use the accessor
/// methods rather than accessing fields directly:
///
/// - [`record()`](Self::record) — the full [`TaskRecord`] with payload, priority, etc.
/// - [`token()`](Self::token) — [`CancellationToken`] for preemption support
/// - [`progress()`](Self::progress) — [`ProgressReporter`] for reporting progress
/// - [`submit()`](Self::submit) / [`submit_typed()`](Self::submit_typed) — submit continuation tasks
/// - [`spawn_child()`](Self::spawn_child) — spawn hierarchical child tasks
pub struct TaskContext {
    pub(crate) record: TaskRecord,
    pub(crate) token: CancellationToken,
    pub(crate) progress: ProgressReporter,
    pub(crate) scheduler: WeakScheduler,
    pub(crate) app_state: StateSnapshot,
    /// Module-scoped state snapshot, taken at dispatch time for the task's owning module.
    /// Checked before `app_state` in [`state`](Self::state).
    pub(crate) module_state: StateSnapshot,
    pub(crate) child_spawner: Option<ChildSpawner>,
    pub(crate) io: Arc<IoTracker>,
}

impl TaskContext {
    // ── Accessors ────────────────────────────────────────────────────

    /// The persisted task record (id, key, priority, payload, etc.).
    pub fn record(&self) -> &TaskRecord {
        &self.record
    }

    /// Cancellation token — check `token().is_cancelled()` for preemption.
    pub fn token(&self) -> &CancellationToken {
        &self.token
    }

    /// Check whether this task has been cancelled, returning a
    /// [`TaskError::cancelled()`] if so.
    ///
    /// Call this at safe yield points inside an executor to cooperatively
    /// respond to cancellation:
    ///
    /// ```ignore
    /// for chunk in chunks {
    ///     ctx.check_cancelled()?;
    ///     process(chunk).await;
    /// }
    /// ```
    pub fn check_cancelled(&self) -> Result<(), TaskError> {
        if self.token.is_cancelled() {
            Err(TaskError::cancelled())
        } else {
            Ok(())
        }
    }

    /// Progress reporter for this task.
    pub fn progress(&self) -> &ProgressReporter {
        &self.progress
    }

    // ── Payload ──────────────────────────────────────────────────────

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

    // ── Shared state ─────────────────────────────────────────────────

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
        self.module_state
            .get::<T>()
            .or_else(|| self.app_state.get::<T>())
    }

    // ── IO tracking ──────────────────────────────────────────────────

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

    /// Record actual bytes received over the network during this task's execution.
    ///
    /// Can be called multiple times — values are accumulated. The scheduler
    /// reads the total after the executor returns.
    pub fn record_net_rx_bytes(&self, bytes: i64) {
        self.io.net_rx_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record actual bytes transmitted over the network during this task's execution.
    ///
    /// Can be called multiple times — values are accumulated. The scheduler
    /// reads the total after the executor returns.
    pub fn record_net_tx_bytes(&self, bytes: i64) {
        self.io.net_tx_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    // ── Byte-level progress ────────────────────────────────────────────

    /// Set the total number of bytes expected for byte-level progress.
    ///
    /// Call once when the total is known (e.g. from a `Content-Length` header).
    pub fn set_bytes_total(&self, total: u64) {
        self.progress.set_bytes_total(total);
    }

    /// Increment completed bytes by `delta` for byte-level progress.
    ///
    /// Call per chunk in a streaming transfer.
    pub fn add_bytes(&self, delta: u64) {
        self.progress.add_bytes(delta);
    }

    /// Set both completed and total bytes to absolute values.
    pub fn report_bytes(&self, completed: u64, total: u64) {
        self.progress.report_bytes(completed, total);
    }

    // ── Task submission (scoped scheduler access) ────────────────────

    /// Submit a continuation or follow-up task.
    ///
    /// This is the primary way to enqueue new work from inside an executor
    /// without exposing the full [`Scheduler`](crate::Scheduler) handle.
    pub async fn submit(&self, sub: &TaskSubmission) -> Result<SubmitOutcome, StoreError> {
        let scheduler = self
            .scheduler
            .upgrade()
            .ok_or_else(|| StoreError::Database("scheduler has been shut down".into()))?;
        scheduler.submit(sub).await
    }

    /// Submit a [`TypedTask`], handling serialization automatically.
    ///
    /// Uses the priority from [`TypedTask::priority()`].
    pub async fn submit_typed<T: TypedTask>(&self, task: &T) -> Result<SubmitOutcome, StoreError> {
        let scheduler = self
            .scheduler
            .upgrade()
            .ok_or_else(|| StoreError::Database("scheduler has been shut down".into()))?;
        scheduler.submit_typed(task).await
    }

    /// Submit a [`TypedTask`] with an explicit priority override.
    pub async fn submit_typed_at<T: TypedTask>(
        &self,
        task: &T,
        priority: crate::Priority,
    ) -> Result<SubmitOutcome, StoreError> {
        let scheduler = self
            .scheduler
            .upgrade()
            .ok_or_else(|| StoreError::Database("scheduler has been shut down".into()))?;
        scheduler.submit_typed_at(task, priority).await
    }

    // ── Child tasks ──────────────────────────────────────────────────

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
