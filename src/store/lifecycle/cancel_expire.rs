//! Pause, resume, cancellation, and TTL expiry.
//!
//! Both cancellation (user-initiated) and expiry (time-driven) share the same
//! terminal transition pattern: record in history → clean up edges/tags → delete.
//! They are kept together because of this shared structure.

use crate::store::row_mapping::row_to_task_record;
use crate::store::{StoreError, TaskStore};
use crate::task::{IoBudget, TaskRecord};

use super::{compute_duration_ms, insert_history, HistoryStatus};

// ── Pause / Resume ──────────────────────────────────────────────────

impl TaskStore {
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

    // ── Cancellation (user-initiated) ──────────────────────────────

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
            HistoryStatus::Cancelled,
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

        crate::store::delete_task_tags(&mut conn, id).await?;
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
            HistoryStatus::Cancelled,
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

        crate::store::delete_task_tags(&mut conn, task.id).await?;
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

    // ── Bulk pause / resume by type prefix ─────────────────────────

    /// Pause all pending tasks whose `task_type` starts with `prefix`.
    ///
    /// Updates their status from `pending` to `paused` in a single SQL statement.
    /// Returns the number of tasks paused.
    pub async fn pause_pending_by_type_prefix(&self, prefix: &str) -> Result<u64, StoreError> {
        let pattern = format!("{prefix}%");
        let result = sqlx::query(
            "UPDATE tasks SET status = 'paused' WHERE task_type LIKE ? AND status = 'pending'",
        )
        .bind(&pattern)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Resume all paused tasks whose `task_type` starts with `prefix`.
    ///
    /// Updates their status from `paused` to `pending` in a single SQL statement.
    /// Returns the number of tasks resumed.
    pub async fn resume_paused_by_type_prefix(&self, prefix: &str) -> Result<u64, StoreError> {
        let pattern = format!("{prefix}%");
        let result = sqlx::query(
            "UPDATE tasks SET status = 'pending' WHERE task_type LIKE ? AND status = 'paused'",
        )
        .bind(&pattern)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    // ── Expiry (time-driven) ───────────────────────────────────────

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
            task.tags = crate::store::load_task_tags(&mut conn, task.id).await?;

            // Record in history as expired.
            insert_history(
                &mut conn,
                &task,
                HistoryStatus::Expired,
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
                child.tags = crate::store::load_task_tags(&mut conn, child.id).await?;
                insert_history(
                    &mut conn,
                    &child,
                    HistoryStatus::Expired,
                    &IoBudget::default(),
                    None,
                    None,
                )
                .await?;
                crate::store::delete_task_tags(&mut conn, child.id).await?;
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
            crate::store::delete_task_tags(&mut conn, task.id).await?;
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
        task.tags = crate::store::load_task_tags(&mut conn, task.id).await?;

        insert_history(
            &mut conn,
            &task,
            HistoryStatus::Expired,
            &IoBudget::default(),
            None,
            None,
        )
        .await?;

        crate::store::delete_task_tags(&mut conn, task.id).await?;
        sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(task.id)
            .execute(&mut *conn)
            .await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        Ok(Some(task))
    }
}
