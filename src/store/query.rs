//! Read-only query methods for the active queue and history tables.

use sqlx::Row;

use crate::task::{TaskHistoryRecord, TaskLookup, TaskRecord, TypeStats};

use super::row_mapping::{row_to_history_record, row_to_task_record};
use super::{StoreError, TaskStore};

impl TaskStore {
    // ── Query: active queue ─────────────────────────────────────────

    /// All currently running tasks.
    pub async fn running_tasks(&self) -> Result<Vec<TaskRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT * FROM tasks WHERE status = 'running' ORDER BY priority ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_task_record).collect())
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
        Ok(rows.iter().map(row_to_task_record).collect())
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
        Ok(rows.iter().map(row_to_task_record).collect())
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
        Ok(rows.iter().map(row_to_task_record).collect())
    }

    /// Look up an active task by its row id. Returns `None` if no active
    /// task with that id exists.
    pub async fn task_by_id(&self, id: i64) -> Result<Option<TaskRecord>, StoreError> {
        let row = sqlx::query("SELECT * FROM tasks WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(row_to_task_record))
    }

    /// Look up an active task by its dedup key. Returns `None` if no active
    /// task with that key exists.
    pub async fn task_by_key(&self, key: &str) -> Result<Option<TaskRecord>, StoreError> {
        let row = sqlx::query("SELECT * FROM tasks WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(row_to_task_record))
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
        Ok(row.as_ref().map(row_to_history_record))
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
        Ok(rows.iter().map(row_to_history_record).collect())
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
        Ok(rows.iter().map(row_to_history_record).collect())
    }

    /// History for a specific key (all past runs of that key).
    pub async fn history_by_key(&self, key: &str) -> Result<Vec<TaskHistoryRecord>, StoreError> {
        let rows =
            sqlx::query("SELECT * FROM task_history WHERE key = ? ORDER BY completed_at DESC")
                .bind(key)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.iter().map(row_to_history_record).collect())
    }

    /// Failed tasks from history.
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
            Some(r) => Ok(TaskLookup::History(row_to_history_record(&r))),
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
        Ok(rows.iter().map(row_to_task_record).collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::priority::Priority;
    use crate::task::{HistoryStatus, TaskLookup, TaskMetrics, TaskStatus, TaskSubmission};

    use super::super::TaskStore;

    async fn test_store() -> TaskStore {
        TaskStore::open_memory().await.unwrap()
    }

    fn make_submission(key: &str, priority: Priority) -> TaskSubmission {
        TaskSubmission::new("test")
            .key(key)
            .priority(priority)
            .payload_raw(b"hello".to_vec())
            .expected_io(1000, 500)
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
            .complete(
                task.id,
                &TaskMetrics {
                    read_bytes: 100,
                    write_bytes: 50,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let hist = store.history_by_key(&sub.effective_key()).await.unwrap();
        assert_eq!(hist.len(), 1);
        let hist_id = hist[0].id;

        let record = store.history_by_id(hist_id).await.unwrap().unwrap();
        assert_eq!(record.key, sub.effective_key());
        assert_eq!(record.actual_read_bytes, Some(100));

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
                .complete(
                    task.id,
                    &TaskMetrics {
                        read_bytes: 1000,
                        write_bytes: 500,
                        ..Default::default()
                    },
                )
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
        store
            .complete(task.id, &TaskMetrics::default())
            .await
            .unwrap();

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
            store
                .complete(task.id, &TaskMetrics::default())
                .await
                .unwrap();
        }

        let hist = store.history(100, 0).await.unwrap();
        assert_eq!(hist.len(), 5);

        let deleted = store.prune_history_by_count(3).await.unwrap();
        assert_eq!(deleted, 2);

        let hist = store.history(100, 0).await.unwrap();
        assert_eq!(hist.len(), 3);
    }
}
