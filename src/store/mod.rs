//! SQLite-backed persistence layer for the task queue and history.
//!
//! [`TaskStore`] manages the active task queue and completed/failed/cancelled/superseded
//! history in a single SQLite database. It handles deduplication, priority upgrades,
//! retries, parent-child hierarchy, cancellation history, superseding, task
//! dependencies, and automatic history pruning.
//!
//! # Dependency-related errors
//!
//! Submitting a task with dependencies can produce these [`StoreError`] variants:
//!
//! - [`InvalidDependency(i64)`](StoreError::InvalidDependency) — the referenced
//!   task ID does not exist in the active queue or history
//! - [`DependencyFailed(i64)`](StoreError::DependencyFailed) — the referenced
//!   task has already failed or been cancelled
//! - [`CyclicDependency`](StoreError::CyclicDependency) — a circular dependency
//!   was detected at submission time
//!
//! Most users interact with the store through [`Scheduler`](crate::Scheduler)
//! methods like [`submit`](crate::Scheduler::submit) and
//! [`task_lookup`](crate::Scheduler::task_lookup). Direct access is available
//! via [`Scheduler::store()`](crate::Scheduler::store) for queries and
//! diagnostics.

mod dependencies;
mod hierarchy;
mod lifecycle;
mod query;
pub(crate) mod row_mapping;
mod submit;

pub use lifecycle::FailBackoff;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::task::{SubmitOutcome, TaskSubmission, MAX_PAYLOAD_BYTES};

/// Message sent through the submit coalescing channel.
///
/// Each caller packs its [`TaskSubmission`] and a oneshot response channel.
/// The leader drains the channel and processes the batch in a single
/// SQLite transaction, then sends individual results back.
pub(crate) struct SubmitMsg {
    pub submission: TaskSubmission,
    pub response_tx: tokio::sync::oneshot::Sender<Result<SubmitOutcome, StoreError>>,
}

/// Serde-friendly error type for Tauri IPC and API boundaries.
///
/// Wraps the internal `sqlx::Error` into a serializable form so that
/// callers do not need manual conversion at every call site.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum StoreError {
    #[error("payload exceeds maximum size of {MAX_PAYLOAD_BYTES} bytes")]
    PayloadTooLarge,
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("dependency task {0} does not exist")]
    InvalidDependency(i64),
    #[error("dependency task {0} has already failed")]
    DependencyFailed(i64),
    #[error("circular dependency detected")]
    CyclicDependency,
    #[error("invalid tag: {0}")]
    InvalidTag(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid state: {0}")]
    InvalidState(String),
}

impl From<sqlx::Error> for StoreError {
    fn from(e: sqlx::Error) -> Self {
        StoreError::Database(e.to_string())
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Serialization(e.to_string())
    }
}

/// History retention policy for automatic pruning of old records.
///
/// Applied during `complete()` and `fail()` to keep the `task_history`
/// table bounded. Set to `None` to disable auto-pruning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetentionPolicy {
    /// Keep at most this many history records (oldest pruned first).
    MaxCount(i64),
    /// Keep records from the last N days.
    MaxAgeDays(i64),
}

/// Configuration for the SQLite connection pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreConfig {
    /// Maximum number of connections in the pool.
    ///
    /// Higher values reduce contention when multiple Tauri commands and
    /// background tasks access the store concurrently. Setting this too
    /// high on a single SQLite file provides diminishing returns since
    /// SQLite serializes writes.
    ///
    /// Default: 16.
    pub max_connections: u32,

    /// Optional retention policy for automatic history pruning.
    ///
    /// When set, completed/failed tasks are pruned during `complete()` and
    /// `fail()` to keep the history table bounded.
    pub retention_policy: Option<RetentionPolicy>,

    /// How many completions between automatic prune runs.
    ///
    /// Pruning runs once every `prune_interval` calls to `complete()` or
    /// `fail()` instead of on every call. Default: 100.
    pub prune_interval: u64,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            max_connections: 16,
            retention_policy: Some(RetentionPolicy::MaxCount(10_000)),
            prune_interval: 100,
        }
    }
}

/// SQLite-backed persistence layer for the task queue and history.
///
/// Most users interact with [`TaskStore`] indirectly through [`Scheduler`](crate::Scheduler),
/// but direct access is available via [`Scheduler::store()`](crate::Scheduler::store) for
/// queries and diagnostics.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> Result<(), taskmill::store::StoreError> {
/// use taskmill::store::TaskStore;
/// use taskmill::task::{TaskSubmission, IoBudget, TaskStatus};
/// use taskmill::priority::Priority;
///
/// let store = TaskStore::open_memory().await?;
///
/// // Submit a task.
/// let sub = TaskSubmission::new("thumbnail")
///     .key("photo-1")
///     .payload_raw(br#"{"path":"/a.jpg"}"#.to_vec())
///     .expected_io(IoBudget::disk(4096, 1024));
/// let outcome = store.submit(&sub).await?;
/// assert!(outcome.is_inserted());
///
/// // Pop the highest-priority task and mark it running.
/// let task = store.pop_next().await?.unwrap();
/// assert_eq!(task.status, TaskStatus::Running);
///
/// // Complete it — moves to history.
/// store.complete(task.id, &IoBudget::disk(4096, 1024)).await?;
/// assert!(store.task_by_id(task.id).await?.is_none()); // gone from active queue
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct TaskStore {
    pub(crate) pool: SqlitePool,
    pub(crate) retention_policy: Option<RetentionPolicy>,
    pub(crate) prune_interval: u64,
    pub(crate) completion_count: std::sync::Arc<AtomicU64>,
    /// Fast-path flag: `false` means no tags have been inserted into
    /// `task_tags`, so `populate_tags` can skip the query entirely.
    pub(crate) has_tags: std::sync::Arc<AtomicBool>,
    /// Fast-path flag: `false` means no tasks with `parent_id` have been
    /// submitted, so `active_children_count` checks can be skipped.
    pub(crate) has_hierarchy: std::sync::Arc<AtomicBool>,
    /// Fast-path flag: `false` means no task has ever transitioned to
    /// `running`, so the requeue UPDATE in `skip_existing` can be skipped
    /// (there are no running tasks to mark for requeue).
    pub(crate) has_running: Arc<AtomicBool>,
    /// Send side of the submit coalescing channel.
    pub(crate) submit_tx: tokio::sync::mpsc::UnboundedSender<SubmitMsg>,
    /// Receive side, `Arc`-wrapped for leader election (try_lock pattern).
    pub(crate) submit_rx: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<SubmitMsg>>>,
}

impl TaskStore {
    /// Open (or create) a taskmill database at the given path with default config.
    pub async fn open(path: &str) -> Result<Self, StoreError> {
        Self::open_with_config(path, StoreConfig::default()).await
    }

    /// Open (or create) a taskmill database at the given path with custom config.
    pub async fn open_with_config(path: &str, config: StoreConfig) -> Result<Self, StoreError> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(config.max_connections)
            .connect_with(opts)
            .await?;

        let (submit_tx, submit_rx) = tokio::sync::mpsc::unbounded_channel();
        let store = Self {
            pool,
            retention_policy: config.retention_policy,
            prune_interval: config.prune_interval,
            completion_count: Arc::new(AtomicU64::new(0)),
            // Conservative for file-backed stores that may have existing tags/hierarchy.
            has_tags: Arc::new(AtomicBool::new(true)),
            has_hierarchy: Arc::new(AtomicBool::new(true)),
            // Conservative: file-backed stores may have running tasks from a
            // previous session (before recover_running resets them).
            has_running: Arc::new(AtomicBool::new(true)),
            submit_tx,
            submit_rx: Arc::new(Mutex::new(submit_rx)),
        };
        store.migrate().await?;
        store.recover_running().await?;
        Ok(store)
    }

    /// Open an in-memory database (for testing).
    pub async fn open_memory() -> Result<Self, StoreError> {
        let opts = SqliteConnectOptions::new()
            .filename(":memory:")
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await?;

        let (submit_tx, submit_rx) = tokio::sync::mpsc::unbounded_channel();
        let store = Self {
            pool,
            retention_policy: Some(RetentionPolicy::MaxCount(10_000)),
            prune_interval: 100,
            completion_count: Arc::new(AtomicU64::new(0)),
            // In-memory stores start empty — no tags or hierarchy to query.
            has_tags: Arc::new(AtomicBool::new(false)),
            has_hierarchy: Arc::new(AtomicBool::new(false)),
            has_running: Arc::new(AtomicBool::new(false)),
            submit_tx,
            submit_rx: Arc::new(Mutex::new(submit_rx)),
        };
        store.migrate().await?;
        Ok(store)
    }

    // ── Migrations ───────────────────────────────────────────────────

    /// Run the migration SQL.
    ///
    /// Migrations are idempotent (`CREATE TABLE/INDEX IF NOT EXISTS`).
    async fn migrate(&self) -> Result<(), StoreError> {
        sqlx::raw_sql(include_str!("../../migrations/001_tasks.sql"))
            .execute(&self.pool)
            .await?;
        sqlx::raw_sql(include_str!("../../migrations/002_task_history.sql"))
            .execute(&self.pool)
            .await?;
        sqlx::raw_sql(include_str!("../../migrations/003_task_deps.sql"))
            .execute(&self.pool)
            .await?;
        sqlx::raw_sql(include_str!("../../migrations/004_task_tags.sql"))
            .execute(&self.pool)
            .await?;

        // 010: paused_groups table + pause_reasons column on tasks.
        // The CREATE TABLE is idempotent, but ALTER TABLE ADD COLUMN will
        // fail if the column already exists (fresh databases include it in
        // 001_tasks.sql). Run each statement individually and tolerate the
        // "duplicate column" error from the ALTER.
        sqlx::raw_sql(
            "CREATE TABLE IF NOT EXISTS paused_groups (
                group_key   TEXT    NOT NULL PRIMARY KEY,
                paused_at   INTEGER NOT NULL,
                resume_at   INTEGER
            );",
        )
        .execute(&self.pool)
        .await?;

        // ALTER TABLE ADD COLUMN is not idempotent — tolerate failure.
        let _ =
            sqlx::raw_sql("ALTER TABLE tasks ADD COLUMN pause_reasons INTEGER NOT NULL DEFAULT 0;")
                .execute(&self.pool)
                .await;

        // Backfill: existing paused tasks get PREEMPTION bit.
        sqlx::raw_sql(
            "UPDATE tasks SET pause_reasons = 1 WHERE status = 'paused' AND pause_reasons = 0;",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ── Recovery ─────────────────────────────────────────────────────

    /// Restart recovery: reset any `running` tasks back to `pending`.
    /// `waiting` parents are left as-is — their children will be reset to
    /// pending and will eventually re-trigger parent finalization.
    pub(crate) async fn recover_running(&self) -> Result<(), StoreError> {
        let result = sqlx::query(
            "UPDATE tasks SET status = 'pending', started_at = NULL WHERE status = 'running'",
        )
        .execute(&self.pool)
        .await?;
        let count = result.rows_affected();
        if count > 0 {
            tracing::info!(count, "recovered interrupted tasks back to pending");
        }

        // Clean up stale dependency edges pointing to tasks that no longer
        // exist (e.g. crashed mid-cancellation). Then unblock any tasks with
        // zero remaining edges.
        let stale = sqlx::query(
            "DELETE FROM task_deps
             WHERE depends_on_id NOT IN (SELECT id FROM tasks)",
        )
        .execute(&self.pool)
        .await?;
        if stale.rows_affected() > 0 {
            tracing::info!(
                count = stale.rows_affected(),
                "cleaned up stale dependency edges"
            );
            // Unblock tasks that now have zero remaining deps.
            let unblocked = sqlx::query(
                "UPDATE tasks SET status = 'pending'
                 WHERE status = 'blocked'
                   AND id NOT IN (SELECT task_id FROM task_deps)",
            )
            .execute(&self.pool)
            .await?;
            if unblocked.rows_affected() > 0 {
                tracing::info!(
                    count = unblocked.rows_affected(),
                    "unblocked tasks after stale edge cleanup"
                );
            }
        }

        Ok(())
    }

    // ── Pool access ──────────────────────────────────────────────────

    /// Get a reference to the underlying connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Begin an IMMEDIATE transaction for write operations.
    ///
    /// Unlike `pool.begin()` which uses `BEGIN DEFERRED`, this acquires the
    /// write lock upfront. This prevents deadlocks when multiple transactions
    /// read-then-write concurrently — the busy_timeout is properly honored
    /// instead of SQLite returning SQLITE_BUSY immediately.
    ///
    /// The returned connection auto-rollbacks on drop (sqlx resets pooled
    /// connections with open transactions).
    pub(crate) async fn begin_write(
        &self,
    ) -> Result<sqlx::pool::PoolConnection<sqlx::Sqlite>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;
        Ok(conn)
    }

    // ── Pruning ──────────────────────────────────────────────────────

    /// Prune history records older than `max_age_days` days.
    /// Returns the number of records deleted.
    pub async fn prune_history_by_age(&self, max_age_days: i64) -> Result<u64, StoreError> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(max_age_days)).timestamp_millis();
        let result = sqlx::query("DELETE FROM task_history WHERE completed_at < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Prune history to keep at most `keep_latest` records.
    /// Returns the number of records deleted.
    pub async fn prune_history_by_count(&self, keep_latest: i64) -> Result<u64, StoreError> {
        let result = sqlx::query(
            "DELETE FROM task_history WHERE id NOT IN (
                 SELECT id FROM task_history ORDER BY completed_at DESC LIMIT ?
             )",
        )
        .bind(keep_latest)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Increment the completion counter and prune every `prune_interval` completions.
    /// Errors are logged rather than propagated since the task itself already committed.
    pub(crate) async fn maybe_prune(&self) {
        if self.retention_policy.is_none() {
            return;
        }
        let count = self.completion_count.fetch_add(1, Ordering::Relaxed);
        if count % self.prune_interval != 0 {
            return;
        }
        if let Err(e) = self.auto_prune().await {
            tracing::warn!("history prune failed: {e}");
        }
    }

    /// Apply the configured retention policy, if any.
    async fn auto_prune(&self) -> Result<(), StoreError> {
        match &self.retention_policy {
            Some(RetentionPolicy::MaxCount(n)) => {
                self.prune_history_by_count(*n).await?;
            }
            Some(RetentionPolicy::MaxAgeDays(days)) => {
                self.prune_history_by_age(*days).await?;
            }
            None => {}
        }
        Ok(())
    }

    // ── Lifecycle ────────────────────────────────────────────────────

    /// Close the store and flush WAL.
    pub async fn close(&self) {
        // Consolidate the WAL file into the main database before closing.
        if let Err(e) = sqlx::raw_sql("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await
        {
            tracing::warn!(error = %e, "WAL checkpoint failed during close");
        }
        self.pool.close().await;
    }

    /// Delete a history record by id. Returns true if a row was deleted.
    ///
    /// Also removes associated history tags.
    pub async fn delete_history(&self, history_id: i64) -> Result<bool, StoreError> {
        let mut conn = self.begin_write().await?;
        sqlx::query("DELETE FROM task_history_tags WHERE history_rowid = ?")
            .bind(history_id)
            .execute(&mut *conn)
            .await?;
        let result = sqlx::query("DELETE FROM task_history WHERE id = ?")
            .bind(history_id)
            .execute(&mut *conn)
            .await?;
        sqlx::query("COMMIT").execute(&mut *conn).await?;
        Ok(result.rows_affected() > 0)
    }

    /// Delete a task from the active queue by id. Returns true if a row was deleted.
    pub async fn delete(&self, id: i64) -> Result<bool, StoreError> {
        let mut conn = self.begin_write().await?;
        delete_task_tags(&mut conn, id).await?;
        let result = sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(id)
            .execute(&mut *conn)
            .await?;
        sqlx::query("COMMIT").execute(&mut *conn).await?;
        Ok(result.rows_affected() > 0)
    }
}

/// Delete tags for a task. Called before or after deleting the task row itself.
pub(crate) async fn delete_task_tags(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    task_id: i64,
) -> Result<(), StoreError> {
    sqlx::query("DELETE FROM task_tags WHERE task_id = ?")
        .bind(task_id)
        .execute(&mut **conn)
        .await?;
    Ok(())
}

/// Load tags for a single task within an existing connection/transaction.
pub(crate) async fn load_task_tags(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    task_id: i64,
) -> Result<std::collections::HashMap<String, String>, StoreError> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM task_tags WHERE task_id = ?")
            .bind(task_id)
            .fetch_all(&mut **conn)
            .await?;
    Ok(rows.into_iter().collect())
}

/// Insert tags for a task into the task_tags table.
pub(crate) async fn insert_tags(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    task_id: i64,
    tags: &std::collections::HashMap<String, String>,
) -> Result<(), StoreError> {
    for (key, value) in tags {
        sqlx::query("INSERT INTO task_tags (task_id, key, value) VALUES (?, ?, ?)")
            .bind(task_id)
            .bind(key)
            .bind(value)
            .execute(&mut **conn)
            .await?;
    }
    Ok(())
}

/// Insert tags and mark the store's `has_tags` flag if non-empty.
pub(crate) async fn insert_tags_flagged(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
    task_id: i64,
    tags: &std::collections::HashMap<String, String>,
    has_tags_flag: &AtomicBool,
) -> Result<(), StoreError> {
    if !tags.is_empty() {
        has_tags_flag.store(true, Ordering::Relaxed);
    }
    insert_tags(conn, task_id, tags).await
}
