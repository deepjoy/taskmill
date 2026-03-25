//! [`TaskContext`] — the execution context passed to each task executor.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::domain::{DomainHandle, DomainKey};
use crate::module::{ModuleHandle, ModuleRegistry};
use crate::scheduler::{ProgressReporter, WeakScheduler};
use crate::store::StoreError;
use crate::task::{SubmitOutcome, TaskError, TaskRecord, TaskSubmission, TypedTask};

use super::child_spawner::ChildSpawner;
use super::io_tracker::IoTracker;
use super::state::StateSnapshot;

/// Execution context passed to a [`TypedExecutor`](crate::TypedExecutor).
///
/// Provides access to the task record, cancellation token, progress reporter,
/// shared application state, and domain-scoped task submission. Use the accessor
/// methods rather than accessing fields directly:
///
/// - [`record()`](Self::record) — the full [`TaskRecord`] with payload, priority, etc.
/// - [`token()`](Self::token) — [`CancellationToken`] for preemption support
/// - [`progress()`](Self::progress) — [`ProgressReporter`] for reporting progress
/// - [`domain()`](Self::domain) — type-safe cross-domain task submission
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
    /// Registry of all registered modules — used to construct [`ModuleHandle`] instances.
    pub(crate) module_registry: Arc<ModuleRegistry>,
    /// Name of the module that owns this task (e.g. `"media"`). Empty string for
    /// tasks running outside the module system (via `Scheduler::new`).
    pub(crate) owning_module: String,
    /// Aging config from the scheduler, used for child priority inheritance.
    /// `None` = aging disabled.
    pub(crate) aging_config: Option<std::sync::Arc<crate::scheduler::aging::AgingConfig>>,
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
        match self.record.deserialize_payload().map_err(TaskError::from)? {
            Some(v) => Ok(v),
            // No payload stored — try deserializing from JSON `null`.
            // This succeeds for unit-struct typed tasks (e.g. `struct Noop;`)
            // that are submitted via raw `TaskSubmission::new(...)`.
            None => serde_json::from_value::<T>(serde_json::Value::Null)
                .map_err(|_| TaskError::new("missing payload")),
        }
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

    /// Retrieve domain-scoped state.
    ///
    /// Semantically equivalent to [`state::<S>()`](Self::state) but
    /// documents that `S` was registered via [`Domain::state()`](crate::Domain::state)
    /// for domain `D`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cfg = ctx.domain_state::<Media, MediaConfig>()
    ///     .ok_or_else(|| TaskError::permanent("missing config"))?;
    /// ```
    pub fn domain_state<D: DomainKey, S: Send + Sync + 'static>(&self) -> Option<&S> {
        let _ = std::marker::PhantomData::<D>;
        self.state::<S>()
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

    // ── Domain access ────────────────────────────────────────────────

    /// Returns a typed [`DomainHandle`] for cross-domain task submission.
    ///
    /// # Panics
    ///
    /// Panics if `D` was not registered with the scheduler. For a fallible
    /// variant, use [`try_domain`](Self::try_domain).
    ///
    /// # Example
    ///
    /// ```ignore
    /// impl TypedExecutor<VideoUploaded> for VideoProcessor {
    ///     async fn execute(&self, event: VideoUploaded, ctx: DomainTaskContext<'_, Media>) -> Result<(), TaskError> {
    ///         let media = ctx.domain::<Media>();
    ///         media.submit(Thumbnail { path: event.path.clone(), size: 256 })
    ///             .await
    ///             .map_err(TaskError::retryable)?;
    ///         Ok(())
    ///     }
    /// }
    /// ```
    pub fn domain<D: DomainKey>(&self) -> DomainHandle<D> {
        self.try_domain::<D>()
            .unwrap_or_else(|| panic!(
                "domain '{}' is not registered — did you forget to add .domain(...) to the SchedulerBuilder?",
                D::NAME
            ))
    }

    /// Returns a typed [`DomainHandle`] for the given domain, or `None`
    /// if the domain is not registered.
    pub fn try_domain<D: DomainKey>(&self) -> Option<DomainHandle<D>> {
        let scheduler = self.scheduler.upgrade()?;
        scheduler.try_domain::<D>()
    }

    // ── Module access (internal) ─────────────────────────────────────

    /// Returns a scoped handle for this task's owning module.
    ///
    /// Used internally by [`spawn_child`](Self::spawn_child) and
    /// [`spawn_children`](Self::spawn_children).
    pub(crate) fn current_module(&self) -> ModuleHandle {
        self.module(&self.owning_module)
    }

    /// Returns a scoped handle for the named module.
    pub(crate) fn module(&self, name: &str) -> ModuleHandle {
        self.try_module(name)
            .unwrap_or_else(|| panic!("module '{name}' is not registered — did you forget to add .domain(...) to the SchedulerBuilder?"))
    }

    /// Returns a scoped handle for the named module, or `None` if not registered.
    pub(crate) fn try_module(&self, name: &str) -> Option<ModuleHandle> {
        let entry = self.module_registry.get(name)?;
        let scheduler = self.scheduler.upgrade()?;
        Some(ModuleHandle::new(scheduler, entry))
    }

    // ── Child tasks ──────────────────────────────────────────────────

    /// Spawn a child task that will be tracked under this task as parent.
    ///
    /// The child's `parent_id` is set automatically, and it inherits the
    /// parent's remaining TTL and tags. The child's task type is auto-prefixed
    /// with the owning module's namespace (same-module child).
    ///
    /// For cross-module children, use
    /// `ctx.domain::<Other>().submit_with(task).parent(id).await`.
    pub async fn spawn_child(&self, sub: TaskSubmission) -> Result<SubmitOutcome, StoreError> {
        let spawner = self
            .child_spawner
            .as_ref()
            .expect("spawn_child called on a context without ChildSpawner");

        if self.owning_module.is_empty() {
            // Legacy path — no module system (Scheduler::new), use ChildSpawner directly.
            return spawner.spawn(sub).await;
        }

        // Module-aware path: inherit TTL/tags then route through the module handle
        // so the task type is auto-prefixed and module defaults are applied.
        let sub = spawner.prepare(sub);
        self.current_module().submit(sub).await
    }

    /// Spawn multiple child tasks, each tracked under this task as parent.
    ///
    /// Each submission inherits the parent's remaining TTL and tags, and is
    /// auto-prefixed with the owning module's namespace.
    pub async fn spawn_children(
        &self,
        submissions: Vec<TaskSubmission>,
    ) -> Result<Vec<SubmitOutcome>, StoreError> {
        let spawner = self
            .child_spawner
            .as_ref()
            .expect("spawn_children called on a context without ChildSpawner");

        if self.owning_module.is_empty() {
            // Legacy path — no module system.
            let mut submissions = submissions;
            return spawner.spawn_batch(&mut submissions).await;
        }

        // Module-aware path: prepare each submission then batch-submit via module handle.
        let prepared: Vec<_> = submissions
            .into_iter()
            .map(|sub| spawner.prepare(sub))
            .collect();
        self.current_module().submit_batch(prepared).await
    }
}
