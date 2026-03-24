//! Scheduler types: events, snapshots, and configuration.
//!
//! [`SchedulerEvent`] variants cover the full task lifecycle. Dependency-related
//! events:
//!
//! - [`TaskUnblocked { task_id }`](SchedulerEvent::TaskUnblocked) — a blocked
//!   task became pending after all its dependencies completed successfully
//! - [`DependencyFailed { task_id, failed_dependency }`](SchedulerEvent::DependencyFailed)
//!   — a blocked task was cancelled because a dependency failed

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tokio::time::Duration;

use crate::priority::Priority;

use super::progress::{EstimatedProgress, TaskProgress};
use super::rate_limit::RateLimitInfo;

// ── Snapshot ────────────────────────────────────────────────────────

/// Single-call status snapshot for dashboard UIs.
///
/// Captures queue depths, running tasks, progress, and backpressure in
/// one serializable struct — ideal for returning from a Tauri command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerSnapshot {
    /// Tasks currently executing.
    pub running: Vec<crate::task::TaskRecord>,
    /// Number of tasks waiting to be dispatched.
    pub pending_count: i64,
    /// Number of tasks paused (preempted).
    pub paused_count: i64,
    /// Number of parent tasks waiting for children to complete.
    pub waiting_count: i64,
    /// Progress estimates for every running task.
    pub progress: Vec<EstimatedProgress>,
    /// Aggregate backpressure (0.0–1.0).
    pub pressure: f32,
    /// Per-source pressure breakdown for diagnostics.
    pub pressure_breakdown: Vec<(String, f32)>,
    /// Current maximum concurrency setting.
    pub max_concurrency: usize,
    /// Byte-level progress for tasks reporting transfer progress.
    pub byte_progress: Vec<TaskProgress>,
    /// Whether the scheduler is globally paused.
    pub is_paused: bool,
    /// Active recurring schedules with their next run times.
    pub recurring_schedules: Vec<crate::task::RecurringScheduleInfo>,
    /// Tasks currently blocked waiting for dependencies.
    pub blocked_count: i64,
    /// Groups that are currently paused, with the timestamp each was paused.
    pub paused_groups: Vec<PausedGroupInfo>,
    /// Configured rate limits with current utilization.
    pub rate_limits: Vec<RateLimitInfo>,
    /// Priority aging configuration (if enabled).
    pub aging_config: Option<super::aging::AgingConfig>,
}

/// Information about a paused group for snapshot/dashboard display.
///
/// `paused_at` and `resume_at` are stored as epoch milliseconds (INTEGER) in SQLite.
/// Converted to `DateTime<Utc>` for the public API (display layer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PausedGroupInfo {
    pub group: String,
    pub paused_at: chrono::DateTime<chrono::Utc>,
    pub paused_task_count: i64,
    /// When the group will auto-resume (if time-boxed).
    pub resume_at: Option<chrono::DateTime<chrono::Utc>>,
}

// ── Task Event Header ────────────────────────────────────────────────

/// Common fields shared by task-specific [`SchedulerEvent`] variants.
///
/// Extracted to avoid repeating `task_id`, `task_type`, `key`, `label`
/// in every variant. Use [`SchedulerEvent::header()`] to access it
/// generically.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEventHeader {
    pub task_id: i64,
    pub task_type: String,
    /// Module name extracted from the `task_type` prefix (everything before
    /// `"::"`, e.g. `"media"` for `"media::thumbnail"`).
    /// Empty string for task types that have no module prefix.
    pub module: String,
    pub key: String,
    pub label: String,
    /// Key-value metadata tags from the task record.
    pub tags: HashMap<String, String>,
    /// Stored (base) priority.
    pub base_priority: Priority,
    /// Effective priority at the time the event was emitted.
    /// Computed by the scheduler from the `AgingConfig`; equals
    /// `base_priority` when aging is disabled or the task hasn't aged.
    pub effective_priority: Priority,
}

// ── Events ──────────────────────────────────────────────────────────

/// Events emitted by the scheduler for UI integration and observability.
///
/// Subscribe via the `tokio::sync::broadcast::Receiver` returned by
/// [`Scheduler::subscribe`](super::Scheduler::subscribe) or passed through the builder.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SchedulerEvent {
    /// A task was dispatched and is now running.
    Dispatched(TaskEventHeader),
    /// A task completed successfully.
    Completed(TaskEventHeader),
    /// A task failed (may be retried or permanently failed).
    Failed {
        header: TaskEventHeader,
        error: String,
        will_retry: bool,
        /// How long until the next retry attempt. `None` if the task will not
        /// be retried or if the retry is immediate (no backoff).
        retry_after: Option<Duration>,
    },
    /// A task was preempted by higher-priority work.
    Preempted(TaskEventHeader),
    /// A task was cancelled by the application.
    Cancelled(TaskEventHeader),
    /// A task was superseded by a new submission with the same dedup key.
    Superseded {
        /// Header of the old (replaced) task.
        old: TaskEventHeader,
        /// ID of the newly inserted replacement task.
        new_task_id: i64,
    },
    /// Progress update from a running task.
    Progress {
        header: TaskEventHeader,
        /// Progress percentage (0.0 to 1.0).
        percent: f32,
        /// Optional human-readable message from the executor.
        message: Option<String>,
    },
    /// A parent task entered the waiting state after its executor returned
    /// and it has active children.
    Waiting { task_id: i64, children_count: i64 },
    /// A batch of tasks was submitted.
    BatchSubmitted {
        /// Number of tasks in the batch (total input count).
        count: usize,
        /// Task IDs that were inserted (new tasks only, not upgrades/requeues).
        inserted_ids: Vec<i64>,
    },
    /// A task expired because its TTL elapsed before execution started.
    TaskExpired {
        header: TaskEventHeader,
        /// How long the task lived before expiring.
        age: Duration,
    },
    /// A recurring task instance was skipped because the previous instance
    /// hasn't been dispatched yet (pile-up prevention).
    RecurringSkipped {
        header: TaskEventHeader,
        reason: String,
    },
    /// A recurring task instance completed and the next instance was
    /// (or was not) created.
    RecurringCompleted {
        header: TaskEventHeader,
        execution_count: i64,
        /// `None` if `max_executions` reached or schedule paused.
        next_run: Option<chrono::DateTime<chrono::Utc>>,
    },
    /// A blocked task became pending after all its dependencies completed.
    TaskUnblocked { task_id: i64 },
    /// A task exhausted its retries and was moved to dead-letter state.
    ///
    /// The task failed with a retryable error but has reached its `max_retries`
    /// limit. Use [`Scheduler::retry_dead_letter`](super::Scheduler::retry_dead_letter)
    /// to re-submit.
    DeadLettered {
        header: TaskEventHeader,
        error: String,
        retry_count: i32,
    },
    /// A blocked task was cancelled because a dependency failed.
    DependencyFailed {
        task_id: i64,
        failed_dependency: i64,
    },
    /// The scheduler was globally paused via [`Scheduler::pause_all`](super::Scheduler::pause_all).
    Paused,
    /// The scheduler was resumed via [`Scheduler::resume_all`](super::Scheduler::resume_all).
    Resumed,
    /// A task group was paused.
    GroupPaused {
        group: String,
        pending_count: usize,
        running_count: usize,
    },
    /// A task group was resumed.
    GroupResumed { group: String, resumed_count: usize },
}

impl SchedulerEvent {
    /// Returns the [`TaskEventHeader`] if this event is task-specific.
    pub fn header(&self) -> Option<&TaskEventHeader> {
        match self {
            Self::Dispatched(h) | Self::Completed(h) | Self::Preempted(h) | Self::Cancelled(h) => {
                Some(h)
            }
            Self::Failed { header, .. }
            | Self::Progress { header, .. }
            | Self::Superseded { old: header, .. }
            | Self::TaskExpired { header, .. }
            | Self::RecurringSkipped { header, .. }
            | Self::RecurringCompleted { header, .. }
            | Self::DeadLettered { header, .. } => Some(header),
            Self::Waiting { .. }
            | Self::BatchSubmitted { .. }
            | Self::TaskUnblocked { .. }
            | Self::DependencyFailed { .. }
            | Self::Paused
            | Self::Resumed
            | Self::GroupPaused { .. }
            | Self::GroupResumed { .. } => None,
        }
    }
}

// ── Config ──────────────────────────────────────────────────────────

/// How the scheduler behaves during shutdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownMode {
    /// Cancel all running tasks immediately (default).
    Hard,
    /// Stop accepting new dispatches, wait for running tasks to complete
    /// (up to the given timeout), then cancel any remaining.
    Graceful(Duration),
}

/// Scheduler configuration.
///
/// All fields have sensible defaults (see [`Default`] impl). Most users
/// configure via [`SchedulerBuilder`](super::SchedulerBuilder) methods rather than constructing
/// this directly.
pub struct SchedulerConfig {
    /// Maximum concurrent running tasks. Adjusted dynamically via
    /// [`Scheduler::set_max_concurrency`](super::Scheduler::set_max_concurrency). Default: 4.
    ///
    /// Increase for IO-bound workloads where tasks spend most of their time
    /// waiting on network or disk. Decrease for CPU-bound work or when running
    /// on battery/mobile.
    pub max_concurrency: usize,
    /// Maximum retries before permanent failure. Default: 3.
    ///
    /// Only applies to tasks that return [`TaskError::retryable`](crate::TaskError::retryable). Non-retryable
    /// errors fail immediately regardless of this setting.
    pub max_retries: i32,
    /// Priority threshold for preemption. Tasks at or above this priority
    /// (lower numeric value = higher priority) trigger preemption of
    /// lower-priority running tasks. Default: [`Priority::REALTIME`].
    ///
    /// Set to [`Priority::HIGH`] if you want `HIGH`-priority tasks to also
    /// preempt. Set to `Priority::new(0)` to effectively disable preemption
    /// (only priority 0 would trigger it).
    pub preempt_priority: Priority,
    /// Interval between scheduler polls when idle. Default: 500ms.
    ///
    /// The scheduler also wakes immediately on task submission, so this mainly
    /// affects how quickly paused tasks are resumed and how often housekeeping
    /// runs. Lower values increase responsiveness at the cost of CPU usage.
    /// On mobile targets, the notify-based wake means the CPU can sleep between
    /// submissions regardless of this interval.
    pub poll_interval: Duration,
    /// How many recent completed tasks to sample for IO throughput estimation.
    /// Default: 20.
    ///
    /// Used by the IO budget gate to estimate how much disk bandwidth running
    /// tasks consume. Larger values smooth out outliers but adapt more slowly
    /// to changing workloads.
    pub throughput_sample_size: i32,
    /// Shutdown behavior. Default: [`ShutdownMode::Hard`].
    pub shutdown_mode: ShutdownMode,
    /// Interval for byte-level progress ticker. `None` (default) disables it.
    ///
    /// When set, a background task polls active tasks' byte counters at this
    /// interval and emits [`TaskProgress`] events on a dedicated channel.
    pub progress_interval: Option<Duration>,
    /// Timeout for [`TypedExecutor::on_cancel`](crate::TypedExecutor::on_cancel)
    /// hooks. If a cancel hook does not complete within this duration it is
    /// aborted. Default: 30 seconds.
    pub cancel_hook_timeout: Duration,
    /// Default TTL applied to tasks that don't specify one. `None` (default)
    /// means no global TTL.
    pub default_ttl: Option<Duration>,
    /// How often to sweep for expired tasks. `None` disables periodic sweeps
    /// (dispatch-time checks still apply). Default: `Some(30s)`.
    pub expiry_sweep_interval: Option<Duration>,
    /// Priority aging configuration. `None` (default) disables aging.
    pub aging_config: Option<super::aging::AgingConfig>,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 4,
            max_retries: 3,
            preempt_priority: Priority::REALTIME,
            poll_interval: Duration::from_millis(500),
            throughput_sample_size: 20,
            shutdown_mode: ShutdownMode::Hard,
            progress_interval: None,
            cancel_hook_timeout: Duration::from_secs(30),
            default_ttl: None,
            expiry_sweep_interval: Some(Duration::from_secs(30)),
            aging_config: None,
        }
    }
}
