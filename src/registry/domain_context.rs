//! [`DomainTaskContext`] — domain-parameterized execution context wrapper.
//!
//! This module provides a zero-cost wrapper around [`TaskContext`] that carries
//! domain identity as a type parameter. This enables compile-time–safe child
//! spawning: [`spawn_child_with`](DomainTaskContext::spawn_child_with) only
//! accepts tasks belonging to the same domain `D`.
//!
//! # Why a wrapper instead of `TaskContext<D>`?
//!
//! Directly parameterizing `TaskContext` would break type erasure. The registry
//! stores all executors in a single `HashMap<String, Arc<dyn ErasedExecutor>>`,
//! and `ErasedExecutor` methods take `&TaskContext`. Making `TaskContext` generic
//! would require per-domain registries and force the dispatcher to know `D` at
//! runtime. The wrapper approach preserves the untyped internals while presenting
//! a typed API to executor authors.

use std::marker::PhantomData;

use tokio_util::sync::CancellationToken;

use crate::domain::{DomainHandle, DomainKey};
use crate::scheduler::ProgressReporter;
use crate::store::StoreError;
use crate::task::{SubmitOutcome, TaskError, TaskRecord, TaskSubmission, TypedTask};

use super::context::TaskContext;

/// Domain-parameterized execution context passed to [`TypedExecutor`](crate::TypedExecutor).
///
/// A zero-cost wrapper around the internal [`TaskContext`] that carries the
/// domain identity `D` as a type parameter. This enables:
///
/// - **Compile-time–safe child spawning** via [`spawn_child_with`](Self::spawn_child_with):
///   only tasks where `T::Domain == D` are accepted.
/// - **No escape hatch**: the untyped `spawn_child(TaskSubmission)` is not
///   accessible through this wrapper.
///
/// Obtain a handle to a different domain via [`domain`](Self::domain) for
/// cross-domain submission.
///
/// # Example
///
/// ```ignore
/// impl TypedExecutor<ScanRootTask> for RootScanner {
///     async fn execute(
///         &self,
///         task: ScanRootTask,
///         ctx: DomainTaskContext<'_, Scanner>,
///     ) -> Result<(), TaskError> {
///         ctx.spawn_child_with(ScanL1DirTask { bucket: task.bucket, .. })
///             .key(&format!("{}:{}", task.bucket, task.prefix))
///             .await?;
///         Ok(())
///     }
/// }
/// ```
pub struct DomainTaskContext<'a, D: DomainKey> {
    pub(crate) inner: &'a TaskContext,
    _domain: PhantomData<D>,
}

impl<'a, D: DomainKey> DomainTaskContext<'a, D> {
    /// Construct a new `DomainTaskContext` wrapping the given `TaskContext`.
    ///
    /// This is `pub(crate)` — only [`TypedExecutorAdapter`](crate::domain::TypedExecutorAdapter)
    /// should construct these.
    pub(crate) fn new(ctx: &'a TaskContext) -> Self {
        Self {
            inner: ctx,
            _domain: PhantomData,
        }
    }

    // ── Delegated accessors ─────────────────────────────────────────

    /// The persisted task record (id, key, priority, payload, etc.).
    pub fn record(&self) -> &TaskRecord {
        self.inner.record()
    }

    /// Cancellation token — check `token().is_cancelled()` for preemption.
    pub fn token(&self) -> &CancellationToken {
        self.inner.token()
    }

    /// Check whether this task has been cancelled, returning a
    /// [`TaskError::cancelled()`] if so.
    pub fn check_cancelled(&self) -> Result<(), TaskError> {
        self.inner.check_cancelled()
    }

    /// Progress reporter for this task.
    pub fn progress(&self) -> &ProgressReporter {
        self.inner.progress()
    }

    // ── Shared state ────────────────────────────────────────────────

    /// Retrieve shared application state registered via
    /// [`SchedulerBuilder::app_state`](crate::SchedulerBuilder::app_state) or
    /// [`Domain::state`](crate::Domain::state).
    pub fn state<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.inner.state::<T>()
    }

    /// Retrieve domain-scoped state.
    pub fn domain_state<D2: DomainKey, S: Send + Sync + 'static>(&self) -> Option<&S> {
        self.inner.domain_state::<D2, S>()
    }

    // ── IO tracking ─────────────────────────────────────────────────

    /// Record actual bytes read during this task's execution.
    pub fn record_read_bytes(&self, bytes: i64) {
        self.inner.record_read_bytes(bytes);
    }

    /// Record actual bytes written during this task's execution.
    pub fn record_write_bytes(&self, bytes: i64) {
        self.inner.record_write_bytes(bytes);
    }

    /// Record actual bytes received over the network.
    pub fn record_net_rx_bytes(&self, bytes: i64) {
        self.inner.record_net_rx_bytes(bytes);
    }

    /// Record actual bytes transmitted over the network.
    pub fn record_net_tx_bytes(&self, bytes: i64) {
        self.inner.record_net_tx_bytes(bytes);
    }

    // ── Byte-level progress ─────────────────────────────────────────

    /// Set the total number of bytes expected for byte-level progress.
    pub fn set_bytes_total(&self, total: u64) {
        self.inner.set_bytes_total(total);
    }

    /// Increment completed bytes by `delta` for byte-level progress.
    pub fn add_bytes(&self, delta: u64) {
        self.inner.add_bytes(delta);
    }

    /// Set both completed and total bytes to absolute values.
    pub fn report_bytes(&self, completed: u64, total: u64) {
        self.inner.report_bytes(completed, total);
    }

    // ── Domain access ───────────────────────────────────────────────

    /// Returns a typed [`DomainHandle`] for cross-domain task submission.
    ///
    /// # Panics
    ///
    /// Panics if `D2` was not registered with the scheduler.
    pub fn domain<D2: DomainKey>(&self) -> DomainHandle<D2> {
        self.inner.domain::<D2>()
    }

    /// Returns a typed [`DomainHandle`] for the given domain, or `None`
    /// if the domain is not registered.
    pub fn try_domain<D2: DomainKey>(&self) -> Option<DomainHandle<D2>> {
        self.inner.try_domain::<D2>()
    }

    // ── Typed child spawning ────────────────────────────────────────

    /// Spawn a same-domain child task with compile-time type safety.
    ///
    /// Only accepts tasks where `T::Domain == D`. Returns a
    /// [`ChildSpawnBuilder`] for optional per-call overrides (`.key()`,
    /// `.priority()`, etc.), then `.await` to submit.
    ///
    /// # Example
    ///
    /// ```ignore
    /// ctx.spawn_child_with(ScanL1DirTask { bucket, prefix })
    ///     .key(&format!("{bucket}:{prefix}"))
    ///     .await?;
    /// ```
    pub fn spawn_child_with<T: TypedTask<Domain = D>>(
        &self,
        task: T,
    ) -> ChildSpawnBuilder<'a, D, T> {
        ChildSpawnBuilder {
            ctx: self.inner,
            task,
            override_key: None,
            override_priority: None,
            override_ttl: None,
            override_group: None,
            _domain: PhantomData,
        }
    }

    /// Spawn multiple same-domain children in one call.
    ///
    /// All tasks must be the same type `T`. For mixed-type fan-out, use
    /// separate [`spawn_child_with`](Self::spawn_child_with) calls per type.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let children: Vec<ScanL1DirTask> = prefixes.into_iter()
    ///     .map(|p| ScanL1DirTask { bucket: bucket.clone(), prefix: p })
    ///     .collect();
    /// ctx.spawn_children_with(children).await?;
    /// ```
    pub async fn spawn_children_with<T: TypedTask<Domain = D>>(
        &self,
        tasks: impl IntoIterator<Item = T>,
    ) -> Result<Vec<SubmitOutcome>, StoreError> {
        let submissions: Vec<TaskSubmission> = tasks
            .into_iter()
            .map(|t| TaskSubmission::from_typed(&t))
            .collect();
        self.inner.spawn_children(submissions).await
    }
}

// ── ChildSpawnBuilder ───────────────────────────────────────────────

/// Builder for spawning a single typed child task with optional per-call
/// overrides.
///
/// Created by [`DomainTaskContext::spawn_child_with`]. Chain override methods
/// then `.await` to submit.
///
/// Implements [`IntoFuture`] so bare `.await` works:
///
/// ```ignore
/// ctx.spawn_child_with(task).key("my-key").await?;
/// ```
pub struct ChildSpawnBuilder<'a, D: DomainKey, T: TypedTask<Domain = D>> {
    ctx: &'a TaskContext,
    task: T,
    override_key: Option<String>,
    override_priority: Option<crate::priority::Priority>,
    override_ttl: Option<std::time::Duration>,
    override_group: Option<String>,
    _domain: PhantomData<D>,
}

impl<'a, D: DomainKey, T: TypedTask<Domain = D>> ChildSpawnBuilder<'a, D, T> {
    /// Override the dedup key for this child task.
    pub fn key(mut self, k: impl Into<String>) -> Self {
        self.override_key = Some(k.into());
        self
    }

    /// Override the priority for this child task.
    pub fn priority(mut self, p: crate::priority::Priority) -> Self {
        self.override_priority = Some(p);
        self
    }

    /// Override the time-to-live for this child task.
    pub fn ttl(mut self, d: std::time::Duration) -> Self {
        self.override_ttl = Some(d);
        self
    }

    /// Override the group key for this child task.
    pub fn group(mut self, key: impl Into<String>) -> Self {
        self.override_group = Some(key.into());
        self
    }

    /// Submit the child task.
    ///
    /// When aging is enabled and no explicit priority override is set,
    /// the child inherits the higher of the parent's effective priority
    /// and the child's configured priority (lower numeric value wins).
    pub async fn submit(self) -> Result<SubmitOutcome, StoreError> {
        let mut sub = TaskSubmission::from_typed(&self.task);
        if let Some(k) = self.override_key {
            sub = sub.key(k);
        }
        if let Some(p) = self.override_priority {
            sub = sub.priority(p);
        } else if let Some(ref config) = self.ctx.aging_config {
            let parent = self.ctx.record();
            let parent_effective = parent.effective_priority(Some(config));
            // Take the higher priority (lower numeric value) of parent's
            // effective and child's configured priority.
            let child_config = <T as TypedTask>::config()
                .priority
                .unwrap_or(crate::priority::Priority::NORMAL);
            let inherited =
                crate::priority::Priority::new(parent_effective.value().min(child_config.value()));
            sub = sub.priority(inherited);
        }
        if let Some(d) = self.override_ttl {
            sub = sub.ttl(d);
        }
        if let Some(g) = self.override_group {
            sub = sub.group(g);
        }
        self.ctx.spawn_child(sub).await
    }
}

impl<'a, D: DomainKey, T: TypedTask<Domain = D>> std::future::IntoFuture
    for ChildSpawnBuilder<'a, D, T>
{
    type Output = Result<SubmitOutcome, StoreError>;
    type IntoFuture =
        std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.submit())
    }
}
