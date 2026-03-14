//! Scheduler types: events, snapshots, and configuration.

use serde::{Deserialize, Serialize};
use tokio::time::Duration;

use crate::priority::Priority;

use super::progress::EstimatedProgress;

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
    /// Whether the scheduler is globally paused.
    pub is_paused: bool,
}

// ── Events ──────────────────────────────────────────────────────────

/// Events emitted by the scheduler for UI integration and observability.
///
/// Subscribe via the `tokio::sync::broadcast::Receiver` returned by
/// [`Scheduler::subscribe`] or passed through the builder.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SchedulerEvent {
    /// A task was dispatched and is now running.
    Dispatched {
        task_id: i64,
        task_type: String,
        key: String,
        label: String,
    },
    /// A task completed successfully.
    Completed {
        task_id: i64,
        task_type: String,
        key: String,
        label: String,
    },
    /// A task failed (may be retried or permanently failed).
    Failed {
        task_id: i64,
        task_type: String,
        key: String,
        label: String,
        error: String,
        will_retry: bool,
    },
    /// A task was preempted by higher-priority work.
    Preempted {
        task_id: i64,
        task_type: String,
        key: String,
        label: String,
    },
    /// A task was cancelled by the application.
    Cancelled {
        task_id: i64,
        task_type: String,
        key: String,
        label: String,
    },
    /// Progress update from a running task.
    Progress {
        task_id: i64,
        task_type: String,
        key: String,
        label: String,
        /// Progress percentage (0.0 to 1.0).
        percent: f32,
        /// Optional human-readable message from the executor.
        message: Option<String>,
    },
    /// A parent task entered the waiting state after its executor returned
    /// and it has active children.
    Waiting { task_id: i64, children_count: i64 },
    /// The scheduler was globally paused via [`Scheduler::pause_all`].
    Paused,
    /// The scheduler was resumed via [`Scheduler::resume_all`].
    Resumed,
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
        }
    }
}
