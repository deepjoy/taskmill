//! Read-only scheduler queries: active tasks, progress, and snapshots.

use crate::store::StoreError;

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
        let pressure = self.inner.gate.pressure().await;
        let pressure_breakdown = self.inner.gate.pressure_breakdown().await;
        let max_concurrency = self.max_concurrency();

        Ok(SchedulerSnapshot {
            running,
            pending_count,
            paused_count,
            waiting_count,
            progress,
            pressure,
            pressure_breakdown,
            max_concurrency,
            is_paused: self.is_paused(),
        })
    }
}
