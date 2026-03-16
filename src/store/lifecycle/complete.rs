//! Task completion: move to history, handle recurring re-creation, requeue.

use crate::store::row_mapping::row_to_task_record;
use crate::store::{StoreError, TaskStore};
use crate::task::IoBudget;

use super::{compute_duration_ms, insert_history};

impl TaskStore {
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

        let _recurring = Self::complete_inner(&mut conn, &task, metrics).await?;

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
    ///
    /// Returns `Some((next_run, execution_count))` if a recurring next
    /// instance was created, `None` otherwise.
    pub async fn complete_with_record(
        &self,
        task: &crate::task::TaskRecord,
        metrics: &IoBudget,
    ) -> Result<Option<(chrono::DateTime<chrono::Utc>, i64)>, StoreError> {
        tracing::debug!(task_id = task.id, "store.complete_with_record: BEGIN tx");
        let mut conn = self.begin_write().await?;

        let recurring_info = Self::complete_inner(&mut conn, task, metrics).await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        drop(conn);
        tracing::debug!(task_id = task.id, "store.complete_with_record: COMMIT ok");

        self.maybe_prune().await;

        Ok(recurring_info)
    }

    /// Shared completion logic: insert history, handle recurring next instance,
    /// then handle requeue or delete.
    ///
    /// Returns `Some((next_run, exec_count))` if a recurring next instance was
    /// created, `None` otherwise.
    async fn complete_inner(
        conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
        task: &crate::task::TaskRecord,
        metrics: &IoBudget,
    ) -> Result<Option<(chrono::DateTime<chrono::Utc>, i64)>, StoreError> {
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

        // Read tags into memory before potential deletion (needed for recurring re-creation).
        let saved_tags: Vec<(String, String)> = if task.recurring_interval_secs.is_some() {
            sqlx::query_as("SELECT key, value FROM task_tags WHERE task_id = ?")
                .bind(task.id)
                .fetch_all(&mut **conn)
                .await?
        } else {
            Vec::new()
        };

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
            // Don't create recurring next instance if requeued.
            return Ok(None);
        }

        // Task was deleted — clean up orphaned tags.
        crate::store::delete_task_tags(conn, task.id).await?;

        // Handle recurring tasks: create the next instance after deleting
        // the completed one (to avoid UNIQUE constraint on key).
        let mut recurring_info = None;
        if let Some(interval) = task.recurring_interval_secs {
            if !task.recurring_paused {
                let execution_count = task.recurring_execution_count + 1;
                let should_create = task
                    .recurring_max_executions
                    .map_or(true, |max| execution_count < max);

                if should_create {
                    // Pile-up prevention: check if a pending instance already exists
                    // (e.g. from a concurrent submit with the same key).
                    let existing: Option<(i64,)> =
                        sqlx::query_as("SELECT id FROM tasks WHERE key = ? AND status = 'pending'")
                            .bind(&task.key)
                            .fetch_optional(&mut **conn)
                            .await?;

                    if existing.is_none() {
                        let next_run = chrono::Utc::now() + chrono::Duration::seconds(interval);
                        let next_run_str = next_run.format("%Y-%m-%d %H:%M:%S").to_string();
                        let fail_fast_val: i32 = if task.fail_fast { 1 } else { 0 };

                        // Compute TTL columns for the next instance.
                        let expires_at_str: Option<String> = match (task.ttl_seconds, task.ttl_from)
                        {
                            (Some(ttl_secs), crate::task::TtlFrom::Submission) => {
                                let exp = chrono::Utc::now() + chrono::Duration::seconds(ttl_secs);
                                Some(exp.format("%Y-%m-%d %H:%M:%S").to_string())
                            }
                            _ => None,
                        };

                        let recurring_result = sqlx::query(
                            "INSERT INTO tasks (task_type, key, label, priority, status, payload,
                                expected_read_bytes, expected_write_bytes,
                                expected_net_rx_bytes, expected_net_tx_bytes,
                                parent_id, fail_fast, group_key,
                                ttl_seconds, ttl_from, expires_at,
                                run_after, recurring_interval_secs,
                                recurring_max_executions, recurring_execution_count,
                                recurring_paused, max_retries)
                             VALUES (?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?)",
                        )
                        .bind(&task.task_type)
                        .bind(&task.key)
                        .bind(&task.label)
                        .bind(task.priority.value() as i32)
                        .bind(&task.payload)
                        .bind(task.expected_io.disk_read)
                        .bind(task.expected_io.disk_write)
                        .bind(task.expected_io.net_rx)
                        .bind(task.expected_io.net_tx)
                        .bind(task.parent_id)
                        .bind(fail_fast_val)
                        .bind(&task.group_key)
                        .bind(task.ttl_seconds)
                        .bind(task.ttl_from.as_str())
                        .bind(&expires_at_str)
                        .bind(&next_run_str)
                        .bind(task.recurring_interval_secs)
                        .bind(task.recurring_max_executions)
                        .bind(execution_count)
                        .bind(task.max_retries)
                        .execute(&mut **conn)
                        .await?;

                        // Copy tags to the new recurring instance.
                        let next_id = recurring_result.last_insert_rowid();
                        for (key, value) in &saved_tags {
                            sqlx::query(
                                "INSERT INTO task_tags (task_id, key, value) VALUES (?, ?, ?)",
                            )
                            .bind(next_id)
                            .bind(key)
                            .bind(value)
                            .execute(&mut **conn)
                            .await?;
                        }

                        recurring_info = Some((next_run, execution_count));
                    }
                    // If existing.is_some(), skip (pile-up prevention).
                }
            }
        }

        Ok(recurring_info)
    }
}
