//! Task submission: deduplication, priority upgrade, requeue logic,
//! intra-batch last-wins dedup, and transaction chunking for large batches.

use std::collections::{HashMap, HashSet, VecDeque};

use sqlx::Row;

use crate::task::{
    DependencyFailurePolicy, DuplicateStrategy, SubmitOutcome, TaskSubmission, TtlFrom,
    MAX_PAYLOAD_BYTES, MAX_TAGS_PER_TASK, MAX_TAG_KEY_LEN, MAX_TAG_VALUE_LEN,
};

use super::row_mapping::row_to_task_record;
use super::{StoreError, TaskStore};

/// Maximum number of tasks per transaction chunk. Batches larger than this
/// are split into multiple transactions to avoid holding the SQLite write
/// lock for too long.
const BATCH_CHUNK_SIZE: usize = 10_000;

/// Validate tag constraints: key length, value length, max count.
fn validate_tags(tags: &HashMap<String, String>) -> Result<(), StoreError> {
    if tags.len() > MAX_TAGS_PER_TASK {
        return Err(StoreError::InvalidTag(format!(
            "too many tags: {} > {MAX_TAGS_PER_TASK}",
            tags.len()
        )));
    }
    for (k, v) in tags {
        if k.len() > MAX_TAG_KEY_LEN {
            return Err(StoreError::InvalidTag(format!(
                "tag key too long: {} > {MAX_TAG_KEY_LEN}",
                k.len()
            )));
        }
        if v.len() > MAX_TAG_VALUE_LEN {
            return Err(StoreError::InvalidTag(format!(
                "tag value too long: {} > {MAX_TAG_VALUE_LEN}",
                v.len()
            )));
        }
    }
    Ok(())
}

/// Core dedup logic for a single task submission within an existing connection.
///
/// Performs the three-step dedup: INSERT OR IGNORE → upgrade priority on
/// pending/paused → mark requeue on running. Shared by both `submit()` and
/// `submit_batch()` to eliminate duplication.
pub(crate) async fn submit_one(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    sub: &TaskSubmission,
) -> Result<SubmitOutcome, StoreError> {
    if let Some(ref err) = sub.payload_error {
        return Err(StoreError::Serialization(err.clone()));
    }

    validate_tags(&sub.tags)?;

    let key = sub.effective_key();
    let priority = sub.priority.value() as i32;
    let fail_fast_val: i32 = if sub.fail_fast { 1 } else { 0 };

    // Compute TTL columns.
    let ttl_seconds = sub.ttl.map(|d| d.as_secs() as i64);
    let ttl_from_str = sub.ttl_from.as_str();
    let expires_at: Option<String> = match (sub.ttl, sub.ttl_from) {
        (Some(ttl), TtlFrom::Submission) => {
            let exp = chrono::Utc::now() + ttl;
            Some(exp.format("%Y-%m-%d %H:%M:%S").to_string())
        }
        _ => None, // FirstAttempt: set on pop; no TTL: NULL
    };

    // Compute scheduling columns.
    let run_after_str: Option<String> = sub
        .run_after
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());
    let recurring_interval_secs: Option<i64> =
        sub.recurring.as_ref().map(|r| r.interval.as_secs() as i64);
    let recurring_max_executions: Option<i64> = sub
        .recurring
        .as_ref()
        .and_then(|r| r.max_executions.map(|n| n as i64));

    // Reject recurring tasks with a parent (not supported).
    if sub.parent_id.is_some() && sub.recurring.is_some() {
        return Err(StoreError::Database(
            "recurring tasks cannot be children (parent_id must be None)".into(),
        ));
    }

    let on_dep_failure_str = sub.on_dependency_failure.as_str();

    let result = sqlx::query(
        "INSERT OR IGNORE INTO tasks (task_type, key, label, priority, payload, expected_read_bytes, expected_write_bytes, expected_net_rx_bytes, expected_net_tx_bytes, parent_id, fail_fast, group_key, ttl_seconds, ttl_from, expires_at, run_after, recurring_interval_secs, recurring_max_executions, on_dep_failure)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&sub.task_type)
    .bind(&key)
    .bind(&sub.label)
    .bind(priority)
    .bind(&sub.payload)
    .bind(sub.expected_io.disk_read)
    .bind(sub.expected_io.disk_write)
    .bind(sub.expected_io.net_rx)
    .bind(sub.expected_io.net_tx)
    .bind(sub.parent_id)
    .bind(fail_fast_val)
    .bind(&sub.group_key)
    .bind(ttl_seconds)
    .bind(ttl_from_str)
    .bind(&expires_at)
    .bind(&run_after_str)
    .bind(recurring_interval_secs)
    .bind(recurring_max_executions)
    .bind(on_dep_failure_str)
    .execute(&mut **conn)
    .await?;

    if result.rows_affected() > 0 {
        let task_id = result.last_insert_rowid();

        // Insert tags.
        super::insert_tags(conn, task_id, &sub.tags).await?;

        // Handle dependencies if any.
        if !sub.dependencies.is_empty() {
            // First resolve which deps are active (need edges) vs already
            // completed (skip) vs failed/missing (error). This also validates
            // all dep IDs before we attempt cycle detection.
            let (active_deps, effective_status) = resolve_dependency_edges(
                conn,
                task_id,
                &sub.dependencies,
                sub.on_dependency_failure,
            )
            .await?;

            // Only check for cycles among active deps (completed deps have no edges).
            if !active_deps.is_empty() {
                detect_cycle(conn, task_id, &active_deps).await?;
            }

            if effective_status == crate::task::TaskStatus::Blocked {
                sqlx::query("UPDATE tasks SET status = 'blocked' WHERE id = ?")
                    .bind(task_id)
                    .execute(&mut **conn)
                    .await?;
            }
        }

        return Ok(SubmitOutcome::Inserted(task_id));
    }

    // Dedup hit — branch on the duplicate strategy.
    match sub.on_duplicate {
        DuplicateStrategy::Reject => Ok(SubmitOutcome::Rejected),
        DuplicateStrategy::Supersede => supersede_existing(conn, sub, &key).await,
        DuplicateStrategy::Skip => skip_existing(conn, &key, priority).await,
    }
}

/// Default dedup behaviour: try priority upgrade, then requeue, then no-op.
async fn skip_existing(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    key: &str,
    priority: i32,
) -> Result<SubmitOutcome, StoreError> {
    // Try to upgrade priority on pending/paused tasks.
    let row = sqlx::query(
        "UPDATE tasks SET priority = ?
         WHERE key = ? AND status IN ('pending', 'paused') AND priority > ?
         RETURNING id",
    )
    .bind(priority)
    .bind(key)
    .bind(priority)
    .fetch_optional(&mut **conn)
    .await?;

    if let Some(r) = row {
        return Ok(SubmitOutcome::Upgraded(r.get("id")));
    }

    // Dedup hit on running/paused task — mark for re-queue.
    let row = sqlx::query(
        "UPDATE tasks SET requeue = 1, requeue_priority = ?
         WHERE key = ? AND status IN ('running', 'paused')
           AND (requeue = 0 OR requeue_priority > ?)
         RETURNING id",
    )
    .bind(priority)
    .bind(key)
    .bind(priority)
    .fetch_optional(&mut **conn)
    .await?;

    match row {
        Some(r) => Ok(SubmitOutcome::Requeued(r.get("id"))),
        None => Ok(SubmitOutcome::Duplicate),
    }
}

/// Supersede: record old task in history as "superseded", then replace.
///
/// - **Pending/Paused**: UPDATE the existing row in-place with new payload,
///   priority, IO estimates, and reset retry_count. Keeps the same row ID.
/// - **Running/Waiting**: DELETE the existing row and INSERT a new one
///   (the scheduler layer handles cancellation of the active execution).
pub(crate) async fn supersede_existing(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    sub: &TaskSubmission,
    key: &str,
) -> Result<SubmitOutcome, StoreError> {
    // Fetch existing task.
    let row = sqlx::query("SELECT * FROM tasks WHERE key = ?")
        .bind(key)
        .fetch_optional(&mut **conn)
        .await?;

    let Some(row) = row else {
        // Raced with deletion — treat as fresh insert.
        return Ok(SubmitOutcome::Duplicate);
    };

    let existing = row_to_task_record(&row);
    let replaced_id = existing.id;

    // Record old task in history as "superseded".
    super::lifecycle::insert_history(
        conn,
        &existing,
        "superseded",
        &crate::task::IoBudget::default(),
        existing
            .started_at
            .map(|s| (chrono::Utc::now() - s).num_milliseconds()),
        None,
    )
    .await?;

    let priority = sub.priority.value() as i32;
    let fail_fast_val: i32 = if sub.fail_fast { 1 } else { 0 };

    // Compute TTL columns for the new submission.
    let ttl_seconds = sub.ttl.map(|d| d.as_secs() as i64);
    let ttl_from_str = sub.ttl_from.as_str();
    let expires_at: Option<String> = match (sub.ttl, sub.ttl_from) {
        (Some(ttl), TtlFrom::Submission) => {
            let exp = chrono::Utc::now() + ttl;
            Some(exp.format("%Y-%m-%d %H:%M:%S").to_string())
        }
        _ => None,
    };

    match existing.status {
        crate::task::TaskStatus::Pending
        | crate::task::TaskStatus::Paused
        | crate::task::TaskStatus::Blocked => {
            // In-place update — keeps the row ID and queue position.
            sqlx::query(
                "UPDATE tasks SET
                    label = ?, priority = ?, payload = ?,
                    expected_read_bytes = ?, expected_write_bytes = ?,
                    expected_net_rx_bytes = ?, expected_net_tx_bytes = ?,
                    retry_count = 0, last_error = NULL, status = 'pending',
                    requeue = 0, requeue_priority = NULL, fail_fast = ?, group_key = ?,
                    ttl_seconds = ?, ttl_from = ?, expires_at = ?
                 WHERE id = ?",
            )
            .bind(&sub.label)
            .bind(priority)
            .bind(&sub.payload)
            .bind(sub.expected_io.disk_read)
            .bind(sub.expected_io.disk_write)
            .bind(sub.expected_io.net_rx)
            .bind(sub.expected_io.net_tx)
            .bind(fail_fast_val)
            .bind(&sub.group_key)
            .bind(ttl_seconds)
            .bind(ttl_from_str)
            .bind(&expires_at)
            .bind(replaced_id)
            .execute(&mut **conn)
            .await?;

            // Replace tags: delete old, insert new.
            super::delete_task_tags(conn, replaced_id).await?;
            super::insert_tags(conn, replaced_id, &sub.tags).await?;

            Ok(SubmitOutcome::Superseded {
                new_task_id: replaced_id,
                replaced_task_id: replaced_id,
            })
        }
        crate::task::TaskStatus::Running | crate::task::TaskStatus::Waiting => {
            // Delete existing and insert new.
            super::delete_task_tags(conn, replaced_id).await?;
            sqlx::query("DELETE FROM tasks WHERE id = ?")
                .bind(replaced_id)
                .execute(&mut **conn)
                .await?;

            let result = sqlx::query(
                "INSERT INTO tasks (task_type, key, label, priority, payload,
                    expected_read_bytes, expected_write_bytes, expected_net_rx_bytes,
                    expected_net_tx_bytes, parent_id, fail_fast, group_key,
                    ttl_seconds, ttl_from, expires_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&sub.task_type)
            .bind(key)
            .bind(&sub.label)
            .bind(priority)
            .bind(&sub.payload)
            .bind(sub.expected_io.disk_read)
            .bind(sub.expected_io.disk_write)
            .bind(sub.expected_io.net_rx)
            .bind(sub.expected_io.net_tx)
            .bind(sub.parent_id)
            .bind(fail_fast_val)
            .bind(&sub.group_key)
            .bind(ttl_seconds)
            .bind(ttl_from_str)
            .bind(&expires_at)
            .execute(&mut **conn)
            .await?;

            let new_task_id = result.last_insert_rowid();
            super::insert_tags(conn, new_task_id, &sub.tags).await?;

            Ok(SubmitOutcome::Superseded {
                new_task_id,
                replaced_task_id: replaced_id,
            })
        }
    }
}

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
async fn resolve_dependency_edges(
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

/// Cycle detection: iterative BFS from each dep upward through
/// the dependency graph. If we encounter `new_task_id`, there's a cycle.
async fn detect_cycle(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    new_task_id: i64,
    deps: &[i64],
) -> Result<(), StoreError> {
    let mut visited = HashSet::new();
    let mut queue: VecDeque<i64> = deps.iter().copied().collect();

    while let Some(current) = queue.pop_front() {
        if current == new_task_id {
            return Err(StoreError::CyclicDependency);
        }
        if !visited.insert(current) {
            continue;
        }
        // Find what `current` depends on.
        let upstream: Vec<(i64,)> =
            sqlx::query_as("SELECT depends_on_id FROM task_deps WHERE task_id = ?")
                .bind(current)
                .fetch_all(&mut **conn)
                .await?;
        queue.extend(upstream.into_iter().map(|(id,)| id));
    }
    Ok(())
}

impl TaskStore {
    /// Submit a new task.
    ///
    /// Returns [`SubmitOutcome::Inserted`] if the task was enqueued,
    /// [`SubmitOutcome::Upgraded`] if a duplicate existed but its priority
    /// was upgraded, or [`SubmitOutcome::Duplicate`] if a duplicate existed
    /// with equal or higher priority.
    ///
    /// When `sub.key` is `None`, the dedup key is auto-generated by hashing
    /// the task type and payload.
    pub async fn submit(&self, sub: &TaskSubmission) -> Result<SubmitOutcome, StoreError> {
        if let Some(ref p) = sub.payload {
            if p.len() > MAX_PAYLOAD_BYTES {
                return Err(StoreError::PayloadTooLarge);
            }
        }
        validate_tags(&sub.tags)?;

        let mut conn = self.begin_write().await?;
        tracing::debug!(task_type = %sub.task_type, "store.submit: INSERT start");
        let outcome = submit_one(&mut conn, sub).await?;
        tracing::debug!(task_type = %sub.task_type, "store.submit: INSERT end");
        sqlx::query("COMMIT").execute(&mut *conn).await?;
        Ok(outcome)
    }

    /// Submit multiple tasks in a single transaction. Returns a `Vec` with one
    /// [`SubmitOutcome`] per input.
    ///
    /// This is significantly faster than calling [`submit`](Self::submit) in a
    /// loop because all inserts share a single SQLite transaction (one
    /// `BEGIN`/`COMMIT` pair instead of N implicit transactions).
    ///
    /// **Intra-batch dedup:** When multiple tasks in the same batch share a
    /// dedup key, only the last occurrence is submitted (last-wins). Earlier
    /// duplicates receive [`SubmitOutcome::Duplicate`].
    ///
    /// **Chunking:** Batches larger than 10,000 tasks are split into
    /// sub-transactions to avoid holding the SQLite write lock for too long.
    /// This means very large batches are not fully atomic, but task submission
    /// is idempotent so re-submitting after a partial failure is safe.
    pub async fn submit_batch(
        &self,
        submissions: &[TaskSubmission],
    ) -> Result<Vec<SubmitOutcome>, StoreError> {
        // Pre-validate all payloads and tags before starting the transaction
        // to avoid partial inserts on validation errors.
        for sub in submissions {
            if let Some(ref p) = sub.payload {
                if p.len() > MAX_PAYLOAD_BYTES {
                    return Err(StoreError::PayloadTooLarge);
                }
            }
            validate_tags(&sub.tags)?;
        }

        // Intra-batch dedup: last-wins. Map each effective key to its last
        // occurrence index so earlier duplicates are skipped.
        let mut last_occurrence: HashMap<String, usize> = HashMap::new();
        for (i, sub) in submissions.iter().enumerate() {
            last_occurrence.insert(sub.effective_key(), i);
        }

        let mut results = Vec::with_capacity(submissions.len());

        for chunk in submissions.chunks(BATCH_CHUNK_SIZE) {
            let chunk_offset = results.len();
            let mut conn = self.begin_write().await?;

            for (i, sub) in chunk.iter().enumerate() {
                let global_i = chunk_offset + i;
                if last_occurrence[&sub.effective_key()] != global_i {
                    results.push(SubmitOutcome::Duplicate);
                } else {
                    results.push(submit_one(&mut conn, sub).await?);
                }
            }

            sqlx::query("COMMIT").execute(&mut *conn).await?;
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use crate::priority::Priority;
    use crate::task::{IoBudget, SubmitOutcome, TaskSubmission, MAX_PAYLOAD_BYTES};

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
    async fn submit_and_pop() {
        use crate::task::TaskStatus;
        let store = test_store().await;
        let sub = make_submission("job-1", Priority::NORMAL);
        let expected_key = sub.effective_key();

        let outcome = store.submit(&sub).await.unwrap();
        assert!(outcome.is_inserted());

        let task = store.pop_next().await.unwrap().unwrap();
        assert_eq!(task.key, expected_key);
        assert_eq!(task.status, TaskStatus::Running);
        assert!(task.started_at.is_some());
    }

    #[tokio::test]
    async fn dedup_prevents_duplicate_key() {
        let store = test_store().await;
        let sub = make_submission("dup-key", Priority::NORMAL);

        let first = store.submit(&sub).await.unwrap();
        assert!(first.is_inserted());

        let second = store.submit(&sub).await.unwrap();
        assert_eq!(second, SubmitOutcome::Duplicate);
    }

    #[tokio::test]
    async fn dedup_upgrades_priority() {
        let store = test_store().await;

        let sub_normal = make_submission("upgrade-me", Priority::NORMAL);
        let first = store.submit(&sub_normal).await.unwrap();
        assert!(first.is_inserted());

        let sub_high = make_submission("upgrade-me", Priority::HIGH);
        let second = store.submit(&sub_high).await.unwrap();
        assert!(matches!(second, SubmitOutcome::Upgraded(_)));

        let key = sub_normal.effective_key();
        let task = store.task_by_key(&key).await.unwrap().unwrap();
        assert_eq!(task.priority, Priority::HIGH);

        let sub_bg = make_submission("upgrade-me", Priority::BACKGROUND);
        let third = store.submit(&sub_bg).await.unwrap();
        assert_eq!(third, SubmitOutcome::Duplicate);

        let task = store.task_by_key(&key).await.unwrap().unwrap();
        assert_eq!(task.priority, Priority::HIGH);
    }

    #[tokio::test]
    async fn dedup_requeues_when_running() {
        let store = test_store().await;

        let sub = make_submission("running-task", Priority::NORMAL);
        store.submit(&sub).await.unwrap();
        let task = store.pop_next().await.unwrap().unwrap();

        let sub_high = make_submission("running-task", Priority::HIGH);
        let outcome = store.submit(&sub_high).await.unwrap();
        assert!(matches!(outcome, SubmitOutcome::Requeued(_)));

        let key = sub.effective_key();
        let running = store.task_by_key(&key).await.unwrap().unwrap();
        assert!(running.requeue);
        assert_eq!(running.requeue_priority, Some(Priority::HIGH));

        store.complete(task.id, &IoBudget::default()).await.unwrap();

        let requeued = store.task_by_key(&key).await.unwrap().unwrap();
        assert_eq!(requeued.status, crate::task::TaskStatus::Pending);
        assert_eq!(requeued.priority, Priority::HIGH);
        assert!(!requeued.requeue);
        assert_eq!(requeued.requeue_priority, None);

        let popped = store.pop_next().await.unwrap().unwrap();
        assert_eq!(popped.id, task.id);
    }

    #[tokio::test]
    async fn dedup_requeue_already_requeued_same_priority() {
        let store = test_store().await;

        let sub = make_submission("rq-dup", Priority::NORMAL);
        store.submit(&sub).await.unwrap();
        store.pop_next().await.unwrap();

        let sub_high = make_submission("rq-dup", Priority::HIGH);
        let outcome = store.submit(&sub_high).await.unwrap();
        assert!(matches!(outcome, SubmitOutcome::Requeued(_)));

        let outcome2 = store.submit(&sub_high).await.unwrap();
        assert_eq!(outcome2, SubmitOutcome::Duplicate);
    }

    #[tokio::test]
    async fn dedup_requeue_upgrades_priority() {
        let store = test_store().await;

        let sub = make_submission("rq-upgrade", Priority::BACKGROUND);
        store.submit(&sub).await.unwrap();
        store.pop_next().await.unwrap();

        let sub_normal = make_submission("rq-upgrade", Priority::NORMAL);
        let outcome = store.submit(&sub_normal).await.unwrap();
        assert!(matches!(outcome, SubmitOutcome::Requeued(_)));

        let sub_high = make_submission("rq-upgrade", Priority::HIGH);
        let outcome2 = store.submit(&sub_high).await.unwrap();
        assert!(matches!(outcome2, SubmitOutcome::Requeued(_)));

        let key = sub.effective_key();
        let task = store.task_by_key(&key).await.unwrap().unwrap();
        assert_eq!(task.requeue_priority, Some(Priority::HIGH));
    }

    #[tokio::test]
    async fn permanent_failure_drops_requeue() {
        let store = test_store().await;

        let sub = make_submission("fail-rq", Priority::NORMAL);
        store.submit(&sub).await.unwrap();
        let task = store.pop_next().await.unwrap().unwrap();

        let sub_high = make_submission("fail-rq", Priority::HIGH);
        store.submit(&sub_high).await.unwrap();

        store
            .fail(task.id, "boom", false, 0, &IoBudget::default())
            .await
            .unwrap();

        let outcome = store.submit(&sub).await.unwrap();
        assert!(outcome.is_inserted());
    }

    #[tokio::test]
    async fn dedup_allows_same_key_different_types() {
        let store = test_store().await;

        let sub_a = TaskSubmission::new("type_a").key("shared-key");
        let sub_b = TaskSubmission::new("type_b").key("shared-key");

        let first = store.submit(&sub_a).await.unwrap();
        assert!(first.is_inserted());

        let second = store.submit(&sub_b).await.unwrap();
        assert!(second.is_inserted());
    }

    #[tokio::test]
    async fn dedup_by_payload_when_no_key() {
        let store = test_store().await;

        let sub = TaskSubmission::new("ingest").payload_raw(b"same-data".to_vec());

        let first = store.submit(&sub).await.unwrap();
        assert!(first.is_inserted());

        let second = store.submit(&sub).await.unwrap();
        assert_eq!(second, SubmitOutcome::Duplicate);

        let sub2 = TaskSubmission::new("ingest").payload_raw(b"different-data".to_vec());
        let third = store.submit(&sub2).await.unwrap();
        assert!(third.is_inserted());
    }

    #[tokio::test]
    async fn payload_size_limit() {
        use crate::store::StoreError;
        let store = test_store().await;
        let mut sub = make_submission("big", Priority::NORMAL);
        sub.payload = Some(vec![0u8; MAX_PAYLOAD_BYTES + 1]);

        let err = store.submit(&sub).await.unwrap_err();
        assert!(matches!(err, StoreError::PayloadTooLarge));
    }

    #[tokio::test]
    async fn submit_batch_inserts_all() {
        let store = test_store().await;
        let subs: Vec<_> = (0..5)
            .map(|i| make_submission(&format!("batch-{i}"), Priority::NORMAL))
            .collect();

        let results = store.submit_batch(&subs).await.unwrap();
        assert_eq!(results.len(), 5);
        assert!(results.iter().all(|r| r.is_inserted()));

        let count = store.pending_count().await.unwrap();
        assert_eq!(count, 5);
    }

    #[tokio::test]
    async fn submit_batch_dedup() {
        let store = test_store().await;
        let sub = make_submission("dup", Priority::NORMAL);

        // Intra-batch dedup: last-wins, so the first is Duplicate and the
        // second (last occurrence) is Inserted.
        let results = store
            .submit_batch(&[sub.clone(), sub.clone()])
            .await
            .unwrap();
        assert_eq!(results[0], SubmitOutcome::Duplicate);
        assert!(results[1].is_inserted());

        // Re-submitting the same key hits the DB-level dedup.
        let results = store.submit_batch(&[sub]).await.unwrap();
        assert_eq!(results[0], SubmitOutcome::Duplicate);
    }

    #[tokio::test]
    async fn submit_batch_empty() {
        let store = test_store().await;
        let results = store.submit_batch(&[]).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn submit_batch_intra_dedup_last_wins() {
        let store = test_store().await;

        // Two tasks with the same dedup key but different priorities.
        // Last-wins: the second task (HIGH) should be inserted, first skipped.
        let sub_normal = make_submission("same-key", Priority::NORMAL);
        let sub_high = make_submission("same-key", Priority::HIGH);

        let results = store
            .submit_batch(&[sub_normal.clone(), sub_high.clone()])
            .await
            .unwrap();
        assert_eq!(results[0], SubmitOutcome::Duplicate);
        assert!(results[1].is_inserted());

        // Verify the stored task has the second task's priority.
        let key = sub_normal.effective_key();
        let task = store.task_by_key(&key).await.unwrap().unwrap();
        assert_eq!(task.priority, Priority::HIGH);
    }

    #[tokio::test]
    async fn submit_batch_large_chunking() {
        use super::BATCH_CHUNK_SIZE;

        let store = test_store().await;
        let count = BATCH_CHUNK_SIZE + 100;
        let subs: Vec<_> = (0..count)
            .map(|i| make_submission(&format!("chunk-{i}"), Priority::NORMAL))
            .collect();

        let results = store.submit_batch(&subs).await.unwrap();
        assert_eq!(results.len(), count);
        assert!(results.iter().all(|r| r.is_inserted()));

        let pending = store.pending_count().await.unwrap();
        assert_eq!(pending, count as i64);
    }

    #[tokio::test]
    async fn submit_batch_rejects_oversized_payload() {
        use crate::store::StoreError;
        let store = test_store().await;
        let sub = make_submission("ok", Priority::NORMAL);
        let big = TaskSubmission::new("test")
            .key("big")
            .payload_raw(vec![0u8; MAX_PAYLOAD_BYTES + 1]);

        let err = store.submit_batch(&[sub.clone(), big]).await.unwrap_err();
        assert!(matches!(err, StoreError::PayloadTooLarge));

        let count = store.pending_count().await.unwrap();
        assert_eq!(count, 0);
    }
}
