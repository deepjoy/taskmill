//! Dependency resolution and cycle detection for task submission.

use std::collections::{HashMap, HashSet};

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
    if deps.is_empty() {
        return Ok((Vec::new(), crate::task::TaskStatus::Pending));
    }

    // --- Step 1: Batch-check history for all deps (one query). ---
    // History is authoritative: SQLite may reuse row IDs of deleted tasks,
    // so a completed dep's ID could now belong to a different active task.
    let placeholders = deps.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let history_query = format!(
        "SELECT h.id, h.status FROM task_history h
         WHERE h.id IN ({placeholders})
         AND h.completed_at = (
             SELECT MAX(h2.completed_at) FROM task_history h2 WHERE h2.id = h.id
         )"
    );
    let mut q = sqlx::query_as::<_, (i64, String)>(&history_query);
    for &dep_id in deps {
        q = q.bind(dep_id);
    }
    let history_rows = q.fetch_all(&mut **conn).await?;

    let mut history_map: HashMap<i64, String> = HashMap::with_capacity(history_rows.len());
    for (id, status) in history_rows {
        history_map.insert(id, status);
    }

    // Process history results; collect deps that need an active-queue check.
    let mut need_active_check = Vec::new();
    for &dep_id in deps {
        if let Some(status) = history_map.get(&dep_id) {
            match status.as_str() {
                "completed" => { /* already done, no edge needed */ }
                _ => match policy {
                    DependencyFailurePolicy::Cancel | DependencyFailurePolicy::Fail => {
                        return Err(StoreError::DependencyFailed(dep_id));
                    }
                    DependencyFailurePolicy::Ignore => { /* skip */ }
                },
            }
        } else {
            need_active_check.push(dep_id);
        }
    }

    if need_active_check.is_empty() {
        return Ok((Vec::new(), crate::task::TaskStatus::Pending));
    }

    // --- Step 2: Batch-check active tasks (one query). ---
    let placeholders2 = need_active_check
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let active_query = format!("SELECT id FROM tasks WHERE id IN ({placeholders2})");
    let mut q2 = sqlx::query_as::<_, (i64,)>(&active_query);
    for &dep_id in &need_active_check {
        q2 = q2.bind(dep_id);
    }
    let active_rows = q2.fetch_all(&mut **conn).await?;
    let active_set: HashSet<i64> = active_rows.into_iter().map(|(id,)| id).collect();

    // Validate: any dep not in history AND not active is invalid.
    let mut active_deps = Vec::with_capacity(active_set.len());
    for &dep_id in &need_active_check {
        if active_set.contains(&dep_id) {
            active_deps.push(dep_id);
        } else {
            return Err(StoreError::InvalidDependency(dep_id));
        }
    }

    // --- Step 3: Batch-insert edges (one query). ---
    if !active_deps.is_empty() {
        let values = active_deps
            .iter()
            .map(|_| "(?, ?)")
            .collect::<Vec<_>>()
            .join(", ");
        let insert_query =
            format!("INSERT INTO task_deps (task_id, depends_on_id) VALUES {values}");
        let mut q3 = sqlx::query(&insert_query);
        for &dep_id in &active_deps {
            q3 = q3.bind(task_id).bind(dep_id);
        }
        q3.execute(&mut **conn).await?;
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
/// Seeds one CTE with **all** dependency IDs and walks the entire upstream
/// ancestry in a single SQL round-trip. If `new_task_id` appears anywhere
/// in the ancestor set, a cycle exists (because we just added
/// `new_task_id → deps`, so any path from deps back to new_task_id is a
/// cycle).
pub(super) async fn detect_cycle(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    new_task_id: i64,
    deps: &[i64],
) -> Result<(), StoreError> {
    if deps.is_empty() {
        return Ok(());
    }

    let seeds = deps.iter().map(|_| "(?)").collect::<Vec<_>>().join(", ");
    let query = format!(
        "WITH RECURSIVE ancestors(id) AS (
             VALUES {seeds}
             UNION
             SELECT td.depends_on_id
             FROM task_deps td
             JOIN ancestors a ON td.task_id = a.id
         )
         SELECT 1 FROM ancestors WHERE id = ? LIMIT 1"
    );

    let mut q = sqlx::query_as::<_, (i32,)>(&query);
    for &dep_id in deps {
        q = q.bind(dep_id);
    }
    q = q.bind(new_task_id);

    if q.fetch_optional(&mut **conn).await?.is_some() {
        return Err(StoreError::CyclicDependency);
    }

    Ok(())
}
