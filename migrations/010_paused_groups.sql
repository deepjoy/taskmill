-- Group-level pause state.
-- New columns use epoch milliseconds (INTEGER) per the epoch-for-storage convention.
CREATE TABLE IF NOT EXISTS paused_groups (
    group_key   TEXT    NOT NULL PRIMARY KEY,
    paused_at   INTEGER NOT NULL,   -- bound from Rust: Utc::now().timestamp_millis()
    resume_at   INTEGER             -- optional auto-resume deadline (epoch milliseconds)
);

-- Per-task pause attribution: bitmask of active pause reasons.
-- Bit values: PREEMPTION=1, MODULE=2, GLOBAL=4, GROUP=8.
-- A task is paused when pause_reasons != 0.
ALTER TABLE tasks ADD COLUMN pause_reasons INTEGER NOT NULL DEFAULT 0;

-- Backfill: any tasks currently in 'paused' status were paused by preemption
-- (module/global pauses don't survive restarts as they use in-memory flags).
UPDATE tasks SET pause_reasons = 1 WHERE status = 'paused';
