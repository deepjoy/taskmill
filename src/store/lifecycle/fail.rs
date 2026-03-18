//! Task failure: retry with backoff or move to history.

use crate::store::row_mapping::row_to_task_record;
use crate::store::{StoreError, TaskStore};
use crate::task::{BackoffStrategy, IoBudget};

use super::{compute_duration_ms, insert_history, HistoryStatus};

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
