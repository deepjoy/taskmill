//! Pause, resume, cancellation, and TTL expiry.
//!
//! Both cancellation (user-initiated) and expiry (time-driven) share the same
//! terminal transition pattern: record in history → clean up edges/tags → delete.
//! They are kept together because of this shared structure.

use crate::store::row_mapping::row_to_task_record;
use crate::store::{StoreError, TaskStore};
use crate::task::{IoBudget, PauseReasons, TaskRecord};

use super::{compute_duration_ms, insert_history, HistoryStatus};

// ── Pause / Resume ──────────────────────────────────────────────────

impl TaskStore {
    /// Pause a running task. ORs the reason bit into `pause_reasons`.
    pub async fn pause(&self, id: i64, reason: PauseReasons) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE tasks SET status = 'paused', started_at = NULL,
                              pause_reasons = pause_reasons | ?
             WHERE id = ?",
        )
        .bind(reason.bits())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Resume a paused task back to pending. Clears all pause reasons.
    pub async fn resume(&self, id: i64) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE tasks SET status = 'pending', pause_reasons = 0
             WHERE id = ? AND status = 'paused'",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Return paused tasks that have the PREEMPTION bit set (for auto-resume).
    pub async fn preemption_paused_tasks(&self) -> Result<Vec<TaskRecord>, StoreError> {
        let rows = sqlx::query(
            "SELECT * FROM tasks
             WHERE status = 'paused' AND (pause_reasons & 1) != 0
             ORDER BY priority ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// Clear the PREEMPTION bit from a paused task. If no other pause reasons
    /// remain, the task transitions back to pending.
    pub async fn resume_preempted(&self, id: i64) -> Result<(), StoreError> {
        // Fully resume if PREEMPTION is the only reason.
        let result = sqlx::query(
            "UPDATE tasks SET status = 'pending', pause_reasons = 0
             WHERE id = ? AND status = 'paused' AND pause_reasons = 1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            // Clear PREEMPTION bit but stay paused (other reasons remain).
            sqlx::query(
                "UPDATE tasks SET pause_reasons = pause_reasons & ~1
                 WHERE id = ? AND status = 'paused' AND (pause_reasons & 1) != 0",
            )
            .bind(id)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Clear a specific pause-reason bit from all paused tasks. Tasks whose
    /// `pause_reasons` becomes 0 transition back to pending.
    /// Returns the count of tasks that fully resumed.
    pub async fn clear_pause_bit(&self, reason: PauseReasons) -> Result<u64, StoreError> {
        let bit = reason.bits();

        // Fully resume tasks where this is the sole reason.
        let fully_resumed = sqlx::query(
            "UPDATE tasks SET status = 'pending', pause_reasons = 0
             WHERE status = 'paused' AND pause_reasons = ?",
        )
        .bind(bit)
        .execute(&self.pool)
        .await?
        .rows_affected();

        // Clear the bit from multi-reason tasks (stays paused).
        sqlx::query(
            "UPDATE tasks SET pause_reasons = pause_reasons & ~?
             WHERE status = 'paused' AND (pause_reasons & ?) != 0",
        )
        .bind(bit)
        .bind(bit)
        .execute(&self.pool)
        .await?;

        Ok(fully_resumed)
    }

    // ── Group Pause State ──────────────────────────────────────────

    /// Record a group as paused. Returns `true` if the group was newly paused
    /// (not already paused — idempotent).
    pub async fn pause_group_state(
        &self,
        group_key: &str,
        resume_at: Option<i64>, // epoch milliseconds
    ) -> Result<bool, StoreError> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let result = sqlx::query(
            "INSERT OR IGNORE INTO paused_groups (group_key, paused_at, resume_at) VALUES (?, ?, ?)",
        )
        .bind(group_key)
        .bind(now_ms)
        .bind(resume_at)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Remove a group from the paused state. Returns `true` if the group was
    /// actually paused (idempotent).
    pub async fn resume_group_state(&self, group_key: &str) -> Result<bool, StoreError> {
        let result = sqlx::query("DELETE FROM paused_groups WHERE group_key = ?")
            .bind(group_key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List all currently paused groups with their pause timestamps.
    pub async fn paused_groups(&self) -> Result<Vec<(String, i64)>, StoreError> {
        let rows: Vec<(String, i64)> =
            sqlx::query_as("SELECT group_key, paused_at FROM paused_groups ORDER BY paused_at")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    /// Check if a specific group is currently paused.
    pub async fn is_group_paused(&self, group_key: &str) -> Result<bool, StoreError> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM paused_groups WHERE group_key = ?")
            .bind(group_key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    /// Find groups whose `resume_at` has elapsed (for auto-resume).
    pub async fn groups_due_for_resume(&self) -> Result<Vec<String>, StoreError> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT group_key FROM paused_groups
             WHERE resume_at IS NOT NULL AND resume_at <= ?",
        )
        .bind(now_ms)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(k,)| k).collect())
    }

    // ── Bulk pause / resume by group ──────────────────────────────

    /// Pause tasks in a group. ORs the GROUP bit into all pending and
    /// already-paused tasks in the group:
    /// - Pending tasks → paused with GROUP bit.
    /// - Already-paused tasks → GROUP bit added (prevents premature resume
    ///   by other mechanisms clearing their own bit).
    ///
    /// Returns the count of newly paused tasks (status changed from pending).
    pub async fn pause_tasks_in_group(&self, group_key: &str) -> Result<u64, StoreError> {
        // Add GROUP bit to already-paused tasks (no status change, just adds the bit).
        sqlx::query(
            "UPDATE tasks SET pause_reasons = pause_reasons | 8
             WHERE group_key = ? AND status = 'paused' AND (pause_reasons & 8) = 0",
        )
        .bind(group_key)
        .execute(&self.pool)
        .await?;

        // Pause pending tasks with GROUP bit.
        let result = sqlx::query(
            "UPDATE tasks SET status = 'paused',
                              pause_reasons = pause_reasons | 8
             WHERE group_key = ? AND status = 'pending'",
        )
        .bind(group_key)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Resume group-paused tasks. Two-step process:
    ///
    /// 1. **Fully resume** tasks where GROUP is the sole reason (pause_reasons = 8).
    /// 2. **Clear GROUP bit** from multi-reason tasks (they stay paused under
    ///    their remaining reasons).
    ///
    /// Returns the count of tasks that fully resumed (became pending).
    pub async fn resume_paused_by_group(&self, group_key: &str) -> Result<u64, StoreError> {
        // 1. Fully resume tasks where GROUP is the sole reason.
        let fully_resumed = sqlx::query(
            "UPDATE tasks SET status = 'pending', pause_reasons = 0
             WHERE group_key = ? AND status = 'paused' AND pause_reasons = 8",
        )
        .bind(group_key)
        .execute(&self.pool)
        .await?
        .rows_affected();

        // 2. Clear GROUP bit from multi-reason tasks (stays paused).
        sqlx::query(
            "UPDATE tasks SET pause_reasons = pause_reasons & ~8
             WHERE group_key = ? AND status = 'paused' AND (pause_reasons & 8) != 0",
        )
        .bind(group_key)
        .execute(&self.pool)
        .await?;

        Ok(fully_resumed)
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
            false,
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
            false,
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

    /// Pause all pending tasks whose `task_type` starts with `prefix` (module pause).
    ///
    /// ORs the MODULE bit. Tasks already paused for other reasons get the
    /// additional MODULE bit.
    /// Returns the number of tasks whose status changed to paused.
    pub async fn pause_pending_by_type_prefix(&self, prefix: &str) -> Result<u64, StoreError> {
        let pattern = format!("{prefix}%");
        let result = sqlx::query(
            "UPDATE tasks SET status = 'paused',
                              pause_reasons = pause_reasons | 2
             WHERE task_type LIKE ? AND status IN ('pending', 'paused')",
        )
        .bind(&pattern)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Resume module-paused tasks by type prefix. Clears the MODULE bit.
    /// Tasks fully resume (become pending) only if no other pause reasons remain.
    /// Returns the count of tasks that fully resumed.
    pub async fn resume_paused_by_type_prefix(&self, prefix: &str) -> Result<u64, StoreError> {
        let pattern = format!("{prefix}%");

        // Fully resume tasks where MODULE is the only reason.
        let fully_resumed = sqlx::query(
            "UPDATE tasks SET status = 'pending', pause_reasons = 0
             WHERE task_type LIKE ? AND status = 'paused' AND pause_reasons = 2",
        )
        .bind(&pattern)
        .execute(&self.pool)
        .await?
        .rows_affected();

        // Clear MODULE bit from tasks with other active reasons (stays paused).
        sqlx::query(
            "UPDATE tasks SET pause_reasons = pause_reasons & ~2
             WHERE task_type LIKE ? AND status = 'paused' AND (pause_reasons & 2) != 0",
        )
        .bind(&pattern)
        .execute(&self.pool)
        .await?;

        Ok(fully_resumed)
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

        let now_ms = chrono::Utc::now().timestamp_millis();

        // Find expired tasks (including blocked tasks — TTL ticks normally).
        let rows = sqlx::query(
            "SELECT * FROM tasks
             WHERE expires_at IS NOT NULL
               AND expires_at <= ?
               AND status IN ('pending', 'paused', 'blocked')
             ORDER BY expires_at ASC
             LIMIT 500",
        )
        .bind(now_ms)
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
                false,
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
                    false,
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

        let now_ms = chrono::Utc::now().timestamp_millis();
        let row = sqlx::query(
            "SELECT * FROM tasks
             WHERE id = ?
               AND expires_at IS NOT NULL
               AND expires_at <= ?
               AND status IN ('pending', 'paused')",
        )
        .bind(id)
        .bind(now_ms)
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
            false,
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
