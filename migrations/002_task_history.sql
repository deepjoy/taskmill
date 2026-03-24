-- Completed and failed task history for queries and IO learning.
CREATE TABLE IF NOT EXISTS task_history (
    id                    INTEGER PRIMARY KEY,
    task_type             TEXT    NOT NULL,
    key                   TEXT    NOT NULL,
    label                 TEXT    NOT NULL DEFAULT '',
    priority              INTEGER NOT NULL,
    status                TEXT    NOT NULL,
    payload               BLOB,
    memo                  BLOB    DEFAULT NULL,
    expected_read_bytes   INTEGER NOT NULL DEFAULT 0,
    expected_write_bytes  INTEGER NOT NULL DEFAULT 0,
    expected_net_rx_bytes INTEGER NOT NULL DEFAULT 0,
    expected_net_tx_bytes INTEGER NOT NULL DEFAULT 0,
    actual_read_bytes     INTEGER,
    actual_write_bytes    INTEGER,
    actual_net_rx_bytes   INTEGER,
    actual_net_tx_bytes   INTEGER,
    retry_count           INTEGER NOT NULL DEFAULT 0,
    max_retries           INTEGER,
    last_error            TEXT,
    created_at            INTEGER NOT NULL,
    started_at            INTEGER,
    completed_at          INTEGER NOT NULL,
    duration_ms           INTEGER,
    run_after             INTEGER,
    parent_id             INTEGER,
    fail_fast             INTEGER NOT NULL DEFAULT 1,
    group_key             TEXT,
    ttl_seconds           INTEGER,
    ttl_from              TEXT    NOT NULL DEFAULT 'submission',
    expires_at            INTEGER
);

-- Aggregate stats and filtered queries by task type.
-- Covers history_stats (aggregation), history_by_type (ordered pagination),
-- and avg_throughput (filtered subquery).
CREATE INDEX IF NOT EXISTS idx_history_type
    ON task_history (task_type, completed_at DESC);

-- IO learning: recent completions by task type.
CREATE INDEX IF NOT EXISTS idx_history_type_completed
    ON task_history (task_type, completed_at DESC)
    WHERE status = 'completed';

-- Task lookup by key (dedup and status checks).
CREATE INDEX IF NOT EXISTS idx_history_key
    ON task_history (key, completed_at DESC);

-- Paginating and pruning history by completion time.
CREATE INDEX IF NOT EXISTS idx_history_completed
    ON task_history (completed_at DESC);

-- Filtering history by status (e.g. listing failed tasks).
CREATE INDEX IF NOT EXISTS idx_history_status
    ON task_history (status, completed_at DESC);

-- Lookup children of a parent task.
CREATE INDEX IF NOT EXISTS idx_task_history_parent
    ON task_history (parent_id)
    WHERE parent_id IS NOT NULL;
