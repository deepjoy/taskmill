-- Delayed dispatch: task is pending but not eligible until this timestamp.
-- NULL = immediately eligible (current behavior, backwards compatible).
ALTER TABLE tasks ADD COLUMN run_after INTEGER;

-- Recurring task template fields.
-- Only set on the "template" row that spawns recurring instances.
ALTER TABLE tasks ADD COLUMN recurring_interval_secs INTEGER;
ALTER TABLE tasks ADD COLUMN recurring_max_executions INTEGER;
ALTER TABLE tasks ADD COLUMN recurring_execution_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tasks ADD COLUMN recurring_paused INTEGER NOT NULL DEFAULT 0;

-- History: preserve the scheduling metadata for diagnostics.
ALTER TABLE task_history ADD COLUMN run_after INTEGER;

-- Partial index: only pending tasks with a future run_after need time-gating.
-- The scheduler's peek query uses this to skip not-yet-ready tasks.
CREATE INDEX IF NOT EXISTS idx_tasks_run_after
    ON tasks (run_after ASC)
    WHERE run_after IS NOT NULL AND status = 'pending';
