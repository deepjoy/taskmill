//! Row-to-struct mapping helpers for SQLite query results.

use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::priority::Priority;
use crate::task::{HistoryStatus, TaskHistoryRecord, TaskRecord, TaskStatus};

pub(crate) fn parse_datetime(s: &str) -> DateTime<Utc> {
    // SQLite stores as "YYYY-MM-DD HH:MM:SS". Parse with chrono.
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .map(|ndt| ndt.and_utc())
        .unwrap_or_default()
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

    TaskRecord {
        id: row.get("id"),
        task_type: row.get("task_type"),
        key: row.get("key"),
        label: row.get("label"),
        priority: Priority::new(priority_val as u8),
        status: status_str.parse().unwrap_or(TaskStatus::Pending),
        payload: row.get("payload"),
        expected_read_bytes: row.get("expected_read_bytes"),
        expected_write_bytes: row.get("expected_write_bytes"),
        expected_net_rx_bytes: row.get("expected_net_rx_bytes"),
        expected_net_tx_bytes: row.get("expected_net_tx_bytes"),
        retry_count: row.get("retry_count"),
        last_error: row.get("last_error"),
        created_at: parse_datetime(&created_at_str),
        started_at: started_at_str.map(|s| parse_datetime(&s)),
        requeue: requeue_val != 0,
        requeue_priority: requeue_priority_val.map(|p| Priority::new(p as u8)),
        parent_id,
        fail_fast: fail_fast_val != 0,
        group_key: row.get("group_key"),
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

    TaskHistoryRecord {
        id: row.get("id"),
        task_type: row.get("task_type"),
        key: row.get("key"),
        label: row.get("label"),
        priority: Priority::new(priority_val as u8),
        status: status_str.parse().unwrap_or(HistoryStatus::Failed),
        payload: row.get("payload"),
        expected_read_bytes: row.get("expected_read_bytes"),
        expected_write_bytes: row.get("expected_write_bytes"),
        expected_net_rx_bytes: row.get("expected_net_rx_bytes"),
        expected_net_tx_bytes: row.get("expected_net_tx_bytes"),
        actual_read_bytes: row.get("actual_read_bytes"),
        actual_write_bytes: row.get("actual_write_bytes"),
        actual_net_rx_bytes: row.get("actual_net_rx_bytes"),
        actual_net_tx_bytes: row.get("actual_net_tx_bytes"),
        retry_count: row.get("retry_count"),
        last_error: row.get("last_error"),
        created_at: parse_datetime(&created_at_str),
        started_at: started_at_str.map(|s| parse_datetime(&s)),
        completed_at: parse_datetime(&completed_at_str),
        duration_ms: row.get("duration_ms"),
        parent_id,
        fail_fast: fail_fast_val != 0,
        group_key: row.get("group_key"),
    }
}
