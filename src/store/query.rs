//! Read-only query methods for the active queue and history tables.

use std::collections::HashMap;

use sqlx::Row;

use crate::task::{TaskHistoryRecord, TaskLookup, TaskRecord, TypeStats};

use super::row_mapping::{row_to_history_record, row_to_task_record};
use super::{StoreError, TaskStore};

impl TaskStore {
    // ── Tag population ─────────────────────────────────────────────

    /// Populate tags for a slice of task records from the task_tags table.
    pub(crate) async fn populate_tags(&self, records: &mut [TaskRecord]) -> Result<(), StoreError> {
        if records.is_empty() {
            return Ok(());
        }
        let ids: Vec<i64> = records.iter().map(|r| r.id).collect();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query =
            format!("SELECT task_id, key, value FROM task_tags WHERE task_id IN ({placeholders})");
        let mut q = sqlx::query_as::<_, (i64, String, String)>(&query);
        for id in &ids {
            q = q.bind(id);
        }
        let tag_rows = q.fetch_all(&self.pool).await?;

        let mut tag_map: HashMap<i64, HashMap<String, String>> = HashMap::new();
        for (task_id, key, value) in tag_rows {
            tag_map.entry(task_id).or_default().insert(key, value);
        }
        for record in records {
            if let Some(tags) = tag_map.remove(&record.id) {
                record.tags = tags;
            }
        }
        Ok(())
    }

    /// Populate tags for a slice of history records from the task_history_tags table.
    pub(crate) async fn populate_history_tags(
        &self,
        records: &mut [TaskHistoryRecord],
    ) -> Result<(), StoreError> {
        if records.is_empty() {
            return Ok(());
        }
        let ids: Vec<i64> = records.iter().map(|r| r.id).collect();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT history_rowid, key, value FROM task_history_tags WHERE history_rowid IN ({placeholders})"
        );
        let mut q = sqlx::query_as::<_, (i64, String, String)>(&query);
        for id in &ids {
            q = q.bind(id);
        }
        let tag_rows = q.fetch_all(&self.pool).await?;

        let mut tag_map: HashMap<i64, HashMap<String, String>> = HashMap::new();
        for (history_rowid, key, value) in tag_rows {
            tag_map.entry(history_rowid).or_default().insert(key, value);
        }
        for record in records {
            if let Some(tags) = tag_map.remove(&record.id) {
                record.tags = tags;
            }
        }
        Ok(())
    }

    // ── Query: active queue ─────────────────────────────────────────

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

    // ── Query: history ──────────────────────────────────────────────

    /// Look up a history record by its row id.
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
        let mut records: Vec<TaskHistoryRecord> = rows.iter().map(row_to_history_record).collect();
        self.populate_history_tags(&mut records).await?;
        Ok(records)
    }

    /// History filtered by task type.
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
        let mut records: Vec<TaskHistoryRecord> = rows.iter().map(row_to_history_record).collect();
        self.populate_history_tags(&mut records).await?;
        Ok(records)
    }

    /// History for a specific key (all past runs of that key).
    pub async fn history_by_key(&self, key: &str) -> Result<Vec<TaskHistoryRecord>, StoreError> {
        let rows =
            sqlx::query("SELECT * FROM task_history WHERE key = ? ORDER BY completed_at DESC")
                .bind(key)
                .fetch_all(&self.pool)
                .await?;
        let mut records: Vec<TaskHistoryRecord> = rows.iter().map(row_to_history_record).collect();
        self.populate_history_tags(&mut records).await?;
        Ok(records)
    }

    /// Failed tasks from history.
    pub async fn failed_tasks(&self, limit: i32) -> Result<Vec<TaskHistoryRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT * FROM task_history WHERE status = 'failed' ORDER BY completed_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        let mut records: Vec<TaskHistoryRecord> = rows.iter().map(row_to_history_record).collect();
        self.populate_history_tags(&mut records).await?;
        Ok(records)
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

    // ── Unified lookup ──────────────────────────────────────────────

    /// Look up a task by its dedup key, checking the active queue first
    /// and falling back to history.
    ///
    /// This is the low-level building block for [`Scheduler::task_lookup`](crate::Scheduler::task_lookup).
    /// The `key` parameter is the pre-computed SHA-256 dedup key (as
    /// returned by [`generate_dedup_key`](crate::task::generate_dedup_key)
    /// or [`TaskSubmission::effective_key`]).
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

    // ── Waiting ─────────────────────────────────────────────────────

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

    // ── Tag-based queries ───────────────────────────────────────────

    /// Find active tasks matching all specified tag filters (AND semantics).
    ///
    /// Each `(key, value)` pair adds an INNER JOIN, so only tasks matching
    /// **all** filters are returned. Optionally filter by status.
    pub async fn tasks_by_tags(
        &self,
        filters: &[(&str, &str)],
        status: Option<crate::task::TaskStatus>,
    ) -> Result<Vec<TaskRecord>, StoreError> {
        if filters.is_empty() {
            return Ok(Vec::new());
        }

        let mut sql = String::from("SELECT t.* FROM tasks t");
        for (i, _) in filters.iter().enumerate() {
            sql.push_str(&format!(
                " INNER JOIN task_tags tt{i} ON t.id = tt{i}.task_id AND tt{i}.key = ? AND tt{i}.value = ?"
            ));
        }
        if let Some(ref s) = status {
            sql.push_str(&format!(" WHERE t.status = '{}'", s.as_str()));
        }
        sql.push_str(" ORDER BY t.priority ASC, t.id ASC");

        let mut q = sqlx::query(&sql);
        for (key, value) in filters {
            q = q.bind(key).bind(value);
        }
        let rows = q.fetch_all(&self.pool).await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// Count active tasks matching all specified tag filters (AND semantics).
    pub async fn count_by_tags(
        &self,
        filters: &[(&str, &str)],
        status: Option<crate::task::TaskStatus>,
    ) -> Result<i64, StoreError> {
        if filters.is_empty() {
            return Ok(0);
        }

        let mut sql = String::from("SELECT COUNT(*) FROM tasks t");
        for (i, _) in filters.iter().enumerate() {
            sql.push_str(&format!(
                " INNER JOIN task_tags tt{i} ON t.id = tt{i}.task_id AND tt{i}.key = ? AND tt{i}.value = ?"
            ));
        }
        if let Some(ref s) = status {
            sql.push_str(&format!(" WHERE t.status = '{}'", s.as_str()));
        }

        let mut q = sqlx::query_as::<_, (i64,)>(&sql);
        for (key, value) in filters {
            q = q.bind(key).bind(value);
        }
        let (count,) = q.fetch_one(&self.pool).await?;
        Ok(count)
    }

    /// List distinct values for a tag key across active tasks, with counts.
    ///
    /// Returns `(value, count)` pairs sorted by count descending.
    pub async fn tag_values(&self, key: &str) -> Result<Vec<(String, i64)>, StoreError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT value, COUNT(*) as cnt FROM task_tags WHERE key = ? GROUP BY value ORDER BY cnt DESC",
        )
        .bind(key)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Count active tasks grouped by a tag key's values.
    ///
    /// Returns `(tag_value, count)` pairs sorted by count descending.
    /// Optionally filter by task status.
    pub async fn count_by_tag(
        &self,
        key: &str,
        status: Option<crate::task::TaskStatus>,
    ) -> Result<Vec<(String, i64)>, StoreError> {
        let (sql, bind_status) = match status {
            Some(ref s) => (
                "SELECT tt.value, COUNT(*) as cnt FROM task_tags tt \
                 JOIN tasks t ON t.id = tt.task_id \
                 WHERE tt.key = ? AND t.status = ? \
                 GROUP BY tt.value ORDER BY cnt DESC"
                    .to_string(),
                Some(s.as_str()),
            ),
            None => (
                "SELECT tt.value, COUNT(*) as cnt FROM task_tags tt \
                 JOIN tasks t ON t.id = tt.task_id \
                 WHERE tt.key = ? \
                 GROUP BY tt.value ORDER BY cnt DESC"
                    .to_string(),
                None,
            ),
        };

        let mut q = sqlx::query_as::<_, (String, i64)>(&sql).bind(key);
        if let Some(status_str) = bind_status {
            q = q.bind(status_str);
        }
        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    // ── Scheduling ─────────────────────────────────────────────────

    /// Returns the earliest `run_after` timestamp among pending tasks, if any.
    pub async fn next_run_after(
        &self,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, StoreError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT run_after FROM tasks
             WHERE status = 'pending' AND run_after IS NOT NULL
             ORDER BY run_after ASC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(s,)| super::row_mapping::parse_datetime(&s)))
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

    // ── Dependencies ───────────────────────────────────────────────

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

#[cfg(test)]
mod tests {
    use crate::priority::Priority;
    use crate::task::{HistoryStatus, IoBudget, TaskLookup, TaskStatus, TaskSubmission};

    use super::super::TaskStore;

    async fn test_store() -> TaskStore {
        TaskStore::open_memory().await.unwrap()
    }

    fn make_submission(key: &str, priority: Priority) -> TaskSubmission {
        TaskSubmission::new("test")
            .key(key)
            .priority(priority)
            .payload_raw(b"hello".to_vec())
            .expected_io(IoBudget::disk(1000, 500))
    }

    #[tokio::test]
    async fn task_by_id_lookup() {
        let store = test_store().await;
        let sub = make_submission("by-id", Priority::NORMAL);
        let id = store.submit(&sub).await.unwrap().id().unwrap();

        let task = store.task_by_id(id).await.unwrap().unwrap();
        assert_eq!(task.id, id);
        assert_eq!(task.key, sub.effective_key());

        assert!(store.task_by_id(9999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn history_by_id_lookup() {
        let store = test_store().await;
        let sub = make_submission("hist-id", Priority::NORMAL);
        store.submit(&sub).await.unwrap();
        let task = store.pop_next().await.unwrap().unwrap();

        store
            .complete(task.id, &IoBudget::disk(100, 50))
            .await
            .unwrap();

        let hist = store.history_by_key(&sub.effective_key()).await.unwrap();
        assert_eq!(hist.len(), 1);
        let hist_id = hist[0].id;

        let record = store.history_by_id(hist_id).await.unwrap().unwrap();
        assert_eq!(record.key, sub.effective_key());
        assert_eq!(record.actual_io.unwrap().disk_read, 100);

        assert!(store.history_by_id(9999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn history_stats_computation() {
        let store = test_store().await;

        for i in 0..3 {
            let sub = make_submission(&format!("stat-{i}"), Priority::NORMAL);
            store.submit(&sub).await.unwrap();
            let task = store.pop_next().await.unwrap().unwrap();
            store
                .complete(task.id, &IoBudget::disk(1000, 500))
                .await
                .unwrap();
        }

        let stats = store.history_stats("test").await.unwrap();
        assert_eq!(stats.count, 3);
        assert!(stats.failure_rate == 0.0);
    }

    #[tokio::test]
    async fn open_with_custom_config() {
        let store = TaskStore::open_memory().await.unwrap();
        let count = store.pending_count().await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn delete_task() {
        let store = test_store().await;
        let sub = make_submission("del-me", Priority::NORMAL);
        let key = sub.effective_key();
        store.submit(&sub).await.unwrap();

        let task = store.task_by_key(&key).await.unwrap().unwrap();
        assert!(store.delete(task.id).await.unwrap());
        assert!(store.task_by_key(&key).await.unwrap().is_none());

        assert!(!store.delete(task.id).await.unwrap());
    }

    #[tokio::test]
    async fn task_lookup_active() {
        let store = test_store().await;
        let sub = make_submission("lookup-active", Priority::NORMAL);
        let key = sub.effective_key();
        store.submit(&sub).await.unwrap();

        let result = store.task_lookup(&key).await.unwrap();
        assert!(matches!(result, TaskLookup::Active(ref r) if r.status == TaskStatus::Pending));

        store.pop_next().await.unwrap();
        let result = store.task_lookup(&key).await.unwrap();
        assert!(matches!(result, TaskLookup::Active(ref r) if r.status == TaskStatus::Running));
    }

    #[tokio::test]
    async fn task_lookup_history() {
        let store = test_store().await;
        let sub = make_submission("lookup-hist", Priority::NORMAL);
        let key = sub.effective_key();
        store.submit(&sub).await.unwrap();
        let task = store.pop_next().await.unwrap().unwrap();
        store.complete(task.id, &IoBudget::default()).await.unwrap();

        let result = store.task_lookup(&key).await.unwrap();
        assert!(
            matches!(result, TaskLookup::History(ref r) if r.status == HistoryStatus::Completed)
        );
    }

    #[tokio::test]
    async fn task_lookup_not_found() {
        let store = test_store().await;
        let key = crate::task::generate_dedup_key("nope", Some(b"nope"));
        let result = store.task_lookup(&key).await.unwrap();
        assert!(matches!(result, TaskLookup::NotFound));
    }

    #[tokio::test]
    async fn prune_by_count() {
        let store = test_store().await;

        for i in 0..5 {
            let sub = make_submission(&format!("prune-{i}"), Priority::NORMAL);
            store.submit(&sub).await.unwrap();
            let task = store.pop_next().await.unwrap().unwrap();
            store.complete(task.id, &IoBudget::default()).await.unwrap();
        }

        let hist = store.history(100, 0).await.unwrap();
        assert_eq!(hist.len(), 5);

        let deleted = store.prune_history_by_count(3).await.unwrap();
        assert_eq!(deleted, 2);

        let hist = store.history(100, 0).await.unwrap();
        assert_eq!(hist.len(), 3);
    }

    // ── Tag query tests ───────────────────────────────────────────────

    #[tokio::test]
    async fn tasks_by_tags_single_filter() {
        let store = test_store().await;

        store
            .submit(&TaskSubmission::new("test").key("tbt-1").tag("env", "prod"))
            .await
            .unwrap();
        store
            .submit(
                &TaskSubmission::new("test")
                    .key("tbt-2")
                    .tag("env", "staging"),
            )
            .await
            .unwrap();
        store
            .submit(&TaskSubmission::new("test").key("tbt-3").tag("env", "prod"))
            .await
            .unwrap();

        let results = store.tasks_by_tags(&[("env", "prod")], None).await.unwrap();
        assert_eq!(results.len(), 2);

        let results = store
            .tasks_by_tags(&[("env", "staging")], None)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn tasks_by_tags_multiple_filters_and() {
        let store = test_store().await;

        store
            .submit(
                &TaskSubmission::new("test")
                    .key("multi-1")
                    .tag("env", "prod")
                    .tag("region", "us"),
            )
            .await
            .unwrap();
        store
            .submit(
                &TaskSubmission::new("test")
                    .key("multi-2")
                    .tag("env", "prod")
                    .tag("region", "eu"),
            )
            .await
            .unwrap();
        store
            .submit(
                &TaskSubmission::new("test")
                    .key("multi-3")
                    .tag("env", "staging")
                    .tag("region", "us"),
            )
            .await
            .unwrap();

        // AND semantics: only task matching both filters.
        let results = store
            .tasks_by_tags(&[("env", "prod"), ("region", "us")], None)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);

        // With status filter.
        let results = store
            .tasks_by_tags(&[("env", "prod")], Some(TaskStatus::Pending))
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn count_by_tag_groups() {
        let store = test_store().await;

        for i in 0..3 {
            store
                .submit(
                    &TaskSubmission::new("test")
                        .key(format!("free-{i}"))
                        .tag("tier", "free"),
                )
                .await
                .unwrap();
        }
        for i in 0..2 {
            store
                .submit(
                    &TaskSubmission::new("test")
                        .key(format!("pro-{i}"))
                        .tag("tier", "pro"),
                )
                .await
                .unwrap();
        }

        let groups = store.count_by_tag("tier", None).await.unwrap();
        assert_eq!(groups.len(), 2);
        // Sorted by count descending.
        assert_eq!(groups[0].0, "free");
        assert_eq!(groups[0].1, 3);
        assert_eq!(groups[1].0, "pro");
        assert_eq!(groups[1].1, 2);
    }

    #[tokio::test]
    async fn tag_values_distinct() {
        let store = test_store().await;

        store
            .submit(&TaskSubmission::new("test").key("tv-1").tag("color", "red"))
            .await
            .unwrap();
        store
            .submit(&TaskSubmission::new("test").key("tv-2").tag("color", "red"))
            .await
            .unwrap();
        store
            .submit(&TaskSubmission::new("test").key("tv-3").tag("color", "blue"))
            .await
            .unwrap();

        let values = store.tag_values("color").await.unwrap();
        assert_eq!(values.len(), 2);
        // Sorted by count descending.
        assert_eq!(values[0], ("red".to_string(), 2));
        assert_eq!(values[1], ("blue".to_string(), 1));

        // Non-existent key returns empty.
        let empty = store.tag_values("nonexistent").await.unwrap();
        assert!(empty.is_empty());
    }
}
