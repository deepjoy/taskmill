//! Executor registration, shared state, and the [`TaskContext`] passed to each task.
//!
//! Register a [`TypedExecutor<T>`](crate::TypedExecutor) per task type via
//! [`Domain::task::<T>(executor)`](crate::Domain::task). At dispatch time the
//! scheduler looks up the executor by name and calls the typed
//! [`execute`](crate::TypedExecutor::execute) method with the deserialized
//! payload and a [`TaskContext`] containing the persisted record, a
//! cancellation token, a progress reporter, and any shared application state
//! registered via [`Domain::state`](crate::Domain::state) or
//! [`SchedulerBuilder::app_state`](crate::SchedulerBuilder::app_state).

pub(crate) mod child_spawner;
mod context;
pub(crate) mod io_tracker;
pub(crate) mod state;

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use crate::task::retry::RetryPolicy;
use crate::task::TaskError;

pub(crate) use child_spawner::{ChildSpawner, ParentContext};
pub use context::TaskContext;
pub(crate) use io_tracker::IoTracker;
pub(crate) use state::{StateMap, StateSnapshot};

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
///         ctx.progress().report(0.5, Some("halfway".into()));
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

    /// Called when a running or waiting task is explicitly cancelled.
    ///
    /// Use this for cleanup of external resources (e.g. aborting an S3
    /// multipart upload). The default implementation is a no-op.
    ///
    /// The provided [`TaskContext`] has a fresh cancellation token (not the
    /// one that was cancelled) and no child spawner. The hook runs with a
    /// timeout configured via
    /// [`SchedulerBuilder::cancel_hook_timeout`](crate::SchedulerBuilder::cancel_hook_timeout).
    fn on_cancel<'a>(
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
    type_ttls: HashMap<String, std::time::Duration>,
    type_retry_policies: HashMap<String, RetryPolicy>,
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

    fn on_cancel_erased<'a>(
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

    fn on_cancel_erased<'a>(
        &'a self,
        ctx: &'a TaskContext,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), TaskError>> + Send + 'a>> {
        Box::pin(self.on_cancel(ctx))
    }
}

impl TaskTypeRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            types: HashMap::new(),
            type_ttls: HashMap::new(),
            type_retry_policies: HashMap::new(),
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

    /// Register an executor with a per-type default TTL.
    pub fn register_with_ttl<E: TaskExecutor>(
        &mut self,
        name: &str,
        executor: Arc<E>,
        ttl: std::time::Duration,
    ) {
        self.register(name, executor);
        self.type_ttls.insert(name.to_string(), ttl);
    }

    /// Register an executor with a per-type retry policy.
    pub fn register_with_retry_policy<E: TaskExecutor>(
        &mut self,
        name: &str,
        executor: Arc<E>,
        policy: RetryPolicy,
    ) {
        self.register(name, executor);
        self.type_retry_policies.insert(name.to_string(), policy);
    }

    /// Look up the per-type default TTL for a task type.
    pub fn type_ttl(&self, name: &str) -> Option<&std::time::Duration> {
        self.type_ttls.get(name)
    }

    /// Look up the per-type retry policy for a task type.
    pub fn type_retry_policy(&self, name: &str) -> Option<&RetryPolicy> {
        self.type_retry_policies.get(name)
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

    /// Register a pre-erased executor with a per-type TTL.
    pub(crate) fn register_erased_with_ttl(
        &mut self,
        name: &str,
        executor: Arc<dyn ErasedExecutor>,
        ttl: std::time::Duration,
    ) {
        self.register_erased(name, executor);
        self.type_ttls.insert(name.to_string(), ttl);
    }

    /// Set a retry policy for an already-registered task type.
    pub(crate) fn set_retry_policy(&mut self, name: &str, policy: RetryPolicy) {
        self.type_retry_policies.insert(name.to_string(), policy);
    }

    /// Register a pre-erased executor with a per-type retry policy.
    pub(crate) fn register_erased_with_retry_policy(
        &mut self,
        name: &str,
        executor: Arc<dyn ErasedExecutor>,
        policy: RetryPolicy,
    ) {
        self.register_erased(name, executor);
        self.type_retry_policies.insert(name.to_string(), policy);
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
    use crate::task::retry::{BackoffStrategy, RetryPolicy};

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

    #[test]
    fn register_with_retry_policy_stores_policy() {
        let mut reg = TaskTypeRegistry::new();
        let policy = RetryPolicy {
            strategy: BackoffStrategy::Exponential {
                initial: std::time::Duration::from_secs(1),
                max: std::time::Duration::from_secs(60),
                multiplier: 2.0,
            },
            max_retries: 5,
        };
        reg.register_with_retry_policy("api-call", Arc::new(NoopExecutor), policy);

        assert!(reg.get("api-call").is_some());
        let retrieved = reg.type_retry_policy("api-call").unwrap();
        assert_eq!(retrieved.max_retries, 5);
    }

    #[test]
    fn type_retry_policy_returns_none_for_missing() {
        let mut reg = TaskTypeRegistry::new();
        reg.register("plain", Arc::new(NoopExecutor));

        assert!(reg.type_retry_policy("plain").is_none());
        assert!(reg.type_retry_policy("nonexistent").is_none());
    }

    #[test]
    fn register_erased_with_retry_policy_stores_policy() {
        let mut reg = TaskTypeRegistry::new();
        let policy = RetryPolicy {
            strategy: BackoffStrategy::Constant {
                delay: std::time::Duration::from_secs(10),
            },
            max_retries: 7,
        };
        let executor: Arc<dyn ErasedExecutor> = Arc::new(NoopExecutor);
        reg.register_erased_with_retry_policy("erased-type", executor, policy);

        assert!(reg.get("erased-type").is_some());
        let retrieved = reg.type_retry_policy("erased-type").unwrap();
        assert_eq!(retrieved.max_retries, 7);
    }
}
