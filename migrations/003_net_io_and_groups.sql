-- Network IO columns for tasks table.
ALTER TABLE tasks ADD COLUMN expected_net_rx_bytes INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tasks ADD COLUMN expected_net_tx_bytes INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tasks ADD COLUMN group_key TEXT;

-- Network IO columns for task_history table.
ALTER TABLE task_history ADD COLUMN expected_net_rx_bytes INTEGER NOT NULL DEFAULT 0;
ALTER TABLE task_history ADD COLUMN expected_net_tx_bytes INTEGER NOT NULL DEFAULT 0;
ALTER TABLE task_history ADD COLUMN actual_net_rx_bytes INTEGER;
ALTER TABLE task_history ADD COLUMN actual_net_tx_bytes INTEGER;
ALTER TABLE task_history ADD COLUMN group_key TEXT;

-- Index for group concurrency checks (running tasks per group).
CREATE INDEX IF NOT EXISTS idx_tasks_group_running
    ON tasks (group_key, status)
    WHERE group_key IS NOT NULL AND status = 'running';
