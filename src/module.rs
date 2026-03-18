//! Module definition — a self-contained bundle of task executors, defaults,
//! and resource policy.
//!
//! Define a [`Module`] on the library side, then register it with
//! [`SchedulerBuilder::module`](crate::SchedulerBuilder::module) on the
//! application side. All executor task types are automatically prefixed with
//! `"{name}::"` at registration time.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::priority::Priority;
use crate::registry::{ErasedExecutor, TaskExecutor};
use crate::task::retry::RetryPolicy;
use crate::task::TypedTask;

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use crate::priority::Priority;
    use crate::registry::{TaskContext, TaskExecutor};
    use crate::task::retry::{BackoffStrategy, RetryPolicy};
    use crate::task::{TaskError, TypedTask};

    use super::*;

    struct NoopExecutor;

    impl TaskExecutor for NoopExecutor {
        async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
            Ok(())
        }
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct ThumbTask {
        path: String,
    }

    impl TypedTask for ThumbTask {
        const TASK_TYPE: &'static str = "thumbnail";
    }

    #[test]
    fn new_stores_name_and_typed_executor_reads_task_type() {
        let module = Module::new("media").typed_executor::<ThumbTask, _>(Arc::new(NoopExecutor));

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
