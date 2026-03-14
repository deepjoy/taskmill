//! Progress reporting and throughput-based extrapolation.
//!
//! Executors call [`ProgressReporter::report`] (via [`TaskContext::progress`](crate::TaskContext::progress))
//! to emit percentage updates as [`SchedulerEvent::Progress`]
//! events. The scheduler combines these with historical throughput data to
//! produce [`EstimatedProgress`] snapshots, available via
//! [`Scheduler::estimated_progress`](super::Scheduler::estimated_progress) or
//! the [`SchedulerSnapshot`](super::SchedulerSnapshot).

use serde::{Deserialize, Serialize};

use crate::store::TaskStore;
use crate::task::TaskRecord;

use super::dispatch::ActiveTaskMap;
use super::SchedulerEvent;

// ── Progress Reporter ──────────────────────────────────────────────

/// Handle passed to executors for reporting progress back to the scheduler.
///
/// Progress reports are emitted as [`SchedulerEvent::Progress`]
/// events, making them available to the UI via the same broadcast channel.
/// The reporter also updates the in-memory [`ActiveTaskMap`] directly,
/// eliminating the need for a per-task broadcast listener.
///
/// # Example
///
/// ```ignore
/// // Inside a TaskExecutor::execute implementation:
/// async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
///     let items = vec![/* ... */];
///     for (i, item) in items.iter().enumerate() {
///         // process item...
///         ctx.progress().report_fraction(i as u64 + 1, items.len() as u64, None);
///     }
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct ProgressReporter {
    task_id: i64,
    task_type: String,
    key: String,
    label: String,
    event_tx: tokio::sync::broadcast::Sender<SchedulerEvent>,
    /// Direct handle for updating internal progress tracking without a
    /// broadcast roundtrip.
    active: ActiveTaskMap,
}

impl ProgressReporter {
    pub(crate) fn new(
        task_id: i64,
        task_type: String,
        key: String,
        label: String,
        event_tx: tokio::sync::broadcast::Sender<SchedulerEvent>,
        active: ActiveTaskMap,
    ) -> Self {
        Self {
            task_id,
            task_type,
            key,
            label,
            event_tx,
            active,
        }
    }

    /// Report progress as a percentage (0.0 to 1.0) with an optional message.
    pub fn report(&self, percent: f32, message: Option<String>) {
        let clamped = percent.clamp(0.0, 1.0);
        // Update internal progress tracking directly (sync, no broadcast roundtrip).
        self.active.update_progress(self.task_id, clamped);
        // Broadcast for external subscribers (UI / Tauri).
        let _ = self.event_tx.send(SchedulerEvent::Progress {
            task_id: self.task_id,
            task_type: self.task_type.clone(),
            key: self.key.clone(),
            label: self.label.clone(),
            percent: clamped,
            message,
        });
    }

    /// Report progress as a fraction (completed / total) with an optional message.
    pub fn report_fraction(&self, completed: u64, total: u64, message: Option<String>) {
        let percent = if total == 0 {
            1.0
        } else {
            completed as f32 / total as f32
        };
        self.report(percent, message);
    }
}

// ── Estimated Progress ─────────────────────────────────────────────

/// Estimated progress for a running task, combining executor-reported progress
/// with throughput-based extrapolation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimatedProgress {
    pub task_id: i64,
    pub task_type: String,
    pub key: String,
    pub label: String,
    /// Executor-reported progress (0.0 to 1.0), if available.
    pub reported_percent: Option<f32>,
    /// Throughput-extrapolated progress (0.0 to 1.0), if history data exists.
    pub extrapolated_percent: Option<f32>,
    /// Best available progress estimate.
    pub percent: f32,
}

/// Extrapolate progress for a single active task using historical throughput.
///
/// Blends executor-reported progress with time-based extrapolation from
/// `store.history_stats()`. This is a pure query — no side effects.
pub(crate) async fn extrapolate(
    record: &TaskRecord,
    reported_progress: Option<f32>,
    reported_at: Option<chrono::DateTime<chrono::Utc>>,
    store: &TaskStore,
) -> EstimatedProgress {
    let reported = reported_progress;

    let extrapolated = if let Some(started) = record.started_at {
        let now = chrono::Utc::now();
        if let Ok(stats) = store.history_stats(&record.task_type).await {
            if stats.avg_duration_ms > 0.0 {
                // Historical throughput: fraction of work completed per ms.
                let hist_throughput = 1.0 / stats.avg_duration_ms;

                match (reported, reported_at) {
                    // We have a progress anchor — blend throughputs and
                    // extrapolate from the last report.
                    (Some(rp), Some(rat)) => {
                        let elapsed_to_report = (rat - started).num_milliseconds().max(1) as f64;
                        let current_throughput = rp as f64 / elapsed_to_report;
                        let blended = (hist_throughput + current_throughput) / 2.0;
                        let since_report = (now - rat).num_milliseconds().max(0) as f64;
                        Some((rp as f64 + blended * since_report).min(0.99) as f32)
                    }
                    // No report yet — pure time-based extrapolation.
                    _ => {
                        let elapsed_ms = (now - started).num_milliseconds() as f64;
                        Some((elapsed_ms * hist_throughput).min(0.99) as f32)
                    }
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Best estimate: prefer reported, fall back to extrapolated, then 0.
    let percent = reported.or(extrapolated).unwrap_or(0.0);

    EstimatedProgress {
        task_id: record.id,
        task_type: record.task_type.clone(),
        key: record.key.clone(),
        label: record.label.clone(),
        reported_percent: reported,
        extrapolated_percent: extrapolated,
        percent,
    }
}
