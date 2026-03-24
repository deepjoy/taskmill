//! Read-only scheduler queries: active tasks, progress, and snapshots.

use crate::store::StoreError;

use super::event::PausedGroupInfo;
use super::progress::TaskProgress;
use super::{EstimatedProgress, Scheduler, SchedulerSnapshot};

impl Scheduler {
    /// Snapshot of currently active (in-memory) tasks.
    pub async fn active_tasks(&self) -> Vec<crate::task::TaskRecord> {
        self.inner.active.records(None)
    }

    /// Get estimated progress for all running tasks.
    ///
    /// Combines executor-reported progress with throughput-based extrapolation
    /// using historical average duration for each task type.
    pub async fn estimated_progress(&self) -> Vec<EstimatedProgress> {
        let snapshots: Vec<_> = self.inner.active.progress_snapshots(None);
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
        let snapshots = self.inner.active.byte_progress_snapshots(None);
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

    /// Return the IDs of active tasks matching all specified tag filters (AND semantics).
    ///
    /// Delegates to [`TaskStore::task_ids_by_tags`](crate::TaskStore::task_ids_by_tags).
    pub async fn task_ids_by_tags(
        &self,
        filters: &[(&str, &str)],
        status: Option<crate::task::TaskStatus>,
    ) -> Result<Vec<i64>, StoreError> {
        self.inner.store.task_ids_by_tags(filters, status).await
    }

    /// Count active tasks grouped by a tag key's values.
    ///
    /// Delegates to [`TaskStore::count_by_tag`](crate::TaskStore::count_by_tag).
    pub async fn count_by_tag(
        &self,
        key: &str,
        status: Option<crate::task::TaskStatus>,
    ) -> Result<Vec<(String, i64)>, StoreError> {
        self.inner.store.count_by_tag(key, status).await
    }

    /// List distinct values for a tag key across active tasks, with counts.
    ///
    /// Delegates to [`TaskStore::tag_values`](crate::TaskStore::tag_values).
    pub async fn tag_values(&self, key: &str) -> Result<Vec<(String, i64)>, StoreError> {
        self.inner.store.tag_values(key).await
    }

    /// Discover all tag keys matching a prefix across active tasks.
    pub async fn tag_keys_by_prefix(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        self.inner.store.tag_keys_by_prefix(prefix).await
    }

    /// Return IDs of active tasks with any tag key matching the given prefix.
    pub async fn task_ids_by_tag_key_prefix(
        &self,
        prefix: &str,
        status: Option<crate::task::TaskStatus>,
    ) -> Result<Vec<i64>, StoreError> {
        self.inner
            .store
            .task_ids_by_tag_key_prefix(prefix, status)
            .await
    }

    /// Count active tasks with any tag key matching the given prefix.
    pub async fn count_by_tag_key_prefix(
        &self,
        prefix: &str,
        status: Option<crate::task::TaskStatus>,
    ) -> Result<i64, StoreError> {
        self.inner
            .store
            .count_by_tag_key_prefix(prefix, status)
            .await
    }

    /// Dead-lettered tasks (retries exhausted), newest first.
    ///
    /// These are tasks that failed with a retryable error but exhausted their
    /// retry limit. Use [`retry_dead_letter`](Self::retry_dead_letter) to
    /// re-submit them.
    pub async fn dead_letter_tasks(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<crate::task::TaskHistoryRecord>, StoreError> {
        self.inner.store.dead_letter_tasks(limit, offset).await
    }

    /// Capture a single status snapshot for dashboard UIs.
    ///
    /// Gathers running tasks, queue depths, progress estimates, and
    /// backpressure in one call — exactly what a Tauri command would
    /// return to the frontend.
    pub async fn snapshot(&self) -> Result<SchedulerSnapshot, StoreError> {
        let running = self.inner.active.records(None);
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

        // Paused groups with per-group task counts.
        let paused_groups_rows = self.inner.store.paused_group_info().await?;
        let paused_groups = paused_groups_rows
            .into_iter()
            .map(
                |(group, paused_at_ms, resume_at_ms, paused_task_count)| PausedGroupInfo {
                    group,
                    paused_at: chrono::DateTime::from_timestamp_millis(paused_at_ms)
                        .unwrap_or_default(),
                    paused_task_count,
                    resume_at: resume_at_ms.and_then(chrono::DateTime::from_timestamp_millis),
                },
            )
            .collect();

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
            paused_groups,
        })
    }
}
