//! Queries against the active task queue.

use crate::store::row_mapping::row_to_task_record;
use crate::store::{StoreError, TaskStore};
use crate::task::{TaskLookup, TaskRecord};

use super::super::row_mapping::row_to_history_record;

impl TaskStore {
    /// All currently running tasks.
    pub async fn running_tasks(&self) -> Result<Vec<TaskRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT * FROM tasks WHERE status = 'running' ORDER BY priority ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// Count of running tasks.
    pub async fn running_count(&self) -> Result<i64, StoreError> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE status = 'running'")
            .fetch_one(&self.pool)
            .await?;
        Ok(count.0)
    }

    /// Pending tasks, ordered by priority then age. Limit controls page size.
    pub async fn pending_tasks(&self, limit: i32) -> Result<Vec<TaskRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT * FROM tasks WHERE status = 'pending' ORDER BY priority ASC, id ASC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// Count of pending tasks.
    pub async fn pending_count(&self) -> Result<i64, StoreError> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE status = 'pending'")
            .fetch_one(&self.pool)
            .await?;
        Ok(count.0)
    }

    /// Pending tasks filtered by type.
    pub async fn pending_by_type(&self, task_type: &str) -> Result<Vec<TaskRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT * FROM tasks WHERE status = 'pending' AND task_type = ? ORDER BY priority ASC, id ASC",
        )
        .bind(task_type)
        .fetch_all(&self.pool)
        .await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// Count of paused tasks.
    pub async fn paused_count(&self) -> Result<i64, StoreError> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE status = 'paused'")
            .fetch_one(&self.pool)
            .await?;
        Ok(count.0)
    }

    /// Paused tasks.
    pub async fn paused_tasks(&self) -> Result<Vec<TaskRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT * FROM tasks WHERE status = 'paused' ORDER BY priority ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// Look up an active task by its row id. Returns `None` if no active
    /// task with that id exists.
    pub async fn task_by_id(&self, id: i64) -> Result<Option<TaskRecord>, StoreError> {
        let row = sqlx::query("SELECT * FROM tasks WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        let mut record = row.as_ref().map(row_to_task_record);
        if let Some(ref mut r) = record {
            self.populate_tags(std::slice::from_mut(r)).await?;
        }
        Ok(record)
    }

    /// Look up an active task by its dedup key. Returns `None` if no active
    /// task with that key exists.
    pub async fn task_by_key(&self, key: &str) -> Result<Option<TaskRecord>, StoreError> {
        let row = sqlx::query("SELECT * FROM tasks WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        let mut record = row.as_ref().map(row_to_task_record);
        if let Some(ref mut r) = record {
            self.populate_tags(std::slice::from_mut(r)).await?;
        }
        Ok(record)
    }

    /// Sum of expected read/write bytes for all running tasks.
    pub async fn running_io_totals(&self) -> Result<(i64, i64), StoreError> {
        let row: (i64, i64) = sqlx::query_as(
            "SELECT COALESCE(SUM(expected_read_bytes), 0), COALESCE(SUM(expected_write_bytes), 0)
             FROM tasks WHERE status = 'running'",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Sum of expected network rx/tx bytes for all running tasks.
    pub async fn running_net_io_totals(&self) -> Result<(i64, i64), StoreError> {
        let row: (i64, i64) = sqlx::query_as(
            "SELECT COALESCE(SUM(expected_net_rx_bytes), 0), COALESCE(SUM(expected_net_tx_bytes), 0)
             FROM tasks WHERE status = 'running'",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Count of running tasks in a specific group.
    pub async fn running_count_for_group(&self, group_key: &str) -> Result<i64, StoreError> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE group_key = ? AND status = 'running'")
                .bind(group_key)
                .fetch_one(&self.pool)
                .await?;
        Ok(count.0)
    }

    /// Count of waiting tasks (parents waiting for children).
    pub async fn waiting_count(&self) -> Result<i64, StoreError> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE status = 'waiting'")
            .fetch_one(&self.pool)
            .await?;
        Ok(count.0)
    }

    /// Waiting tasks (parents waiting for children).
    pub async fn waiting_tasks(&self) -> Result<Vec<TaskRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT * FROM tasks WHERE status = 'waiting' ORDER BY priority ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// All active tasks in a specific group.
    pub async fn tasks_by_group(&self, group_key: &str) -> Result<Vec<TaskRecord>, StoreError> {
        let rows = sqlx::query("SELECT * FROM tasks WHERE group_key = ? ORDER BY id ASC")
            .bind(group_key)
            .fetch_all(&self.pool)
            .await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// All active tasks of a specific type.
    pub async fn tasks_by_type(&self, task_type: &str) -> Result<Vec<TaskRecord>, StoreError> {
        let rows = sqlx::query("SELECT * FROM tasks WHERE task_type = ? ORDER BY id ASC")
            .bind(task_type)
            .fetch_all(&self.pool)
            .await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// All active tasks (any status).
    pub async fn all_active_tasks(&self) -> Result<Vec<TaskRecord>, StoreError> {
        let rows = sqlx::query("SELECT * FROM tasks ORDER BY id ASC")
            .fetch_all(&self.pool)
            .await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// All active tasks whose `task_type` starts with `prefix` (module-scoped query).
    ///
    /// Uses `task_type LIKE '{prefix}%'` against the indexed `task_type` column.
    pub async fn tasks_by_type_prefix(&self, prefix: &str) -> Result<Vec<TaskRecord>, StoreError> {
        let pattern = format!("{prefix}%");
        let rows =
            sqlx::query("SELECT * FROM tasks WHERE task_type LIKE ? ORDER BY priority ASC, id ASC")
                .bind(&pattern)
                .fetch_all(&self.pool)
                .await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// Count of pending tasks whose `task_type` starts with `prefix`.
    pub async fn pending_count_by_prefix(&self, prefix: &str) -> Result<i64, StoreError> {
        let pattern = format!("{prefix}%");
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM tasks WHERE task_type LIKE ? AND status = 'pending'",
        )
        .bind(&pattern)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0)
    }

    /// Count of paused tasks whose `task_type` starts with `prefix`.
    pub async fn paused_count_by_prefix(&self, prefix: &str) -> Result<i64, StoreError> {
        let pattern = format!("{prefix}%");
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM tasks WHERE task_type LIKE ? AND status = 'paused'",
        )
        .bind(&pattern)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0)
    }

    /// Return the dependency edges for a given task (what it depends on).
    pub async fn task_dependencies(&self, task_id: i64) -> Result<Vec<i64>, StoreError> {
        let rows: Vec<(i64,)> =
            sqlx::query_as("SELECT depends_on_id FROM task_deps WHERE task_id = ?")
                .bind(task_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Return tasks that are blocked waiting on a given task.
    pub async fn task_dependents(&self, task_id: i64) -> Result<Vec<i64>, StoreError> {
        let rows: Vec<(i64,)> =
            sqlx::query_as("SELECT task_id FROM task_deps WHERE depends_on_id = ?")
                .bind(task_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Return blocked tasks (for snapshot/debugging).
    pub async fn blocked_tasks(&self) -> Result<Vec<TaskRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT * FROM tasks WHERE status = 'blocked' ORDER BY priority ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// Count of blocked tasks.
    pub async fn blocked_count(&self) -> Result<i64, StoreError> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE status = 'blocked'")
            .fetch_one(&self.pool)
            .await?;
        Ok(count.0)
    }

    /// Look up a task by its dedup key, checking the active queue first
    /// and falling back to history.
    ///
    /// This is the low-level building block for [`Scheduler::task_lookup`](crate::Scheduler::task_lookup).
    /// The `key` parameter is the pre-computed SHA-256 dedup key (as
    /// returned by [`generate_dedup_key`](crate::task::generate_dedup_key)
    /// or `TaskSubmission::effective_key`).
    pub async fn task_lookup(&self, key: &str) -> Result<TaskLookup, StoreError> {
        // Check active queue first (pending / running / paused).
        // task_by_key already populates tags.
        if let Some(record) = self.task_by_key(key).await? {
            return Ok(TaskLookup::Active(record));
        }

        // Fall back to the most recent history entry.
        let row = sqlx::query(
            "SELECT * FROM task_history WHERE key = ? ORDER BY completed_at DESC LIMIT 1",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let mut hist = row_to_history_record(&r);
                self.populate_history_tags(std::slice::from_mut(&mut hist))
                    .await?;
                Ok(TaskLookup::History(hist))
            }
            None => Ok(TaskLookup::NotFound),
        }
    }
}
