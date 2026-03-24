-- Active queue: pending, running, paused, and blocked tasks.
-- The UNIQUE(key) constraint provides key-based deduplication —
-- submitting a task with an existing key is a no-op (INSERT OR IGNORE).
-- When a duplicate is submitted while the existing task is running or paused,
-- the requeue flag is set so the task re-runs after the current execution.
CREATE TABLE IF NOT EXISTS tasks (
    id                        INTEGER PRIMARY KEY,
    task_type                 TEXT    NOT NULL,
    key                       TEXT    NOT NULL,
    label                     TEXT    NOT NULL DEFAULT '',
    priority                  INTEGER NOT NULL,
    status                    TEXT    NOT NULL DEFAULT 'pending',
    payload                   BLOB,
    memo                      BLOB    DEFAULT NULL,
    expected_read_bytes       INTEGER NOT NULL DEFAULT 0,
    expected_write_bytes      INTEGER NOT NULL DEFAULT 0,
    expected_net_rx_bytes     INTEGER NOT NULL DEFAULT 0,
    expected_net_tx_bytes     INTEGER NOT NULL DEFAULT 0,
    retry_count               INTEGER NOT NULL DEFAULT 0,
    max_retries               INTEGER,
    last_error                TEXT,
    created_at                INTEGER NOT NULL,
    started_at                INTEGER,
    run_after                 INTEGER,
    requeue                   INTEGER NOT NULL DEFAULT 0,
    requeue_priority          INTEGER,
    parent_id                 INTEGER,
    fail_fast                 INTEGER NOT NULL DEFAULT 1,
    group_key                 TEXT,
    on_dep_failure            TEXT    NOT NULL DEFAULT 'cancel',
    ttl_seconds               INTEGER,
    ttl_from                  TEXT    NOT NULL DEFAULT 'submission',
    expires_at                INTEGER,
    recurring_interval_secs   INTEGER,
    recurring_max_executions  INTEGER,
    recurring_execution_count INTEGER NOT NULL DEFAULT 0,
    recurring_paused          INTEGER NOT NULL DEFAULT 0,
    pause_reasons             INTEGER NOT NULL DEFAULT 0,
    UNIQUE(key)
);

-- Scheduler hot path: pop highest-priority pending task.
CREATE INDEX IF NOT EXISTS idx_tasks_pending
    ON tasks (status, priority ASC, id ASC)
    WHERE status = 'pending';

-- Lookup children of a parent task.
CREATE INDEX IF NOT EXISTS idx_tasks_parent
    ON tasks (parent_id)
    WHERE parent_id IS NOT NULL;

-- Group concurrency checks (running tasks per group).
CREATE INDEX IF NOT EXISTS idx_tasks_group_running
    ON tasks (group_key, status)
    WHERE group_key IS NOT NULL AND status = 'running';

-- TTL expiry lookups for pending/paused/blocked tasks.
CREATE INDEX IF NOT EXISTS idx_tasks_expires
    ON tasks (expires_at ASC)
    WHERE expires_at IS NOT NULL AND status IN ('pending', 'paused', 'blocked');

-- Pending tasks with a future run_after (time-gating).
CREATE INDEX IF NOT EXISTS idx_tasks_run_after
    ON tasks (run_after ASC)
    WHERE run_after IS NOT NULL AND status = 'pending';

-- Blocked tasks (waiting on dependencies).
CREATE INDEX IF NOT EXISTS idx_tasks_blocked
    ON tasks (status)
    WHERE status = 'blocked';
