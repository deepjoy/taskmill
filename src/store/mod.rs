//! SQLite-backed persistence layer for the task queue and history.
//!
//! [`TaskStore`] manages the active task queue and completed/failed/cancelled/superseded
//! history in a single SQLite database. It handles deduplication, priority upgrades,
//! retries, parent-child hierarchy, cancellation history, superseding, and automatic
//! history pruning.
//!
//! Most users interact with the store through [`Scheduler`](crate::Scheduler)
//! methods like [`submit`](crate::Scheduler::submit) and
//! [`task_lookup`](crate::Scheduler::task_lookup). Direct access is available
//! via [`Scheduler::store()`](crate::Scheduler::store) for queries and
//! diagnostics.

mod hierarchy;
mod lifecycle;
mod query;
pub(crate) mod row_mapping;
mod submit;

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

use crate::task::MAX_PAYLOAD_BYTES;

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

        let store = Self {
            pool,
            retention_policy: config.retention_policy,
            prune_interval: config.prune_interval,
            completion_count: std::sync::Arc::new(AtomicU64::new(0)),
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

        let store = Self {
            pool,
            retention_policy: Some(RetentionPolicy::MaxCount(10_000)),
            prune_interval: 100,
            completion_count: std::sync::Arc::new(AtomicU64::new(0)),
        };
        store.migrate().await?;
        Ok(store)
    }

    // ── Migrations ───────────────────────────────────────────────────

    /// Run the migration SQL.
    ///
    /// Migrations are idempotent: `CREATE TABLE IF NOT EXISTS` for the
    /// initial schema and `ALTER TABLE ADD COLUMN` for incremental changes
    /// (SQLite returns "duplicate column name" if the column already exists,
    /// which we silently ignore).
    async fn migrate(&self) -> Result<(), StoreError> {
        sqlx::raw_sql(include_str!("../../migrations/001_tasks.sql"))
            .execute(&self.pool)
            .await?;
        Self::run_alter_migration(
            &self.pool,
            include_str!("../../migrations/002_add_label.sql"),
        )
        .await?;
        Self::run_alter_migration(
            &self.pool,
            include_str!("../../migrations/003_net_io_and_groups.sql"),
        )
        .await?;
        Ok(())
    }

    /// Run a migration containing `ALTER TABLE` statements, ignoring
    /// "duplicate column name" errors (column already exists from a
    /// previous run).
    async fn run_alter_migration(pool: &SqlitePool, sql: &str) -> Result<(), StoreError> {
        for statement in sql.split(';') {
            // Strip comment-only lines but keep SQL after comments.
            let meaningful: String = statement
                .lines()
                .filter(|line| !line.trim_start().starts_with("--"))
                .collect::<Vec<_>>()
                .join("\n");
            let trimmed = meaningful.trim();
            if trimmed.is_empty() {
                continue;
            }
            match sqlx::raw_sql(trimmed).execute(pool).await {
                Ok(_) => {}
                Err(e) if e.to_string().contains("duplicate column name") => {
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
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
        let result =
            sqlx::query("DELETE FROM task_history WHERE completed_at < datetime('now', ?)")
                .bind(format!("-{max_age_days} days"))
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

    /// Delete a task from the active queue by id. Returns true if a row was deleted.
    pub async fn delete(&self, id: i64) -> Result<bool, StoreError> {
        let result = sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
