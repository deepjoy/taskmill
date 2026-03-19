//! Dependency resolution and cycle detection for task submission.

use crate::store::StoreError;
use crate::task::DependencyFailurePolicy;

/// Resolve dependency edges for a newly created task.
///
/// For each dependency:
/// - If active in `tasks` (and not the task itself): insert an edge into `task_deps`.
/// - If in `task_history` as `completed`: already satisfied, no edge needed.
/// - If in `task_history` with a failure status: apply the failure policy.
/// - If not found anywhere: `InvalidDependency` error.
///
/// Returns `(active_dep_ids, effective_status)`:
/// - `active_dep_ids`: IDs of dependencies that had edges inserted (for cycle detection).
/// - `effective_status`: `Blocked` if any edges were inserted, `Pending` if all resolved.
pub(super) async fn resolve_dependency_edges(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    task_id: i64,
    deps: &[i64],
    policy: DependencyFailurePolicy,
) -> Result<(Vec<i64>, crate::task::TaskStatus), StoreError> {
    let mut active_deps = Vec::new();

    for &dep_id in deps {
        // Check history FIRST. SQLite may reuse row IDs of deleted tasks,
        // so a completed dep's ID could now belong to a different active task.
        // History is authoritative for previously-completed/failed tasks.
        let history_status: Option<(String,)> = sqlx::query_as(
            "SELECT status FROM task_history WHERE id = ? ORDER BY completed_at DESC LIMIT 1",
        )
        .bind(dep_id)
        .fetch_optional(&mut **conn)
        .await?;

        if let Some((ref status,)) = history_status {
            match status.as_str() {
                "completed" => { /* already done, no edge needed */ }
                _ => {
                    // Dep failed/cancelled/expired — apply failure policy.
                    match policy {
                        DependencyFailurePolicy::Cancel | DependencyFailurePolicy::Fail => {
                            return Err(StoreError::DependencyFailed(dep_id));
                        }
                        DependencyFailurePolicy::Ignore => { /* skip */ }
                    }
                }
            }
            continue;
        }

        // Not in history — check if dep exists in active queue.
        let active: Option<(i64,)> = sqlx::query_as("SELECT id FROM tasks WHERE id = ?")
            .bind(dep_id)
            .fetch_optional(&mut **conn)
            .await?;

        if active.is_some() {
            // Dep is still active — insert edge.
            sqlx::query("INSERT INTO task_deps (task_id, depends_on_id) VALUES (?, ?)")
                .bind(task_id)
                .bind(dep_id)
                .execute(&mut **conn)
                .await?;
            active_deps.push(dep_id);
            continue;
        }

        // Not in history and not in active queue.
        return Err(StoreError::InvalidDependency(dep_id));
    }

    let status = if active_deps.is_empty() {
        crate::task::TaskStatus::Pending
    } else {
        crate::task::TaskStatus::Blocked
    };

    Ok((active_deps, status))
}

/// Cycle detection via recursive CTE.
///
/// Walks the entire upstream ancestry of `deps` in a single SQL query
/// instead of issuing one SELECT per BFS level. This reduces the number
/// of Rust↔SQLite round-trips from O(chain_depth) to O(1).
pub(super) async fn detect_cycle(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    new_task_id: i64,
    deps: &[i64],
) -> Result<(), StoreError> {
    for &dep_id in deps {
        let found: Option<(i64,)> = sqlx::query_as(
            "WITH RECURSIVE ancestors(id) AS (
                 SELECT ? AS id
                 UNION
                 SELECT td.depends_on_id
                 FROM task_deps td
                 JOIN ancestors a ON td.task_id = a.id
             )
             SELECT id FROM ancestors WHERE id = ? LIMIT 1",
        )
        .bind(dep_id)
        .bind(new_task_id)
        .fetch_optional(&mut **conn)
        .await?;

        if found.is_some() {
            return Err(StoreError::CyclicDependency);
        }
    }
    Ok(())
}
