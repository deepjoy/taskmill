-- Group-level pause state.
-- New columns use epoch milliseconds (INTEGER) per the epoch-for-storage convention.
CREATE TABLE IF NOT EXISTS paused_groups (
    group_key   TEXT    NOT NULL PRIMARY KEY,
    paused_at   INTEGER NOT NULL,   -- bound from Rust: Utc::now().timestamp_millis()
    resume_at   INTEGER             -- optional auto-resume deadline (epoch milliseconds)
);
