-- Dependency edges: (task_id=A, depends_on_id=B) means
-- "A cannot start until B completes."
CREATE TABLE IF NOT EXISTS task_deps (
    task_id       INTEGER NOT NULL,
    depends_on_id INTEGER NOT NULL,
    PRIMARY KEY (task_id, depends_on_id)
);

-- "Who depends on me?" query (used when a task completes).
CREATE INDEX IF NOT EXISTS idx_task_deps_depends_on
    ON task_deps (depends_on_id);
