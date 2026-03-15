//! Task lifecycle transitions: pop, complete, fail, pause, resume, and
//! dependency resolution.

use crate::task::{DependencyFailurePolicy, IoBudget, TaskRecord};

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
            retry_count, last_error, created_at, started_at, duration_ms, parent_id, fail_fast, group_key,
            ttl_seconds, ttl_from, expires_at, run_after)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(task.ttl_seconds)
    .bind(task.ttl_from.as_str())
    .bind(
        task.expires_at
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
    )
    .bind(
        task.run_after
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
    )
    .execute(&mut **conn)
    .await?;

    // Copy tags from task_tags to task_history_tags.
    let history_rowid = sqlx::query_scalar::<_, i64>("SELECT last_insert_rowid()")
        .fetch_one(&mut **conn)
        .await?;
    sqlx::query(
        "INSERT INTO task_history_tags (history_rowid, key, value)
         SELECT ?, key, value FROM task_tags WHERE task_id = ?",
    )
    .bind(history_rowid)
    .bind(task.id)
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
    /// Returns `None` if the queue is empty. Tasks with a future `run_after`
    /// timestamp are excluded (not yet eligible for dispatch).
    pub async fn peek_next(&self) -> Result<Option<TaskRecord>, StoreError> {
        let row = sqlx::query(
            "SELECT * FROM tasks
             WHERE id = (
                 SELECT id FROM tasks
                 WHERE status = 'pending'
                   AND (run_after IS NULL OR run_after <= datetime('now'))
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
                   AND (run_after IS NULL OR run_after <= datetime('now'))
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
        task: &TaskRecord,
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
        task: &TaskRecord,
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
        super::delete_task_tags(conn, task.id).await?;

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
                                recurring_paused)
                             VALUES (?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)",
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

            super::delete_task_tags(conn, task.id).await?;
            sqlx::query("DELETE FROM tasks WHERE id = ?")
                .bind(task.id)
                .execute(&mut **conn)
                .await?;
        }

        Ok(())
    }

    // ── Dependency resolution ────────────────────────────────────────

    /// After a task completes, check if any blocked tasks are now unblocked.
    /// Removes the satisfied edge and transitions blocked tasks to `pending`
    /// when all their dependencies are met.
    ///
    /// Returns IDs of newly-unblocked tasks (for event emission).
    pub async fn resolve_dependents(&self, completed_task_id: i64) -> Result<Vec<i64>, StoreError> {
        let mut conn = self.begin_write().await?;

        // Find tasks that depend on the completed task.
        let dependent_ids: Vec<(i64,)> =
            sqlx::query_as("SELECT task_id FROM task_deps WHERE depends_on_id = ?")
                .bind(completed_task_id)
                .fetch_all(&mut *conn)
                .await?;

        // Remove the satisfied edges.
        sqlx::query("DELETE FROM task_deps WHERE depends_on_id = ?")
            .bind(completed_task_id)
            .execute(&mut *conn)
            .await?;

        let mut unblocked = Vec::new();

        for (dep_id,) in dependent_ids {
            // Check if this dependent has any remaining unresolved deps.
            let (remaining,): (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM task_deps WHERE task_id = ?")
                    .bind(dep_id)
                    .fetch_one(&mut *conn)
                    .await?;

            if remaining == 0 {
                // All deps satisfied — unblock.
                let result = sqlx::query(
                    "UPDATE tasks SET status = 'pending' WHERE id = ? AND status = 'blocked'",
                )
                .bind(dep_id)
                .execute(&mut *conn)
                .await?;
                if result.rows_affected() > 0 {
                    unblocked.push(dep_id);
                }
            }
        }

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        Ok(unblocked)
    }

    /// After a task permanently fails, propagate failure to blocked dependents.
    ///
    /// For each dependent:
    /// - `Cancel`/`Fail` policy: move to history as `DependencyFailed` and
    ///   recursively cascade to that task's own dependents.
    /// - `Ignore` policy: remove the failed edge; if no remaining deps, unblock.
    ///
    /// Returns `(dependency_failed_ids, unblocked_ids)`.
    pub async fn fail_dependents(
        &self,
        failed_task_id: i64,
    ) -> Result<(Vec<i64>, Vec<i64>), StoreError> {
        let mut conn = self.begin_write().await?;
        let (failed, unblocked) = Self::fail_dependents_inner(&mut conn, failed_task_id).await?;
        sqlx::query("COMMIT").execute(&mut *conn).await?;
        Ok((failed, unblocked))
    }

    /// Inner recursive implementation of `fail_dependents`.
    // The return type cannot be simplified: Rust lacks native recursive async,
    // so we box the future manually, and the lifetime `'a` prevents a type alias.
    #[allow(clippy::type_complexity)]
    fn fail_dependents_inner<'a>(
        conn: &'a mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
        failed_task_id: i64,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(Vec<i64>, Vec<i64>), StoreError>> + Send + 'a>,
    > {
        Box::pin(async move {
            let dependent_rows: Vec<(i64,)> =
                sqlx::query_as("SELECT task_id FROM task_deps WHERE depends_on_id = ?")
                    .bind(failed_task_id)
                    .fetch_all(&mut **conn)
                    .await?;

            // Clean up edges from the failed task.
            sqlx::query("DELETE FROM task_deps WHERE depends_on_id = ?")
                .bind(failed_task_id)
                .execute(&mut **conn)
                .await?;

            let mut all_failed = Vec::new();
            let mut all_unblocked = Vec::new();

            for (dep_id,) in dependent_rows {
                // Read the dependent's failure policy.
                let policy_row: Option<(String,)> =
                    sqlx::query_as("SELECT on_dep_failure FROM tasks WHERE id = ?")
                        .bind(dep_id)
                        .fetch_optional(&mut **conn)
                        .await?;

                let policy: DependencyFailurePolicy = policy_row
                    .as_ref()
                    .map(|(s,)| s.parse().unwrap_or(DependencyFailurePolicy::Cancel))
                    .unwrap_or(DependencyFailurePolicy::Cancel);

                match policy {
                    DependencyFailurePolicy::Cancel | DependencyFailurePolicy::Fail => {
                        // Move to history as DependencyFailed.
                        let row = sqlx::query("SELECT * FROM tasks WHERE id = ?")
                            .bind(dep_id)
                            .fetch_optional(&mut **conn)
                            .await?;

                        if let Some(row) = row {
                            let task = row_to_task_record(&row);
                            insert_history(
                                conn,
                                &task,
                                "dependency_failed",
                                &IoBudget::default(),
                                None,
                                Some(&format!("dependency task {} failed", failed_task_id)),
                            )
                            .await?;

                            // Clean up this task's own dep edges (as a dependent).
                            sqlx::query("DELETE FROM task_deps WHERE task_id = ?")
                                .bind(dep_id)
                                .execute(&mut **conn)
                                .await?;

                            super::delete_task_tags(conn, dep_id).await?;
                            sqlx::query("DELETE FROM tasks WHERE id = ?")
                                .bind(dep_id)
                                .execute(&mut **conn)
                                .await?;

                            all_failed.push(dep_id);

                            // Recursively cascade to this task's own dependents.
                            let (sub_failed, sub_unblocked) =
                                Self::fail_dependents_inner(conn, dep_id).await?;
                            all_failed.extend(sub_failed);
                            all_unblocked.extend(sub_unblocked);
                        }
                    }
                    DependencyFailurePolicy::Ignore => {
                        // Remove the failed edge; check if remaining deps are satisfied.
                        let (remaining,): (i64,) =
                            sqlx::query_as("SELECT COUNT(*) FROM task_deps WHERE task_id = ?")
                                .bind(dep_id)
                                .fetch_one(&mut **conn)
                                .await?;

                        if remaining == 0 {
                            let result = sqlx::query(
                            "UPDATE tasks SET status = 'pending' WHERE id = ? AND status = 'blocked'",
                        )
                        .bind(dep_id)
                        .execute(&mut **conn)
                        .await?;
                            if result.rows_affected() > 0 {
                                all_unblocked.push(dep_id);
                            }
                        }
                    }
                }
            }

            Ok((all_failed, all_unblocked))
        })
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
    /// Also cleans up dependency edges and cascades failure to dependents.
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

        // Clean up edges where this task depends on others (task_id side).
        // Do NOT clean up depends_on_id side — fail_dependents needs those.
        sqlx::query("DELETE FROM task_deps WHERE task_id = ?")
            .bind(id)
            .execute(&mut *conn)
            .await?;

        super::delete_task_tags(&mut conn, id).await?;
        sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(id)
            .execute(&mut *conn)
            .await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        drop(conn); // Release connection before acquiring another in fail_dependents.

        // Cascade failure to dependents (treat cancellation as failure).
        // This runs in a separate transaction since fail_dependents begins its own.
        let _ = self.fail_dependents(id).await;

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

        // Clean up edges where this task depends on others (task_id side).
        sqlx::query("DELETE FROM task_deps WHERE task_id = ?")
            .bind(task.id)
            .execute(&mut *conn)
            .await?;

        super::delete_task_tags(&mut conn, task.id).await?;
        sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(task.id)
            .execute(&mut *conn)
            .await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        drop(conn); // Release connection before acquiring another in fail_dependents.

        // Cascade failure to dependents.
        let _ = self.fail_dependents(task.id).await;

        Ok(())
    }

    /// Sweep for expired tasks and move them to history.
    ///
    /// Finds tasks whose `expires_at` has passed and that are still pending
    /// or paused, records them in history as "expired", cascade-expires their
    /// pending/paused children, and deletes them from the active queue.
    ///
    /// Returns the expired task records (for event emission).
    pub async fn expire_tasks(&self) -> Result<Vec<TaskRecord>, StoreError> {
        let mut conn = self.begin_write().await?;

        // Find expired tasks (including blocked tasks — TTL ticks normally).
        let rows = sqlx::query(
            "SELECT * FROM tasks
             WHERE expires_at IS NOT NULL
               AND expires_at <= datetime('now')
               AND status IN ('pending', 'paused', 'blocked')
             ORDER BY expires_at ASC
             LIMIT 500",
        )
        .fetch_all(&mut *conn)
        .await?;

        let mut expired = Vec::with_capacity(rows.len());

        for row in &rows {
            let mut task = row_to_task_record(row);
            task.tags = super::load_task_tags(&mut conn, task.id).await?;

            // Record in history as expired.
            insert_history(
                &mut conn,
                &task,
                "expired",
                &IoBudget::default(),
                None,
                None,
            )
            .await?;

            // Cascade: expire pending/paused children.
            let child_rows = sqlx::query(
                "SELECT * FROM tasks
                 WHERE parent_id = ? AND status IN ('pending', 'paused')",
            )
            .bind(task.id)
            .fetch_all(&mut *conn)
            .await?;

            for child_row in &child_rows {
                let mut child = row_to_task_record(child_row);
                child.tags = super::load_task_tags(&mut conn, child.id).await?;
                insert_history(
                    &mut conn,
                    &child,
                    "expired",
                    &IoBudget::default(),
                    None,
                    None,
                )
                .await?;
                super::delete_task_tags(&mut conn, child.id).await?;
                sqlx::query("DELETE FROM tasks WHERE id = ?")
                    .bind(child.id)
                    .execute(&mut *conn)
                    .await?;
                expired.push(child);
            }

            // Clean up edges where this task depends on others (task_id side).
            // Don't clean depends_on_id side — fail_dependents needs those.
            sqlx::query("DELETE FROM task_deps WHERE task_id = ?")
                .bind(task.id)
                .execute(&mut *conn)
                .await?;

            // Delete the expired task itself.
            super::delete_task_tags(&mut conn, task.id).await?;
            sqlx::query("DELETE FROM tasks WHERE id = ?")
                .bind(task.id)
                .execute(&mut *conn)
                .await?;

            expired.push(task);
        }

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        drop(conn); // Release connection before acquiring another in fail_dependents.

        // Cascade failure to dependents of expired tasks (outside transaction).
        for task in &expired {
            let _ = self.fail_dependents(task.id).await;
        }

        Ok(expired)
    }

    /// Expire a single task by ID if it has passed its `expires_at`.
    ///
    /// Returns `Some(task)` if the task was expired, `None` if it wasn't
    /// found, not expired, or not in an expirable state.
    pub async fn expire_single(&self, id: i64) -> Result<Option<TaskRecord>, StoreError> {
        let mut conn = self.begin_write().await?;

        let row = sqlx::query(
            "SELECT * FROM tasks
             WHERE id = ?
               AND expires_at IS NOT NULL
               AND expires_at <= datetime('now')
               AND status IN ('pending', 'paused')",
        )
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;

        let Some(row) = row else {
            sqlx::query("COMMIT").execute(&mut *conn).await?;
            return Ok(None);
        };

        let mut task = row_to_task_record(&row);
        task.tags = super::load_task_tags(&mut conn, task.id).await?;

        insert_history(
            &mut conn,
            &task,
            "expired",
            &IoBudget::default(),
            None,
            None,
        )
        .await?;

        super::delete_task_tags(&mut conn, task.id).await?;
        sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(task.id)
            .execute(&mut *conn)
            .await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        Ok(Some(task))
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
