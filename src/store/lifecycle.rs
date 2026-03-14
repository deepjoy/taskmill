//! Task lifecycle transitions: pop, complete, fail, pause, resume.

use crate::task::{IoBudget, TaskRecord};

use super::row_mapping::row_to_task_record;
use super::{StoreError, TaskStore};

/// Insert a task record into the history table.
///
/// Shared by `complete()`, `fail()`, and `cancel_to_history()` to eliminate
/// the duplicated 22-column INSERT statement.
pub(crate) async fn insert_history(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    task: &TaskRecord,
    status: &str,
    metrics: &IoBudget,
    duration_ms: Option<i64>,
    last_error: Option<&str>,
) -> Result<(), StoreError> {
    let fail_fast_val: i32 = if task.fail_fast { 1 } else { 0 };
    let retry_count = if status == "failed" {
        task.retry_count + 1
    } else {
        task.retry_count
    };
    sqlx::query(
        "INSERT INTO task_history (task_type, key, label, priority, status, payload,
            expected_read_bytes, expected_write_bytes, expected_net_rx_bytes, expected_net_tx_bytes,
            actual_read_bytes, actual_write_bytes, actual_net_rx_bytes, actual_net_tx_bytes,
            retry_count, last_error, created_at, started_at, duration_ms, parent_id, fail_fast, group_key)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&task.task_type)
    .bind(&task.key)
    .bind(&task.label)
    .bind(task.priority.value() as i32)
    .bind(status)
    .bind(&task.payload)
    .bind(task.expected_io.disk_read)
    .bind(task.expected_io.disk_write)
    .bind(task.expected_io.net_rx)
    .bind(task.expected_io.net_tx)
    .bind(metrics.disk_read)
    .bind(metrics.disk_write)
    .bind(metrics.net_rx)
    .bind(metrics.net_tx)
    .bind(retry_count)
    .bind(last_error)
    .bind(task.created_at.format("%Y-%m-%d %H:%M:%S").to_string())
    .bind(
        task.started_at
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
    )
    .bind(duration_ms)
    .bind(task.parent_id)
    .bind(fail_fast_val)
    .bind(&task.group_key)
    .execute(&mut **conn)
    .await?;
    Ok(())
}

/// Compute the duration in milliseconds from `started_at` to now.
fn compute_duration_ms(task: &TaskRecord) -> Option<i64> {
    task.started_at
        .map(|started| (chrono::Utc::now() - started).num_milliseconds())
}

impl TaskStore {
    // ── Pop / lifecycle ─────────────────────────────────────────────

    /// Peek at the highest-priority pending task without modifying it.
    /// Returns `None` if the queue is empty.
    pub async fn peek_next(&self) -> Result<Option<TaskRecord>, StoreError> {
        let row = sqlx::query(
            "SELECT * FROM tasks
             WHERE id = (
                 SELECT id FROM tasks
                 WHERE status = 'pending'
                 ORDER BY priority ASC, id ASC
                 LIMIT 1
             )",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(row_to_task_record))
    }

    /// Atomically claim a specific pending task by id, setting it to running.
    /// Returns `None` if the task is no longer pending (e.g. claimed by another
    /// dispatcher or cancelled).
    pub async fn pop_by_id(&self, id: i64) -> Result<Option<TaskRecord>, StoreError> {
        tracing::debug!(task_id = id, "store.pop_by_id: UPDATE start");
        let row = sqlx::query(
            "UPDATE tasks SET status = 'running', started_at = datetime('now')
             WHERE id = ? AND status = 'pending'
             RETURNING *",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        tracing::debug!(task_id = id, "store.pop_by_id: UPDATE end");

        Ok(row.as_ref().map(row_to_task_record))
    }

    /// Pop the highest-priority pending task and mark it as running.
    /// Returns `None` if the queue is empty.
    pub async fn pop_next(&self) -> Result<Option<TaskRecord>, StoreError> {
        let row = sqlx::query(
            "UPDATE tasks SET status = 'running', started_at = datetime('now')
             WHERE id = (
                 SELECT id FROM tasks
                 WHERE status = 'pending'
                 ORDER BY priority ASC, id ASC
                 LIMIT 1
             )
             RETURNING *",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| row_to_task_record(&r)))
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

    /// Mark a task as completed and move it to history.
    pub async fn complete(&self, id: i64, metrics: &IoBudget) -> Result<(), StoreError> {
        tracing::debug!(task_id = id, "store.complete: BEGIN tx");
        let mut conn = self.begin_write().await?;

        // Fetch the task to move.
        let row = sqlx::query("SELECT * FROM tasks WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;

        let Some(row) = row else { return Ok(()) };
        let task = row_to_task_record(&row);

        Self::complete_inner(&mut conn, &task, metrics).await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        drop(conn);
        tracing::debug!(task_id = id, "store.complete: COMMIT ok");

        self.maybe_prune().await;

        Ok(())
    }

    /// Mark a task as completed using an in-memory record, avoiding the
    /// redundant `SELECT *` round-trip. The `requeue` flag is still checked
    /// from the database row since it may have been set by a concurrent
    /// `submit()` while the task was running.
    pub async fn complete_with_record(
        &self,
        task: &TaskRecord,
        metrics: &IoBudget,
    ) -> Result<(), StoreError> {
        tracing::debug!(task_id = task.id, "store.complete_with_record: BEGIN tx");
        let mut conn = self.begin_write().await?;

        Self::complete_inner(&mut conn, task, metrics).await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        drop(conn);
        tracing::debug!(task_id = task.id, "store.complete_with_record: COMMIT ok");

        self.maybe_prune().await;

        Ok(())
    }

    /// Shared completion logic: insert history, then handle requeue or delete.
    async fn complete_inner(
        conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
        task: &TaskRecord,
        metrics: &IoBudget,
    ) -> Result<(), StoreError> {
        let duration_ms = compute_duration_ms(task);

        // Insert into history.
        insert_history(
            conn,
            task,
            "completed",
            metrics,
            duration_ms,
            task.last_error.as_deref(),
        )
        .await?;

        // Try to delete (normal completion, requeue = 0).
        let del = sqlx::query("DELETE FROM tasks WHERE id = ? AND requeue = 0")
            .bind(task.id)
            .execute(&mut **conn)
            .await?;

        if del.rows_affected() == 0 {
            // Requeue flag was set by a concurrent submit — reset to pending.
            // No-op if the task was already deleted (cancelled).
            sqlx::query(
                "UPDATE tasks SET status = 'pending',
                    priority = COALESCE(requeue_priority, priority),
                    started_at = NULL, retry_count = 0, last_error = NULL,
                    requeue = 0, requeue_priority = NULL
                 WHERE id = ?",
            )
            .bind(task.id)
            .execute(&mut **conn)
            .await?;
        }

        Ok(())
    }

    /// Mark a task as failed. If `retryable` and under max retries, requeue
    /// it as pending with the same priority. Otherwise move to history as failed.
    pub async fn fail(
        &self,
        id: i64,
        error: &str,
        retryable: bool,
        max_retries: i32,
        metrics: &IoBudget,
    ) -> Result<(), StoreError> {
        tracing::debug!(task_id = id, "store.fail: BEGIN tx");
        let mut conn = self.begin_write().await?;
        tracing::debug!(task_id = id, "store.fail: BEGIN acquired");

        let row = sqlx::query("SELECT * FROM tasks WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;

        let Some(row) = row else { return Ok(()) };
        let task = row_to_task_record(&row);

        Self::fail_inner(&mut conn, &task, error, retryable, max_retries, metrics).await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        drop(conn);
        tracing::debug!(task_id = id, "store.fail: COMMIT ok");

        self.maybe_prune().await;

        Ok(())
    }

    /// Mark a task as failed using an in-memory record, avoiding the
    /// redundant `SELECT *` round-trip.
    pub async fn fail_with_record(
        &self,
        task: &TaskRecord,
        error: &str,
        retryable: bool,
        max_retries: i32,
        metrics: &IoBudget,
    ) -> Result<(), StoreError> {
        tracing::debug!(task_id = task.id, "store.fail_with_record: BEGIN tx");
        let mut conn = self.begin_write().await?;
        tracing::debug!(task_id = task.id, "store.fail_with_record: BEGIN acquired");

        Self::fail_inner(&mut conn, task, error, retryable, max_retries, metrics).await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        drop(conn);
        tracing::debug!(task_id = task.id, "store.fail_with_record: COMMIT ok");

        self.maybe_prune().await;

        Ok(())
    }

    /// Shared failure logic: retry or move to history.
    async fn fail_inner(
        conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
        task: &TaskRecord,
        error: &str,
        retryable: bool,
        max_retries: i32,
        metrics: &IoBudget,
    ) -> Result<(), StoreError> {
        if retryable && task.retry_count < max_retries {
            // Requeue with incremented retry count, same priority.
            sqlx::query(
                "UPDATE tasks SET status = 'pending', started_at = NULL,
                    retry_count = retry_count + 1, last_error = ?
                 WHERE id = ?",
            )
            .bind(error)
            .bind(task.id)
            .execute(&mut **conn)
            .await?;
        } else {
            // Permanent failure — move to history.
            let duration_ms = compute_duration_ms(task);

            insert_history(conn, task, "failed", metrics, duration_ms, Some(error)).await?;

            sqlx::query("DELETE FROM tasks WHERE id = ?")
                .bind(task.id)
                .execute(&mut **conn)
                .await?;
        }

        Ok(())
    }

    /// Pause a running task (for preemption). Sets status to paused.
    pub async fn pause(&self, id: i64) -> Result<(), StoreError> {
        sqlx::query("UPDATE tasks SET status = 'paused', started_at = NULL WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Resume a paused task back to pending.
    pub async fn resume(&self, id: i64) -> Result<(), StoreError> {
        sqlx::query("UPDATE tasks SET status = 'pending' WHERE id = ? AND status = 'paused'")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Move a task to history as cancelled and delete it from the active queue.
    ///
    /// Returns `true` if the task was found and cancelled, `false` if it
    /// did not exist.
    pub async fn cancel_to_history(&self, id: i64) -> Result<bool, StoreError> {
        let mut conn = self.begin_write().await?;

        let row = sqlx::query("SELECT * FROM tasks WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;

        let Some(row) = row else {
            sqlx::query("COMMIT").execute(&mut *conn).await?;
            return Ok(false);
        };
        let task = row_to_task_record(&row);
        let duration_ms = compute_duration_ms(&task);

        insert_history(
            &mut conn,
            &task,
            "cancelled",
            &IoBudget::default(),
            duration_ms,
            None,
        )
        .await?;

        sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(id)
            .execute(&mut *conn)
            .await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        Ok(true)
    }

    /// Move a task to history as cancelled using an in-memory record,
    /// avoiding the redundant `SELECT *` round-trip.
    pub async fn cancel_to_history_with_record(&self, task: &TaskRecord) -> Result<(), StoreError> {
        let mut conn = self.begin_write().await?;
        let duration_ms = compute_duration_ms(task);

        insert_history(
            &mut conn,
            task,
            "cancelled",
            &IoBudget::default(),
            duration_ms,
            None,
        )
        .await?;

        sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(task.id)
            .execute(&mut *conn)
            .await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::priority::Priority;
    use crate::task::{HistoryStatus, IoBudget, TaskStatus, TaskSubmission};

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
    async fn priority_ordering() {
        let store = test_store().await;

        let bg = make_submission("bg", Priority::BACKGROUND);
        let rt = make_submission("rt", Priority::REALTIME);
        let normal = make_submission("normal", Priority::NORMAL);

        let bg_key = bg.effective_key();
        let rt_key = rt.effective_key();
        let normal_key = normal.effective_key();

        store.submit(&bg).await.unwrap();
        store.submit(&rt).await.unwrap();
        store.submit(&normal).await.unwrap();

        let first = store.pop_next().await.unwrap().unwrap();
        assert_eq!(first.key, rt_key);

        let second = store.pop_next().await.unwrap().unwrap();
        assert_eq!(second.key, normal_key);

        let third = store.pop_next().await.unwrap().unwrap();
        assert_eq!(third.key, bg_key);
    }

    #[tokio::test]
    async fn complete_moves_to_history() {
        let store = test_store().await;
        let sub = make_submission("done", Priority::NORMAL);
        let key = sub.effective_key();
        store.submit(&sub).await.unwrap();
        let task = store.pop_next().await.unwrap().unwrap();

        store
            .complete(task.id, &IoBudget::disk(2000, 1000))
            .await
            .unwrap();

        assert!(store.task_by_key(&key).await.unwrap().is_none());

        let hist = store.history_by_key(&key).await.unwrap();
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].status, HistoryStatus::Completed);
        assert_eq!(hist[0].actual_io.unwrap().disk_read, 2000);
    }

    #[tokio::test]
    async fn fail_retryable_requeues() {
        let store = test_store().await;
        let sub = make_submission("retry-me", Priority::HIGH);
        let key = sub.effective_key();
        store.submit(&sub).await.unwrap();
        let task = store.pop_next().await.unwrap().unwrap();

        store
            .fail(task.id, "transient error", true, 3, &IoBudget::default())
            .await
            .unwrap();

        let requeued = store.task_by_key(&key).await.unwrap().unwrap();
        assert_eq!(requeued.status, TaskStatus::Pending);
        assert_eq!(requeued.retry_count, 1);
        assert_eq!(requeued.last_error.as_deref(), Some("transient error"));
    }

    #[tokio::test]
    async fn fail_exhausted_retries_moves_to_history() {
        let store = test_store().await;
        let sub = make_submission("permanent", Priority::NORMAL);
        let key = sub.effective_key();
        store.submit(&sub).await.unwrap();
        let task = store.pop_next().await.unwrap().unwrap();

        store
            .fail(task.id, "err1", true, 1, &IoBudget::default())
            .await
            .unwrap();
        let task = store.pop_next().await.unwrap().unwrap();
        assert_eq!(task.retry_count, 1);
        store
            .fail(task.id, "err2", true, 1, &IoBudget::disk(100, 50))
            .await
            .unwrap();

        assert!(store.task_by_key(&key).await.unwrap().is_none());
        let hist = store.failed_tasks(10).await.unwrap();
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].status, HistoryStatus::Failed);
    }

    #[tokio::test]
    async fn pause_and_resume() {
        let store = test_store().await;
        store
            .submit(&make_submission("pausable", Priority::NORMAL))
            .await
            .unwrap();
        let task = store.pop_next().await.unwrap().unwrap();

        store.pause(task.id).await.unwrap();
        let paused = store.paused_tasks().await.unwrap();
        assert_eq!(paused.len(), 1);
        assert_eq!(paused[0].status, TaskStatus::Paused);

        store.resume(task.id).await.unwrap();
        let pending = store.pending_tasks(10).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].status, TaskStatus::Pending);
    }

    #[tokio::test]
    async fn running_io_totals() {
        let store = test_store().await;

        let sub = TaskSubmission::new("test")
            .key("io-1")
            .priority(Priority::NORMAL)
            .payload_raw(b"hello".to_vec())
            .expected_io(IoBudget::disk(5000, 2000));
        store.submit(&sub).await.unwrap();

        let sub2 = TaskSubmission::new("test")
            .key("io-2")
            .priority(Priority::NORMAL)
            .payload_raw(b"hello".to_vec())
            .expected_io(IoBudget::disk(3000, 1000));
        store.submit(&sub2).await.unwrap();

        store.pop_next().await.unwrap();
        store.pop_next().await.unwrap();

        let (read, write) = store.running_io_totals().await.unwrap();
        assert_eq!(read, 8000);
        assert_eq!(write, 3000);
    }

    #[tokio::test]
    async fn key_freed_after_completion() {
        let store = test_store().await;
        let sub = make_submission("reuse", Priority::NORMAL);
        store.submit(&sub).await.unwrap();
        let task = store.pop_next().await.unwrap().unwrap();
        store.complete(task.id, &IoBudget::default()).await.unwrap();

        let outcome = store.submit(&sub).await.unwrap();
        assert!(outcome.is_inserted());
    }

    #[tokio::test]
    async fn requeue_running_task() {
        let store = test_store().await;
        let sub = make_submission("rq", Priority::NORMAL);
        let key = sub.effective_key();
        store.submit(&sub).await.unwrap();
        let task = store.pop_next().await.unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Running);

        store.requeue(task.id).await.unwrap();
        let t = store.task_by_key(&key).await.unwrap().unwrap();
        assert_eq!(t.status, TaskStatus::Pending);
        assert!(t.started_at.is_none());
    }

    #[tokio::test]
    async fn peek_next_does_not_modify_status() {
        let store = test_store().await;
        let sub = make_submission("peek-me", Priority::NORMAL);
        let key = sub.effective_key();
        store.submit(&sub).await.unwrap();

        let peeked = store.peek_next().await.unwrap().unwrap();
        assert_eq!(peeked.key, key);
        assert_eq!(peeked.status, TaskStatus::Pending);

        let t = store.task_by_key(&key).await.unwrap().unwrap();
        assert_eq!(t.status, TaskStatus::Pending);
        assert!(t.started_at.is_none());

        let peeked2 = store.peek_next().await.unwrap().unwrap();
        assert_eq!(peeked2.id, peeked.id);
    }

    #[tokio::test]
    async fn peek_next_empty_queue() {
        let store = test_store().await;
        assert!(store.peek_next().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn pop_by_id_claims_pending_task() {
        let store = test_store().await;
        let sub = make_submission("claim-me", Priority::NORMAL);
        let key = sub.effective_key();
        let id = store.submit(&sub).await.unwrap().id().unwrap();

        let task = store.pop_by_id(id).await.unwrap().unwrap();
        assert_eq!(task.key, key);
        assert_eq!(task.status, TaskStatus::Running);
        assert!(task.started_at.is_some());
    }

    #[tokio::test]
    async fn pop_by_id_returns_none_if_already_running() {
        let store = test_store().await;
        let sub = make_submission("already-taken", Priority::NORMAL);
        store.submit(&sub).await.unwrap();

        let task = store.pop_next().await.unwrap().unwrap();

        assert!(store.pop_by_id(task.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn pop_by_id_returns_none_for_nonexistent() {
        let store = test_store().await;
        assert!(store.pop_by_id(9999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn peek_then_pop_by_id_workflow() {
        let store = test_store().await;
        let sub = make_submission("peek-pop", Priority::NORMAL);
        let key = sub.effective_key();
        store.submit(&sub).await.unwrap();

        let peeked = store.peek_next().await.unwrap().unwrap();
        let claimed = store.pop_by_id(peeked.id).await.unwrap().unwrap();
        assert_eq!(claimed.key, key);
        assert_eq!(claimed.status, TaskStatus::Running);

        assert!(store.peek_next().await.unwrap().is_none());
    }
}
