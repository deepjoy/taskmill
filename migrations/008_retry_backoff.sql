-- Per-task max_retries (resolved from per-type policy or global default at submit time).
-- NULL means "use global default" — for backward compatibility with tasks
-- created before this migration.
ALTER TABLE tasks ADD COLUMN max_retries INTEGER;
ALTER TABLE task_history ADD COLUMN max_retries INTEGER;
