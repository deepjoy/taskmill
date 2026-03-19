//! Row-to-struct mapping helpers for SQLite query results.

use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::priority::Priority;
use crate::task::{
    DependencyFailurePolicy, HistoryStatus, IoBudget, TaskHistoryRecord, TaskRecord, TaskStatus,
    TtlFrom,
};

pub(crate) fn parse_datetime(s: &str) -> DateTime<Utc> {
    // SQLite stores as "YYYY-MM-DD HH:MM:SS" or "YYYY-MM-DD HH:MM:SS.mmm".
    // Fast fixed-position byte parser instead of the generic chrono parser.
    let b = s.as_bytes();
    if b.len() < 19 {
        return DateTime::<Utc>::default();
    }

    let year = parse_4(b, 0);
    let month = parse_2(b, 5);
    let day = parse_2(b, 8);
    let hour = parse_2(b, 11);
    let min = parse_2(b, 14);
    let sec = parse_2(b, 17);

    let nanos = if b.len() > 20 && b[19] == b'.' {
        parse_frac_nanos(b, 20)
    } else {
        0
    };

    chrono::NaiveDate::from_ymd_opt(year, month, day)
        .and_then(|d| d.and_hms_nano_opt(hour, min, sec, nanos))
        .map(|ndt| ndt.and_utc())
        .unwrap_or_default()
}

#[inline(always)]
fn parse_2(b: &[u8], off: usize) -> u32 {
    (b[off] - b'0') as u32 * 10 + (b[off + 1] - b'0') as u32
}

#[inline(always)]
fn parse_4(b: &[u8], off: usize) -> i32 {
    (b[off] - b'0') as i32 * 1000
        + (b[off + 1] - b'0') as i32 * 100
        + (b[off + 2] - b'0') as i32 * 10
        + (b[off + 3] - b'0') as i32
}

#[inline(always)]
fn parse_frac_nanos(b: &[u8], start: usize) -> u32 {
    let frac_len = (b.len() - start).min(9);
    let mut val: u32 = 0;
    for i in 0..frac_len {
        val = val * 10 + (b[start + i] - b'0') as u32;
    }
    // Pad to 9 digits (nanoseconds).
    for _ in frac_len..9 {
        val *= 10;
    }
    val
}

pub(crate) fn row_to_task_record(row: &sqlx::sqlite::SqliteRow) -> TaskRecord {
    let priority_val: i32 = row.get("priority");
    let status_str: String = row.get("status");
    let created_at_str: String = row.get("created_at");
    let started_at_str: Option<String> = row.get("started_at");

    let requeue_val: i32 = row.get("requeue");
    let requeue_priority_val: Option<i32> = row.get("requeue_priority");
    let parent_id: Option<i64> = row.get("parent_id");
    let fail_fast_val: i32 = row.get("fail_fast");

    let ttl_from_str: String = row.get("ttl_from");
    let expires_at_str: Option<String> = row.get("expires_at");
    let run_after_str: Option<String> = row.get("run_after");
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
        created_at: parse_datetime(&created_at_str),
        started_at: started_at_str.map(|s| parse_datetime(&s)),
        requeue: requeue_val != 0,
        requeue_priority: requeue_priority_val.map(|p| Priority::new(p as u8)),
        parent_id,
        fail_fast: fail_fast_val != 0,
        group_key: row.get("group_key"),
        ttl_seconds: row.get("ttl_seconds"),
        ttl_from: ttl_from_str.parse().unwrap_or(TtlFrom::Submission),
        expires_at: expires_at_str.map(|s| parse_datetime(&s)),
        run_after: run_after_str.map(|s| parse_datetime(&s)),
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
    }
}

pub(crate) fn row_to_history_record(row: &sqlx::sqlite::SqliteRow) -> TaskHistoryRecord {
    let priority_val: i32 = row.get("priority");
    let status_str: String = row.get("status");
    let created_at_str: String = row.get("created_at");
    let started_at_str: Option<String> = row.get("started_at");
    let completed_at_str: String = row.get("completed_at");
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
    let expires_at_str: Option<String> = row.get("expires_at");
    let run_after_str: Option<String> = row.get("run_after");

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
        created_at: parse_datetime(&created_at_str),
        started_at: started_at_str.map(|s| parse_datetime(&s)),
        completed_at: parse_datetime(&completed_at_str),
        duration_ms: row.get("duration_ms"),
        parent_id,
        fail_fast: fail_fast_val != 0,
        group_key: row.get("group_key"),
        ttl_seconds: row.get("ttl_seconds"),
        ttl_from: ttl_from_str.parse().unwrap_or(TtlFrom::Submission),
        expires_at: expires_at_str.map(|s| parse_datetime(&s)),
        run_after: run_after_str.map(|s| parse_datetime(&s)),
        // Tags are populated separately from the task_history_tags table.
        tags: std::collections::HashMap::new(),
        max_retries: row.get("max_retries"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_whole_seconds() {
        let dt = parse_datetime("2024-01-15 09:30:45");
        assert_eq!(dt.to_string(), "2024-01-15 09:30:45 UTC");
    }

    #[test]
    fn parse_fractional_millis() {
        let dt = parse_datetime("2024-01-15 09:30:45.123");
        assert_eq!(dt.to_string(), "2024-01-15 09:30:45.123 UTC");
        assert_eq!(dt.timestamp_subsec_millis(), 123);
    }

    #[test]
    fn parse_fractional_micros() {
        let dt = parse_datetime("2024-01-15 09:30:45.123456");
        assert_eq!(dt.to_string(), "2024-01-15 09:30:45.123456 UTC");
    }

    #[test]
    fn parse_short_string_returns_default() {
        let dt = parse_datetime("bad");
        assert_eq!(dt, DateTime::<Utc>::default());
    }

    #[test]
    fn parse_empty_returns_default() {
        let dt = parse_datetime("");
        assert_eq!(dt, DateTime::<Utc>::default());
    }
}
