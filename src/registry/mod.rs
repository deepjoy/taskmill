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

pub(crate) mod child_spawner;
mod context;
pub(crate) mod io_tracker;
pub(crate) mod state;

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use crate::task::TaskError;

pub(crate) use child_spawner::ChildSpawner;
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

    /// Look up the per-type default TTL for a task type.
    pub fn type_ttl(&self, name: &str) -> Option<&std::time::Duration> {
        self.type_ttls.get(name)
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
