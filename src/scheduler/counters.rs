//! Internal atomic counters for scheduler throughput metrics.
//!
//! Always maintained regardless of feature flags. Exposed via
//! [`MetricsSnapshot`] for consumers who don't use the `metrics` crate.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// Internal atomic counters for scheduler throughput metrics.
///
/// Always maintained regardless of feature flags. Counters are
/// incremented at the code path where the event happens (submit,
/// dispatch, completion, failure, etc.) and exposed via
/// [`Scheduler::metrics_snapshot()`](super::Scheduler::metrics_snapshot).
pub(crate) struct SchedulerCounters {
    pub submitted: AtomicU64,
    pub dispatched: AtomicU64,
    pub completed: AtomicU64,
    pub failed: AtomicU64,
    pub failed_retryable: AtomicU64,
    pub retried: AtomicU64,
    pub dead_lettered: AtomicU64,
    pub superseded: AtomicU64,
    pub cancelled: AtomicU64,
    pub expired: AtomicU64,
    pub preempted: AtomicU64,
    pub batches_submitted: AtomicU64,
    pub gate_denials: AtomicU64,
    pub rate_limit_throttles: AtomicU64,
    pub group_pauses: AtomicU64,
    pub group_resumes: AtomicU64,
    pub dependency_failures: AtomicU64,
    pub recurring_skipped: AtomicU64,
}

impl SchedulerCounters {
    pub(crate) fn new() -> Self {
        Self {
            submitted: AtomicU64::new(0),
            dispatched: AtomicU64::new(0),
            completed: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            failed_retryable: AtomicU64::new(0),
            retried: AtomicU64::new(0),
            dead_lettered: AtomicU64::new(0),
            superseded: AtomicU64::new(0),
            cancelled: AtomicU64::new(0),
            expired: AtomicU64::new(0),
            preempted: AtomicU64::new(0),
            batches_submitted: AtomicU64::new(0),
            gate_denials: AtomicU64::new(0),
            rate_limit_throttles: AtomicU64::new(0),
            group_pauses: AtomicU64::new(0),
            group_resumes: AtomicU64::new(0),
            dependency_failures: AtomicU64::new(0),
            recurring_skipped: AtomicU64::new(0),
        }
    }

    /// Take a snapshot of all counter values.
    pub(crate) fn snapshot(&self) -> CounterSnapshot {
        CounterSnapshot {
            submitted: self.submitted.load(Relaxed),
            dispatched: self.dispatched.load(Relaxed),
            completed: self.completed.load(Relaxed),
            failed: self.failed.load(Relaxed),
            failed_retryable: self.failed_retryable.load(Relaxed),
            retried: self.retried.load(Relaxed),
            dead_lettered: self.dead_lettered.load(Relaxed),
            superseded: self.superseded.load(Relaxed),
            cancelled: self.cancelled.load(Relaxed),
            expired: self.expired.load(Relaxed),
            preempted: self.preempted.load(Relaxed),
            batches_submitted: self.batches_submitted.load(Relaxed),
            gate_denials: self.gate_denials.load(Relaxed),
            rate_limit_throttles: self.rate_limit_throttles.load(Relaxed),
            group_pauses: self.group_pauses.load(Relaxed),
            group_resumes: self.group_resumes.load(Relaxed),
            dependency_failures: self.dependency_failures.load(Relaxed),
            recurring_skipped: self.recurring_skipped.load(Relaxed),
        }
    }
}

/// Snapshot of counter values only (no gauges). Used internally to build
/// [`MetricsSnapshot`].
pub(crate) struct CounterSnapshot {
    pub submitted: u64,
    pub dispatched: u64,
    pub completed: u64,
    pub failed: u64,
    pub failed_retryable: u64,
    pub retried: u64,
    pub dead_lettered: u64,
    pub superseded: u64,
    pub cancelled: u64,
    pub expired: u64,
    pub preempted: u64,
    pub batches_submitted: u64,
    pub gate_denials: u64,
    pub rate_limit_throttles: u64,
    pub group_pauses: u64,
    pub group_resumes: u64,
    pub dependency_failures: u64,
    pub recurring_skipped: u64,
}

/// Point-in-time counter and gauge snapshot for consumers who don't use
/// the `metrics` crate.
///
/// All counter values are cumulative since scheduler creation. Gauge values
/// reflect the current instant. Available without any feature flags.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricsSnapshot {
    // Counters (cumulative)
    pub submitted: u64,
    pub dispatched: u64,
    pub completed: u64,
    pub failed: u64,
    pub failed_retryable: u64,
    pub retried: u64,
    pub dead_lettered: u64,
    pub superseded: u64,
    pub cancelled: u64,
    pub expired: u64,
    pub preempted: u64,
    pub batches_submitted: u64,
    pub gate_denials: u64,
    pub rate_limit_throttles: u64,
    pub group_pauses: u64,
    pub group_resumes: u64,
    pub dependency_failures: u64,
    pub recurring_skipped: u64,
    // Gauges (point-in-time)
    pub pending: i64,
    pub running: usize,
    pub blocked: i64,
    pub paused: i64,
    pub waiting: i64,
    pub pressure: f32,
    pub max_concurrency: usize,
    pub groups_paused: usize,
}
