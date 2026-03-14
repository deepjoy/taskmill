//! Read-only scheduler queries: active tasks, progress, and snapshots.

use crate::store::StoreError;

use super::progress::TaskProgress;
use super::{EstimatedProgress, Scheduler, SchedulerSnapshot};

impl Scheduler {
    /// Snapshot of currently active (in-memory) tasks.
    pub async fn active_tasks(&self) -> Vec<crate::task::TaskRecord> {
        self.inner.active.records()
    }

    /// Get estimated progress for all running tasks.
    ///
    /// Combines executor-reported progress with throughput-based extrapolation
    /// using historical average duration for each task type.
    pub async fn estimated_progress(&self) -> Vec<EstimatedProgress> {
        let snapshots: Vec<_> = self.inner.active.progress_snapshots();
        let mut results = Vec::with_capacity(snapshots.len());
        for (record, reported, reported_at) in snapshots {
            results.push(
                super::progress::extrapolate(&record, reported, reported_at, &self.inner.store)
                    .await,
            );
        }
        results
    }

    /// Byte-level progress for all active tasks reporting bytes.
    ///
    /// Returns instantaneous values (throughput = 0) — for smoothed throughput
    /// and ETA, use [`subscribe_progress`](Self::subscribe_progress).
    pub fn byte_progress(&self) -> Vec<TaskProgress> {
        let snapshots = self.inner.active.byte_progress_snapshots();
        snapshots
            .into_iter()
            .filter(|(_, _, _, _, completed, _, _, _)| *completed > 0)
            .map(
                |(
                    task_id,
                    task_type,
                    key,
                    label,
                    bytes_completed,
                    bytes_total,
                    _parent_id,
                    started_at,
                )| {
                    TaskProgress {
                        task_id,
                        task_type,
                        key,
                        label,
                        bytes_completed,
                        bytes_total,
                        throughput_bps: 0.0,
                        elapsed: started_at.elapsed(),
                        eta: None,
                    }
                },
            )
            .collect()
    }

    /// Capture a single status snapshot for dashboard UIs.
    ///
    /// Gathers running tasks, queue depths, progress estimates, and
    /// backpressure in one call — exactly what a Tauri command would
    /// return to the frontend.
    pub async fn snapshot(&self) -> Result<SchedulerSnapshot, StoreError> {
        let running = self.inner.active.records();
        let pending_count = self.inner.store.pending_count().await?;
        let paused_count = self.inner.store.paused_count().await?;
        let waiting_count = self.inner.store.waiting_count().await?;
        let progress = self.estimated_progress().await;
        let byte_progress = self.byte_progress();
        let pressure = self.inner.gate.pressure().await;
        let pressure_breakdown = self.inner.gate.pressure_breakdown().await;
        let max_concurrency = self.max_concurrency();

        Ok(SchedulerSnapshot {
            running,
            pending_count,
            paused_count,
            waiting_count,
            progress,
            byte_progress,
            pressure,
            pressure_breakdown,
            max_concurrency,
            is_paused: self.is_paused(),
        })
    }
}
