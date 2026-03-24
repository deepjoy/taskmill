//! Dependency resolution after task completion or failure.

use crate::store::row_mapping::row_to_task_record;
use crate::store::{StoreError, TaskStore};
use crate::task::{DependencyFailurePolicy, IoBudget};

use super::lifecycle::{insert_history, HistoryStatus};

impl TaskStore {
    /// After a task completes, check if any blocked tasks are now unblocked.
    /// Removes the satisfied edge and transitions blocked tasks to `pending`
    /// when all their dependencies are met.
    ///
    /// Returns IDs of newly-unblocked tasks (for event emission).
    pub async fn resolve_dependents(&self, completed_task_id: i64) -> Result<Vec<i64>, StoreError> {
        let mut conn = self.begin_write().await?;
        let unblocked = Self::resolve_dependents_inner(&mut conn, completed_task_id).await?;
        sqlx::query("COMMIT").execute(&mut *conn).await?;
        Ok(unblocked)
    }

    /// Inner dependency resolution that runs within an existing transaction.
    ///
    /// Uses `DELETE ... RETURNING` to combine edge lookup + deletion into a
    /// single query, then a single batched `UPDATE ... RETURNING` to unblock
    /// all dependents whose remaining deps are now zero.
    pub(crate) async fn resolve_dependents_inner(
        conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
        completed_task_id: i64,
    ) -> Result<Vec<i64>, StoreError> {
        // Step 1: Delete satisfied edges and collect affected task IDs.
        let dependent_ids: Vec<(i64,)> =
            sqlx::query_as("DELETE FROM task_deps WHERE depends_on_id = ? RETURNING task_id")
                .bind(completed_task_id)
                .fetch_all(&mut **conn)
                .await?;

        if dependent_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Step 2: Unblock tasks with zero remaining deps in one UPDATE.
        let placeholders = dependent_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "UPDATE tasks SET status = 'pending'
             WHERE status = 'blocked'
               AND id IN ({placeholders})
               AND NOT EXISTS (SELECT 1 FROM task_deps WHERE task_deps.task_id = tasks.id)
             RETURNING id"
        );
        let mut q = sqlx::query_as::<_, (i64,)>(&sql);
        for (dep_id,) in &dependent_ids {
            q = q.bind(dep_id);
        }
        let unblocked: Vec<(i64,)> = q.fetch_all(&mut **conn).await?;

        // Step 3: If any newly-unblocked task's group is paused, downgrade to
        // paused with the GROUP reason bit instead of leaving it as pending.
        for (task_id,) in &unblocked {
            sqlx::query(
                "UPDATE tasks SET status = 'paused',
                                  pause_reasons = pause_reasons | 8
                 WHERE id = ? AND status = 'pending'
                   AND group_key IN (SELECT group_key FROM paused_groups)",
            )
            .bind(task_id)
            .execute(&mut **conn)
            .await?;
        }

        Ok(unblocked.into_iter().map(|(id,)| id).collect())
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
            // Delete edges from the failed task and collect affected task IDs.
            let dependent_rows: Vec<(i64,)> =
                sqlx::query_as("DELETE FROM task_deps WHERE depends_on_id = ? RETURNING task_id")
                    .bind(failed_task_id)
                    .fetch_all(&mut **conn)
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
                                HistoryStatus::DependencyFailed,
                                &IoBudget::default(),
                                None,
                                Some(&format!("dependency task {} failed", failed_task_id)),
                                false,
                            )
                            .await?;

                            // Clean up this task's own dep edges (as a dependent).
                            sqlx::query("DELETE FROM task_deps WHERE task_id = ?")
                                .bind(dep_id)
                                .execute(&mut **conn)
                                .await?;

                            crate::store::delete_task_tags(conn, dep_id).await?;
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
                                // If the task's group is paused, downgrade to paused.
                                sqlx::query(
                                    "UPDATE tasks SET status = 'paused',
                                                      pause_reasons = pause_reasons | 8
                                     WHERE id = ? AND status = 'pending'
                                       AND group_key IN (SELECT group_key FROM paused_groups)",
                                )
                                .bind(dep_id)
                                .execute(&mut **conn)
                                .await?;
                                all_unblocked.push(dep_id);
                            }
                        }
                    }
                }
            }

            Ok((all_failed, all_unblocked))
        })
    }
}
