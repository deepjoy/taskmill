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
    has_hierarchy_flag: Option<&std::sync::atomic::AtomicBool>,
    skip_requeue: bool,
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
    let expires_at: Option<i64> = match (sub.ttl, sub.ttl_from) {
        (Some(ttl), TtlFrom::Submission) => {
            let exp = chrono::Utc::now() + ttl;
            Some(exp.timestamp_millis())
        }
        _ => None, // FirstAttempt: set on pop; no TTL: NULL
    };

    // Compute scheduling columns.
    let run_after_ms: Option<i64> = sub.run_after.map(|dt| dt.timestamp_millis());
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

    let now_ms = chrono::Utc::now().timestamp_millis();

    let result = sqlx::query(
        "INSERT OR IGNORE INTO tasks (task_type, key, label, priority, payload, expected_read_bytes, expected_write_bytes, expected_net_rx_bytes, expected_net_tx_bytes, parent_id, fail_fast, group_key, ttl_seconds, ttl_from, expires_at, run_after, recurring_interval_secs, recurring_max_executions, on_dep_failure, max_retries, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(expires_at)
    .bind(run_after_ms)
    .bind(recurring_interval_secs)
    .bind(recurring_max_executions)
    .bind(on_dep_failure_str)
    .bind(sub.max_retries)
    .bind(now_ms)
    .execute(&mut **conn)
    .await?;

    if result.rows_affected() > 0 {
        let task_id = result.last_insert_rowid();

        // Mark hierarchy flag if this task has a parent.
        if sub.parent_id.is_some() {
            if let Some(flag) = has_hierarchy_flag {
                flag.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }

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

        // If the task's group is paused, insert as paused with the GROUP reason.
        let is_group_paused = if let Some(ref gk) = sub.group_key {
            let row: Option<(i64,)> =
                sqlx::query_as("SELECT 1 FROM paused_groups WHERE group_key = ?")
                    .bind(gk)
                    .fetch_optional(&mut **conn)
                    .await?;
            row.is_some()
        } else {
            false
        };

        if is_group_paused {
            sqlx::query(
                "UPDATE tasks SET status = 'paused',
                                  pause_reasons = pause_reasons | 8
                 WHERE id = ? AND status = 'pending'",
            )
            .bind(task_id)
            .execute(&mut **conn)
            .await?;
        }

        return Ok(SubmitOutcome::Inserted {
            id: task_id,
            group_paused: is_group_paused,
        });
    }

    // Dedup hit — branch on the duplicate strategy.
    match sub.on_duplicate {
        DuplicateStrategy::Reject => Ok(SubmitOutcome::Rejected),
        DuplicateStrategy::Supersede => dedup::supersede_existing(conn, sub, &key).await,
        DuplicateStrategy::Skip => dedup::skip_existing(conn, &key, priority, skip_requeue).await,
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
    ///
    /// **Coalescing:** Concurrent callers are automatically batched into a
    /// single SQLite transaction (leader election pattern), giving batch-level
    /// throughput without API changes.
    pub async fn submit(&self, sub: &TaskSubmission) -> Result<SubmitOutcome, StoreError> {
        // Pre-validate before touching the channel or the database.
        if let Some(ref err) = sub.payload_error {
            return Err(StoreError::Serialization(err.clone()));
        }
        if let Some(ref p) = sub.payload {
            if p.len() > MAX_PAYLOAD_BYTES {
                return Err(StoreError::PayloadTooLarge);
            }
        }
        validate_tags(&sub.tags)?;

        // Try to become the leader. If we win, drain any stranded channel
        // messages and process everything in a single transaction.
        if let Ok(mut rx_guard) = self.submit_rx.try_lock() {
            let mut stranded: Vec<super::SubmitMsg> = Vec::new();
            while let Ok(m) = rx_guard.try_recv() {
                stranded.push(m);
            }
            drop(rx_guard);

            if stranded.is_empty() {
                // Uncontended fast path — submit directly without channel
                // overhead (no clone, no oneshot).
                return self.submit_direct(sub).await;
            }

            // Stranded messages exist — batch them together with ours.
            let (tx, rx) = tokio::sync::oneshot::channel();
            stranded.push(super::SubmitMsg {
                submission: sub.clone(),
                response_tx: tx,
            });
            self.process_submit_batch(stranded).await;
            return rx
                .await
                .map_err(|_| StoreError::Database("submit response dropped".into()))?;
        }

        // Another leader is draining — coalesce via channel.
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = super::SubmitMsg {
            submission: sub.clone(),
            response_tx: tx,
        };
        self.submit_tx
            .send(msg)
            .map_err(|_| StoreError::Database("submit channel closed".into()))?;

        rx.await
            .map_err(|_| StoreError::Database("submit response dropped".into()))?
    }

    /// Direct submit without coalescing (old code path).
    ///
    /// Used by the fast path when no contention is detected.
    async fn submit_direct(&self, sub: &TaskSubmission) -> Result<SubmitOutcome, StoreError> {
        let mut conn = self.begin_write().await?;
        let skip_requeue = !self.has_running.load(std::sync::atomic::Ordering::Relaxed);
        let outcome = submit_one(
            &mut conn,
            sub,
            Some(&self.has_tags),
            Some(&self.has_hierarchy),
            skip_requeue,
        )
        .await?;
        sqlx::query("COMMIT").execute(&mut *conn).await?;
        Ok(outcome)
    }

    /// Process a batch of coalesced submit messages in a single transaction.
    ///
    /// Each message gets its own `submit_one` call, but they all share one
    /// `BEGIN IMMEDIATE` / `COMMIT` pair. Results are sent back via oneshot.
    pub(crate) async fn process_submit_batch(&self, batch: Vec<super::SubmitMsg>) {
        if batch.is_empty() {
            return;
        }

        let skip_requeue = !self.has_running.load(std::sync::atomic::Ordering::Relaxed);

        // Try to open a single shared transaction.
        let conn = self.begin_write().await;
        let mut conn = match conn {
            Ok(c) => c,
            Err(e) => {
                let msg = e.to_string();
                for m in batch {
                    let _ = m.response_tx.send(Err(StoreError::Database(msg.clone())));
                }
                return;
            }
        };

        let mut results: Vec<Result<SubmitOutcome, StoreError>> = Vec::with_capacity(batch.len());
        for m in &batch {
            results.push(
                submit_one(
                    &mut conn,
                    &m.submission,
                    Some(&self.has_tags),
                    Some(&self.has_hierarchy),
                    skip_requeue,
                )
                .await,
            );
        }

        match sqlx::query("COMMIT").execute(&mut *conn).await {
            Ok(_) => {
                for (m, result) in batch.into_iter().zip(results) {
                    let _ = m.response_tx.send(result);
                }
            }
            Err(e) => {
                let msg = e.to_string();
                for m in batch {
                    let _ = m.response_tx.send(Err(StoreError::Database(msg.clone())));
                }
            }
        }
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

        let skip_requeue = !self.has_running.load(std::sync::atomic::Ordering::Relaxed);

        for chunk in submissions.chunks(BATCH_CHUNK_SIZE) {
            let chunk_offset = results.len();
            let mut conn = self.begin_write().await?;

            for (i, sub) in chunk.iter().enumerate() {
                let global_i = chunk_offset + i;
                if last_occurrence[&sub.effective_key()] != global_i {
                    results.push(SubmitOutcome::Duplicate);
                } else {
                    results.push(
                        submit_one(
                            &mut conn,
                            sub,
                            Some(&self.has_tags),
                            Some(&self.has_hierarchy),
                            skip_requeue,
                        )
                        .await?,
                    );
                }
            }

            sqlx::query("COMMIT").execute(&mut *conn).await?;
        }

        Ok(results)
    }
}
