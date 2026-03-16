//! Pop / peek / requeue operations for task dispatch.

use crate::store::row_mapping::row_to_task_record;
use crate::store::{StoreError, TaskStore};
use crate::task::TaskRecord;

impl TaskStore {
    /// Peek at the highest-priority pending task without modifying it.
    /// Returns `None` if the queue is empty. Tasks with a future `run_after`
    /// timestamp are excluded (not yet eligible for dispatch).
    pub async fn peek_next(&self) -> Result<Option<TaskRecord>, StoreError> {
        let row = sqlx::query(
            "SELECT * FROM tasks
             WHERE id = (
                 SELECT id FROM tasks
                 WHERE status = 'pending'
                   AND (run_after IS NULL OR run_after <= strftime('%Y-%m-%d %H:%M:%f', 'now'))
                 ORDER BY priority ASC, id ASC
                 LIMIT 1
             )",
        )
        .fetch_optional(&self.pool)
        .await?;

        let mut record = row.as_ref().map(row_to_task_record);
        if let Some(ref mut r) = record {
            self.populate_tags(std::slice::from_mut(r)).await?;
        }
        Ok(record)
    }

    /// Atomically claim a specific pending task by id, setting it to running.
    /// Returns `None` if the task is no longer pending (e.g. claimed by another
    /// dispatcher or cancelled).
    ///
    /// For tasks with `ttl_from = 'first_attempt'`, sets `expires_at` on the
    /// first pop (when `expires_at IS NULL` and `ttl_seconds IS NOT NULL`).
    pub async fn pop_by_id(&self, id: i64) -> Result<Option<TaskRecord>, StoreError> {
        tracing::debug!(task_id = id, "store.pop_by_id: UPDATE start");
        let row = sqlx::query(
            "UPDATE tasks SET
                status = 'running',
                started_at = datetime('now'),
                expires_at = CASE
                    WHEN ttl_from = 'first_attempt' AND ttl_seconds IS NOT NULL AND expires_at IS NULL
                    THEN datetime('now', '+' || ttl_seconds || ' seconds')
                    ELSE expires_at
                END
             WHERE id = ? AND status = 'pending'
             RETURNING *",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        tracing::debug!(task_id = id, "store.pop_by_id: UPDATE end");

        let mut record = row.as_ref().map(row_to_task_record);
        if let Some(ref mut r) = record {
            self.populate_tags(std::slice::from_mut(r)).await?;
        }
        Ok(record)
    }

    /// Pop the highest-priority pending task and mark it as running.
    /// Returns `None` if the queue is empty. Tasks with a future `run_after`
    /// timestamp are excluded.
    ///
    /// For tasks with `ttl_from = 'first_attempt'`, sets `expires_at` on
    /// the first pop.
    pub async fn pop_next(&self) -> Result<Option<TaskRecord>, StoreError> {
        let row = sqlx::query(
            "UPDATE tasks SET
                status = 'running',
                started_at = datetime('now'),
                expires_at = CASE
                    WHEN ttl_from = 'first_attempt' AND ttl_seconds IS NOT NULL AND expires_at IS NULL
                    THEN datetime('now', '+' || ttl_seconds || ' seconds')
                    ELSE expires_at
                END
             WHERE id = (
                 SELECT id FROM tasks
                 WHERE status = 'pending'
                   AND (run_after IS NULL OR run_after <= strftime('%Y-%m-%d %H:%M:%f', 'now'))
                 ORDER BY priority ASC, id ASC
                 LIMIT 1
             )
             RETURNING *",
        )
        .fetch_optional(&self.pool)
        .await?;

        let mut record = row.map(|r| row_to_task_record(&r));
        if let Some(ref mut r) = record {
            self.populate_tags(std::slice::from_mut(r)).await?;
        }
        Ok(record)
    }

    /// Atomically requeue a running task back to pending.
    ///
    /// Used when a task is popped but then rejected by backpressure or IO
    /// budget checks. Unlike pause+resume, this is a single atomic operation
    /// that never puts the task in an intermediate state visible to queries.
    pub async fn requeue(&self, id: i64) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE tasks SET status = 'pending', started_at = NULL WHERE id = ? AND status = 'running'",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
