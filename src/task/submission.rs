//! Task submission types: [`TaskSubmission`] for single tasks, [`BatchSubmission`]
//! for building batches with shared defaults, [`SubmitOutcome`] for per-task
//! results, and [`BatchOutcome`] for categorized batch summaries.

use serde::{Deserialize, Serialize};

use crate::priority::Priority;

use super::dedup::generate_dedup_key;
use super::typed::TypedTask;
use super::IoBudget;

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
}

impl SubmitOutcome {
    /// Returns the task ID if the task was inserted, upgraded, or requeued.
    pub fn id(&self) -> Option<i64> {
        match self {
            Self::Inserted(id) | Self::Upgraded(id) | Self::Requeued(id) => Some(*id),
            Self::Duplicate => None,
        }
    }

    /// Returns `true` if a new task was inserted.
    pub fn is_inserted(&self) -> bool {
        matches!(self, Self::Inserted(_))
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
    tasks: Vec<TaskSubmission>,
}

impl BatchSubmission {
    /// Create a new empty batch submission builder.
    pub fn new() -> Self {
        Self {
            default_group: None,
            default_priority: None,
            tasks: Vec::new(),
        }
    }

    /// Set a default group key applied to tasks without an explicit group.
    pub fn default_group(mut self, group: impl Into<String>) -> Self {
        self.default_group = Some(group.into());
        self
    }

    /// Set a default priority applied to tasks still at [`Priority::NORMAL`].
    pub fn default_priority(mut self, priority: Priority) -> Self {
        self.default_priority = Some(priority);
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
    /// Deferred serialization error from [`payload_json`](Self::payload_json).
    /// Surfaced at submit time as [`StoreError::Serialization`](crate::StoreError::Serialization).
    #[serde(skip)]
    pub(crate) payload_error: Option<String>,
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
            payload_error: None,
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
    /// pulling task type, priority, IO estimates, key, label, and group key
    /// from the trait.
    pub fn from_typed<T: TypedTask>(task: &T) -> Self {
        let mut sub = Self::new(T::TASK_TYPE)
            .priority(task.priority())
            .payload_json(task)
            .expected_io(task.expected_io());
        if let Some(k) = task.key() {
            sub = sub.key(k);
        }
        if let Some(l) = task.label() {
            sub = sub.label(l);
        }
        if let Some(g) = task.group_key() {
            sub = sub.group(g);
        }
        sub
    }
}
