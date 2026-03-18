//! [`SubmitBuilder`] — ergonomic task submission with module defaults and
//! per-call field overrides.
//!
//! Returned by `ModuleHandle::submit` and `ModuleHandle::submit_typed`.
//! Implements [`IntoFuture`] so bare `.await` works for the common case.
//! Chain override methods before `.await` to override individual fields.
//!
//! # Resolution order (highest → lowest priority)
//!
//! ## `submit_typed()` path
//!
//! 1. [`SubmitBuilder`] per-call override (chained methods)
//! 2. Module defaults — override TypedTask values when the module has an
//!    explicit setting
//! 3. [`TypedTask`](crate::TypedTask) trait method return values
//! 4. Scheduler global defaults (applied inside `Scheduler::submit`)
//!
//! ## `submit()` path
//!
//! 1. [`SubmitBuilder`] per-call override (highest)
//! 2. Fields explicitly set on the [`TaskSubmission`] (non-zero / non-`None`)
//! 3. Module defaults (fill in zero/`None` submission fields)
//! 4. Scheduler global defaults (applied inside `Scheduler::submit`)

use std::collections::HashMap;
use std::future::IntoFuture;
use std::pin::Pin;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::priority::Priority;
use crate::scheduler::Scheduler;
use crate::store::StoreError;
use crate::task::{SubmitOutcome, TaskSubmission, TtlFrom};

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

/// [`TypedTask`](crate::TypedTask)-level defaults used in the 5-layer precedence
/// chain for the `submit_typed()` path.
///
/// These hold the values returned by the `TypedTask` trait methods. In the
/// chain they sit **below** module defaults — a module setting with an explicit
/// value unconditionally wins over the corresponding TypedTask value.
#[derive(Clone)]
pub(crate) struct TypedTaskDefaults {
    pub priority: Priority,
    pub group: Option<String>,
    pub ttl: Option<Duration>,
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
    /// Present only for the `submit_typed()` path. Stores the raw values
    /// returned by `TypedTask` methods so that `resolve()` can apply module
    /// defaults on top of them (module wins over TypedTask).
    ///
    /// `None` for the `submit()` path, where the `TaskSubmission` fields are
    /// treated as explicit values that beat module defaults.
    typed_defaults: Option<TypedTaskDefaults>,
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
            typed_defaults: None,
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

    /// Attach [`TypedTask`](crate::TypedTask) defaults for the 5-layer
    /// precedence chain.
    ///
    /// Called internally by `ModuleHandle::submit_typed()`. Marks this builder
    /// as being on the `submit_typed()` path so that `resolve()` applies module
    /// defaults **on top of** TypedTask values rather than only filling in
    /// zero/`None` fields.
    pub(crate) fn with_typed_defaults(mut self, td: TypedTaskDefaults) -> Self {
        self.typed_defaults = Some(td);
        self
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

    /// Apply all default layers and per-call overrides, returning the
    /// scheduler and the fully resolved [`TaskSubmission`].
    ///
    /// Two modes determined by whether [`with_typed_defaults`](Self::with_typed_defaults)
    /// was called:
    ///
    /// **`submit_typed()` path** (typed_defaults present):
    /// 1. TypedTask values are set as the base (layer 4).
    /// 2. Module defaults override them where the module has an explicit
    ///    setting (layer 3).
    /// 3. SubmitBuilder per-call overrides trump everything (layer 1).
    ///
    /// **`submit()` path** (typed_defaults absent):
    /// 1. Module defaults fill in only where the submission is at its
    ///    zero/`None` value (layer 3 fills in layer 2 gaps).
    /// 2. SubmitBuilder per-call overrides trump everything (layer 1).
    ///
    /// Layer 5 (Scheduler global defaults, e.g. global TTL) is applied later
    /// inside `Scheduler::submit()`.
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

        // ── 2+3. Apply TypedTask defaults then module overrides ───────────────
        if let Some(td) = self.typed_defaults {
            // ── submit_typed() path ───────────────────────────────────────────
            // TypedTask values are the baseline (layer 4). Module defaults
            // unconditionally override them when the module has an explicit
            // setting (layer 3).
            sub.priority = td.priority;
            sub.group_key = td.group;
            sub.ttl = td.ttl;
            // TypedTask tags are the base; module tags add new keys only.
            sub.tags = td.tags;
            for (k, v) in &self.module_defaults.tags {
                sub.tags.entry(k.clone()).or_insert_with(|| v.clone());
            }
            if let Some(p) = self.module_defaults.priority {
                sub.priority = p;
            }
            if let Some(g) = self.module_defaults.group {
                sub.group_key = Some(g);
            }
            if let Some(t) = self.module_defaults.ttl {
                sub.ttl = Some(t);
            }
        } else {
            // ── submit() path ─────────────────────────────────────────────────
            // Module defaults fill in only where the submission is at its
            // zero/None value (submission explicit values beat module defaults).
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
        }

        // ── 4. Apply per-call overrides (layer 1 — always highest priority) ──
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
        let parent_id = self.override_parent_id;
        let (scheduler, mut resolved) = self.resolve();

        // When `.parent()` was set, inherit remaining TTL and tags from the
        // parent record — matching `ChildSpawner` behaviour.
        //
        // - TTL: inherited only when no layer (builder override, module default,
        //   TypedTask) has already set a TTL on the child.
        // - Tags: parent tags fill in keys not already present on the child;
        //   child-set tags always win on key conflicts.
        if let Some(pid) = parent_id {
            if let Ok(Some(parent)) = scheduler.store().task_by_id(pid).await {
                // Inherit remaining parent TTL only if no layer set a child TTL.
                if resolved.ttl.is_none() {
                    if let Some(parent_ttl_secs) = parent.ttl_seconds {
                        let parent_ttl = Duration::from_secs(parent_ttl_secs as u64);
                        let ttl_start = match parent.ttl_from {
                            TtlFrom::Submission => Some(parent.created_at),
                            // If the parent hasn't started yet we can't compute
                            // remaining TTL, so we skip inheritance.
                            TtlFrom::FirstAttempt => parent.started_at,
                        };
                        if let Some(start) = ttl_start {
                            let elapsed = Utc::now() - start;
                            let elapsed_std = elapsed.to_std().unwrap_or_default();
                            if let Some(remaining) = parent_ttl.checked_sub(elapsed_std) {
                                if remaining > Duration::ZERO {
                                    resolved.ttl = Some(remaining);
                                    resolved.ttl_from = TtlFrom::Submission;
                                }
                            }
                        }
                    }
                }
                // Merge parent tags: parent fills in keys the child doesn't have.
                for (k, v) in &parent.tags {
                    resolved.tags.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
        }

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

    // ── Step 5: 5-layer precedence chain ─────────────────────────────────────

    /// Layer 3 (module default) overrides layer 4 (TypedTask default).
    ///
    /// TypedTask priority=HIGH, module priority=BACKGROUND → BACKGROUND wins.
    #[tokio::test]
    async fn module_default_overrides_typed_task_priority() {
        let scheduler = make_scheduler().await;
        let defaults = ModuleSubmitDefaults {
            priority: Some(Priority::BACKGROUND),
            ..Default::default()
        };
        let typed_defaults = super::TypedTaskDefaults {
            priority: Priority::HIGH,
            group: None,
            ttl: None,
            tags: Default::default(),
        };
        let sub = TaskSubmission::new("task");

        let outcome = SubmitBuilder::new(sub, scheduler.clone(), "media", defaults)
            .with_typed_defaults(typed_defaults)
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
    }

    /// Layer 1 (SubmitBuilder override) wins over layer 3 (module default).
    ///
    /// Module priority=BACKGROUND, SubmitBuilder override=REALTIME → REALTIME wins.
    #[tokio::test]
    async fn submit_builder_override_wins_over_module_default_priority() {
        let scheduler = make_scheduler().await;
        let defaults = ModuleSubmitDefaults {
            priority: Some(Priority::BACKGROUND),
            ..Default::default()
        };
        let typed_defaults = super::TypedTaskDefaults {
            priority: Priority::NORMAL,
            group: None,
            ttl: None,
            tags: Default::default(),
        };
        let sub = TaskSubmission::new("task");

        let outcome = SubmitBuilder::new(sub, scheduler.clone(), "media", defaults)
            .with_typed_defaults(typed_defaults)
            .priority(Priority::HIGH)
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
    }

    /// Layer 2 (explicit TaskSubmission field) beats layer 3 (module default)
    /// in the `submit()` path.
    ///
    /// Module group="api", submission group="gpu" → "gpu" wins.
    #[tokio::test]
    async fn submission_explicit_group_beats_module_default_in_submit_path() {
        let scheduler = make_scheduler().await;
        let defaults = ModuleSubmitDefaults {
            group: Some("api".into()),
            ..Default::default()
        };
        // submit() path: no typed_defaults, submission has explicit group.
        let sub = TaskSubmission::new("task").group("gpu");

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

        assert_eq!(task.group_key.as_deref(), Some("gpu"));
    }

    /// Layer 5 (scheduler global TTL) applies when no other layer sets TTL.
    #[tokio::test]
    async fn global_ttl_applies_when_no_layer_sets_ttl() {
        let store = crate::store::TaskStore::open_memory().await.unwrap();
        let global_ttl_secs: i64 = 3600;
        let scheduler = Scheduler::new(
            store,
            SchedulerConfig {
                default_ttl: Some(std::time::Duration::from_secs(global_ttl_secs as u64)),
                ..Default::default()
            },
            Arc::new(crate::registry::TaskTypeRegistry::new()),
            crate::backpressure::CompositePressure::new(),
            crate::backpressure::ThrottlePolicy::default_three_tier(),
        );

        // No TTL set at any layer.
        let sub = TaskSubmission::new("task");
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

        assert_eq!(task.ttl_seconds, Some(global_ttl_secs));
    }
}
