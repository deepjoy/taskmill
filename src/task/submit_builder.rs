//! [`SubmitBuilder`] — ergonomic task submission with module defaults and
//! per-call field overrides.
//!
//! Returned by `ModuleHandle::submit` / `ModuleHandle::submit_typed` and by
//! `DomainHandle::submit` / `DomainHandle::submit_with`. The typed submission
//! path (accepting a [`TypedTask`](crate::TypedTask) value) now goes through
//! the domain API — `DomainHandle::submit` is the primary entry point.
//! Implements [`IntoFuture`] so bare `.await` works for the common case.
//! Chain override methods before `.await` to override individual fields.
//!
//! # Resolution order (highest → lowest priority)
//!
//! ## `submit_typed()` / `DomainHandle::submit` path
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
use crate::task::submission::DependencyFailurePolicy;
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

/// Ergonomic task submission builder returned by `ModuleHandle::submit` /
/// `ModuleHandle::submit_typed` and by `DomainHandle::submit` /
/// `DomainHandle::submit_with`.
///
/// Implements [`IntoFuture`] so bare `.await` submits with all defaults
/// applied. Chain override methods before `.await` to override individual
/// fields for this call only.
///
/// ```ignore
/// // Common case — zero ceremony
/// media.submit(thumb).await?;
///
/// // Override one field
/// media.submit_with(thumb)
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
    override_on_dependency_failure: Option<DependencyFailurePolicy>,
    override_tags: HashMap<String, String>,
    override_parent_id: Option<i64>,
    override_fail_fast: Option<bool>,
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
            override_on_dependency_failure: None,
            override_tags: HashMap::new(),
            override_parent_id: None,
            override_fail_fast: None,
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

    /// Set the dependency failure policy (`Cancel`, `Fail`, or `Ignore`).
    pub fn on_dependency_failure(mut self, policy: DependencyFailurePolicy) -> Self {
        self.override_on_dependency_failure = Some(policy);
        self
    }

    /// Add a metadata tag. Override tags win over submission-level and
    /// module-level tags for the same key.
    pub fn tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.override_tags.insert(key.into(), value.into());
        self
    }

    /// Control whether a child failure immediately fails the parent.
    ///
    /// Defaults to `true`. Set to `false` to let the parent's `finalize()`
    /// run even when some children fail.
    pub fn fail_fast(mut self, fail_fast: bool) -> Self {
        self.override_fail_fast = Some(fail_fast);
        self
    }

    /// Set the parent task ID for hierarchical tasks (parent-child
    /// relationship). This does **not** establish a dependency — use
    /// [`depends_on`](Self::depends_on) for that.
    pub fn parent(mut self, parent_id: i64) -> Self {
        self.override_parent_id = Some(parent_id);
        self
    }

    fn merge_module_tags(sub: &mut TaskSubmission, module_tags: &HashMap<String, String>) {
        for (k, v) in module_tags {
            sub.tags.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }

    /// Apply all default layers and per-call overrides, returning the
    /// scheduler and the fully resolved [`TaskSubmission`].
    ///
    /// Delegates to three focused methods that each handle one layer of the
    /// precedence chain:
    ///
    /// 1. [`apply_prefix`](Self::apply_prefix) — prefix `task_type` with the
    ///    module name.
    /// 2. [`apply_defaults`](Self::apply_defaults) — layers 2–4 (TypedTask
    ///    baseline, module defaults, or submission-explicit values).
    /// 3. [`apply_overrides`](Self::apply_overrides) — layer 1 (per-call
    ///    chained overrides, always highest priority).
    ///
    /// Layer 5 (Scheduler global defaults, e.g. global TTL) is applied later
    /// inside `Scheduler::submit()`.
    fn resolve(mut self) -> (Scheduler, TaskSubmission) {
        self.apply_prefix();
        self.apply_defaults();
        self.apply_overrides();
        (self.scheduler, self.submission)
    }

    /// Prefix `task_type` with the module name (e.g. `"thumbnail"` →
    /// `"media::thumbnail"`). Updates `label` when it matches the old
    /// unprefixed type.
    fn apply_prefix(&mut self) {
        if !self.module_name.is_empty() {
            let old_type = self.submission.task_type.clone();
            self.submission.task_type = format!("{}::{}", self.module_name, old_type);
            if self.submission.label == old_type {
                self.submission.label = self.submission.task_type.clone();
            }
        }
    }

    /// Layers 2–4: apply typed/module/submission defaults.
    ///
    /// **`submit_typed()` path** (`typed_defaults` present):
    /// TypedTask values are the baseline (layer 4). Module defaults
    /// unconditionally override them (layer 3).
    ///
    /// **`submit()` path** (`typed_defaults` absent):
    /// Submission values are the baseline (layer 2). Module defaults fill in
    /// only where the submission is at its zero/`None` value (layer 3).
    fn apply_defaults(&mut self) {
        if let Some(td) = self.typed_defaults.take() {
            // ── submit_typed() path ───────────────────────────────────────
            // TypedTask values are the baseline (layer 4).
            self.submission.priority = td.priority;
            self.submission.group_key = td.group;
            self.submission.ttl = td.ttl;
            self.submission.tags = td.tags;
            // Module defaults: tags merge first, then scalars override
            // unconditionally (layer 3 beats layer 4).
            Self::merge_module_tags(&mut self.submission, &self.module_defaults.tags);
            self.apply_module_scalar_defaults(true);
        } else {
            // ── submit() path ─────────────────────────────────────────────
            // Module defaults fill gaps only (layer 3 fills layer 2 gaps).
            self.apply_module_scalar_defaults(false);
            Self::merge_module_tags(&mut self.submission, &self.module_defaults.tags);
        }
    }

    /// Apply module-level scalar defaults (priority, group, TTL).
    ///
    /// When `unconditional` is `true` (typed path), module values always
    /// override the current submission value. When `false` (untyped path),
    /// they fill in only where the submission is at its zero/`None` value
    /// (`Priority::NORMAL` is treated as "not explicitly set").
    fn apply_module_scalar_defaults(&mut self, unconditional: bool) {
        if let Some(p) = self.module_defaults.priority {
            if unconditional || self.submission.priority == Priority::NORMAL {
                self.submission.priority = p;
            }
        }
        if let Some(ref g) = self.module_defaults.group {
            if unconditional || self.submission.group_key.is_none() {
                self.submission.group_key = Some(g.clone());
            }
        }
        if let Some(t) = self.module_defaults.ttl {
            if unconditional || self.submission.ttl.is_none() {
                self.submission.ttl = Some(t);
            }
        }
    }

    /// Layer 1: per-call overrides — always highest priority.
    ///
    /// Values set via chained builder methods (`.priority()`, `.group()`,
    /// etc.) unconditionally overwrite whatever layers 2–4 produced.
    fn apply_overrides(&mut self) {
        if let Some(p) = self.override_priority.take() {
            self.submission.priority = p;
        }
        if let Some(g) = self.override_group.take() {
            self.submission.group_key = Some(g);
        }
        if let Some(k) = self.override_key.take() {
            self.submission.label = k.clone();
            self.submission.dedup_key = Some(k);
        }
        if let Some(ra) = self.override_run_after.take() {
            self.submission.run_after = Some(ra);
        }
        if let Some(t) = self.override_ttl.take() {
            self.submission.ttl = Some(t);
        }
        if !self.override_depends_on.is_empty() {
            self.submission
                .dependencies
                .append(&mut self.override_depends_on);
        }
        if let Some(p) = self.override_on_dependency_failure.take() {
            self.submission.on_dependency_failure = p;
        }
        for (k, v) in std::mem::take(&mut self.override_tags) {
            self.submission.tags.insert(k, v);
        }
        if let Some(pid) = self.override_parent_id.take() {
            self.submission.parent_id = Some(pid);
        }
        if let Some(ff) = self.override_fail_fast.take() {
            self.submission.fail_fast = ff;
        }
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
