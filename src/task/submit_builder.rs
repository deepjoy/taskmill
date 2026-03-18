//! [`SubmitBuilder`] — ergonomic task submission with module defaults and
//! per-call field overrides.
//!
//! Returned by `ModuleHandle::submit` and `ModuleHandle::submit_typed`.
//! Implements [`IntoFuture`] so bare `.await` works for the common case.
//! Chain override methods before `.await` to override individual fields.
//!
//! # Resolution order (highest → lowest priority)
//!
//! 1. Explicit [`SubmitBuilder`] override (set via chaining methods)
//! 2. Fields explicitly set on the [`TaskSubmission`]
//! 3. Module defaults (from the [`Module`](crate::module::Module) that owns the task type)
//! 4. Scheduler global defaults (applied by the scheduler)

use std::collections::HashMap;
use std::future::IntoFuture;
use std::pin::Pin;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::priority::Priority;
use crate::scheduler::Scheduler;
use crate::store::StoreError;
use crate::task::{SubmitOutcome, TaskSubmission};

/// Module-level defaults applied to every submission through a module handle.
///
/// Fields are `Option` so each default is independently optional. When a field
/// is `None` the submission's own value (or the scheduler global default) is used.
#[derive(Default, Clone)]
pub struct ModuleSubmitDefaults {
    pub priority: Option<Priority>,
    pub group: Option<String>,
    pub ttl: Option<Duration>,
    /// Tags merged into every submission. Submission-level tags win on key conflicts.
    pub tags: HashMap<String, String>,
}

/// Ergonomic task submission builder returned by `ModuleHandle::submit` and
/// `ModuleHandle::submit_typed`.
///
/// Implements [`IntoFuture`] so bare `.await` submits with all defaults
/// applied. Chain override methods before `.await` to override individual
/// fields for this call only.
///
/// ```ignore
/// // Common case — zero ceremony
/// handle.submit_typed(&thumb).await?;
///
/// // Override one field
/// handle.submit_typed(&thumb)
///     .priority(Priority::CRITICAL)
///     .run_after(Duration::from_secs(30))
///     .await?;
/// ```
pub struct SubmitBuilder {
    /// Base submission. `task_type` is **not** yet prefixed with the module
    /// name — prefixing happens at submit time in [`resolve`](Self::resolve).
    submission: TaskSubmission,
    /// Scheduler reference used to actually submit the resolved task.
    scheduler: Scheduler,
    /// Module name used to prefix `task_type` (e.g. `"media"`).
    /// Empty string = no prefix (bare scheduler usage without a module).
    module_name: String,
    /// Module-level defaults applied where the submission is at its zero value.
    module_defaults: ModuleSubmitDefaults,
    // ── Per-call override fields ─────────────────────────────────────────────
    // These are `Option` so they are applied only when explicitly set.
    override_priority: Option<Priority>,
    override_group: Option<String>,
    override_key: Option<String>,
    override_run_after: Option<DateTime<Utc>>,
    override_ttl: Option<Duration>,
    override_depends_on: Vec<i64>,
    override_tags: HashMap<String, String>,
    override_parent_id: Option<i64>,
}

impl SubmitBuilder {
    /// Create a new `SubmitBuilder`.
    ///
    /// Prefer `ModuleHandle::submit` or `ModuleHandle::submit_typed` over
    /// constructing this directly.
    pub fn new(
        submission: TaskSubmission,
        scheduler: Scheduler,
        module_name: impl Into<String>,
        module_defaults: ModuleSubmitDefaults,
    ) -> Self {
        Self {
            submission,
            scheduler,
            module_name: module_name.into(),
            module_defaults,
            override_priority: None,
            override_group: None,
            override_key: None,
            override_run_after: None,
            override_ttl: None,
            override_depends_on: Vec::new(),
            override_tags: HashMap::new(),
            override_parent_id: None,
        }
    }

    /// Override the task priority. Takes precedence over both the module
    /// default and any priority set directly on the `TaskSubmission`.
    pub fn priority(mut self, priority: Priority) -> Self {
        self.override_priority = Some(priority);
        self
    }

    /// Override the group key for per-group concurrency limiting.
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.override_group = Some(group.into());
        self
    }

    /// Override the dedup key (also sets the display label to match).
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.override_key = Some(key.into());
        self
    }

    /// Delay dispatch: task is eligible after `delay` from now.
    pub fn run_after(mut self, delay: Duration) -> Self {
        self.override_run_after = Some(Utc::now() + delay);
        self
    }

    /// Override the time-to-live for this submission.
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.override_ttl = Some(ttl);
        self
    }

    /// Add a task dependency. The task enters `blocked` status until `task_id`
    /// completes. Merged with any dependencies already on the `TaskSubmission`.
    pub fn depends_on(mut self, task_id: i64) -> Self {
        self.override_depends_on.push(task_id);
        self
    }

    /// Add multiple task dependencies. Merged with existing.
    pub fn depends_on_all(mut self, ids: impl IntoIterator<Item = i64>) -> Self {
        self.override_depends_on.extend(ids);
        self
    }

    /// Add a metadata tag. Override tags win over submission-level and
    /// module-level tags for the same key.
    pub fn tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.override_tags.insert(key.into(), value.into());
        self
    }

    /// Set the parent task ID for hierarchical tasks (parent-child
    /// relationship). This does **not** establish a dependency — use
    /// [`depends_on`](Self::depends_on) for that.
    pub fn parent(mut self, parent_id: i64) -> Self {
        self.override_parent_id = Some(parent_id);
        self
    }

    /// Apply module defaults and per-call overrides to the base submission,
    /// returning the scheduler and the fully resolved `TaskSubmission`.
    ///
    /// Applies fields in priority order:
    /// 1. Per-call overrides (highest)
    /// 2. Module defaults (where submission is at its zero/default value)
    /// 3. Base `TaskSubmission` fields (lowest, already set by caller)
    fn resolve(self) -> (Scheduler, TaskSubmission) {
        let scheduler = self.scheduler;
        let mut sub = self.submission;

        // ── 1. Prefix task_type with the module name ─────────────────────────
        if !self.module_name.is_empty() {
            let old_type = sub.task_type.clone();
            sub.task_type = format!("{}::{}", self.module_name, old_type);
            // Update label if it was the default (equal to the old task_type).
            if sub.label == old_type {
                sub.label = sub.task_type.clone();
            }
        }

        // ── 2. Apply module defaults where the submission is at its zero value ─
        //
        // Priority: treat `NORMAL` as "not explicitly set" — the same
        // convention used by `BatchSubmission::build`.
        if sub.priority == Priority::NORMAL {
            if let Some(p) = self.module_defaults.priority {
                sub.priority = p;
            }
        }
        if sub.group_key.is_none() {
            if let Some(g) = self.module_defaults.group {
                sub.group_key = Some(g);
            }
        }
        if sub.ttl.is_none() {
            if let Some(t) = self.module_defaults.ttl {
                sub.ttl = Some(t);
            }
        }
        // Module tags: add keys not already on the submission (submission wins).
        for (k, v) in &self.module_defaults.tags {
            sub.tags.entry(k.clone()).or_insert_with(|| v.clone());
        }

        // ── 3. Apply per-call overrides (highest priority) ───────────────────
        if let Some(p) = self.override_priority {
            sub.priority = p;
        }
        if let Some(g) = self.override_group {
            sub.group_key = Some(g);
        }
        if let Some(k) = self.override_key {
            sub.label = k.clone();
            sub.dedup_key = Some(k);
        }
        if let Some(ra) = self.override_run_after {
            sub.run_after = Some(ra);
        }
        if let Some(t) = self.override_ttl {
            sub.ttl = Some(t);
        }
        if !self.override_depends_on.is_empty() {
            sub.dependencies.extend(self.override_depends_on);
        }
        for (k, v) in self.override_tags {
            sub.tags.insert(k, v);
        }
        if let Some(pid) = self.override_parent_id {
            sub.parent_id = Some(pid);
        }

        (scheduler, sub)
    }

    /// Submit the task, returning the outcome.
    ///
    /// This is the method called by `IntoFuture`. You can also call it directly
    /// if you need to name the future type.
    pub async fn submit(self) -> Result<SubmitOutcome, StoreError> {
        let (scheduler, resolved) = self.resolve();
        scheduler.submit(&resolved).await
    }
}

impl IntoFuture for SubmitBuilder {
    type Output = Result<SubmitOutcome, StoreError>;
    type IntoFuture = Pin<Box<dyn std::future::Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.submit())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::backpressure::{CompositePressure, ThrottlePolicy};
    use crate::priority::Priority;
    use crate::registry::TaskTypeRegistry;
    use crate::scheduler::{Scheduler, SchedulerConfig};
    use crate::store::TaskStore;
    use crate::task::TaskSubmission;

    use super::{ModuleSubmitDefaults, SubmitBuilder};

    async fn make_scheduler() -> Scheduler {
        let store = TaskStore::open_memory().await.unwrap();
        Scheduler::new(
            store,
            SchedulerConfig::default(),
            Arc::new(TaskTypeRegistry::new()),
            CompositePressure::new(),
            ThrottlePolicy::default_three_tier(),
        )
    }

    /// Bare `.await` applies module defaults (priority + group) and prefixes
    /// the task type with the module name.
    #[tokio::test]
    async fn module_defaults_applied_on_bare_await() {
        let scheduler = make_scheduler().await;
        let defaults = ModuleSubmitDefaults {
            priority: Some(Priority::BACKGROUND),
            group: Some("media-pipeline".into()),
            ..Default::default()
        };
        let sub = TaskSubmission::new("thumbnail");

        let outcome = SubmitBuilder::new(sub, scheduler.clone(), "media", defaults)
            .await
            .unwrap();

        let task_id = outcome.id().unwrap();
        let task = scheduler
            .inner
            .store
            .task_by_id(task_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(task.priority, Priority::BACKGROUND);
        assert_eq!(task.group_key.as_deref(), Some("media-pipeline"));
        assert_eq!(task.task_type, "media::thumbnail");
    }

    /// Chaining `.priority()` and `.group()` overrides module defaults.
    #[tokio::test]
    async fn chained_overrides_take_precedence_over_module_defaults() {
        let scheduler = make_scheduler().await;
        let defaults = ModuleSubmitDefaults {
            priority: Some(Priority::BACKGROUND),
            group: Some("default-group".into()),
            ..Default::default()
        };
        let sub = TaskSubmission::new("thumbnail");

        let outcome = SubmitBuilder::new(sub, scheduler.clone(), "media", defaults)
            .priority(Priority::HIGH)
            .group("override-group")
            .await
            .unwrap();

        let task_id = outcome.id().unwrap();
        let task = scheduler
            .inner
            .store
            .task_by_id(task_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(task.priority, Priority::HIGH);
        assert_eq!(task.group_key.as_deref(), Some("override-group"));
    }

    /// The stored `task_type` is prefixed with the module name.
    #[tokio::test]
    async fn task_type_is_prefixed_with_module_name() {
        let scheduler = make_scheduler().await;
        let sub = TaskSubmission::new("thumbnail");

        let outcome = SubmitBuilder::new(
            sub,
            scheduler.clone(),
            "media",
            ModuleSubmitDefaults::default(),
        )
        .await
        .unwrap();

        let task_id = outcome.id().unwrap();
        let task = scheduler
            .inner
            .store
            .task_by_id(task_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(task.task_type, "media::thumbnail");
    }
}
