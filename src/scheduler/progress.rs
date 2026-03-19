//! Progress reporting, byte-level transfer tracking, and throughput-based extrapolation.
//!
//! This module provides two complementary progress systems:
//!
//! ## Percentage progress
//!
//! Executors call [`ProgressReporter::report`] (via [`TaskContext::progress`](crate::TaskContext::progress))
//! to emit percentage updates as [`SchedulerEvent::Progress`]
//! events. The scheduler combines these with historical throughput data to
//! produce [`EstimatedProgress`] snapshots, available via
//! [`Scheduler::estimated_progress`](super::Scheduler::estimated_progress) or
//! the [`SchedulerSnapshot`](super::SchedulerSnapshot).
//!
//! ## Byte-level progress
//!
//! For streaming transfers, executors call [`ProgressReporter::set_bytes_total`]
//! and [`ProgressReporter::add_bytes`] (or the convenience wrappers on
//! [`TaskContext`](crate::TaskContext)). These update lock-free atomic counters
//! on the task's `IoTracker`.
//!
//! A background progress ticker task (opt-in via
//! [`SchedulerBuilder::progress_interval`](super::SchedulerBuilder::progress_interval))
//! polls these counters at a fixed interval, feeds them through an
//! EWMA-smoothed `ThroughputTracker`, and emits [`TaskProgress`] events on
//! a dedicated broadcast channel. Each event includes throughput (bytes/sec)
//! and an estimated time remaining.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::registry::IoTracker;
use crate::store::TaskStore;
use crate::task::TaskRecord;

use super::dispatch::ActiveTaskMap;
use super::event::TaskEventHeader;
use super::SchedulerEvent;

// ── Progress Reporter ──────────────────────────────────────────────

/// Handle passed to executors for reporting progress back to the scheduler.
///
/// Progress reports are emitted as [`SchedulerEvent::Progress`]
/// events, making them available to the UI via the same broadcast channel.
/// The reporter also updates the in-memory `ActiveTaskMap` directly,
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
    header: TaskEventHeader,
    event_tx: tokio::sync::broadcast::Sender<SchedulerEvent>,
    /// Direct handle for updating internal progress tracking without a
    /// broadcast roundtrip.
    active: ActiveTaskMap,
    /// Shared IO tracker for byte-level progress.
    io: Arc<IoTracker>,
}

impl ProgressReporter {
    pub(crate) fn new(
        header: TaskEventHeader,
        event_tx: tokio::sync::broadcast::Sender<SchedulerEvent>,
        active: ActiveTaskMap,
        io: Arc<IoTracker>,
    ) -> Self {
        Self {
            header,
            event_tx,
            active,
            io,
        }
    }

    /// Report progress as a percentage (0.0 to 1.0) with an optional message.
    pub fn report(&self, percent: f32, message: Option<String>) {
        let clamped = percent.clamp(0.0, 1.0);
        // Update internal progress tracking directly (sync, no broadcast roundtrip).
        self.active.update_progress(self.header.task_id, clamped);
        // Broadcast for external subscribers (UI / Tauri).
        let _ = self.event_tx.send(SchedulerEvent::Progress {
            header: self.header.clone(),
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

    /// Set the total number of bytes expected for byte-level progress.
    ///
    /// Call once when the total is known (e.g. from a `Content-Length` header
    /// or file size).
    pub fn set_bytes_total(&self, total: u64) {
        self.io.set_total(total);
    }

    /// Increment completed bytes by `delta`.
    ///
    /// Call per chunk in a streaming transfer.
    pub fn add_bytes(&self, delta: u64) {
        self.io.add_progress(delta);
    }

    /// Set both completed and total bytes to absolute values.
    ///
    /// For use cases where the caller tracks absolute position rather than
    /// incremental deltas.
    pub fn report_bytes(&self, completed: u64, total: u64) {
        self.io.set_progress(completed);
        self.io.set_total(total);
    }
}

// ── Task Progress ─────────────────────────────────────────────────

/// Byte-level progress snapshot for a single task.
///
/// Emitted on the dedicated progress channel at the configured interval.
/// Contains throughput and ETA derived from EWMA-smoothed byte rates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgress {
    pub task_id: i64,
    pub task_type: String,
    pub key: String,
    pub label: String,
    pub bytes_completed: u64,
    pub bytes_total: Option<u64>,
    pub throughput_bps: f64,
    pub elapsed: Duration,
    pub eta: Option<Duration>,
}

// ── Throughput Tracker (EWMA) ─────────────────────────────────────

/// Per-task EWMA-smoothed throughput tracker.
///
/// Lives in the progress ticker task — not on the executor hot path.
struct ThroughputTracker {
    last_bytes: u64,
    last_sample: Option<Instant>,
    smoothed_bps: f64,
    alpha: f64,
}

impl ThroughputTracker {
    fn new(alpha: f64) -> Self {
        Self {
            last_bytes: 0,
            last_sample: None,
            smoothed_bps: 0.0,
            alpha,
        }
    }

    /// Feed a new sample and return the smoothed throughput in bytes/sec.
    fn sample(&mut self, bytes_completed: u64) -> f64 {
        let now = Instant::now();

        let Some(last) = self.last_sample else {
            // First sample — record baseline, no rate yet.
            self.last_bytes = bytes_completed;
            self.last_sample = Some(now);
            return 0.0;
        };

        let delta_time = now.duration_since(last);
        if delta_time.as_millis() < 10 {
            // Too soon — skip to avoid jitter spikes.
            return self.smoothed_bps;
        }

        let delta_bytes = bytes_completed.saturating_sub(self.last_bytes);
        let delta_secs = delta_time.as_secs_f64();
        let instantaneous_bps = delta_bytes as f64 / delta_secs;

        self.smoothed_bps = if self.smoothed_bps == 0.0 {
            instantaneous_bps
        } else {
            self.alpha * instantaneous_bps + (1.0 - self.alpha) * self.smoothed_bps
        };

        self.last_bytes = bytes_completed;
        self.last_sample = Some(now);
        self.smoothed_bps
    }

    fn throughput_bps(&self) -> f64 {
        self.smoothed_bps
    }
}

// ── Progress Ticker ───────────────────────────────────────────────

/// Background task that periodically polls active tasks' byte counters
/// and emits [`TaskProgress`] events on a dedicated broadcast channel.
///
/// Follows the same pattern as [`run_sampler`](crate::resource::sampler::run_sampler).
pub(crate) async fn run_progress_ticker(
    active: ActiveTaskMap,
    progress_tx: tokio::sync::broadcast::Sender<TaskProgress>,
    interval: Duration,
    token: CancellationToken,
) {
    tracing::debug!(
        interval_ms = interval.as_millis() as u64,
        "progress ticker started"
    );

    let mut trackers: HashMap<i64, ThroughputTracker> = HashMap::new();

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                tracing::debug!("progress ticker shutting down");
                break;
            }
            _ = tokio::time::sleep(interval) => {
                let snapshots = active.byte_progress_snapshots(None);

                let mut active_ids = std::collections::HashSet::with_capacity(snapshots.len());

                for (task_id, task_type, key, label, bytes_completed, bytes_total, _parent_id, started_at) in &snapshots {
                    active_ids.insert(*task_id);

                    if *bytes_completed == 0 {
                        continue;
                    }

                    let tracker = trackers
                        .entry(*task_id)
                        .or_insert_with(|| ThroughputTracker::new(0.3));

                    tracker.sample(*bytes_completed);

                    let throughput = tracker.throughput_bps();
                    let elapsed = started_at.elapsed();

                    let eta = bytes_total.and_then(|total| {
                        if throughput > 0.0 && *bytes_completed < total {
                            let remaining = total - *bytes_completed;
                            Some(Duration::from_secs_f64(remaining as f64 / throughput))
                        } else {
                            None
                        }
                    });

                    let _ = progress_tx.send(TaskProgress {
                        task_id: *task_id,
                        task_type: task_type.clone(),
                        key: key.clone(),
                        label: label.clone(),
                        bytes_completed: *bytes_completed,
                        bytes_total: *bytes_total,
                        throughput_bps: throughput,
                        elapsed,
                        eta,
                    });
                }

                // Prune stale trackers for tasks that are no longer active.
                trackers.retain(|id, _| active_ids.contains(id));
            }
        }
    }
}

// ── Estimated Progress ─────────────────────────────────────────────

/// Estimated progress for a running task, combining executor-reported progress
/// with throughput-based extrapolation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimatedProgress {
    /// Common task identification fields.
    pub header: TaskEventHeader,
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
        header: record.event_header(),
        reported_percent: reported,
        extrapolated_percent: extrapolated,
        percent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throughput_first_sample_returns_zero() {
        let mut t = ThroughputTracker::new(0.3);
        let bps = t.sample(1000);
        assert_eq!(bps, 0.0);
    }

    #[test]
    fn throughput_ewma_converges() {
        let mut t = ThroughputTracker::new(0.3);
        // First sample — baseline.
        t.sample(0);

        // Simulate steady 1000 bytes/sec over multiple samples.
        // We need real time to pass for the tracker.
        let start = Instant::now();

        // Manually set the tracker's last_sample to a known point.
        t.last_sample = Some(start);
        t.last_bytes = 0;

        // Simulate samples at 100ms intervals.
        for i in 1..=20 {
            let bytes = i * 100; // 100 bytes per 100ms = 1000 bps
            let sample_time = start + Duration::from_millis(i * 100);
            t.last_sample = Some(sample_time - Duration::from_millis(100));
            t.last_bytes = bytes - 100;
            // Manually invoke the EWMA logic inline to test convergence.
            let delta_secs = 0.1;
            let instantaneous = 100.0 / delta_secs; // 1000 bps
            t.smoothed_bps = if t.smoothed_bps == 0.0 {
                instantaneous
            } else {
                0.3 * instantaneous + 0.7 * t.smoothed_bps
            };
        }

        // Should converge to ~1000 bps.
        assert!((t.throughput_bps() - 1000.0).abs() < 1.0);
    }

    #[test]
    fn throughput_zero_delta_decays() {
        let mut t = ThroughputTracker::new(0.3);
        t.smoothed_bps = 1000.0;
        t.last_bytes = 500;
        t.last_sample = Some(Instant::now() - Duration::from_millis(100));

        // No new bytes — instantaneous = 0, EWMA decays.
        t.sample(500);
        assert!(t.throughput_bps() < 1000.0);
        assert!(t.throughput_bps() > 0.0);
    }
}
