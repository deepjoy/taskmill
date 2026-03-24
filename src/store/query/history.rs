//! Queries against the task history table.

use sqlx::Row;

use crate::store::row_mapping::row_to_history_record;
use crate::store::{StoreError, TaskStore};
use crate::task::{TaskHistoryRecord, TypeStats};

impl TaskStore {
    /// Look up a history record by its row id.
    ///
    /// Tags are populated eagerly (single-record lookup is cheap).
    pub async fn history_by_id(&self, id: i64) -> Result<Option<TaskHistoryRecord>, StoreError> {
        let row = sqlx::query("SELECT * FROM task_history WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        let mut record = row.as_ref().map(row_to_history_record);
        if let Some(ref mut r) = record {
            self.populate_history_tags(std::slice::from_mut(r)).await?;
        }
        Ok(record)
    }

    /// Recent history entries, newest first.
    ///
    /// Tags are **not** populated — call [`populate_history_tags`](Self::populate_history_tags)
    /// if needed.
    pub async fn history(
        &self,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<TaskHistoryRecord>, StoreError> {
        let rows =
            sqlx::query("SELECT * FROM task_history ORDER BY completed_at DESC LIMIT ? OFFSET ?")
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.iter().map(row_to_history_record).collect())
    }

    /// History filtered by task type.
    ///
    /// Tags are **not** populated.
    pub async fn history_by_type(
        &self,
        task_type: &str,
        limit: i32,
    ) -> Result<Vec<TaskHistoryRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT * FROM task_history WHERE task_type = ? ORDER BY completed_at DESC LIMIT ?",
        )
        .bind(task_type)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_history_record).collect())
    }

    /// Most recent history record for a specific key.
    ///
    /// Tags are populated eagerly (single-record lookup).
    pub async fn latest_history_by_key(
        &self,
        key: &str,
    ) -> Result<Option<TaskHistoryRecord>, StoreError> {
        let row = sqlx::query(
            "SELECT * FROM task_history WHERE key = ? ORDER BY completed_at DESC LIMIT 1",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;
        let mut record = row.as_ref().map(row_to_history_record);
        if let Some(ref mut r) = record {
            self.populate_history_tags(std::slice::from_mut(r)).await?;
        }
        Ok(record)
    }

    /// History for a specific key (all past runs of that key).
    ///
    /// Tags are **not** populated.
    pub async fn history_by_key(&self, key: &str) -> Result<Vec<TaskHistoryRecord>, StoreError> {
        let rows =
            sqlx::query("SELECT * FROM task_history WHERE key = ? ORDER BY completed_at DESC")
                .bind(key)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.iter().map(row_to_history_record).collect())
    }

    /// Dead-lettered tasks from history filtered by `task_type` prefix.
    ///
    /// Tags are **not** populated.
    pub async fn dead_letter_tasks_by_prefix(
        &self,
        prefix: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<TaskHistoryRecord>, StoreError> {
        let pattern = format!("{prefix}%");
        let rows = sqlx::query(
            "SELECT * FROM task_history WHERE status = 'dead_letter' AND task_type LIKE ? ORDER BY completed_at DESC LIMIT ? OFFSET ?",
        )
        .bind(&pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_history_record).collect())
    }

    /// Dead-lettered tasks from history (retries exhausted).
    ///
    /// Tags are **not** populated.
    pub async fn dead_letter_tasks(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<TaskHistoryRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT * FROM task_history WHERE status = 'dead_letter' ORDER BY completed_at DESC LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_history_record).collect())
    }

    /// Failed tasks from history.
    ///
    /// Tags are **not** populated.
    pub async fn failed_tasks(&self, limit: i32) -> Result<Vec<TaskHistoryRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT * FROM task_history WHERE status = 'failed' ORDER BY completed_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_history_record).collect())
    }

    /// Aggregate stats for a task type from completed history.
    pub async fn history_stats(&self, task_type: &str) -> Result<TypeStats, StoreError> {
        let row = sqlx::query(
            "SELECT
                COUNT(*) as total,
                COALESCE(AVG(CASE WHEN status = 'completed' THEN duration_ms END), 0.0) as avg_dur,
                COALESCE(AVG(CASE WHEN status = 'completed' THEN actual_read_bytes END), 0.0) as avg_read,
                COALESCE(AVG(CASE WHEN status = 'completed' THEN actual_write_bytes END), 0.0) as avg_write,
                CAST(SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) AS REAL) / MAX(COUNT(*), 1) as fail_rate
             FROM task_history WHERE task_type = ?",
        )
        .bind(task_type)
        .fetch_one(&self.pool)
        .await?;

        Ok(TypeStats {
            count: row.get::<i64, _>("total"),
            avg_duration_ms: row.get::<f64, _>("avg_dur"),
            avg_read_bytes: row.get::<f64, _>("avg_read"),
            avg_write_bytes: row.get::<f64, _>("avg_write"),
            failure_rate: row.get::<f64, _>("fail_rate"),
        })
    }

    /// Average IO throughput (bytes/sec) for recently completed tasks of a type.
    /// Used by the scheduler for IO budget estimation.
    pub async fn avg_throughput(
        &self,
        task_type: &str,
        recent_limit: i32,
    ) -> Result<(f64, f64), StoreError> {
        let row: (f64, f64) = sqlx::query_as(
            "SELECT
                COALESCE(AVG(CASE WHEN duration_ms > 0 THEN actual_read_bytes * 1000.0 / duration_ms END), 0),
                COALESCE(AVG(CASE WHEN duration_ms > 0 THEN actual_write_bytes * 1000.0 / duration_ms END), 0)
             FROM (
                 SELECT actual_read_bytes, actual_write_bytes, duration_ms
                 FROM task_history
                 WHERE task_type = ? AND status = 'completed' AND duration_ms > 0
                 ORDER BY completed_at DESC
                 LIMIT ?
             )",
        )
        .bind(task_type)
        .bind(recent_limit)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }
}
