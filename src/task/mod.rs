//! Task types, submission parameters, and the [`TypedTask`] trait.
//!
//! This module defines the data structures that flow through the scheduler:
//! [`TaskSubmission`] for enqueuing work, [`BatchSubmission`] for building
//! batches with shared defaults, [`BatchOutcome`] for categorized batch
//! results, [`TaskRecord`] for in-flight tasks, [`TaskHistoryRecord`] for
//! completed/failed/cancelled/superseded results, [`DuplicateStrategy`] for
//! controlling duplicate-key handling, and [`TypedTask`] for strongly-typed
//! task payloads with built-in serialization.
//!
//! # Task lifecycle states
//!
//! Active tasks have a [`TaskStatus`]:
//!
//! - `Pending` / `Running` / `Paused` / `Waiting` — standard lifecycle states
//! - [`Blocked`](TaskStatus::Blocked) — task is waiting for dependencies to
//!   complete before becoming eligible for dispatch
//!
//! Terminal tasks have a [`HistoryStatus`]:
//!
//! - `Completed` / `Failed` / `Cancelled` / `Superseded` / `Expired` — standard outcomes
//! - [`DependencyFailed`](HistoryStatus::DependencyFailed) — task was cancelled
//!   because a dependency failed, per its [`DependencyFailurePolicy`]
//!
//! Submit tasks via [`Scheduler::submit`](crate::Scheduler::submit),
//! [`Scheduler::submit_typed`](crate::Scheduler::submit_typed), or
//! [`Scheduler::submit_batch`](crate::Scheduler::submit_batch). Executors
//! receive a [`TaskContext`](crate::TaskContext) with the deserialized record
//! and report results via [`TaskError`].

pub mod dedup;
mod error;
pub mod retry;
mod submission;
#[cfg(test)]
mod tests;
pub mod typed;

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::priority::Priority;

pub use dedup::{generate_dedup_key, MAX_PAYLOAD_BYTES};
pub use error::TaskError;
pub use retry::{BackoffStrategy, RetryPolicy};
pub use submission::{
    BatchOutcome, BatchSubmission, DependencyFailurePolicy, DuplicateStrategy, RecurringSchedule,
    SubmitOutcome, TaskSubmission, MAX_TAGS_PER_TASK, MAX_TAG_KEY_LEN, MAX_TAG_VALUE_LEN,
};
pub use typed::TypedTask;

/// When the TTL clock starts ticking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtlFrom {
    /// TTL starts at submission time (default). `expires_at` is set immediately.
    #[default]
    Submission,
    /// TTL starts at first execution attempt. `expires_at` is set when the
    /// task transitions from pending to running for the first time.
    FirstAttempt,
}

impl TtlFrom {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Submission => "submission",
            Self::FirstAttempt => "first_attempt",
        }
    }
}

impl std::str::FromStr for TtlFrom {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "submission" => Ok(Self::Submission),
            "first_attempt" => Ok(Self::FirstAttempt),
            other => Err(format!("unknown TtlFrom: {other}")),
        }
    }
}

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
    /// Task is waiting for dependencies to complete before becoming eligible
    /// for dispatch. Transitions to `Pending` when all dependencies are
    /// satisfied, or to history as `DependencyFailed` if a dependency fails.
    Blocked,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Waiting => "waiting",
            Self::Blocked => "blocked",
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
            "blocked" => Ok(Self::Blocked),
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
    Cancelled,
    Superseded,
    Expired,
    /// A dependency failed and this task was auto-cancelled per its
    /// [`DependencyFailurePolicy`](crate::DependencyFailurePolicy).
    DependencyFailed,
}

impl HistoryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Superseded => "superseded",
            Self::Expired => "expired",
            Self::DependencyFailed => "dependency_failed",
        }
    }
}

impl std::str::FromStr for HistoryStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "superseded" => Ok(Self::Superseded),
            "expired" => Ok(Self::Expired),
            "dependency_failed" => Ok(Self::DependencyFailed),
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
    /// Expected IO budget declared at submission.
    pub expected_io: IoBudget,
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
    /// Original TTL duration in seconds. `None` means no TTL.
    pub ttl_seconds: Option<i64>,
    /// When the TTL clock starts.
    pub ttl_from: TtlFrom,
    /// Pre-computed expiry datetime. `None` means never expires.
    pub expires_at: Option<DateTime<Utc>>,
    /// Delayed dispatch: task is pending but not eligible until this
    /// timestamp. `None` means immediately eligible.
    pub run_after: Option<DateTime<Utc>>,
    /// Recurring interval in seconds. `None` means non-recurring.
    pub recurring_interval_secs: Option<i64>,
    /// Maximum number of recurring executions. `None` means unlimited.
    pub recurring_max_executions: Option<i64>,
    /// Number of recurring executions completed so far.
    pub recurring_execution_count: i64,
    /// Whether the recurring schedule is paused (no new instances created).
    pub recurring_paused: bool,
    /// IDs of tasks this task depends on (populated from `task_deps` table).
    /// Empty for tasks with no dependencies.
    pub dependencies: Vec<i64>,
    /// What happens when a dependency fails.
    pub on_dependency_failure: DependencyFailurePolicy,
    /// Key-value metadata tags for filtering, grouping, and display.
    pub tags: HashMap<String, String>,
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

    /// Build a [`TaskEventHeader`](crate::scheduler::event::TaskEventHeader) from this record.
    pub fn event_header(&self) -> crate::scheduler::event::TaskEventHeader {
        crate::scheduler::event::TaskEventHeader {
            task_id: self.id,
            task_type: self.task_type.clone(),
            key: self.key.clone(),
            label: self.label.clone(),
            tags: self.tags.clone(),
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
    /// Expected IO budget declared at submission.
    pub expected_io: IoBudget,
    /// Actual IO recorded by the executor, if available.
    pub actual_io: Option<IoBudget>,
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
    /// Original TTL duration in seconds. `None` means no TTL.
    pub ttl_seconds: Option<i64>,
    /// When the TTL clock starts.
    pub ttl_from: TtlFrom,
    /// Pre-computed expiry datetime. `None` means never expires.
    pub expires_at: Option<DateTime<Utc>>,
    /// Delayed dispatch timestamp at submission time (diagnostic).
    pub run_after: Option<DateTime<Utc>>,
    /// Key-value metadata tags for filtering, grouping, and display.
    pub tags: HashMap<String, String>,
}

/// IO budget for a task: expected or actual disk and network IO bytes.
///
/// Used in [`TaskSubmission`] for expected IO (scheduling), [`TaskRecord`] for
/// persisted expectations, and as the snapshot returned by the IO tracker after
/// execution completes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IoBudget {
    /// Disk bytes read.
    pub disk_read: i64,
    /// Disk bytes written.
    pub disk_write: i64,
    /// Network bytes received.
    pub net_rx: i64,
    /// Network bytes transmitted.
    pub net_tx: i64,
}

impl IoBudget {
    /// Create an `IoBudget` with only disk IO set.
    pub fn disk(read: i64, write: i64) -> Self {
        Self {
            disk_read: read,
            disk_write: write,
            ..Default::default()
        }
    }

    /// Create an `IoBudget` with only network IO set.
    pub fn net(rx: i64, tx: i64) -> Self {
        Self {
            net_rx: rx,
            net_tx: tx,
            ..Default::default()
        }
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

/// Information about an active recurring schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringScheduleInfo {
    pub task_id: i64,
    pub task_type: String,
    pub label: String,
    pub interval_secs: i64,
    pub next_run: Option<DateTime<Utc>>,
    pub execution_count: i64,
    pub max_executions: Option<i64>,
    pub paused: bool,
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
