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

    /// Find active tasks matching all specified tag filters (AND semantics).
    ///
    /// Delegates to [`TaskStore::tasks_by_tags`].
    pub async fn tasks_by_tags(
        &self,
        filters: &[(&str, &str)],
        status: Option<crate::task::TaskStatus>,
    ) -> Result<Vec<crate::task::TaskRecord>, StoreError> {
        self.inner.store.tasks_by_tags(filters, status).await
    }

    /// Count active tasks grouped by a tag key's values.
    ///
    /// Delegates to [`TaskStore::count_by_tag`].
    pub async fn count_by_tag(
        &self,
        key: &str,
        status: Option<crate::task::TaskStatus>,
    ) -> Result<Vec<(String, i64)>, StoreError> {
        self.inner.store.count_by_tag(key, status).await
    }

    /// List distinct values for a tag key across active tasks, with counts.
    ///
    /// Delegates to [`TaskStore::tag_values`].
    pub async fn tag_values(&self, key: &str) -> Result<Vec<(String, i64)>, StoreError> {
        self.inner.store.tag_values(key).await
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
        let recurring_schedules = self.inner.store.recurring_schedules().await?;
        let blocked_count = self.inner.store.blocked_count().await?;

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
            recurring_schedules,
            blocked_count,
        })
    }
}
