//! Row-to-struct mapping helpers for SQLite query results.

use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::priority::Priority;
use crate::task::{
    DependencyFailurePolicy, HistoryStatus, IoBudget, PauseReasons, TaskHistoryRecord, TaskRecord,
    TaskStatus, TtlFrom,
};

/// Convert a `DateTime<Utc>` to epoch milliseconds for SQLite INTEGER storage.
#[allow(dead_code)]
pub(crate) fn epoch_ms(dt: DateTime<Utc>) -> i64 {
    dt.timestamp_millis()
}

/// Convert epoch milliseconds from SQLite INTEGER back to `DateTime<Utc>`.
///
/// Returns `1970-01-01T00:00:00Z` on corrupt/invalid data — same silent-fallback
/// behavior as the previous `parse_datetime` on malformed TEXT.
pub(crate) fn from_epoch_ms(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap_or_default()
}

pub(crate) fn row_to_task_record(row: &sqlx::sqlite::SqliteRow) -> TaskRecord {
    let priority_val: i32 = row.get("priority");
    let status_str: String = row.get("status");

    let requeue_val: i32 = row.get("requeue");
    let requeue_priority_val: Option<i32> = row.get("requeue_priority");
    let parent_id: Option<i64> = row.get("parent_id");
    let fail_fast_val: i32 = row.get("fail_fast");

    let ttl_from_str: String = row.get("ttl_from");
    let recurring_paused_val: i32 = row.get("recurring_paused");
    let on_dep_failure_str: String = row.get("on_dep_failure");

    TaskRecord {
        id: row.get("id"),
        task_type: row.get("task_type"),
        key: row.get("key"),
        label: row.get("label"),
        priority: Priority::new(priority_val as u8),
        status: status_str.parse().unwrap_or(TaskStatus::Pending),
        payload: row.get("payload"),
        expected_io: IoBudget {
            disk_read: row.get("expected_read_bytes"),
            disk_write: row.get("expected_write_bytes"),
            net_rx: row.get("expected_net_rx_bytes"),
            net_tx: row.get("expected_net_tx_bytes"),
        },
        retry_count: row.get("retry_count"),
        last_error: row.get("last_error"),
        created_at: from_epoch_ms(row.get("created_at")),
        started_at: row.get::<Option<i64>, _>("started_at").map(from_epoch_ms),
        requeue: requeue_val != 0,
        requeue_priority: requeue_priority_val.map(|p| Priority::new(p as u8)),
        parent_id,
        fail_fast: fail_fast_val != 0,
        group_key: row.get("group_key"),
        ttl_seconds: row.get("ttl_seconds"),
        ttl_from: ttl_from_str.parse().unwrap_or(TtlFrom::Submission),
        expires_at: row.get::<Option<i64>, _>("expires_at").map(from_epoch_ms),
        run_after: row.get::<Option<i64>, _>("run_after").map(from_epoch_ms),
        recurring_interval_secs: row.get("recurring_interval_secs"),
        recurring_max_executions: row.get("recurring_max_executions"),
        recurring_execution_count: row.get("recurring_execution_count"),
        recurring_paused: recurring_paused_val != 0,
        // Dependencies are populated separately from the task_deps table.
        dependencies: Vec::new(),
        on_dependency_failure: on_dep_failure_str
            .parse()
            .unwrap_or(DependencyFailurePolicy::Cancel),
        // Tags are populated separately from the task_tags table.
        tags: std::collections::HashMap::new(),
        max_retries: row.get("max_retries"),
        memo: row.get("memo"),
        pause_reasons: PauseReasons::from_bits(row.try_get::<i64, _>("pause_reasons").unwrap_or(0)),
        pause_duration_ms: row.try_get::<i64, _>("pause_duration_ms").unwrap_or(0),
        paused_at_ms: row
            .try_get::<Option<i64>, _>("paused_at_ms")
            .unwrap_or(None),
    }
}

pub(crate) fn row_to_history_record(row: &sqlx::sqlite::SqliteRow) -> TaskHistoryRecord {
    let priority_val: i32 = row.get("priority");
    let status_str: String = row.get("status");
    let parent_id: Option<i64> = row.get("parent_id");
    let fail_fast_val: i32 = row.get("fail_fast");

    // Actual IO: if all four columns are NULL, return None; otherwise construct IoBudget.
    let actual_read: Option<i64> = row.get("actual_read_bytes");
    let actual_write: Option<i64> = row.get("actual_write_bytes");
    let actual_rx: Option<i64> = row.get("actual_net_rx_bytes");
    let actual_tx: Option<i64> = row.get("actual_net_tx_bytes");
    let actual_io = actual_read.map(|dr| IoBudget {
        disk_read: dr,
        disk_write: actual_write.unwrap_or(0),
        net_rx: actual_rx.unwrap_or(0),
        net_tx: actual_tx.unwrap_or(0),
    });

    let ttl_from_str: String = row.get("ttl_from");

    TaskHistoryRecord {
        id: row.get("id"),
        task_type: row.get("task_type"),
        key: row.get("key"),
        label: row.get("label"),
        priority: Priority::new(priority_val as u8),
        status: status_str.parse().unwrap_or(HistoryStatus::Failed),
        payload: row.get("payload"),
        expected_io: IoBudget {
            disk_read: row.get("expected_read_bytes"),
            disk_write: row.get("expected_write_bytes"),
            net_rx: row.get("expected_net_rx_bytes"),
            net_tx: row.get("expected_net_tx_bytes"),
        },
        actual_io,
        retry_count: row.get("retry_count"),
        last_error: row.get("last_error"),
        created_at: from_epoch_ms(row.get("created_at")),
        started_at: row.get::<Option<i64>, _>("started_at").map(from_epoch_ms),
        completed_at: from_epoch_ms(row.get("completed_at")),
        duration_ms: row.get("duration_ms"),
        parent_id,
        fail_fast: fail_fast_val != 0,
        group_key: row.get("group_key"),
        ttl_seconds: row.get("ttl_seconds"),
        ttl_from: ttl_from_str.parse().unwrap_or(TtlFrom::Submission),
        expires_at: row.get::<Option<i64>, _>("expires_at").map(from_epoch_ms),
        run_after: row.get::<Option<i64>, _>("run_after").map(from_epoch_ms),
        // Tags are populated separately from the task_history_tags table.
        tags: std::collections::HashMap::new(),
        max_retries: row.get("max_retries"),
        memo: row.get("memo"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_ms_round_trip() {
        let now = Utc::now();
        let ms = epoch_ms(now);
        let back = from_epoch_ms(ms);
        // Round-trip preserves millisecond precision.
        assert_eq!(now.timestamp_millis(), back.timestamp_millis());
    }

    #[test]
    fn epoch_ms_zero() {
        let dt = from_epoch_ms(0);
        assert_eq!(dt, DateTime::<Utc>::default());
    }

    #[test]
    fn epoch_ms_negative() {
        // Negative epoch (before 1970) should still round-trip.
        let ms = -86_400_000i64; // 1969-12-31
        let dt = from_epoch_ms(ms);
        assert_eq!(dt.timestamp_millis(), ms);
    }

    #[test]
    fn epoch_ms_known_value() {
        // 2024-01-15 09:30:45.123 UTC
        let dt = chrono::NaiveDate::from_ymd_opt(2024, 1, 15)
            .unwrap()
            .and_hms_milli_opt(9, 30, 45, 123)
            .unwrap()
            .and_utc();
        let ms = epoch_ms(dt);
        let back = from_epoch_ms(ms);
        assert_eq!(back.to_string(), "2024-01-15 09:30:45.123 UTC");
    }
}
