//! Task lifecycle transitions: pop, complete, fail, pause, resume, and
//! dependency resolution.

mod cancel_expire;
mod transitions;

#[cfg(test)]
mod tests;

pub use transitions::FailBackoff;

use crate::task::{IoBudget, TaskRecord};

use super::StoreError;

/// Terminal status values for tasks moved to history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoryStatus {
    Completed,
    Failed,
    DeadLetter,
    Cancelled,
    Expired,
    Superseded,
    DependencyFailed,
}

impl HistoryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::DeadLetter => "dead_letter",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
            Self::Superseded => "superseded",
            Self::DependencyFailed => "dependency_failed",
        }
    }

    /// Whether this status increments `retry_count` in history.
    pub fn increments_retries(self) -> bool {
        matches!(self, Self::Failed | Self::DeadLetter)
    }
}

/// Insert a task record into the history table.
///
/// Shared by `complete()`, `fail()`, and `cancel_to_history()` to eliminate
/// the duplicated 22-column INSERT statement.
///
/// When `skip_tags` is `true` the history-tag copy (`INSERT INTO
/// task_history_tags … SELECT FROM task_tags`) is skipped. Callers that
/// know no tags have been inserted (e.g. via the `has_tags` fast-path
/// flag) can set this to avoid an unnecessary SQL round-trip.
pub(crate) async fn insert_history(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    task: &TaskRecord,
    status: HistoryStatus,
    metrics: &IoBudget,
    duration_ms: Option<i64>,
    last_error: Option<&str>,
    skip_tags: bool,
) -> Result<(), StoreError> {
    let fail_fast_val: i32 = if task.fail_fast { 1 } else { 0 };
    let retry_count = if status.increments_retries() {
        task.retry_count + 1
    } else {
        task.retry_count
    };
    let result = sqlx::query(
        "INSERT INTO task_history (task_type, key, label, priority, status, payload,
            expected_read_bytes, expected_write_bytes, expected_net_rx_bytes, expected_net_tx_bytes,
            actual_read_bytes, actual_write_bytes, actual_net_rx_bytes, actual_net_tx_bytes,
            retry_count, last_error, created_at, started_at, duration_ms, parent_id, fail_fast, group_key,
            ttl_seconds, ttl_from, expires_at, run_after, max_retries, memo)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&task.task_type)
    .bind(&task.key)
    .bind(&task.label)
    .bind(task.priority.value() as i32)
    .bind(status.as_str())
    .bind(&task.payload)
    .bind(task.expected_io.disk_read)
    .bind(task.expected_io.disk_write)
    .bind(task.expected_io.net_rx)
    .bind(task.expected_io.net_tx)
    .bind(metrics.disk_read)
    .bind(metrics.disk_write)
    .bind(metrics.net_rx)
    .bind(metrics.net_tx)
    .bind(retry_count)
    .bind(last_error)
    .bind(task.created_at.format("%Y-%m-%d %H:%M:%S").to_string())
    .bind(
        task.started_at
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
    )
    .bind(duration_ms)
    .bind(task.parent_id)
    .bind(fail_fast_val)
    .bind(&task.group_key)
    .bind(task.ttl_seconds)
    .bind(task.ttl_from.as_str())
    .bind(
        task.expires_at
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
    )
    .bind(
        task.run_after
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
    )
    .bind(task.max_retries)
    .bind(&task.memo)
    .execute(&mut **conn)
    .await?;

    // Copy tags from task_tags to task_history_tags.
    if !skip_tags {
        let history_rowid = result.last_insert_rowid();
        sqlx::query(
            "INSERT INTO task_history_tags (history_rowid, key, value)
             SELECT ?, key, value FROM task_tags WHERE task_id = ?",
        )
        .bind(history_rowid)
        .bind(task.id)
        .execute(&mut **conn)
        .await?;
    }

    Ok(())
}

/// Compute the duration in milliseconds from `started_at` to now.
fn compute_duration_ms(task: &TaskRecord) -> Option<i64> {
    task.started_at
        .map(|started| (chrono::Utc::now() - started).num_milliseconds())
}
