//! Domain-centric API — typed module identity, typed executors, and typed handles.
//!
//! This module replaces the stringly-typed `Module` / `ModuleHandle` public API
//! with compile-time–enforced domain types. Internally the scheduler still uses
//! module names and prefixes; the domain layer adds type safety on top.
//!
//! # Overview
//!
//! - [`DomainKey`] — marker trait giving a module a compile-time identity.
//! - [`TaskTypeConfig`] — static per-task-type defaults (priority, IO budget, TTL, retry, etc.).
//! - [`TypedExecutor<T>`] — executor trait that receives a deserialized, typed payload.
//! - [`Domain<D>`] — typed module builder (replaces `Module`).
//! - [`DomainHandle<D>`] — typed module handle (replaces `ModuleHandle`).
//! - [`DomainSubmitBuilder<D>`] — typed per-call override builder.

use std::collections::HashMap;
use std::future::{Future, IntoFuture};
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};

use crate::module::{ExecutorOptions, ModuleExecutor, ModuleHandle};
use crate::priority::Priority;
use crate::registry::{DomainTaskContext, ErasedExecutor, TaskContext, TaskExecutor};
use crate::scheduler::progress::TaskProgress;
use crate::scheduler::SchedulerEvent;
use crate::store::StoreError;
use crate::task::retry::RetryPolicy;
use crate::task::submit_builder::TypedTaskDefaults;
use crate::task::{
    DependencyFailurePolicy, DuplicateStrategy, IoBudget, RecurringSchedule, SubmitOutcome,
    TaskError, TaskHistoryRecord, TaskRecord, TaskStatus, TaskSubmission, TtlFrom, TypedTask,
};

// ── DomainKey ────────────────────────────────────────────────────────

/// Marker trait that gives a module a compile-time identity.
///
/// Implement this on a zero-sized struct in the module's own crate.
/// The scheduler uses [`NAME`](Self::NAME) to namespace task types in the
/// database (e.g. `"media::thumbnail"`).
///
/// # Example
///
/// ```ignore
/// pub struct Media;
/// impl DomainKey for Media { const NAME: &'static str = "media"; }
/// ```
pub trait DomainKey: Send + Sync + 'static {
    /// The module name used as a task-type prefix in the database.
    ///
    /// Must not be empty and must not contain `"::"`.
    const NAME: &'static str;
}

// ── TaskTypeConfig ───────────────────────────────────────────────────

/// Static task-type configuration — set once via [`TypedTask::config()`],
/// applies to all instances of that task type.
///
/// All fields are `Option` — `None` means "use the next layer's default"
/// (domain default → scheduler global default → built-in default).
///
/// # Example
///
/// ```ignore
/// fn config() -> TaskTypeConfig {
///     TaskTypeConfig::new()
///         .priority(Priority::NORMAL)
///         .expected_io(IoBudget::disk(4096, 1024))
///         .ttl(Duration::from_secs(3600))
///         .retry(RetryPolicy::exponential(3, Duration::from_secs(1), Duration::from_secs(60)))
///         .on_duplicate(DuplicateStrategy::Supersede)
/// }
/// ```
#[derive(Default, Clone)]
pub struct TaskTypeConfig {
    pub priority: Option<Priority>,
    pub expected_io: Option<IoBudget>,
    pub group_key: Option<String>,
    pub on_duplicate: Option<DuplicateStrategy>,
    pub ttl: Option<Duration>,
    pub ttl_from: Option<TtlFrom>,
    pub run_after: Option<Duration>,
    pub recurring: Option<RecurringSchedule>,
    pub retry_policy: Option<RetryPolicy>,
}

impl TaskTypeConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn priority(mut self, p: Priority) -> Self {
        self.priority = Some(p);
        self
    }

    pub fn expected_io(mut self, io: IoBudget) -> Self {
        self.expected_io = Some(io);
        self
    }

    pub fn group(mut self, key: impl Into<String>) -> Self {
        self.group_key = Some(key.into());
        self
    }

    pub fn on_duplicate(mut self, s: DuplicateStrategy) -> Self {
        self.on_duplicate = Some(s);
        self
    }

    pub fn ttl(mut self, d: Duration) -> Self {
        self.ttl = Some(d);
        self
    }

    pub fn ttl_from(mut self, t: TtlFrom) -> Self {
        self.ttl_from = Some(t);
        self
    }

    pub fn run_after(mut self, d: Duration) -> Self {
        self.run_after = Some(d);
        self
    }

    pub fn recurring(mut self, s: RecurringSchedule) -> Self {
        self.recurring = Some(s);
        self
    }

    pub fn retry(mut self, p: RetryPolicy) -> Self {
        self.retry_policy = Some(p);
        self
    }
}

// ── TaskTypeOptions ──────────────────────────────────────────────────

/// Per-task-type configuration overrides for [`Domain::task_with()`].
///
/// Use only to override config that cannot be expressed statically
/// (e.g. executor-instance-dependent TTL).
#[derive(Default, Clone)]
pub struct TaskTypeOptions {
    /// Override the per-type default TTL from [`TypedTask::config()`].
    pub ttl: Option<Duration>,
    /// Override the per-type retry policy from [`TypedTask::config()`].
    pub retry_policy: Option<RetryPolicy>,
}

// ── TypedExecutor<T> ─────────────────────────────────────────────────

/// An executor that receives a deserialized, typed payload and a
/// domain-parameterized context.
///
/// Register with [`Domain::task::<T>(executor)`](Domain::task). The library
/// wraps this in an erased adapter so the scheduler engine remains untyped
/// internally.
///
/// The [`DomainTaskContext`] carries the domain identity `D` as a type
/// parameter, enabling compile-time–safe child spawning via
/// [`spawn_child_with`](DomainTaskContext::spawn_child_with).
///
/// # Example
///
/// ```ignore
/// impl TypedExecutor<Thumbnail> for ThumbnailExec {
///     async fn execute(
///         &self,
///         thumb: Thumbnail,
///         ctx: DomainTaskContext<'_, Media>,
///     ) -> Result<(), TaskError> {
///         ctx.check_cancelled()?;
///         ctx.spawn_child_with(ResizeTask { path: thumb.path, size: 256 })
///             .await?;
///         Ok(())
///     }
/// }
/// ```
pub trait TypedExecutor<
    T: TypedTask,
    Memo: Serialize + DeserializeOwned + Send + Sync + 'static = (),
>: Send + Sync + 'static
{
    /// Primary execution. Called once per dispatch.
    ///
    /// Returns a `Memo` that will be persisted and passed to [`finalize()`](Self::finalize)
    /// after all children complete. For the default `Memo = ()`, the return type
    /// is `Result<(), TaskError>` — identical to the pre-memo API.
    fn execute<'a>(
        &'a self,
        payload: T,
        ctx: DomainTaskContext<'a, T::Domain>,
    ) -> impl Future<Output = Result<Memo, TaskError>> + Send + 'a;

    /// Called when all child tasks spawned by this task have settled.
    /// Receives the `Memo` returned by [`execute()`](Self::execute).
    /// Default: no-op.
    fn finalize<'a>(
        &'a self,
        _payload: T,
        _memo: Memo,
        _ctx: DomainTaskContext<'a, T::Domain>,
    ) -> impl Future<Output = Result<(), TaskError>> + Send + 'a {
        async { Ok(()) }
    }

    /// Called when the task is cancelled (preemption, explicit cancel, TTL expiry).
    /// Default: no-op.
    fn on_cancel<'a>(
        &'a self,
        _payload: T,
        _ctx: DomainTaskContext<'a, T::Domain>,
    ) -> impl Future<Output = Result<(), TaskError>> + Send + 'a {
        async { Ok(()) }
    }
}

// ── TypedExecutorAdapter ─────────────────────────────────────────────

/// Internal adapter that wraps a [`TypedExecutor<T, Memo>`] into a [`TaskExecutor`]
/// for the scheduler engine.
///
/// Handles payload deserialization and memo serialization/deserialization.
struct TypedExecutorAdapter<T, M, E> {
    executor: E,
    _marker: PhantomData<fn() -> (T, M)>,
}

impl<T, M, E> TaskExecutor for TypedExecutorAdapter<T, M, E>
where
    T: TypedTask,
    M: Serialize + DeserializeOwned + Send + Sync + 'static,
    E: TypedExecutor<T, M>,
{
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<Option<Vec<u8>>, TaskError> {
        let payload: T = ctx.payload()?;
        let dctx = DomainTaskContext::<T::Domain>::new(ctx);
        let memo = self.executor.execute(payload, dctx).await?;

        // Don't persist () — serialize to None.
        if std::any::TypeId::of::<M>() == std::any::TypeId::of::<()>() {
            return Ok(None);
        }

        let bytes = serde_json::to_vec(&memo)
            .map_err(|e| TaskError::permanent(format!("memo serialization: {e}")))?;
        Ok(Some(bytes))
    }

    async fn finalize<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        let payload: T = ctx.payload()?;
        let memo: M = match &ctx.record().memo {
            Some(bytes) => serde_json::from_slice(bytes)
                .map_err(|e| TaskError::permanent(format!("memo deserialization: {e}")))?,
            None => serde_json::from_value(serde_json::Value::Null)
                .map_err(|e| TaskError::permanent(format!("memo deserialization: {e}")))?,
        };
        let dctx = DomainTaskContext::<T::Domain>::new(ctx);
        self.executor.finalize(payload, memo, dctx).await
    }

    async fn on_cancel<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        let payload: T = ctx.payload()?;
        let dctx = DomainTaskContext::<T::Domain>::new(ctx);
        self.executor.on_cancel(payload, dctx).await
    }
}

/// Build an erased executor from a typed executor and memo type.
fn erase_executor<T, M, E>(executor: E) -> Arc<dyn ErasedExecutor>
where
    T: TypedTask,
    M: Serialize + DeserializeOwned + Send + Sync + 'static,
    E: TypedExecutor<T, M>,
{
    Arc::new(TypedExecutorAdapter {
        executor,
        _marker: PhantomData::<fn() -> (T, M)>,
    })
}

// ── Domain<D> ────────────────────────────────────────────────────────

/// A typed module builder that enforces the link between a [`DomainKey`],
/// its tasks, and their executors at registration time.
///
/// Replaces `Module` with compile-time domain identity.
/// Internally builds a `Module` for the scheduler engine.
///
/// # Example
///
/// ```ignore
/// pub struct Media;
/// impl DomainKey for Media { const NAME: &'static str = "media"; }
///
/// let domain = Domain::<Media>::new()
///     .task::<Thumbnail>(ThumbnailExec::new(cdn))
///     .task::<Transcode>(TranscodeExec::new())
///     .default_retry(RetryPolicy::exponential(3, Duration::from_secs(1), Duration::from_secs(120)))
///     .max_concurrency(4)
///     .state(MediaConfig { cdn_url: "...".into() });
/// ```
pub struct Domain<D: DomainKey> {
    pub(crate) name: String,
    pub(crate) executors: Vec<ModuleExecutor>,
    pub(crate) default_priority: Option<Priority>,
    pub(crate) default_retry_policy: Option<RetryPolicy>,
    pub(crate) default_group: Option<String>,
    pub(crate) default_ttl: Option<Duration>,
    pub(crate) default_tags: HashMap<String, String>,
    pub(crate) max_concurrency: Option<usize>,
    pub(crate) app_state_entries: Vec<(std::any::TypeId, Arc<dyn std::any::Any + Send + Sync>)>,
    _key: PhantomData<D>,
}

impl<D: DomainKey> Domain<D> {
    /// Create a new domain. The module name is taken from `D::NAME`.
    pub fn new() -> Self {
        let name = D::NAME.to_string();
        assert!(!name.is_empty(), "domain name must not be empty");
        assert!(
            !name.contains("::"),
            "domain name must not contain '::' (reserved separator)"
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
            _key: PhantomData,
        }
    }

    /// Register a typed executor for task type `T`.
    ///
    /// The compiler enforces that `T::Domain == D` and that `E` implements
    /// `TypedExecutor<T>`. No `Arc::new()` required — the library wraps
    /// internally.
    ///
    /// TTL and retry policy from [`T::config()`](TypedTask::config) are
    /// automatically registered as per-type defaults.
    ///
    /// # Example
    ///
    /// ```ignore
    /// domain.task::<Thumbnail>(ThumbnailExec::new(cdn))
    /// ```
    pub fn task<T>(self, executor: impl TypedExecutor<T>) -> Self
    where
        T: TypedTask<Domain = D>,
    {
        let config = T::config();
        self.task_inner::<T>(
            erase_executor::<T, (), _>(executor),
            config.ttl,
            config.retry_policy,
        )
    }

    /// Register a typed executor that produces a memo in `execute()` which
    /// is persisted and passed to `finalize()`.
    ///
    /// Both `T` and `Memo` are inferred from the executor's
    /// `TypedExecutor<T, Memo>` impl — turbofish is only needed when the
    /// executor is generic over task types.
    ///
    /// # Example
    ///
    /// ```ignore
    /// domain.task_memo(ScanL1Executor)
    /// ```
    pub fn task_memo<T, Memo>(self, executor: impl TypedExecutor<T, Memo>) -> Self
    where
        T: TypedTask<Domain = D>,
        Memo: Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        let config = T::config();
        self.task_inner::<T>(
            erase_executor::<T, Memo, _>(executor),
            config.ttl,
            config.retry_policy,
        )
    }

    /// Register a typed executor with per-type option overrides.
    ///
    /// Prefer setting defaults in [`T::config()`](TypedTask::config). Use
    /// this only to override config that cannot be expressed statically
    /// (e.g. executor-instance-dependent TTL).
    pub fn task_with<T>(self, executor: impl TypedExecutor<T>, options: TaskTypeOptions) -> Self
    where
        T: TypedTask<Domain = D>,
    {
        let config = T::config();
        let ttl = options.ttl.or(config.ttl);
        let retry_policy = options.retry_policy.or(config.retry_policy);
        self.task_inner::<T>(erase_executor::<T, (), _>(executor), ttl, retry_policy)
    }

    /// Like [`task_with()`](Self::task_with), but for executors that produce
    /// a memo (see [`task_memo()`](Self::task_memo)).
    pub fn task_with_memo<T, Memo>(
        self,
        executor: impl TypedExecutor<T, Memo>,
        options: TaskTypeOptions,
    ) -> Self
    where
        T: TypedTask<Domain = D>,
        Memo: Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        let config = T::config();
        let ttl = options.ttl.or(config.ttl);
        let retry_policy = options.retry_policy.or(config.retry_policy);
        self.task_inner::<T>(erase_executor::<T, Memo, _>(executor), ttl, retry_policy)
    }

    fn task_inner<T: TypedTask>(
        mut self,
        executor: Arc<dyn ErasedExecutor>,
        ttl: Option<Duration>,
        retry_policy: Option<RetryPolicy>,
    ) -> Self {
        self.executors.push(ModuleExecutor {
            task_type: T::TASK_TYPE.to_string(),
            executor,
            options: ExecutorOptions { ttl, retry_policy },
        });
        self
    }

    /// Set the domain-wide default priority.
    pub fn default_priority(mut self, p: Priority) -> Self {
        self.default_priority = Some(p);
        self
    }

    /// Set the domain-wide default retry policy.
    pub fn default_retry(mut self, policy: RetryPolicy) -> Self {
        self.default_retry_policy = Some(policy);
        self
    }

    /// Set the domain-wide default group key.
    pub fn default_group(mut self, key: impl Into<String>) -> Self {
        self.default_group = Some(key.into());
        self
    }

    /// Set the domain-wide default TTL.
    pub fn default_ttl(mut self, d: Duration) -> Self {
        self.default_ttl = Some(d);
        self
    }

    /// Add a default tag applied to all tasks submitted through this domain's
    /// handle. Later calls with the same key overwrite the previous value.
    pub fn default_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.default_tags.insert(key.into(), value.into());
        self
    }

    /// Set the domain-level concurrency cap.
    pub fn max_concurrency(mut self, n: usize) -> Self {
        self.max_concurrency = Some(n);
        self
    }

    /// Attach domain-scoped state. Accessible in executors via
    /// `ctx.state::<S>()`.
    pub fn state<S: Send + Sync + 'static>(self, value: S) -> Self {
        self.state_arc(Arc::new(value))
    }

    /// Attach domain-scoped state from a pre-existing `Arc`.
    pub fn state_arc<S: Send + Sync + 'static>(mut self, value: Arc<S>) -> Self {
        self.app_state_entries
            .push((std::any::TypeId::of::<S>(), value));
        self
    }

    /// Convert this domain into an internal `Module` for the scheduler engine.
    pub(crate) fn into_module(self) -> crate::module::Module {
        crate::module::Module {
            name: self.name,
            executors: self.executors,
            default_priority: self.default_priority,
            default_retry_policy: self.default_retry_policy,
            default_group: self.default_group,
            default_ttl: self.default_ttl,
            default_tags: self.default_tags,
            max_concurrency: self.max_concurrency,
            app_state_entries: self.app_state_entries,
        }
    }
}

impl<D: DomainKey> Default for Domain<D> {
    fn default() -> Self {
        Self::new()
    }
}

// ── DomainHandle<D> ──────────────────────────────────────────────────

/// A typed, clonable handle to the scheduler scoped to domain `D`.
///
/// Obtain at startup via [`Scheduler::domain::<D>()`](crate::Scheduler::domain),
/// then clone into services, Tauri state, Axum extractors, etc.
///
/// All submission methods enforce at compile time that the task's
/// `Domain` matches `D`.
#[derive(Clone)]
pub struct DomainHandle<D: DomainKey> {
    pub(crate) inner: ModuleHandle,
    _key: PhantomData<D>,
}

impl<D: DomainKey> DomainHandle<D> {
    pub(crate) fn new(inner: ModuleHandle) -> Self {
        Self {
            inner,
            _key: PhantomData,
        }
    }

    /// The domain name (e.g. `"media"`).
    pub fn name(&self) -> &str {
        self.inner.name()
    }

    // ── Submission ──────────────────────────────────────────────────

    /// Submit a typed task belonging to this domain.
    ///
    /// Compile error if `T::Domain != D`. Takes ownership of the task
    /// (use `.clone()` if you need the value afterward).
    ///
    /// Applies the precedence chain: `TypedTask::config()` < domain
    /// defaults < scheduler global defaults.
    pub async fn submit<T: TypedTask<Domain = D>>(
        &self,
        task: T,
    ) -> Result<SubmitOutcome, StoreError> {
        self.build_typed_submit(task).await
    }

    /// Submit with per-call overrides (priority, key, dedup strategy, etc.).
    ///
    /// Returns a [`DomainSubmitBuilder`] — chain override methods then
    /// `.await` to execute.
    pub fn submit_with<T: TypedTask<Domain = D>>(&self, task: T) -> DomainSubmitBuilder<D> {
        DomainSubmitBuilder {
            inner: self.build_typed_submit_builder(task),
            _key: PhantomData,
        }
    }

    /// Submit a batch of typed tasks.
    pub async fn submit_batch<T: TypedTask<Domain = D>>(
        &self,
        tasks: impl IntoIterator<Item = T>,
    ) -> Result<Vec<SubmitOutcome>, StoreError> {
        let mut outcomes = Vec::new();
        for task in tasks {
            outcomes.push(self.submit(task).await?);
        }
        Ok(outcomes)
    }

    // ── Lifecycle ───────────────────────────────────────────────────

    /// Cancel a task by ID.
    pub async fn cancel(&self, task_id: i64) -> Result<bool, StoreError> {
        self.inner.cancel(task_id).await
    }

    /// Pause all tasks in this domain.
    pub async fn pause(&self) -> Result<usize, StoreError> {
        self.inner.pause().await
    }

    /// Resume all paused tasks in this domain.
    pub async fn resume(&self) -> Result<usize, StoreError> {
        self.inner.resume().await
    }

    /// Returns `true` if this domain has been explicitly paused.
    pub fn is_paused(&self) -> bool {
        self.inner.is_paused()
    }

    // ── Group pause / resume ────────────────────────────────────────

    /// Pause all tasks in a group. Delegates to the scheduler — group pause is
    /// global, not scoped to this domain.
    pub async fn pause_group(&self, group_key: &str) -> Result<(), StoreError> {
        self.inner.pause_group(group_key).await
    }

    /// Resume a paused group.
    pub async fn resume_group(&self, group_key: &str) -> Result<(), StoreError> {
        self.inner.resume_group(group_key).await
    }

    /// Check if a group is paused.
    pub fn is_group_paused(&self, group_key: &str) -> bool {
        self.inner.is_group_paused(group_key)
    }

    /// List all currently paused groups.
    pub fn paused_groups(&self) -> Vec<String> {
        self.inner.paused_groups()
    }

    // ── Queries ─────────────────────────────────────────────────────

    /// Capture a status snapshot for this domain.
    pub async fn snapshot(&self) -> Result<crate::module::ModuleSnapshot, StoreError> {
        self.inner.snapshot().await
    }

    /// All active tasks in this domain.
    pub fn active_tasks(&self) -> Vec<TaskRecord> {
        self.inner.active_tasks()
    }

    /// Dead-lettered tasks in this domain, newest first.
    pub async fn dead_letter_tasks(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<TaskHistoryRecord>, StoreError> {
        self.inner.dead_letter_tasks(limit, offset).await
    }

    /// Re-submit a dead-lettered task belonging to this domain.
    pub async fn retry_dead_letter(&self, history_id: i64) -> Result<SubmitOutcome, StoreError> {
        self.inner.retry_dead_letter(history_id).await
    }

    /// Cancel all tasks belonging to this domain.
    pub async fn cancel_all(&self) -> Result<Vec<i64>, StoreError> {
        self.inner.cancel_all().await
    }

    /// Cancel domain tasks matching a predicate.
    pub async fn cancel_where(
        &self,
        predicate: impl Fn(&TaskRecord) -> bool,
    ) -> Result<Vec<i64>, StoreError> {
        self.inner.cancel_where(predicate).await
    }

    /// Return IDs of domain tasks matching all specified tag filters (AND semantics).
    pub async fn task_ids_by_tags(
        &self,
        filters: &[(&str, &str)],
        status: Option<TaskStatus>,
    ) -> Result<Vec<i64>, StoreError> {
        self.inner.task_ids_by_tags(filters, status).await
    }

    /// Discover tag keys matching a prefix within this domain.
    pub async fn tag_keys_by_prefix(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        self.inner.tag_keys_by_prefix(prefix).await
    }

    /// Return IDs of domain tasks with any tag key matching the given prefix.
    pub async fn task_ids_by_tag_key_prefix(
        &self,
        prefix: &str,
        status: Option<TaskStatus>,
    ) -> Result<Vec<i64>, StoreError> {
        self.inner.task_ids_by_tag_key_prefix(prefix, status).await
    }

    /// Count domain tasks with any tag key matching the given prefix.
    pub async fn count_by_tag_key_prefix(
        &self,
        prefix: &str,
        status: Option<TaskStatus>,
    ) -> Result<i64, StoreError> {
        self.inner.count_by_tag_key_prefix(prefix, status).await
    }

    /// Cancel all domain tasks with any tag key matching the given prefix.
    pub async fn cancel_by_tag_key_prefix(&self, prefix: &str) -> Result<Vec<i64>, StoreError> {
        self.inner.cancel_by_tag_key_prefix(prefix).await
    }

    /// Set the maximum number of concurrent tasks for this domain.
    pub fn set_max_concurrency(&self, n: usize) {
        self.inner.set_max_concurrency(n);
    }

    /// Read the current concurrency cap for this domain.
    pub fn max_concurrency(&self) -> usize {
        self.inner.max_concurrency()
    }

    // ── Events ──────────────────────────────────────────────────────

    /// Subscribe to all events for this domain.
    pub fn events(&self) -> crate::module::ModuleReceiver<SchedulerEvent> {
        self.inner.subscribe()
    }

    /// Subscribe to events for a specific task type within this domain.
    ///
    /// Returns a [`TypedEventStream<T>`] that filters the global event
    /// broadcast to only events matching `T::TASK_TYPE` within this domain.
    /// Terminal events include the [`TaskHistoryRecord`].
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut stream = media.task_events::<Thumbnail>();
    /// while let Ok(event) = stream.recv().await {
    ///     if let TaskEvent::Completed { record, .. } = event {
    ///         let thumb: Thumbnail = serde_json::from_slice(
    ///             record.payload.as_deref().unwrap(),
    ///         ).unwrap();
    ///         println!("done: {}", thumb.path);
    ///     }
    /// }
    /// ```
    pub fn task_events<T: TypedTask<Domain = D>>(&self) -> TypedEventStream<T> {
        let qualified_type = format!("{}::{}", D::NAME, T::TASK_TYPE);
        TypedEventStream {
            rx: self.inner.scheduler.inner.event_tx.subscribe(),
            qualified_type,
            scheduler: self.inner.scheduler.clone(),
            _marker: PhantomData,
        }
    }

    /// Subscribe to byte-level progress events for this domain.
    pub fn subscribe_progress(&self) -> crate::module::ModuleReceiver<TaskProgress> {
        self.inner.subscribe_progress()
    }

    // ── Recurring task control ──────────────────────────────────────

    /// Pause a recurring schedule. Validates the task belongs to this domain.
    pub async fn pause_recurring(&self, task_id: i64) -> Result<(), StoreError> {
        self.inner.pause_recurring(task_id).await
    }

    /// Resume a paused recurring schedule.
    pub async fn resume_recurring(&self, task_id: i64) -> Result<(), StoreError> {
        self.inner.resume_recurring(task_id).await
    }

    /// Cancel a recurring schedule entirely.
    pub async fn cancel_recurring(&self, task_id: i64) -> Result<bool, StoreError> {
        self.inner.cancel_recurring(task_id).await
    }

    // ── Private ─────────────────────────────────────────────────────

    /// Build a SubmitBuilder for a typed task, applying config and instance values.
    fn build_typed_submit_builder<T: TypedTask<Domain = D>>(
        &self,
        task: T,
    ) -> crate::task::SubmitBuilder {
        let config = T::config();

        let mut sub = TaskSubmission::new(T::TASK_TYPE).payload_json(&task);

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

        self.inner.submit(sub).with_typed_defaults(typed_defaults)
    }

    /// Build and immediately submit a typed task.
    async fn build_typed_submit<T: TypedTask<Domain = D>>(
        &self,
        task: T,
    ) -> Result<SubmitOutcome, StoreError> {
        self.build_typed_submit_builder(task).await
    }
}

// ── DomainSubmitBuilder<D> ───────────────────────────────────────────

/// Builder returned by [`DomainHandle::submit_with()`]. All override methods
/// consume self and return Self. Await to execute.
pub struct DomainSubmitBuilder<D: DomainKey> {
    inner: crate::task::SubmitBuilder,
    _key: PhantomData<D>,
}

impl<D: DomainKey> DomainSubmitBuilder<D> {
    /// Override the task priority.
    pub fn priority(mut self, p: Priority) -> Self {
        self.inner = self.inner.priority(p);
        self
    }

    /// Override the dedup key.
    pub fn key(mut self, k: impl Into<String>) -> Self {
        self.inner = self.inner.key(k);
        self
    }

    /// Override the group key.
    pub fn group(mut self, key: impl Into<String>) -> Self {
        self.inner = self.inner.group(key);
        self
    }

    /// Delay dispatch.
    pub fn run_after(mut self, d: Duration) -> Self {
        self.inner = self.inner.run_after(d);
        self
    }

    /// Override the time-to-live.
    pub fn ttl(mut self, d: Duration) -> Self {
        self.inner = self.inner.ttl(d);
        self
    }

    /// Add a metadata tag.
    pub fn tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.inner = self.inner.tag(key, value);
        self
    }

    /// Add a task dependency.
    pub fn depends_on(mut self, id: i64) -> Self {
        self.inner = self.inner.depends_on(id);
        self
    }

    /// Add multiple task dependencies.
    pub fn depends_on_all(mut self, ids: impl IntoIterator<Item = i64>) -> Self {
        self.inner = self.inner.depends_on_all(ids);
        self
    }

    /// Set a dependency failure policy.
    pub fn on_dependency_failure(mut self, p: DependencyFailurePolicy) -> Self {
        self.inner = self.inner.on_dependency_failure(p);
        self
    }

    /// Control whether a child failure immediately fails the parent.
    ///
    /// Defaults to `true`. Set to `false` to let the parent's `finalize()`
    /// run even when some children fail.
    pub fn fail_fast(mut self, fail_fast: bool) -> Self {
        self.inner = self.inner.fail_fast(fail_fast);
        self
    }

    /// Set the parent task ID.
    pub fn parent(mut self, id: i64) -> Self {
        self.inner = self.inner.parent(id);
        self
    }

    /// Mark this task as a child of the task currently executing in the
    /// given [`DomainTaskContext`].
    ///
    /// This is the idiomatic way to create cross-domain children:
    ///
    /// ```ignore
    /// ctx.domain::<Analytics>()
    ///     .submit_with(ScanStartedEvent { .. })
    ///     .child_of(&ctx)
    ///     .await?;
    /// ```
    pub fn child_of<D2: DomainKey>(self, ctx: &DomainTaskContext<'_, D2>) -> Self {
        self.parent(ctx.record().id)
    }

    /// Submit the task, returning the outcome.
    pub async fn submit(self) -> Result<SubmitOutcome, StoreError> {
        self.inner.submit().await
    }
}

impl<D: DomainKey> IntoFuture for DomainSubmitBuilder<D> {
    type Output = Result<SubmitOutcome, StoreError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.submit())
    }
}

// ── TaskEvent<T> ─────────────────────────────────────────────────────

/// Typed lifecycle event for a specific task type within a domain.
///
/// Obtained via [`TypedEventStream`], which is created by
/// [`DomainHandle::task_events::<T>()`](DomainHandle::task_events).
///
/// Terminal variants ([`Completed`](Self::Completed),
/// [`Failed`](Self::Failed) with `will_retry == false`,
/// [`DeadLettered`](Self::DeadLettered)) include an
/// `Arc<TaskHistoryRecord>` so subscribers can inspect the payload
/// and metadata without an extra DB round-trip in the common case.
///
/// Non-terminal variants are thin — they carry only the task ID and
/// relevant metadata from the event header.
#[derive(Debug, Clone)]
pub enum TaskEvent<T: TypedTask> {
    /// The task was dispatched and is now running.
    Dispatched { id: i64 },
    /// Progress update from the running task.
    Progress {
        id: i64,
        percent: f32,
        message: Option<String>,
    },
    /// The task entered the waiting state (parent waiting for children).
    Waiting { id: i64 },
    /// The task was cancelled.
    Cancelled { id: i64 },
    /// The task completed successfully.
    Completed {
        id: i64,
        record: Arc<TaskHistoryRecord>,
    },
    /// The task failed.
    ///
    /// When `will_retry == true`, the task is requeued and `record` is `None`.
    /// When `will_retry == false`, the task is moved to history and `record`
    /// contains the terminal history entry.
    Failed {
        id: i64,
        error: String,
        will_retry: bool,
        retry_after: Option<Duration>,
        /// Present only when `will_retry == false` (permanent failure).
        record: Option<Arc<TaskHistoryRecord>>,
    },
    /// The task exhausted retries and was moved to dead-letter state.
    DeadLettered {
        id: i64,
        error: String,
        record: Arc<TaskHistoryRecord>,
    },
    /// Variant that carries the type parameter. Never constructed.
    #[doc(hidden)]
    _Phantom(std::convert::Infallible, PhantomData<T>),
}

// ── TypedEventStream<T> ──────────────────────────────────────────────

/// Per-task-type event subscription.
///
/// Wraps the global scheduler event broadcast and filters events to a
/// single task type. Terminal events include the [`TaskHistoryRecord`]
/// fetched from the store.
///
/// Created via [`DomainHandle::task_events::<T>()`](DomainHandle::task_events).
///
/// # Example
///
/// ```ignore
/// let mut stream = media.task_events::<Thumbnail>();
/// while let Ok(event) = stream.recv().await {
///     match event {
///         TaskEvent::Completed { id, record, .. } => {
///             let thumb: Thumbnail = serde_json::from_slice(
///                 record.payload.as_deref().unwrap(),
///             ).unwrap();
///             println!("thumbnail {id} done: {}", thumb.path);
///         }
///         TaskEvent::Failed { id, error, .. } => {
///             eprintln!("thumbnail {id} failed: {error}");
///         }
///         _ => {}
///     }
/// }
/// ```
pub struct TypedEventStream<T: TypedTask> {
    rx: tokio::sync::broadcast::Receiver<SchedulerEvent>,
    /// Full prefixed task type, e.g. `"media::thumbnail"`.
    qualified_type: String,
    /// Scheduler reference for fetching `TaskHistoryRecord` on terminal events.
    scheduler: crate::scheduler::Scheduler,
    _marker: PhantomData<T>,
}

impl<T: TypedTask> TypedEventStream<T> {
    /// Receive the next event for this task type, blocking until one arrives.
    ///
    /// Events for other task types and other modules are silently discarded.
    /// Terminal events (`Completed`, `Failed` with `will_retry == false`,
    /// `DeadLettered`) include the `TaskHistoryRecord` fetched from the store.
    pub async fn recv(&mut self) -> Result<TaskEvent<T>, tokio::sync::broadcast::error::RecvError> {
        loop {
            let event = self.rx.recv().await?;
            if let Some(task_event) = self.try_convert(event).await {
                return Ok(task_event);
            }
        }
    }

    /// Attempt to convert a raw [`SchedulerEvent`] into a typed [`TaskEvent<T>`].
    ///
    /// Returns `None` if the event is not for our task type.
    async fn try_convert(&self, event: SchedulerEvent) -> Option<TaskEvent<T>> {
        match event {
            SchedulerEvent::Dispatched(ref h) if h.task_type == self.qualified_type => {
                Some(TaskEvent::Dispatched { id: h.task_id })
            }
            SchedulerEvent::Completed(ref h) if h.task_type == self.qualified_type => {
                let record = self.fetch_record(&h.key).await;
                match record {
                    Some(r) => Some(TaskEvent::Completed {
                        id: h.task_id,
                        record: Arc::new(r),
                    }),
                    None => {
                        // Record was just written — should not happen.
                        tracing::warn!(
                            task_id = h.task_id,
                            "TypedEventStream: history record not found for completed task"
                        );
                        None
                    }
                }
            }
            SchedulerEvent::Failed {
                ref header,
                ref error,
                will_retry,
                retry_after,
            } if header.task_type == self.qualified_type => {
                let record = if !will_retry {
                    self.fetch_record(&header.key).await.map(Arc::new)
                } else {
                    None
                };
                Some(TaskEvent::Failed {
                    id: header.task_id,
                    error: error.clone(),
                    will_retry,
                    retry_after,
                    record,
                })
            }
            SchedulerEvent::Cancelled(ref h) if h.task_type == self.qualified_type => {
                Some(TaskEvent::Cancelled { id: h.task_id })
            }
            SchedulerEvent::Progress {
                ref header,
                percent,
                ref message,
            } if header.task_type == self.qualified_type => Some(TaskEvent::Progress {
                id: header.task_id,
                percent,
                message: message.clone(),
            }),
            SchedulerEvent::Waiting { task_id, .. } => {
                // Waiting events don't carry a task_type, so we can't filter
                // by type. Skip them unless we can verify ownership.
                // In practice, the caller should use `events()` for Waiting.
                let _ = task_id;
                None
            }
            SchedulerEvent::DeadLettered {
                ref header,
                ref error,
                ..
            } if header.task_type == self.qualified_type => {
                let record = self.fetch_record(&header.key).await;
                match record {
                    Some(r) => Some(TaskEvent::DeadLettered {
                        id: header.task_id,
                        error: error.clone(),
                        record: Arc::new(r),
                    }),
                    None => {
                        tracing::warn!(
                            task_id = header.task_id,
                            "TypedEventStream: history record not found for dead-lettered task"
                        );
                        None
                    }
                }
            }
            _ => None,
        }
    }

    /// Fetch the most recent history record by dedup key.
    async fn fetch_record(&self, key: &str) -> Option<TaskHistoryRecord> {
        self.scheduler
            .inner
            .store
            .latest_history_by_key(key)
            .await
            .ok()
            .flatten()
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDomain;
    impl DomainKey for TestDomain {
        const NAME: &'static str = "test";
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct TestTask {
        data: String,
    }

    impl TypedTask for TestTask {
        type Domain = TestDomain;
        const TASK_TYPE: &'static str = "test-task";
    }

    struct TestExec;

    impl TypedExecutor<TestTask> for TestExec {
        async fn execute<'a>(
            &'a self,
            _payload: TestTask,
            _ctx: DomainTaskContext<'a, TestDomain>,
        ) -> Result<(), TaskError> {
            Ok(())
        }
    }

    #[test]
    fn domain_new_uses_domain_key_name() {
        let domain = Domain::<TestDomain>::new();
        assert_eq!(domain.name, "test");
    }

    #[test]
    fn domain_task_registers_executor() {
        let domain = Domain::<TestDomain>::new().task::<TestTask>(TestExec);
        assert_eq!(domain.executors.len(), 1);
        assert_eq!(domain.executors[0].task_type, "test-task");
    }

    #[test]
    fn domain_task_with_overrides_ttl() {
        let domain = Domain::<TestDomain>::new().task_with::<TestTask>(
            TestExec,
            TaskTypeOptions {
                ttl: Some(Duration::from_secs(999)),
                ..Default::default()
            },
        );
        assert_eq!(
            domain.executors[0].options.ttl,
            Some(Duration::from_secs(999))
        );
    }

    #[test]
    fn domain_default_setters() {
        let domain = Domain::<TestDomain>::new()
            .default_priority(Priority::HIGH)
            .default_retry(RetryPolicy::constant(5, Duration::from_secs(1)))
            .default_group("test-group")
            .default_ttl(Duration::from_secs(3600))
            .max_concurrency(4);

        assert_eq!(domain.default_priority, Some(Priority::HIGH));
        assert!(domain.default_retry_policy.is_some());
        assert_eq!(domain.default_group.as_deref(), Some("test-group"));
        assert_eq!(domain.default_ttl, Some(Duration::from_secs(3600)));
        assert_eq!(domain.max_concurrency, Some(4));
    }

    #[test]
    fn domain_into_module_preserves_fields() {
        let domain = Domain::<TestDomain>::new()
            .task::<TestTask>(TestExec)
            .default_priority(Priority::HIGH)
            .max_concurrency(2);

        let module = domain.into_module();
        assert_eq!(module.name(), "test");
        assert_eq!(module.executors.len(), 1);
        assert_eq!(module.default_priority, Some(Priority::HIGH));
        assert_eq!(module.max_concurrency, Some(2));
    }

    #[test]
    fn task_type_config_builder() {
        let config = TaskTypeConfig::new()
            .priority(Priority::HIGH)
            .expected_io(IoBudget::disk(4096, 1024))
            .ttl(Duration::from_secs(3600))
            .retry(RetryPolicy::exponential(
                3,
                Duration::from_secs(1),
                Duration::from_secs(60),
            ))
            .on_duplicate(DuplicateStrategy::Supersede);

        assert_eq!(config.priority, Some(Priority::HIGH));
        assert!(config.expected_io.is_some());
        assert_eq!(config.ttl, Some(Duration::from_secs(3600)));
        assert!(config.retry_policy.is_some());
        assert_eq!(config.on_duplicate, Some(DuplicateStrategy::Supersede));
    }
}
