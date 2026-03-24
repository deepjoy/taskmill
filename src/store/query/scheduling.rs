//! Scheduling queries and recurring task control.

use crate::scheduler::aging::AgingParams;
use crate::store::row_mapping::row_to_task_record;
use crate::store::{StoreError, TaskStore};
use crate::task::TaskRecord;

impl TaskStore {
    /// Returns the earliest `run_after` timestamp among pending tasks, if any.
    pub async fn next_run_after(
        &self,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, StoreError> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT run_after FROM tasks
             WHERE status = 'pending' AND run_after IS NOT NULL
             ORDER BY run_after ASC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(ms,)| crate::store::row_mapping::from_epoch_ms(ms)))
    }

    /// List active recurring schedules with their next run times.
    pub async fn recurring_schedules(
        &self,
    ) -> Result<Vec<crate::task::RecurringScheduleInfo>, StoreError> {
        let rows = sqlx::query(
            "SELECT * FROM tasks
             WHERE recurring_interval_secs IS NOT NULL
             ORDER BY id ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let task = row_to_task_record(row);
                crate::task::RecurringScheduleInfo {
                    task_id: task.id,
                    task_type: task.task_type,
                    label: task.label,
                    interval_secs: task.recurring_interval_secs.unwrap_or(0),
                    next_run: task.run_after,
                    execution_count: task.recurring_execution_count,
                    max_executions: task.recurring_max_executions,
                    paused: task.recurring_paused,
                }
            })
            .collect())
    }

    /// Set the `run_after` timestamp for a pending task.
    ///
    /// Used by the rate limiter to defer a task until its next token is
    /// available, preventing head-of-line blocking.
    pub async fn set_run_after(
        &self,
        task_id: i64,
        run_after: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE tasks SET run_after = ?
             WHERE id = ? AND status = 'pending'",
        )
        .bind(run_after.timestamp_millis())
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Per-group peek queries (fair scheduling) ──────────────────

    /// Peek the highest effective-priority pending task in a specific group.
    pub async fn peek_next_in_group(
        &self,
        group_key: &str,
        aging: Option<&AgingParams>,
    ) -> Result<Option<TaskRecord>, StoreError> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let row = match aging {
            None => {
                sqlx::query(
                    "SELECT * FROM tasks
                     WHERE id = (
                         SELECT id FROM tasks
                         WHERE status = 'pending'
                           AND group_key = ?
                           AND (run_after IS NULL OR run_after <= ?)
                         ORDER BY priority ASC, id ASC
                         LIMIT 1
                     )",
                )
                .bind(group_key)
                .bind(now_ms)
                .fetch_optional(&self.pool)
                .await?
            }
            Some(ap) => {
                sqlx::query(
                    "SELECT * FROM tasks
                     WHERE id = (
                         SELECT id FROM tasks
                         WHERE status = 'pending'
                           AND group_key = ?
                           AND (run_after IS NULL OR run_after <= ?)
                         ORDER BY
                             MAX(
                                 priority - MAX(0, (? - created_at - pause_duration_ms - ?) / ?),
                                 ?
                             ) ASC,
                             id ASC
                         LIMIT 1
                     )",
                )
                .bind(group_key)
                .bind(now_ms)
                .bind(ap.now_ms)
                .bind(ap.grace_period_ms)
                .bind(ap.aging_interval_ms)
                .bind(ap.max_effective_priority)
                .fetch_optional(&self.pool)
                .await?
            }
        };

        Ok(row.as_ref().map(row_to_task_record))
    }

    /// Peek the highest effective-priority pending task with no group.
    pub async fn peek_next_ungrouped(
        &self,
        aging: Option<&AgingParams>,
    ) -> Result<Option<TaskRecord>, StoreError> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let row = match aging {
            None => {
                sqlx::query(
                    "SELECT * FROM tasks
                     WHERE id = (
                         SELECT id FROM tasks
                         WHERE status = 'pending'
                           AND group_key IS NULL
                           AND (run_after IS NULL OR run_after <= ?)
                         ORDER BY priority ASC, id ASC
                         LIMIT 1
                     )",
                )
                .bind(now_ms)
                .fetch_optional(&self.pool)
                .await?
            }
            Some(ap) => {
                sqlx::query(
                    "SELECT * FROM tasks
                     WHERE id = (
                         SELECT id FROM tasks
                         WHERE status = 'pending'
                           AND group_key IS NULL
                           AND (run_after IS NULL OR run_after <= ?)
                         ORDER BY
                             MAX(
                                 priority - MAX(0, (? - created_at - pause_duration_ms - ?) / ?),
                                 ?
                             ) ASC,
                             id ASC
                         LIMIT 1
                     )",
                )
                .bind(now_ms)
                .bind(ap.now_ms)
                .bind(ap.grace_period_ms)
                .bind(ap.aging_interval_ms)
                .bind(ap.max_effective_priority)
                .fetch_optional(&self.pool)
                .await?
            }
        };

        Ok(row.as_ref().map(row_to_task_record))
    }

    /// Running task counts per group (including ungrouped as None).
    pub async fn running_counts_per_group(
        &self,
    ) -> Result<Vec<(Option<String>, usize)>, StoreError> {
        let rows: Vec<(Option<String>, i64)> = sqlx::query_as(
            "SELECT group_key, COUNT(*) FROM tasks
             WHERE status = 'running'
             GROUP BY group_key",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|(g, c)| (g, c as usize)).collect())
    }

    /// Pending task counts per group (including ungrouped as None).
    /// Only counts tasks eligible for dispatch (not deferred by run_after).
    pub async fn pending_counts_per_group(
        &self,
    ) -> Result<Vec<(Option<String>, usize)>, StoreError> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let rows: Vec<(Option<String>, i64)> = sqlx::query_as(
            "SELECT group_key, COUNT(*) FROM tasks
             WHERE status = 'pending'
               AND (run_after IS NULL OR run_after <= ?)
             GROUP BY group_key",
        )
        .bind(now_ms)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|(g, c)| (g, c as usize)).collect())
    }

    /// Peek the next pending task whose effective priority (with aging)
    /// is at or above the urgent threshold.
    pub async fn peek_next_urgent(
        &self,
        threshold: crate::priority::Priority,
        aging: Option<&AgingParams>,
    ) -> Result<Option<TaskRecord>, StoreError> {
        // Urgent is meaningless without aging — if aging is None, no tasks
        // can have an effective priority different from their stored one.
        let Some(ap) = aging else {
            return Ok(None);
        };
        let now_ms = chrono::Utc::now().timestamp_millis();
        let row = sqlx::query(
            "SELECT * FROM tasks
             WHERE id = (
                 SELECT id FROM tasks
                 WHERE status = 'pending'
                   AND (run_after IS NULL OR run_after <= ?)
                   AND MAX(
                         priority - MAX(0, (? - created_at - pause_duration_ms - ?) / ?),
                         ?
                       ) <= ?
                 ORDER BY
                   MAX(priority - MAX(0, (? - created_at - pause_duration_ms - ?) / ?), ?) ASC,
                   id ASC
                 LIMIT 1
             )",
        )
        .bind(now_ms)
        // WHERE clause aging params
        .bind(ap.now_ms)
        .bind(ap.grace_period_ms)
        .bind(ap.aging_interval_ms)
        .bind(ap.max_effective_priority)
        .bind(threshold.value() as i64)
        // ORDER BY aging params
        .bind(ap.now_ms)
        .bind(ap.grace_period_ms)
        .bind(ap.aging_interval_ms)
        .bind(ap.max_effective_priority)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(row_to_task_record))
    }

    // ── Recurring control ──────────────────────────────────────────

    /// Pause a recurring schedule. The current instance (if running) is
    /// not affected, but no new instances will be created on completion.
    pub async fn pause_recurring(&self, task_id: i64) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE tasks SET recurring_paused = 1
             WHERE id = ? AND recurring_interval_secs IS NOT NULL",
        )
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Resume a paused recurring schedule.
    pub async fn resume_recurring(&self, task_id: i64) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE tasks SET recurring_paused = 0
             WHERE id = ? AND recurring_interval_secs IS NOT NULL",
        )
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Cancel a recurring schedule entirely. Cancels any pending instance
    /// and prevents future ones.
    pub async fn cancel_recurring(&self, task_id: i64) -> Result<bool, StoreError> {
        // First mark as paused to prevent new instances, then cancel.
        sqlx::query(
            "UPDATE tasks SET recurring_paused = 1
             WHERE id = ? AND recurring_interval_secs IS NOT NULL",
        )
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        self.cancel_to_history(task_id).await
    }
}
