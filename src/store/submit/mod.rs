//! Task submission: deduplication, priority upgrade, requeue logic,
//! intra-batch last-wins dedup, and transaction chunking for large batches.

mod dedup;
mod dependencies;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use crate::task::{
    DuplicateStrategy, SubmitOutcome, TaskSubmission, TtlFrom, MAX_PAYLOAD_BYTES,
    MAX_TAGS_PER_TASK, MAX_TAG_KEY_LEN, MAX_TAG_VALUE_LEN,
};

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
    has_tags_flag: Option<&std::sync::atomic::AtomicBool>,
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
        "INSERT OR IGNORE INTO tasks (task_type, key, label, priority, payload, expected_read_bytes, expected_write_bytes, expected_net_rx_bytes, expected_net_tx_bytes, parent_id, fail_fast, group_key, ttl_seconds, ttl_from, expires_at, run_after, recurring_interval_secs, recurring_max_executions, on_dep_failure, max_retries)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(sub.max_retries)
    .execute(&mut **conn)
    .await?;

    if result.rows_affected() > 0 {
        let task_id = result.last_insert_rowid();

        // Insert tags.
        if let Some(flag) = has_tags_flag {
            super::insert_tags_flagged(conn, task_id, &sub.tags, flag).await?;
        } else {
            super::insert_tags(conn, task_id, &sub.tags).await?;
        }

        // Handle dependencies if any.
        if !sub.dependencies.is_empty() {
            let (active_deps, effective_status) = dependencies::resolve_dependency_edges(
                conn,
                task_id,
                &sub.dependencies,
                sub.on_dependency_failure,
            )
            .await?;

            // Only check for cycles among active deps (completed deps have no edges).
            if !active_deps.is_empty() {
                dependencies::detect_cycle(conn, task_id, &active_deps).await?;
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
        DuplicateStrategy::Supersede => dedup::supersede_existing(conn, sub, &key).await,
        DuplicateStrategy::Skip => dedup::skip_existing(conn, &key, priority).await,
    }
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
        let outcome = submit_one(&mut conn, sub, Some(&self.has_tags)).await?;
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
                    results.push(submit_one(&mut conn, sub, Some(&self.has_tags)).await?);
                }
            }

            sqlx::query("COMMIT").execute(&mut *conn).await?;
        }

        Ok(results)
    }
}
