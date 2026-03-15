//! Task submission types: [`TaskSubmission`] for single tasks, [`BatchSubmission`]
//! for building batches with shared defaults, [`SubmitOutcome`] for per-task
//! results, and [`BatchOutcome`] for categorized batch summaries.
//!
//! # Task dependencies
//!
//! Tasks can declare dependencies on other tasks via builder methods:
//!
//! - [`.depends_on(task_id)`](TaskSubmission::depends_on) — add a single dependency
//! - [`.depends_on_all(ids)`](TaskSubmission::depends_on_all) — add multiple dependencies
//! - [`.on_dependency_failure(policy)`](TaskSubmission::on_dependency_failure) — set the
//!   [`DependencyFailurePolicy`] (`Cancel`, `Fail`, or `Ignore`)
//!
//! A task with dependencies enters [`Blocked`](crate::TaskStatus::Blocked) status
//! and transitions to `Pending` only after all dependencies complete successfully.

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::priority::Priority;

use super::dedup::generate_dedup_key;
use super::typed::TypedTask;
use super::{IoBudget, TtlFrom};

/// Maximum length of a tag key in bytes.
pub const MAX_TAG_KEY_LEN: usize = 64;
/// Maximum length of a tag value in bytes.
pub const MAX_TAG_VALUE_LEN: usize = 256;
/// Maximum number of tags per task.
pub const MAX_TAGS_PER_TASK: usize = 32;

/// Configuration for recurring task schedules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringSchedule {
    /// Interval between consecutive executions.
    pub interval: Duration,
    /// Initial delay before the first execution. `None` = start immediately.
    pub initial_delay: Option<Duration>,
    /// Maximum number of executions. `None` = run indefinitely.
    pub max_executions: Option<u64>,
}

/// What happens to a dependent task when one of its dependencies fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyFailurePolicy {
    /// Auto-cancel the dependent and record it as `DependencyFailed` (default).
    #[default]
    Cancel,
    /// Move the dependent to `DependencyFailed` status in history but don't
    /// cancel other dependents in the same chain (for manual intervention).
    Fail,
    /// Ignore the failure and unblock the dependent anyway.
    /// Use with caution — the dependent must handle missing upstream results.
    Ignore,
}

impl DependencyFailurePolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cancel => "cancel",
            Self::Fail => "fail",
            Self::Ignore => "ignore",
        }
    }
}

impl std::str::FromStr for DependencyFailurePolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cancel" => Ok(Self::Cancel),
            "fail" => Ok(Self::Fail),
            "ignore" => Ok(Self::Ignore),
            other => Err(format!("unknown DependencyFailurePolicy: {other}")),
        }
    }
}

/// Strategy for handling duplicate dedup keys on submission.
///
/// Controls what happens when a newly submitted task has the same dedup key as
/// an existing task in the queue. The default (`Skip`) preserves the current
/// dedup behaviour unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DuplicateStrategy {
    /// Current behaviour: attempt priority upgrade or requeue, otherwise no-op.
    #[default]
    Skip,
    /// Cancel the existing task and insert the new one in its place.
    Supersede,
    /// Return an error if a duplicate already exists.
    Reject,
}

/// Result of a task submission attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitOutcome {
    /// Task was inserted as new.
    Inserted(i64),
    /// Duplicate key existed; its priority was upgraded (pending/paused tasks only).
    Upgraded(i64),
    /// Duplicate key existed and is running/paused; marked for re-queue after completion.
    Requeued(i64),
    /// Duplicate key existed; no changes were made.
    Duplicate,
    /// Existing task was superseded (cancelled and replaced).
    Superseded {
        /// ID of the newly inserted task.
        new_task_id: i64,
        /// ID of the task that was replaced.
        replaced_task_id: i64,
    },
    /// Submission rejected because a duplicate exists and strategy is `Reject`.
    Rejected,
}

impl SubmitOutcome {
    /// Returns the task ID if the task was inserted, upgraded, requeued, or superseded.
    pub fn id(&self) -> Option<i64> {
        match self {
            Self::Inserted(id) | Self::Upgraded(id) | Self::Requeued(id) => Some(*id),
            Self::Superseded { new_task_id, .. } => Some(*new_task_id),
            Self::Duplicate | Self::Rejected => None,
        }
    }

    /// Returns `true` if a new task was inserted (including via supersede).
    pub fn is_inserted(&self) -> bool {
        matches!(self, Self::Inserted(_) | Self::Superseded { .. })
    }
}

/// Summary of a batch submission.
///
/// Wraps the per-task [`SubmitOutcome`] results in input order (1:1 with
/// submissions) and provides convenience accessors for categorized views.
#[derive(Debug, Clone)]
pub struct BatchOutcome {
    /// Per-task results, in input order (1:1 with submissions).
    pub outcomes: Vec<SubmitOutcome>,
}

impl BatchOutcome {
    /// Collect task IDs from [`SubmitOutcome::Inserted`] outcomes.
    pub fn inserted(&self) -> Vec<i64> {
        self.outcomes
            .iter()
            .filter_map(|o| match o {
                SubmitOutcome::Inserted(id) => Some(*id),
                _ => None,
            })
            .collect()
    }

    /// Collect task IDs from [`SubmitOutcome::Upgraded`] outcomes.
    pub fn upgraded(&self) -> Vec<i64> {
        self.outcomes
            .iter()
            .filter_map(|o| match o {
                SubmitOutcome::Upgraded(id) => Some(*id),
                _ => None,
            })
            .collect()
    }

    /// Collect task IDs from [`SubmitOutcome::Requeued`] outcomes.
    pub fn requeued(&self) -> Vec<i64> {
        self.outcomes
            .iter()
            .filter_map(|o| match o {
                SubmitOutcome::Requeued(id) => Some(*id),
                _ => None,
            })
            .collect()
    }

    /// Collect new task IDs from [`SubmitOutcome::Superseded`] outcomes.
    pub fn superseded(&self) -> Vec<i64> {
        self.outcomes
            .iter()
            .filter_map(|o| match o {
                SubmitOutcome::Superseded { new_task_id, .. } => Some(*new_task_id),
                _ => None,
            })
            .collect()
    }

    /// Count [`SubmitOutcome::Rejected`] outcomes.
    pub fn rejected_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| matches!(o, SubmitOutcome::Rejected))
            .count()
    }

    /// Count [`SubmitOutcome::Duplicate`] outcomes.
    pub fn duplicated_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| matches!(o, SubmitOutcome::Duplicate))
            .count()
    }

    /// Number of outcomes.
    pub fn len(&self) -> usize {
        self.outcomes.len()
    }

    /// Returns `true` if there are no outcomes.
    pub fn is_empty(&self) -> bool {
        self.outcomes.is_empty()
    }

    /// Iterate over outcomes.
    pub fn iter(&self) -> std::slice::Iter<'_, SubmitOutcome> {
        self.outcomes.iter()
    }
}

impl<'a> IntoIterator for &'a BatchOutcome {
    type Item = &'a SubmitOutcome;
    type IntoIter = std::slice::Iter<'a, SubmitOutcome>;

    fn into_iter(self) -> Self::IntoIter {
        self.outcomes.iter()
    }
}

impl IntoIterator for BatchOutcome {
    type Item = SubmitOutcome;
    type IntoIter = std::vec::IntoIter<SubmitOutcome>;

    fn into_iter(self) -> Self::IntoIter {
        self.outcomes.into_iter()
    }
}

/// Builder for batch submissions with shared defaults.
///
/// Allows setting batch-wide defaults for group and priority that are applied
/// to tasks that don't have explicit values:
///
/// ```ignore
/// use taskmill::{BatchSubmission, TaskSubmission, Priority};
///
/// let submissions = BatchSubmission::new()
///     .default_group("s3://my-bucket")
///     .default_priority(Priority::HIGH)
///     .task(TaskSubmission::new("upload").key("file-1"))
///     .task(TaskSubmission::new("upload").key("file-2").priority(Priority::REALTIME))
///     .build();
/// ```
pub struct BatchSubmission {
    default_group: Option<String>,
    default_priority: Option<Priority>,
    default_ttl: Option<Duration>,
    default_tags: HashMap<String, String>,
    tasks: Vec<TaskSubmission>,
}

impl BatchSubmission {
    /// Create a new empty batch submission builder.
    pub fn new() -> Self {
        Self {
            default_group: None,
            default_priority: None,
            default_ttl: None,
            default_tags: HashMap::new(),
            tasks: Vec::new(),
        }
    }

    /// Set a default group key applied to tasks without an explicit group.
    pub fn default_group(mut self, group: impl Into<String>) -> Self {
        self.default_group = Some(group.into());
        self
    }

    /// Set a default TTL applied to tasks that don't specify one.
    pub fn default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// Set a default priority applied to tasks still at [`Priority::NORMAL`].
    pub fn default_priority(mut self, priority: Priority) -> Self {
        self.default_priority = Some(priority);
        self
    }

    /// Set a default tag applied to tasks that don't already have this key.
    pub fn default_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.default_tags.insert(key.into(), value.into());
        self
    }

    /// Add a single task to the batch.
    pub fn task(mut self, sub: TaskSubmission) -> Self {
        self.tasks.push(sub);
        self
    }

    /// Add multiple tasks to the batch.
    pub fn tasks(mut self, iter: impl IntoIterator<Item = TaskSubmission>) -> Self {
        self.tasks.extend(iter);
        self
    }

    /// Apply defaults and return the final submissions.
    ///
    /// - If a task has no `group_key` and `default_group` is set, the default is applied.
    /// - If a task has `Priority::NORMAL` (the default) and `default_priority` is set, it is overridden.
    pub fn build(mut self) -> Vec<TaskSubmission> {
        for task in &mut self.tasks {
            if task.group_key.is_none() {
                if let Some(ref group) = self.default_group {
                    task.group_key = Some(group.clone());
                }
            }
            if task.priority == Priority::NORMAL {
                if let Some(priority) = self.default_priority {
                    task.priority = priority;
                }
            }
            if task.ttl.is_none() {
                if let Some(ttl) = self.default_ttl {
                    task.ttl = Some(ttl);
                }
            }
            for (k, v) in &self.default_tags {
                task.tags.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        self.tasks
    }
}

impl Default for BatchSubmission {
    fn default() -> Self {
        Self::new()
    }
}

/// Parameters for submitting a new task.
///
/// Use the builder-style constructor [`TaskSubmission::new`] for ergonomic
/// construction with sensible defaults:
///
/// ```ignore
/// use taskmill::{TaskSubmission, Priority, IoBudget};
///
/// let sub = TaskSubmission::new("thumbnail")
///     .key("img-001")
///     .priority(Priority::HIGH)
///     .payload_json(&my_payload)?
///     .expected_io(IoBudget::disk(4096, 1024));
/// ```
///
/// For strongly-typed tasks, prefer [`TaskSubmission::from_typed`] or
/// [`Scheduler::submit_typed`](crate::Scheduler::submit_typed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSubmission {
    pub task_type: String,
    /// Optional dedup key. When `None`, the key is auto-generated by hashing
    /// `task_type` and `payload`, so two submissions with the same type and
    /// payload are deduplicated automatically.
    pub dedup_key: Option<String>,
    /// Human-readable label for UI display. Defaults to `dedup_key` (if set)
    /// or `task_type`. Override with [`label()`](Self::label) if needed.
    pub label: String,
    pub priority: Priority,
    pub payload: Option<Vec<u8>>,
    /// Expected IO budget for scheduling.
    pub expected_io: IoBudget,
    /// Parent task ID for hierarchical tasks. Set automatically by
    /// [`TaskContext::spawn_child`](crate::TaskContext::spawn_child).
    pub parent_id: Option<i64>,
    /// When `true` (default), the first child failure cancels siblings and
    /// fails the parent immediately. When `false`, the parent waits for all
    /// children to finish before resolving. Only meaningful for parent tasks
    /// that spawn children.
    pub fail_fast: bool,
    /// Optional group key for per-group concurrency limiting.
    ///
    /// Tasks with the same group key share a concurrency budget controlled
    /// by [`SchedulerBuilder::group_concurrency`](crate::SchedulerBuilder::group_concurrency).
    /// Use this to prevent hammering a single endpoint (e.g. `"s3://my-bucket"`).
    pub group_key: Option<String>,
    /// Strategy for handling duplicate dedup keys. Default: [`DuplicateStrategy::Skip`].
    #[serde(default)]
    pub on_duplicate: DuplicateStrategy,
    /// Time-to-live for the task. If set, the task will be automatically expired
    /// if it hasn't started executing within this duration (clock starts
    /// based on [`ttl_from`](Self::ttl_from)).
    pub ttl: Option<Duration>,
    /// When the TTL clock starts. Default: [`TtlFrom::Submission`].
    #[serde(default)]
    pub ttl_from: TtlFrom,
    /// Deferred serialization error from [`payload_json`](Self::payload_json).
    /// Surfaced at submit time as [`StoreError::Serialization`](crate::StoreError::Serialization).
    #[serde(skip)]
    pub(crate) payload_error: Option<String>,
    /// Delayed dispatch: task enters `pending` but is not eligible for dispatch
    /// until this timestamp. `None` = immediately eligible.
    pub run_after: Option<DateTime<Utc>>,
    /// Recurring schedule configuration. When set, the task automatically
    /// re-enqueues after each execution.
    pub recurring: Option<RecurringSchedule>,
    /// Task IDs that this task depends on. The task enters `blocked` status
    /// and transitions to `pending` only after all dependencies complete.
    pub dependencies: Vec<i64>,
    /// What happens when a dependency fails. Default: [`DependencyFailurePolicy::Cancel`].
    #[serde(default)]
    pub on_dependency_failure: DependencyFailurePolicy,
    /// Key-value metadata tags for filtering, grouping, and display.
    /// Immutable after submission. Validated against [`MAX_TAG_KEY_LEN`],
    /// [`MAX_TAG_VALUE_LEN`], and [`MAX_TAGS_PER_TASK`] at submit time.
    #[serde(default)]
    pub tags: HashMap<String, String>,
}

impl TaskSubmission {
    /// Create a new submission with sensible defaults.
    ///
    /// Defaults: `Priority::NORMAL`, no payload, no dedup key, zero IO
    /// estimates, no parent, fail-fast enabled.
    ///
    /// Chain builder methods to customise:
    ///
    /// ```ignore
    /// TaskSubmission::new("resize")
    ///     .key("my-file.jpg")
    ///     .priority(Priority::HIGH)
    ///     .payload_json(&data)
    ///     .expected_io(IoBudget::disk(4096, 1024))
    /// ```
    pub fn new(task_type: impl Into<String>) -> Self {
        let task_type = task_type.into();
        let label = task_type.clone();
        Self {
            task_type,
            dedup_key: None,
            label,
            priority: Priority::NORMAL,
            payload: None,
            expected_io: IoBudget::default(),
            parent_id: None,
            fail_fast: true,
            group_key: None,
            on_duplicate: DuplicateStrategy::default(),
            ttl: None,
            ttl_from: TtlFrom::default(),
            payload_error: None,
            run_after: None,
            recurring: None,
            dependencies: Vec::new(),
            on_dependency_failure: DependencyFailurePolicy::default(),
            tags: HashMap::new(),
        }
    }

    /// Set an explicit dedup key.
    ///
    /// When set, deduplication is based on `hash(task_type + ":" + key)`
    /// instead of the payload contents. Useful when different payloads
    /// represent the same logical work (e.g. same file path with different
    /// timestamps).
    pub fn key(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        self.label = key.clone();
        self.dedup_key = Some(key);
        self
    }

    /// Override the display label.
    ///
    /// By default the label is derived from the dedup key (if set via
    /// [`key()`](Self::key)) or the task type. Use this to provide a
    /// custom human-readable description for UI display.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Set the scheduling priority. Default: [`Priority::NORMAL`].
    pub fn priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Set the payload from a serializable value (JSON-encoded).
    ///
    /// Serialization errors are deferred to submit time so the builder
    /// chain is never interrupted. The payload can be deserialized in
    /// the executor via [`TaskContext::payload`](crate::TaskContext::payload).
    pub fn payload_json<T: serde::Serialize>(mut self, data: &T) -> Self {
        match serde_json::to_vec(data) {
            Ok(bytes) => self.payload = Some(bytes),
            Err(e) => self.payload_error = Some(e.to_string()),
        }
        self
    }

    /// Set the payload from raw bytes.
    pub fn payload_raw(mut self, data: Vec<u8>) -> Self {
        self.payload = Some(data);
        self
    }

    /// Set the expected IO budget for scheduling.
    pub fn expected_io(mut self, budget: IoBudget) -> Self {
        self.expected_io = budget;
        self
    }

    /// Set the group key for per-group concurrency limiting.
    ///
    /// Tasks with the same group key share a concurrency budget. Use this
    /// to prevent hammering a single endpoint (e.g. `"s3://my-bucket"`).
    pub fn group(mut self, group_key: impl Into<String>) -> Self {
        self.group_key = Some(group_key.into());
        self
    }

    /// Set the fail-fast flag for parent tasks that spawn children.
    ///
    /// When `true` (the default), the first child failure cancels siblings
    /// and fails the parent immediately. When `false`, the parent waits for
    /// all children to finish before resolving.
    pub fn fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }

    /// Set the duplicate-handling strategy.
    ///
    /// Controls what happens when this submission's dedup key matches an
    /// existing task. Default: [`DuplicateStrategy::Skip`].
    pub fn on_duplicate(mut self, strategy: DuplicateStrategy) -> Self {
        self.on_duplicate = strategy;
        self
    }

    /// Set the time-to-live for the task.
    ///
    /// If the task hasn't started executing within this duration (from
    /// submission or first attempt, depending on [`ttl_from`](Self::ttl_from)),
    /// it is automatically expired.
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set when the TTL clock starts. Default: [`TtlFrom::Submission`].
    pub fn ttl_from(mut self, ttl_from: TtlFrom) -> Self {
        self.ttl_from = ttl_from;
        self
    }

    /// Delay execution by `delay` from submission time.
    ///
    /// The task enters `pending` immediately but is not eligible for
    /// dispatch until `now + delay`. Combines with priority, TTL, etc.
    pub fn run_after(mut self, delay: Duration) -> Self {
        self.run_after = Some(Utc::now() + delay);
        self
    }

    /// Schedule execution at a specific wall-clock time.
    ///
    /// If `at` is in the past, the task is immediately eligible.
    pub fn run_at(mut self, at: DateTime<Utc>) -> Self {
        self.run_after = Some(at);
        self
    }

    /// Make this a recurring task that re-enqueues every `interval`.
    ///
    /// First execution is immediate (unless combined with `run_after`).
    pub fn recurring(mut self, interval: Duration) -> Self {
        self.recurring = Some(RecurringSchedule {
            interval,
            initial_delay: None,
            max_executions: None,
        });
        self
    }

    /// Declare that this task depends on another task completing successfully.
    ///
    /// The task enters `blocked` status and transitions to `pending` only
    /// after all dependencies have completed. Can be called multiple times
    /// to declare multiple dependencies (fan-in).
    pub fn depends_on(mut self, task_id: i64) -> Self {
        self.dependencies.push(task_id);
        self
    }

    /// Declare multiple dependencies at once.
    pub fn depends_on_all(mut self, task_ids: impl IntoIterator<Item = i64>) -> Self {
        self.dependencies.extend(task_ids);
        self
    }

    /// Configure behavior when a dependency fails.
    /// Default: [`DependencyFailurePolicy::Cancel`].
    pub fn on_dependency_failure(mut self, policy: DependencyFailurePolicy) -> Self {
        self.on_dependency_failure = policy;
        self
    }

    /// Add a single metadata tag (key-value pair).
    ///
    /// Tags are schema-free metadata for filtering and grouping. They are
    /// validated at submit time against [`MAX_TAG_KEY_LEN`],
    /// [`MAX_TAG_VALUE_LEN`], and [`MAX_TAGS_PER_TASK`].
    pub fn tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// Set all metadata tags at once, replacing any previously set tags.
    pub fn tags(mut self, tags: HashMap<String, String>) -> Self {
        self.tags = tags;
        self
    }

    /// Make this a recurring task with full schedule control.
    pub fn recurring_schedule(mut self, schedule: RecurringSchedule) -> Self {
        if let Some(delay) = schedule.initial_delay {
            if self.run_after.is_none() {
                self.run_after = Some(Utc::now() + delay);
            }
        }
        self.recurring = Some(schedule);
        self
    }

    /// Resolve the effective dedup key. Always incorporates the task type
    /// so different task types never collide, even with the same logical key.
    ///
    /// - Explicit key: `hash(task_type + ":" + key)`
    /// - No key: `hash(task_type + ":" + payload)`
    pub fn effective_key(&self) -> String {
        match &self.dedup_key {
            Some(k) => generate_dedup_key(&self.task_type, Some(k.as_bytes())),
            None => generate_dedup_key(&self.task_type, self.payload.as_deref()),
        }
    }

    /// Create a submission from a [`TypedTask`], serializing the payload and
    /// pulling task type, priority, IO estimates, key, label, group key,
    /// scheduling delay, and recurring schedule from the trait.
    pub fn from_typed<T: TypedTask>(task: &T) -> Self {
        let mut sub = Self::new(T::TASK_TYPE)
            .priority(task.priority())
            .payload_json(task)
            .expected_io(task.expected_io())
            .on_duplicate(task.on_duplicate())
            .ttl_from(task.ttl_from());
        if let Some(t) = task.ttl() {
            sub = sub.ttl(t);
        }
        if let Some(k) = task.key() {
            sub = sub.key(k);
        }
        if let Some(l) = task.label() {
            sub = sub.label(l);
        }
        if let Some(g) = task.group_key() {
            sub = sub.group(g);
        }
        if let Some(delay) = task.run_after() {
            sub = sub.run_after(delay);
        }
        if let Some(sched) = task.recurring() {
            sub = sub.recurring_schedule(sched);
        }
        let task_tags = task.tags();
        if !task_tags.is_empty() {
            sub = sub.tags(task_tags);
        }
        sub
    }
}
