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
        let now_ms = chrono::Utc::now().timestamp_millis();
        let row = sqlx::query(
            "SELECT * FROM tasks
             WHERE id = (
                 SELECT id FROM tasks
                 WHERE status = 'pending'
                   AND (run_after IS NULL OR run_after <= ?)
                 ORDER BY priority ASC, id ASC
                 LIMIT 1
             )",
        )
        .bind(now_ms)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(row_to_task_record))
    }

    /// Atomically claim a specific pending task by id, setting it to running.
    /// Returns `None` if the task is no longer pending (e.g. claimed by another
    /// dispatcher or cancelled).
    ///
    /// For tasks with `ttl_from = 'first_attempt'`, sets `expires_at` on the
    /// first pop (when `expires_at IS NULL` and `ttl_seconds IS NOT NULL`).
    pub async fn pop_by_id(&self, id: i64) -> Result<Option<TaskRecord>, StoreError> {
        tracing::debug!(task_id = id, "store.pop_by_id: UPDATE start");
        let now_ms = chrono::Utc::now().timestamp_millis();
        let row = sqlx::query(
            "UPDATE tasks SET
                status = 'running',
                started_at = ?,
                expires_at = CASE
                    WHEN ttl_from = 'first_attempt' AND ttl_seconds IS NOT NULL AND expires_at IS NULL
                    THEN ? + (ttl_seconds * 1000)
                    ELSE expires_at
                END
             WHERE id = ? AND status = 'pending'
             RETURNING *",
        )
        .bind(now_ms)
        .bind(now_ms)
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
        let now_ms = chrono::Utc::now().timestamp_millis();
        let result = sqlx::query(
            "UPDATE tasks SET
                status = 'running',
                started_at = ?,
                expires_at = CASE
                    WHEN ttl_from = 'first_attempt' AND ttl_seconds IS NOT NULL AND expires_at IS NULL
                    THEN ? + (ttl_seconds * 1000)
                    ELSE expires_at
                END
             WHERE id = ? AND status = 'pending'",
        )
        .bind(now_ms)
        .bind(now_ms)
        .bind(id)
        .execute(&self.pool)
        .await?;
        let claimed = result.rows_affected() > 0;
        if claimed {
            self.has_running
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(claimed)
    }

    /// Atomically claim up to `limit` highest-priority pending tasks and mark
    /// them as running. Returns an empty vec if no work is available. This
    /// avoids the N sequential `pop_next()` round-trips when filling
    /// concurrency slots.
    pub async fn pop_next_batch(&self, limit: usize) -> Result<Vec<TaskRecord>, StoreError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        if limit == 1 {
            return Ok(self.pop_next().await?.into_iter().collect());
        }

        let now_ms = chrono::Utc::now().timestamp_millis();
        let rows = sqlx::query(
            "UPDATE tasks SET
                status = 'running',
                started_at = ?,
                expires_at = CASE
                    WHEN ttl_from = 'first_attempt' AND ttl_seconds IS NOT NULL AND expires_at IS NULL
                    THEN ? + (ttl_seconds * 1000)
                    ELSE expires_at
                END
             WHERE id IN (
                 SELECT id FROM tasks
                 WHERE status = 'pending'
                   AND (run_after IS NULL OR run_after <= ?)
                   AND (expires_at IS NULL OR expires_at > ?)
                 ORDER BY priority ASC, id ASC
                 LIMIT ?
             )
             RETURNING *",
        )
        .bind(now_ms)
        .bind(now_ms)
        .bind(now_ms)
        .bind(now_ms)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        if !records.is_empty() {
            self.has_running
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(records)
    }

    /// Pop the highest-priority pending task and mark it as running.
    /// Returns `None` if the queue is empty. Tasks with a future `run_after`
    /// timestamp are excluded.
    ///
    /// For tasks with `ttl_from = 'first_attempt'`, sets `expires_at` on
    /// the first pop.
    ///
    /// Tags are **not** populated — callers needing tags should call
    /// [`populate_tags`](Self::populate_tags) explicitly or use
    /// [`task_by_id`](Self::task_by_id).
    pub async fn pop_next(&self) -> Result<Option<TaskRecord>, StoreError> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let row = sqlx::query(
            "UPDATE tasks SET
                status = 'running',
                started_at = ?,
                expires_at = CASE
                    WHEN ttl_from = 'first_attempt' AND ttl_seconds IS NOT NULL AND expires_at IS NULL
                    THEN ? + (ttl_seconds * 1000)
                    ELSE expires_at
                END
             WHERE id = (
                 SELECT id FROM tasks
                 WHERE status = 'pending'
                   AND (run_after IS NULL OR run_after <= ?)
                   AND (expires_at IS NULL OR expires_at > ?)
                 ORDER BY priority ASC, id ASC
                 LIMIT 1
             )
             RETURNING *",
        )
        .bind(now_ms)
        .bind(now_ms)
        .bind(now_ms)
        .bind(now_ms)
        .fetch_optional(&self.pool)
        .await?;

        let record = row.map(|r| row_to_task_record(&r));
        if record.is_some() {
            self.has_running
                .store(true, std::sync::atomic::Ordering::Relaxed);
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

        let skip_tags = !self.has_tags.load(std::sync::atomic::Ordering::Relaxed);
        let _recurring = Self::complete_inner(&mut conn, &task, metrics, skip_tags).await?;

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
        let skip_tags = !self.has_tags.load(std::sync::atomic::Ordering::Relaxed);

        let recurring_info = Self::complete_inner(&mut conn, task, metrics, skip_tags).await?;

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
        let skip_tags = !self.has_tags.load(std::sync::atomic::Ordering::Relaxed);

        let recurring_info = Self::complete_inner(&mut conn, task, metrics, skip_tags).await?;
        let unblocked = Self::resolve_dependents_inner(&mut conn, task.id).await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        drop(conn);
        tracing::debug!(task_id = task.id, "store.complete_and_resolve: COMMIT ok");

        self.maybe_prune().await;

        Ok((recurring_info, unblocked))
    }

    /// Complete a batch of tasks and resolve their dependents in a single transaction.
    ///
    /// Coalesces N individual `complete_with_record_and_resolve` calls into one
    /// `BEGIN IMMEDIATE` / `COMMIT` cycle, amortizing SQLite's WAL sync overhead.
    ///
    /// When none of the tasks in the batch are recurring, an optimised path
    /// batches the DELETE and dependency-resolution SQL into single statements
    /// instead of looping per-task. This reduces the SQL round-trip count from
    /// ~5N to N+4 (N history inserts + 3-4 batched statements).
    ///
    /// Returns a vec of `(task_id, recurring_info, unblocked_ids)` per item.
    pub async fn complete_batch_with_resolve(
        &self,
        items: &[(&crate::task::TaskRecord, &IoBudget)],
    ) -> Result<Vec<(i64, Option<(chrono::DateTime<chrono::Utc>, i64)>, Vec<i64>)>, StoreError>
    {
        if items.is_empty() {
            return Ok(Vec::new());
        }

        // Single-item fast path: avoid the batch overhead.
        if items.len() == 1 {
            let (task, metrics) = items[0];
            let (recurring, unblocked) =
                self.complete_with_record_and_resolve(task, metrics).await?;
            return Ok(vec![(task.id, recurring, unblocked)]);
        }

        let skip_tags = !self.has_tags.load(std::sync::atomic::Ordering::Relaxed);
        let any_recurring = items
            .iter()
            .any(|(t, _)| t.recurring_interval_secs.is_some());

        // Recurring tasks need per-task handling (tag reads, pile-up prevention,
        // next-instance creation). Fall back to the per-item path.
        if any_recurring {
            tracing::debug!(
                count = items.len(),
                "store.complete_batch (per-item): BEGIN tx"
            );
            let mut conn = self.begin_write().await?;

            let mut results = Vec::with_capacity(items.len());
            for (task, metrics) in items {
                let recurring = Self::complete_inner(&mut conn, task, metrics, skip_tags).await?;
                let unblocked = Self::resolve_dependents_inner(&mut conn, task.id).await?;
                results.push((task.id, recurring, unblocked));
            }

            sqlx::query("COMMIT").execute(&mut *conn).await?;
            drop(conn);
            self.maybe_prune().await;
            return Ok(results);
        }

        // ── Batched fast path (no recurring tasks) ───────────────────

        tracing::debug!(count = items.len(), "store.complete_batch: BEGIN tx");
        let mut conn = self.begin_write().await?;

        // Step 1: Insert history rows (per-task — each has unique binds).
        for (task, metrics) in items {
            let duration_ms = compute_duration_ms(task);
            insert_history(
                &mut conn,
                task,
                HistoryStatus::Completed,
                metrics,
                duration_ms,
                task.last_error.as_deref(),
                skip_tags,
            )
            .await?;
        }

        // Step 2: Batch-delete completed tasks (requeue = 0).
        let ids: Vec<i64> = items.iter().map(|(t, _)| t.id).collect();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

        let sql =
            format!("DELETE FROM tasks WHERE id IN ({placeholders}) AND requeue = 0 RETURNING id");
        let mut q = sqlx::query_as::<_, (i64,)>(&sql);
        for id in &ids {
            q = q.bind(id);
        }
        let deleted: Vec<(i64,)> = q.fetch_all(&mut *conn).await?;
        let deleted_set: std::collections::HashSet<i64> = deleted.iter().map(|(id,)| *id).collect();

        // Step 3: Handle requeued tasks (not deleted because requeue = 1).
        for id in &ids {
            if !deleted_set.contains(id) {
                sqlx::query(
                    "UPDATE tasks SET status = 'pending',
                        priority = COALESCE(requeue_priority, priority),
                        started_at = NULL, retry_count = 0, last_error = NULL,
                        requeue = 0, requeue_priority = NULL
                     WHERE id = ?",
                )
                .bind(id)
                .execute(&mut *conn)
                .await?;
            }
        }

        // Step 4: Batch-delete orphaned tags for deleted tasks.
        if !skip_tags && !deleted_set.is_empty() {
            let del_ids: Vec<i64> = deleted_set.iter().copied().collect();
            let tag_placeholders = del_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let tag_sql = format!("DELETE FROM task_tags WHERE task_id IN ({tag_placeholders})");
            let mut tq = sqlx::query(&tag_sql);
            for id in &del_ids {
                tq = tq.bind(id);
            }
            tq.execute(&mut *conn).await?;
        }

        // Step 5: Batch-resolve dependency edges for all completed tasks.
        let dep_sql = format!(
            "DELETE FROM task_deps WHERE depends_on_id IN ({placeholders}) RETURNING task_id"
        );
        let mut dq = sqlx::query_as::<_, (i64,)>(&dep_sql);
        for id in &ids {
            dq = dq.bind(id);
        }
        let dependent_ids: Vec<(i64,)> = dq.fetch_all(&mut *conn).await?;

        let mut all_unblocked: Vec<i64> = Vec::new();
        if !dependent_ids.is_empty() {
            let dep_placeholders = dependent_ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");
            let unblock_sql = format!(
                "UPDATE tasks SET status = 'pending'
                 WHERE status = 'blocked'
                   AND id IN ({dep_placeholders})
                   AND NOT EXISTS (SELECT 1 FROM task_deps WHERE task_deps.task_id = tasks.id)
                 RETURNING id"
            );
            let mut uq = sqlx::query_as::<_, (i64,)>(&unblock_sql);
            for (dep_id,) in &dependent_ids {
                uq = uq.bind(dep_id);
            }
            let unblocked: Vec<(i64,)> = uq.fetch_all(&mut *conn).await?;
            all_unblocked = unblocked.into_iter().map(|(id,)| id).collect();
        }

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        drop(conn);
        tracing::debug!(count = items.len(), "store.complete_batch: COMMIT ok");

        // Prune once for the whole batch.
        self.maybe_prune().await;

        // Build per-task results. Non-recurring → no recurring_info.
        // Unblocked IDs are shared across the batch (not per-task).
        let mut results = Vec::with_capacity(items.len());
        for (i, (task, _)) in items.iter().enumerate() {
            // Attach unblocked list to the last item to avoid duplication.
            let unblocked = if i == items.len() - 1 {
                std::mem::take(&mut all_unblocked)
            } else {
                Vec::new()
            };
            results.push((task.id, None, unblocked));
        }

        Ok(results)
    }

    /// Shared completion logic: insert history, handle recurring next instance,
    /// then handle requeue or delete.
    ///
    /// `skip_tags` is forwarded to [`insert_history`] and also skips the
    /// `delete_task_tags` call when no tags are present in the store.
    ///
    /// Returns `Some((next_run, exec_count))` if a recurring next instance was
    /// created, `None` otherwise.
    async fn complete_inner(
        conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
        task: &crate::task::TaskRecord,
        metrics: &IoBudget,
        skip_tags: bool,
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
            skip_tags,
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
        if !skip_tags {
            crate::store::delete_task_tags(conn, task.id).await?;
        }

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
                        let now = chrono::Utc::now();
                        let next_run = now + chrono::Duration::seconds(interval);
                        let next_run_ms = next_run.timestamp_millis();
                        let now_ms = now.timestamp_millis();
                        let fail_fast_val: i32 = if task.fail_fast { 1 } else { 0 };

                        // Compute TTL columns for the next instance.
                        let expires_at_ms: Option<i64> = match (task.ttl_seconds, task.ttl_from) {
                            (Some(ttl_secs), crate::task::TtlFrom::Submission) => {
                                let exp = now + chrono::Duration::seconds(ttl_secs);
                                Some(exp.timestamp_millis())
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
                                recurring_paused, max_retries, created_at)
                             VALUES (?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
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
                        .bind(expires_at_ms)
                        .bind(next_run_ms)
                        .bind(task.recurring_interval_secs)
                        .bind(task.recurring_max_executions)
                        .bind(execution_count)
                        .bind(task.max_retries)
                        .bind(now_ms)
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

    /// Batch-process terminal failures in a single transaction.
    ///
    /// Each item is a `(task, error, retryable)` triple that has already been
    /// determined to be terminal (retries exhausted or non-retryable). The
    /// method inserts history rows and deletes active tasks in one
    /// `BEGIN IMMEDIATE` / `COMMIT` cycle, amortizing SQLite's WAL sync.
    pub async fn fail_batch(
        &self,
        items: &[(&crate::task::TaskRecord, &str, bool, &IoBudget)],
    ) -> Result<(), StoreError> {
        if items.is_empty() {
            return Ok(());
        }
        if items.len() == 1 {
            let (task, error, retryable, metrics) = items[0];
            return self
                .fail_with_record(task, error, retryable, 0, metrics, &FailBackoff::default())
                .await;
        }

        let skip_tags = !self.has_tags.load(std::sync::atomic::Ordering::Relaxed);

        tracing::debug!(count = items.len(), "store.fail_batch: BEGIN tx");
        let mut conn = self.begin_write().await?;

        // Step 1: Insert history rows (per-task — each has unique binds).
        for &(task, error, retryable, metrics) in items {
            let status = if retryable {
                HistoryStatus::DeadLetter
            } else {
                HistoryStatus::Failed
            };
            let duration_ms = compute_duration_ms(task);
            insert_history(
                &mut conn,
                task,
                status,
                metrics,
                duration_ms,
                Some(error),
                skip_tags,
            )
            .await?;
        }

        // Step 2: Batch-delete active task rows.
        let ids: Vec<i64> = items.iter().map(|(t, _, _, _)| t.id).collect();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("DELETE FROM tasks WHERE id IN ({placeholders})");
        let mut q = sqlx::query(&sql);
        for id in &ids {
            q = q.bind(id);
        }
        q.execute(&mut *conn).await?;

        // Step 3: Batch-delete orphaned tags.
        if !skip_tags {
            let tag_sql = format!("DELETE FROM task_tags WHERE task_id IN ({placeholders})");
            let mut tq = sqlx::query(&tag_sql);
            for id in &ids {
                tq = tq.bind(id);
            }
            tq.execute(&mut *conn).await?;
        }

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        drop(conn);
        tracing::debug!(count = items.len(), "store.fail_batch: COMMIT ok");

        self.maybe_prune().await;
        Ok(())
    }

    /// Increment retry count in-place without changing task status.
    ///
    /// Used by inline immediate retries: the task stays `running` and is
    /// re-executed directly in the same spawned future, avoiding the
    /// requeue-to-pending → pop_next round-trip through SQLite.
    pub async fn increment_retry(&self, task_id: i64, error: &str) -> Result<(), StoreError> {
        sqlx::query("UPDATE tasks SET retry_count = retry_count + 1, last_error = ? WHERE id = ?")
            .bind(error)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Requeue a task for retry without a transaction.
    ///
    /// The single UPDATE is atomically safe and avoids `BEGIN IMMEDIATE` +
    /// `COMMIT` round-trips, reducing connection hold time from 3 SQL
    /// executions to 1. This matters under concurrency because
    /// `open_memory()` uses a single-connection pool — shorter hold time
    /// lets dispatch (`pop_next_batch`) interleave sooner.
    pub async fn requeue_for_retry(
        &self,
        task_id: i64,
        error: &str,
        delay: std::time::Duration,
    ) -> Result<(), StoreError> {
        if delay.is_zero() {
            sqlx::query(
                "UPDATE tasks SET status = 'pending', started_at = NULL,
                    retry_count = retry_count + 1, last_error = ?
                 WHERE id = ?",
            )
            .bind(error)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        } else {
            let run_after_ms = (chrono::Utc::now()
                + chrono::Duration::milliseconds(delay.as_millis() as i64))
            .timestamp_millis();
            sqlx::query(
                "UPDATE tasks SET status = 'pending', started_at = NULL,
                    retry_count = retry_count + 1, last_error = ?,
                    run_after = ?
                 WHERE id = ?",
            )
            .bind(error)
            .bind(run_after_ms)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        }
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
                let run_after_ms = run_after.timestamp_millis();
                sqlx::query(
                    "UPDATE tasks SET status = 'pending', started_at = NULL,
                        retry_count = retry_count + 1, last_error = ?,
                        run_after = ?
                     WHERE id = ?",
                )
                .bind(error)
                .bind(run_after_ms)
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

            insert_history(conn, task, status, metrics, duration_ms, Some(error), false).await?;

            crate::store::delete_task_tags(conn, task.id).await?;
            sqlx::query("DELETE FROM tasks WHERE id = ?")
                .bind(task.id)
                .execute(&mut **conn)
                .await?;
        }

        Ok(())
    }
}
