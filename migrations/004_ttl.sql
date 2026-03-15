-- Migration 004: Task TTL / automatic expiry
-- Adds TTL columns to both tasks and task_history tables.

ALTER TABLE tasks ADD COLUMN ttl_seconds INTEGER;
ALTER TABLE tasks ADD COLUMN ttl_from TEXT NOT NULL DEFAULT 'submission';
ALTER TABLE tasks ADD COLUMN expires_at TEXT;

ALTER TABLE task_history ADD COLUMN ttl_seconds INTEGER;
ALTER TABLE task_history ADD COLUMN ttl_from TEXT NOT NULL DEFAULT 'submission';
ALTER TABLE task_history ADD COLUMN expires_at TEXT;

CREATE INDEX IF NOT EXISTS idx_tasks_expires ON tasks (expires_at ASC)
    WHERE expires_at IS NOT NULL AND status IN ('pending', 'paused');
