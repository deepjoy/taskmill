-- Junction table for task dependency edges.
-- A row (task_id=A, depends_on_id=B) means "A cannot start until B completes."
CREATE TABLE IF NOT EXISTS task_deps (
    task_id       INTEGER NOT NULL,
    depends_on_id INTEGER NOT NULL,
    PRIMARY KEY (task_id, depends_on_id)
);

-- Index for the "who depends on me?" query (used when a task completes).
CREATE INDEX IF NOT EXISTS idx_task_deps_depends_on
    ON task_deps (depends_on_id);

-- New status value: 'blocked' is now valid for tasks.status.
-- No schema change needed — status is TEXT, not an enum.
-- Add partial index for blocked tasks.
CREATE INDEX IF NOT EXISTS idx_tasks_blocked
    ON tasks (status)
    WHERE status = 'blocked';

-- Column to store the dependency failure policy for blocked tasks.
-- Values: 'cancel' (default), 'fail', 'ignore'.
ALTER TABLE tasks ADD COLUMN on_dep_failure TEXT NOT NULL DEFAULT 'cancel';
