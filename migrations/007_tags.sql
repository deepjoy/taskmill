-- Task metadata tags (key-value pairs).
CREATE TABLE IF NOT EXISTS task_tags (
    task_id INTEGER NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (task_id, key)
);

CREATE INDEX IF NOT EXISTS idx_task_tags_kv ON task_tags(key, value);

CREATE TABLE IF NOT EXISTS task_history_tags (
    history_rowid INTEGER NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (history_rowid, key)
);

CREATE INDEX IF NOT EXISTS idx_history_tags_kv ON task_history_tags(key, value);
