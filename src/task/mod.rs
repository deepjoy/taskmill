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

pub mod dedup;
mod error;
mod submission;
#[cfg(test)]
mod tests;
pub mod typed;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::priority::Priority;

pub use dedup::{generate_dedup_key, MAX_PAYLOAD_BYTES};
pub use error::TaskError;
pub use submission::{SubmitOutcome, TaskSubmission};
pub use typed::TypedTask;

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
    /// Estimated network receive bytes for IO budget scheduling.
    pub expected_net_rx_bytes: i64,
    /// Estimated network transmit bytes for IO budget scheduling.
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
    /// Estimated network receive bytes declared at submission.
    pub expected_net_rx_bytes: i64,
    /// Estimated network transmit bytes declared at submission.
    pub expected_net_tx_bytes: i64,
    pub actual_read_bytes: Option<i64>,
    pub actual_write_bytes: Option<i64>,
    /// Actual network receive bytes reported by the executor.
    pub actual_net_rx_bytes: Option<i64>,
    /// Actual network transmit bytes reported by the executor.
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
/// Executors report metrics incrementally via [`TaskContext::record_read_bytes`](crate::TaskContext::record_read_bytes),
/// [`record_write_bytes`](crate::TaskContext::record_write_bytes),
/// [`record_net_rx_bytes`](crate::TaskContext::record_net_rx_bytes), and
/// [`record_net_tx_bytes`](crate::TaskContext::record_net_tx_bytes).
/// This struct is the snapshot read by the scheduler — executors never construct it directly.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskMetrics {
    /// Actual disk bytes read during execution.
    pub read_bytes: i64,
    /// Actual disk bytes written during execution.
    pub write_bytes: i64,
    /// Actual network bytes received during execution.
    pub net_rx_bytes: i64,
    /// Actual network bytes transmitted during execution.
    pub net_tx_bytes: i64,
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
