-- Add a human-readable label column to both tables.
-- The label stores the original dedup_key (or task_type if no explicit key),
-- while the `key` column retains the SHA-256 hash for dedup.
ALTER TABLE tasks ADD COLUMN label TEXT NOT NULL DEFAULT '';
ALTER TABLE task_history ADD COLUMN label TEXT NOT NULL DEFAULT '';
