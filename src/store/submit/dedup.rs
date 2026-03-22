//! Deduplication strategies: skip (default) and supersede.

use sqlx::Row;

use crate::store::row_mapping::row_to_task_record;
use crate::store::StoreError;
use crate::task::{SubmitOutcome, TaskSubmission, TtlFrom};

/// Default dedup behaviour: try priority upgrade, then requeue, then no-op.
pub(super) async fn skip_existing(
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
    crate::store::lifecycle::insert_history(
        conn,
        &existing,
        crate::store::lifecycle::HistoryStatus::Superseded,
        &crate::task::IoBudget::default(),
        existing
            .started_at
            .map(|s| (chrono::Utc::now() - s).num_milliseconds()),
        None,
        false,
    )
    .await?;

    let priority = sub.priority.value() as i32;
    let fail_fast_val: i32 = if sub.fail_fast { 1 } else { 0 };

    // Compute TTL columns for the new submission.
    let ttl_seconds = sub.ttl.map(|d| d.as_secs() as i64);
    let ttl_from_str = sub.ttl_from.as_str();
    let expires_at: Option<i64> = match (sub.ttl, sub.ttl_from) {
        (Some(ttl), TtlFrom::Submission) => {
            let exp = chrono::Utc::now() + ttl;
            Some(exp.timestamp_millis())
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
                    ttl_seconds = ?, ttl_from = ?, expires_at = ?, max_retries = ?
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
            .bind(expires_at)
            .bind(sub.max_retries)
            .bind(replaced_id)
            .execute(&mut **conn)
            .await?;

            // Replace tags: delete old, insert new.
            crate::store::delete_task_tags(conn, replaced_id).await?;
            crate::store::insert_tags(conn, replaced_id, &sub.tags).await?;

            Ok(SubmitOutcome::Superseded {
                new_task_id: replaced_id,
                replaced_task_id: replaced_id,
            })
        }
        crate::task::TaskStatus::Running | crate::task::TaskStatus::Waiting => {
            // Delete existing and insert new.
            crate::store::delete_task_tags(conn, replaced_id).await?;
            sqlx::query("DELETE FROM tasks WHERE id = ?")
                .bind(replaced_id)
                .execute(&mut **conn)
                .await?;

            let now_ms = chrono::Utc::now().timestamp_millis();
            let result = sqlx::query(
                "INSERT INTO tasks (task_type, key, label, priority, payload,
                    expected_read_bytes, expected_write_bytes, expected_net_rx_bytes,
                    expected_net_tx_bytes, parent_id, fail_fast, group_key,
                    ttl_seconds, ttl_from, expires_at, max_retries, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
            .bind(expires_at)
            .bind(sub.max_retries)
            .bind(now_ms)
            .execute(&mut **conn)
            .await?;

            let new_task_id = result.last_insert_rowid();
            crate::store::insert_tags(conn, new_task_id, &sub.tags).await?;

            Ok(SubmitOutcome::Superseded {
                new_task_id,
                replaced_task_id: replaced_id,
            })
        }
    }
}
