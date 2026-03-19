//! Core state machine transitions: pop, complete, fail.
//!
//! Grouping these in one file makes the state machine visible at a glance:
//!
//! ```text
//! pending  → running      (pop_next / pop_by_id)
//! running  → pending      (requeue — backpressure rejection)
//! running  → completed    (complete — moved to history)
//! running  → failed       (fail, non-retryable — moved to history)
//! running  → dead_letter  (fail, retries exhausted — moved to history)
//! running  → pending      (fail, retryable — requeued with backoff)
//! ```
//!
//! The `waiting` transition lives in `hierarchy.rs`, and pause/resume/cancel/expire
//! live in `cancel_expire.rs`.

use crate::store::row_mapping::row_to_task_record;
use crate::store::{StoreError, TaskStore};
use crate::task::{BackoffStrategy, IoBudget, TaskRecord};

use super::{compute_duration_ms, insert_history, HistoryStatus};

// ── Pop / Peek / Requeue ────────────────────────────────────────────

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

    /// Atomically claim a pending task by id, returning `true` if claimed.
    ///
    /// The caller is expected to already hold the full [`TaskRecord`] from a
    /// prior `peek_next` and will update its in-memory fields directly. This
    /// avoids the `RETURNING *` round-trip of [`pop_by_id`](Self::pop_by_id).
    pub(crate) async fn claim_task(&self, id: i64) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE tasks SET
                status = 'running',
                started_at = datetime('now'),
                expires_at = CASE
                    WHEN ttl_from = 'first_attempt' AND ttl_seconds IS NOT NULL AND expires_at IS NULL
                    THEN datetime('now', '+' || ttl_seconds || ' seconds')
                    ELSE expires_at
                END
             WHERE id = ? AND status = 'pending'",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
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

// ── Complete ────────────────────────────────────────────────────────

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

    /// Mark a task as completed and resolve its dependents in a single transaction.
    ///
    /// Combines `complete_with_record` and `resolve_dependents` to avoid
    /// two separate `BEGIN IMMEDIATE` / `COMMIT` cycles.
    ///
    /// Returns `(recurring_info, unblocked_ids)`.
    pub async fn complete_with_record_and_resolve(
        &self,
        task: &crate::task::TaskRecord,
        metrics: &IoBudget,
    ) -> Result<(Option<(chrono::DateTime<chrono::Utc>, i64)>, Vec<i64>), StoreError> {
        tracing::debug!(task_id = task.id, "store.complete_and_resolve: BEGIN tx");
        let mut conn = self.begin_write().await?;

        let recurring_info = Self::complete_inner(&mut conn, task, metrics).await?;
        let unblocked = Self::resolve_dependents_inner(&mut conn, task.id).await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        drop(conn);
        tracing::debug!(task_id = task.id, "store.complete_and_resolve: COMMIT ok");

        self.maybe_prune().await;

        Ok((recurring_info, unblocked))
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
            HistoryStatus::Completed,
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

// ── Fail ────────────────────────────────────────────────────────────

/// Backoff parameters for retry delay computation.
///
/// Bundles the optional backoff strategy and executor-signaled override into a
/// single argument to keep `fail()` / `fail_with_record()` under the clippy
/// argument-count lint.
#[derive(Debug, Default, Clone)]
pub struct FailBackoff<'a> {
    /// Per-type backoff strategy. `None` means immediate retry.
    pub strategy: Option<&'a BackoffStrategy>,
    /// Executor-requested retry delay in milliseconds. Overrides the strategy
    /// when set.
    pub executor_retry_after_ms: Option<u64>,
}

impl TaskStore {
    /// Mark a task as failed. If `retryable` and under max retries, requeue
    /// it as pending with the same priority. Otherwise move to history as failed.
    ///
    /// `backoff` controls the delay before the next retry attempt. See
    /// `fail_inner` for details.
    pub async fn fail(
        &self,
        id: i64,
        error: &str,
        retryable: bool,
        max_retries: i32,
        metrics: &IoBudget,
        backoff: &FailBackoff<'_>,
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

        Self::fail_inner(
            &mut conn,
            &task,
            error,
            retryable,
            max_retries,
            metrics,
            backoff,
        )
        .await?;

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
        task: &crate::task::TaskRecord,
        error: &str,
        retryable: bool,
        max_retries: i32,
        metrics: &IoBudget,
        backoff: &FailBackoff<'_>,
    ) -> Result<(), StoreError> {
        tracing::debug!(task_id = task.id, "store.fail_with_record: BEGIN tx");
        let mut conn = self.begin_write().await?;
        tracing::debug!(task_id = task.id, "store.fail_with_record: BEGIN acquired");

        Self::fail_inner(
            &mut conn,
            task,
            error,
            retryable,
            max_retries,
            metrics,
            backoff,
        )
        .await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        drop(conn);
        tracing::debug!(task_id = task.id, "store.fail_with_record: COMMIT ok");

        self.maybe_prune().await;

        Ok(())
    }

    /// Shared failure logic: retry or move to history.
    ///
    /// When retrying, computes the backoff delay from (in priority order):
    /// 1. `executor_retry_after_ms` — executor-signaled override
    /// 2. `backoff` strategy — per-type backoff computation
    /// 3. Immediate retry (no delay) — backward-compatible default
    ///
    /// The delay is applied by setting `run_after` on the requeued task.
    async fn fail_inner(
        conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
        task: &crate::task::TaskRecord,
        error: &str,
        retryable: bool,
        max_retries: i32,
        metrics: &IoBudget,
        backoff: &FailBackoff<'_>,
    ) -> Result<(), StoreError> {
        if retryable && task.retry_count < max_retries {
            // Compute delay: executor override > backoff strategy > immediate.
            let delay = if let Some(ms) = backoff.executor_retry_after_ms {
                std::time::Duration::from_millis(ms)
            } else if let Some(strategy) = backoff.strategy {
                strategy.delay_for(task.retry_count)
            } else {
                std::time::Duration::ZERO
            };

            if delay.is_zero() {
                // Immediate retry — current behavior.
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
                // Delayed retry — set run_after.
                let run_after =
                    chrono::Utc::now() + chrono::Duration::milliseconds(delay.as_millis() as i64);
                let run_after_str = run_after.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
                sqlx::query(
                    "UPDATE tasks SET status = 'pending', started_at = NULL,
                        retry_count = retry_count + 1, last_error = ?,
                        run_after = ?
                     WHERE id = ?",
                )
                .bind(error)
                .bind(&run_after_str)
                .bind(task.id)
                .execute(&mut **conn)
                .await?;
            }
        } else {
            // Terminal failure — move to history.
            // Distinguish: retryable + exhausted → dead_letter; non-retryable → failed.
            let status = if retryable {
                HistoryStatus::DeadLetter
            } else {
                HistoryStatus::Failed
            };
            let duration_ms = compute_duration_ms(task);

            insert_history(conn, task, status, metrics, duration_ms, Some(error)).await?;

            crate::store::delete_task_tags(conn, task.id).await?;
            sqlx::query("DELETE FROM tasks WHERE id = ?")
                .bind(task.id)
                .execute(&mut **conn)
                .await?;
        }

        Ok(())
    }
}
