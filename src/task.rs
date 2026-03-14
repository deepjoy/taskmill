//! Task types, submission parameters, and the [`TypedTask`] trait.
//!
//! This module defines the data structures that flow through the scheduler:
//! [`TaskSubmission`] for enqueuing work, [`TaskRecord`] for in-flight tasks,
//! [`TaskHistoryRecord`] for completed/failed results, and [`TypedTask`] for
//! strongly-typed task payloads with built-in serialization.
//!
//! Submit tasks via [`Scheduler::submit`](crate::Scheduler::submit) or
//! [`Scheduler::submit_typed`](crate::Scheduler::submit_typed). Executors
//! receive a [`TaskContext`](crate::TaskContext) with the deserialized record
//! and report results via [`TaskError`].

use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::priority::Priority;

/// Maximum payload size in bytes (1 MiB).
pub const MAX_PAYLOAD_BYTES: usize = 1_048_576;

/// Lifecycle state of a task in the active queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Paused,
    /// Parent task whose executor has returned but whose children are still
    /// active. Transitions to `Running` (for finalize) or terminal once all
    /// children complete.
    Waiting,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Waiting => "waiting",
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "paused" => Ok(Self::Paused),
            "waiting" => Ok(Self::Waiting),
            other => Err(format!("unknown TaskStatus: {other}")),
        }
    }
}

/// Terminal state of a task in history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryStatus {
    Completed,
    Failed,
}

impl HistoryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl std::str::FromStr for HistoryStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            other => Err(format!("unknown HistoryStatus: {other}")),
        }
    }
}

/// A task in the active queue (pending, running, or paused).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: i64,
    pub task_type: String,
    pub key: String,
    /// Human-readable label for UI display. Carries the original dedup key
    /// (or `task_type` if no explicit key was given). The `key` field holds
    /// the SHA-256 hash used for deduplication.
    pub label: String,
    pub priority: Priority,
    pub status: TaskStatus,
    pub payload: Option<Vec<u8>>,
    pub expected_read_bytes: i64,
    pub expected_write_bytes: i64,
    pub expected_net_rx_bytes: i64,
    pub expected_net_tx_bytes: i64,
    pub retry_count: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub requeue: bool,
    pub requeue_priority: Option<Priority>,
    /// Parent task ID for hierarchical tasks. `None` for top-level tasks.
    pub parent_id: Option<i64>,
    /// When `true` (default), the first child failure cancels siblings and
    /// fails the parent immediately. When `false`, the parent waits for all
    /// children to finish before resolving.
    pub fail_fast: bool,
    /// Optional group key for per-group concurrency limiting (e.g. an
    /// endpoint URL). Tasks in the same group share a concurrency budget.
    pub group_key: Option<String>,
}

impl TaskRecord {
    /// Deserialize the payload blob into a typed value.
    ///
    /// Returns `None` if the payload is absent, or an error if deserialization fails.
    pub fn deserialize_payload<T: serde::de::DeserializeOwned>(
        &self,
    ) -> Result<Option<T>, serde_json::Error> {
        match &self.payload {
            Some(bytes) => serde_json::from_slice(bytes).map(Some),
            None => Ok(None),
        }
    }
}

/// A task that has completed or permanently failed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskHistoryRecord {
    pub id: i64,
    pub task_type: String,
    pub key: String,
    /// Human-readable label for UI display (see [`TaskRecord::label`]).
    pub label: String,
    pub priority: Priority,
    pub status: HistoryStatus,
    pub payload: Option<Vec<u8>>,
    pub expected_read_bytes: i64,
    pub expected_write_bytes: i64,
    pub expected_net_rx_bytes: i64,
    pub expected_net_tx_bytes: i64,
    pub actual_read_bytes: Option<i64>,
    pub actual_write_bytes: Option<i64>,
    pub actual_net_rx_bytes: Option<i64>,
    pub actual_net_tx_bytes: Option<i64>,
    pub retry_count: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: DateTime<Utc>,
    pub duration_ms: Option<i64>,
    /// Parent task ID for hierarchical tasks.
    pub parent_id: Option<i64>,
    /// Whether the parent used fail-fast semantics.
    pub fail_fast: bool,
    /// Optional group key for per-group concurrency limiting.
    pub group_key: Option<String>,
}

/// Accumulated IO metrics captured by the scheduler after an executor finishes.
///
/// Executors report metrics incrementally via [`TaskContext::record_read_bytes`](crate::TaskContext::record_read_bytes)
/// and [`TaskContext::record_write_bytes`](crate::TaskContext::record_write_bytes).
/// This struct is the snapshot read by the scheduler — executors never construct it directly.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskMetrics {
    pub read_bytes: i64,
    pub write_bytes: i64,
    pub net_rx_bytes: i64,
    pub net_tx_bytes: i64,
}

/// Reported by the executor on failure.
///
/// The scheduler uses the [`retryable`](Self::retryable) flag to decide
/// whether to requeue the task or move it to history as permanently failed:
///
/// - **Non-retryable** ([`TaskError::new`]): the task moves directly to the
///   history table with status `failed`. Use this for logic errors, invalid
///   payloads, or conditions that won't change on retry.
/// - **Retryable** ([`TaskError::retryable`]): the task is requeued as
///   `pending` with an incremented retry count, keeping the same priority.
///   After [`SchedulerConfig::max_retries`](crate::SchedulerConfig::max_retries)
///   attempts (default 3), the task fails permanently.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskError {
    pub message: String,
    pub retryable: bool,
}

impl TaskError {
    /// Create a **non-retryable** error. The task will fail permanently and
    /// move to the history table.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: false,
        }
    }

    /// Create a **retryable** error. The task will be requeued as pending
    /// and retried up to [`SchedulerConfig::max_retries`](crate::SchedulerConfig::max_retries)
    /// times before failing permanently.
    pub fn retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TaskError {}

impl From<String> for TaskError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&str> for TaskError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

impl From<serde_json::Error> for TaskError {
    fn from(e: serde_json::Error) -> Self {
        Self::new(e.to_string())
    }
}

impl From<crate::store::StoreError> for TaskError {
    fn from(e: crate::store::StoreError) -> Self {
        Self::new(e.to_string())
    }
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

/// Generate a dedup key by hashing the task type and payload.
///
/// Produces a hex-encoded SHA-256 digest of `task_type` concatenated with
/// the payload bytes (or an empty slice when there is no payload).
pub fn generate_dedup_key(task_type: &str, payload: Option<&[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(task_type.as_bytes());
    hasher.update(b":");
    if let Some(p) = payload {
        hasher.update(p);
    }
    format!("{:x}", hasher.finalize())
}

/// Parameters for submitting a new task.
///
/// Use the builder-style constructor [`TaskSubmission::new`] for ergonomic
/// construction with sensible defaults:
///
/// ```ignore
/// use taskmill::{TaskSubmission, Priority};
///
/// let sub = TaskSubmission::new("thumbnail")
///     .key("img-001")
///     .priority(Priority::HIGH)
///     .payload_json(&my_payload)?
///     .expected_io(4096, 1024);
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
    pub expected_read_bytes: i64,
    pub expected_write_bytes: i64,
    pub expected_net_rx_bytes: i64,
    pub expected_net_tx_bytes: i64,
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
    ///     .payload_json(&data)?
    ///     .expected_io(4096, 1024)
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
            expected_read_bytes: 0,
            expected_write_bytes: 0,
            expected_net_rx_bytes: 0,
            expected_net_tx_bytes: 0,
            parent_id: None,
            fail_fast: true,
            group_key: None,
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
    /// Returns an error if serialization fails. The payload can be
    /// deserialized in the executor via [`TaskContext::payload`](crate::TaskContext::payload).
    pub fn payload_json<T: serde::Serialize>(
        mut self,
        data: &T,
    ) -> Result<Self, serde_json::Error> {
        self.payload = Some(serde_json::to_vec(data)?);
        Ok(self)
    }

    /// Set the payload from raw bytes.
    pub fn payload_raw(mut self, data: Vec<u8>) -> Self {
        self.payload = Some(data);
        self
    }

    /// Set expected disk IO bytes for budget-based scheduling.
    ///
    /// The scheduler uses these estimates to avoid saturating disk throughput
    /// when [resource monitoring](crate::SchedulerBuilder::with_resource_monitoring)
    /// is enabled. Default: 0 for both.
    pub fn expected_io(mut self, read_bytes: i64, write_bytes: i64) -> Self {
        self.expected_read_bytes = read_bytes;
        self.expected_write_bytes = write_bytes;
        self
    }

    /// Set expected network IO bytes for budget-based scheduling.
    ///
    /// The scheduler uses these estimates to avoid saturating network bandwidth
    /// when [resource monitoring](crate::SchedulerBuilder::with_resource_monitoring)
    /// is enabled. Default: 0 for both.
    pub fn expected_net_io(mut self, rx_bytes: i64, tx_bytes: i64) -> Self {
        self.expected_net_rx_bytes = rx_bytes;
        self.expected_net_tx_bytes = tx_bytes;
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

    /// Create a submission with a typed payload serialized to JSON bytes.
    ///
    /// The dedup key is auto-generated from the task type and serialized payload.
    /// Use `TaskRecord::deserialize_payload()` on the executor side to recover the type.
    #[deprecated(
        since = "2.0.0",
        note = "use `TaskSubmission::new(task_type).payload_json(&data)?.priority(p).expected_io(r, w)` instead"
    )]
    pub fn with_payload<T: serde::Serialize>(
        task_type: &str,
        priority: Priority,
        data: &T,
        expected_read_bytes: i64,
        expected_write_bytes: i64,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self::new(task_type)
            .priority(priority)
            .payload_json(data)?
            .expected_io(expected_read_bytes, expected_write_bytes))
    }
}

/// A strongly-typed task that bundles serialization, task type name, and default
/// IO estimates.
///
/// Implementing this trait collapses the 6 fields of [`TaskSubmission`] into a
/// derive-friendly pattern. Use [`Scheduler::submit_typed`](crate::Scheduler::submit_typed)
/// to submit and [`TaskContext::payload`](crate::TaskContext::payload) on the
/// executor side to deserialize. Each `TypedTask` must have a corresponding
/// [`TaskExecutor`](crate::TaskExecutor) registered under the same
/// [`TASK_TYPE`](Self::TASK_TYPE) name.
///
/// # Example
///
/// ```ignore
/// use serde::{Serialize, Deserialize};
/// use taskmill::{TypedTask, Priority};
///
/// #[derive(Serialize, Deserialize)]
/// struct Thumbnail { path: String, size: u32 }
///
/// impl TypedTask for Thumbnail {
///     const TASK_TYPE: &'static str = "thumbnail";
///     fn expected_read_bytes(&self) -> i64 { 4096 }
///     fn expected_write_bytes(&self) -> i64 { 1024 }
/// }
/// ```
pub trait TypedTask: Serialize + DeserializeOwned + Send + 'static {
    /// Unique name used to register and look up the executor.
    const TASK_TYPE: &'static str;

    /// Estimated bytes this task will read from disk. Default: 0.
    fn expected_read_bytes(&self) -> i64 {
        0
    }

    /// Estimated bytes this task will write to disk. Default: 0.
    fn expected_write_bytes(&self) -> i64 {
        0
    }

    /// Estimated bytes this task will receive over the network. Default: 0.
    fn expected_net_rx_bytes(&self) -> i64 {
        0
    }

    /// Estimated bytes this task will transmit over the network. Default: 0.
    fn expected_net_tx_bytes(&self) -> i64 {
        0
    }

    /// Scheduling priority. Default: [`Priority::NORMAL`].
    fn priority(&self) -> Priority {
        Priority::NORMAL
    }

    /// Optional group key for per-group concurrency limiting. Default: `None`.
    fn group_key(&self) -> Option<String> {
        None
    }
}

impl TaskSubmission {
    /// Create a submission from a [`TypedTask`], serializing the payload and
    /// pulling task type, priority, IO estimates, and group key from the trait.
    pub fn from_typed<T: TypedTask>(task: &T) -> Result<Self, serde_json::Error> {
        let mut sub = Self::new(T::TASK_TYPE)
            .priority(task.priority())
            .payload_json(task)?
            .expected_io(task.expected_read_bytes(), task.expected_write_bytes())
            .expected_net_io(task.expected_net_rx_bytes(), task.expected_net_tx_bytes());
        sub.group_key = task.group_key();
        Ok(sub)
    }
}

/// Unified lookup result for querying a task by its dedup inputs.
///
/// Returned by [`TaskStore::task_lookup`](crate::TaskStore::task_lookup) and
/// [`Scheduler::task_lookup`](crate::Scheduler::task_lookup).
/// Tells the caller whether a task is currently active (pending, running,
/// or paused) or has finished (completed or failed), without requiring
/// them to manually compute the dedup key or query two tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "location", content = "record")]
pub enum TaskLookup {
    /// Task is in the active queue (pending, running, or paused).
    Active(TaskRecord),
    /// Task has finished and is in the history table.
    /// Contains the most recent history entry for that key.
    History(TaskHistoryRecord),
    /// No task with this key exists in either table.
    NotFound,
}

/// Aggregate statistics for a task type from history.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypeStats {
    pub count: i64,
    pub avg_duration_ms: f64,
    pub avg_read_bytes: f64,
    pub avg_write_bytes: f64,
    pub failure_rate: f64,
}

/// Resolution of a parent task after a child completes or fails.
///
/// Returned by [`TaskStore::try_resolve_parent`](crate::TaskStore::try_resolve_parent) to tell the scheduler
/// what action to take on the parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParentResolution {
    /// All children are done and none failed — parent is ready for finalize.
    ReadyToFinalize,
    /// At least one child failed (terminal) — parent should fail.
    Failed(String),
    /// Children are still active — no action needed yet.
    StillWaiting,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Thumbnail {
        path: String,
        size: u32,
    }

    impl TypedTask for Thumbnail {
        const TASK_TYPE: &'static str = "thumbnail";

        fn expected_read_bytes(&self) -> i64 {
            4096
        }

        fn expected_write_bytes(&self) -> i64 {
            1024
        }
    }

    #[test]
    fn typed_task_to_submission() {
        let task = Thumbnail {
            path: "/photos/a.jpg".into(),
            size: 256,
        };
        let sub = TaskSubmission::from_typed(&task).unwrap();

        assert_eq!(sub.task_type, "thumbnail");
        assert_eq!(sub.priority, Priority::NORMAL);
        assert_eq!(sub.expected_read_bytes, 4096);
        assert_eq!(sub.expected_write_bytes, 1024);
        assert!(sub.dedup_key.is_none());

        // Payload round-trips correctly.
        let recovered: Thumbnail = serde_json::from_slice(sub.payload.as_ref().unwrap()).unwrap();
        assert_eq!(recovered, task);
    }

    #[test]
    fn typed_task_custom_priority() {
        #[derive(Serialize, Deserialize)]
        struct Urgent {
            id: u64,
        }

        impl TypedTask for Urgent {
            const TASK_TYPE: &'static str = "urgent";

            fn priority(&self) -> Priority {
                Priority::HIGH
            }
        }

        let sub = TaskSubmission::from_typed(&Urgent { id: 42 }).unwrap();
        assert_eq!(sub.priority, Priority::HIGH);
        assert_eq!(sub.task_type, "urgent");
    }

    #[test]
    fn typed_task_defaults() {
        #[derive(Serialize, Deserialize)]
        struct Minimal;

        impl TypedTask for Minimal {
            const TASK_TYPE: &'static str = "minimal";
        }

        let sub = TaskSubmission::from_typed(&Minimal).unwrap();
        assert_eq!(sub.expected_read_bytes, 0);
        assert_eq!(sub.expected_write_bytes, 0);
        assert_eq!(sub.expected_net_rx_bytes, 0);
        assert_eq!(sub.expected_net_tx_bytes, 0);
        assert_eq!(sub.priority, Priority::NORMAL);
        assert!(sub.group_key.is_none());
    }

    #[test]
    fn typed_task_with_network_and_group() {
        #[derive(Serialize, Deserialize)]
        struct S3Upload {
            bucket: String,
            size: i64,
        }

        impl TypedTask for S3Upload {
            const TASK_TYPE: &'static str = "s3-upload";

            fn expected_net_tx_bytes(&self) -> i64 {
                self.size
            }

            fn group_key(&self) -> Option<String> {
                Some(format!("s3://{}", self.bucket))
            }
        }

        let task = S3Upload {
            bucket: "my-bucket".into(),
            size: 10_000_000,
        };
        let sub = TaskSubmission::from_typed(&task).unwrap();
        assert_eq!(sub.expected_net_tx_bytes, 10_000_000);
        assert_eq!(sub.expected_net_rx_bytes, 0);
        assert_eq!(sub.group_key.as_deref(), Some("s3://my-bucket"));
    }

    #[test]
    fn submission_builder_net_io_and_group() {
        let sub = TaskSubmission::new("upload")
            .expected_net_io(5000, 10000)
            .group("s3://bucket-a");
        assert_eq!(sub.expected_net_rx_bytes, 5000);
        assert_eq!(sub.expected_net_tx_bytes, 10000);
        assert_eq!(sub.group_key.as_deref(), Some("s3://bucket-a"));
    }
}
