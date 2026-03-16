//! Row-to-struct mapping helpers for SQLite query results.

use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::priority::Priority;
use crate::task::{
    DependencyFailurePolicy, HistoryStatus, IoBudget, TaskHistoryRecord, TaskRecord, TaskStatus,
    TtlFrom,
};

pub(crate) fn parse_datetime(s: &str) -> DateTime<Utc> {
    // SQLite stores as "YYYY-MM-DD HH:MM:SS" or "YYYY-MM-DD HH:MM:SS.mmm"
    // (the latter from backoff-computed run_after). Try with fractional seconds
    // first, then fall back to whole-second precision.
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
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
