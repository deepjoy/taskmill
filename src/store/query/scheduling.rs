//! Scheduling queries and recurring task control.

use crate::store::row_mapping::row_to_task_record;
use crate::store::{StoreError, TaskStore};

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
