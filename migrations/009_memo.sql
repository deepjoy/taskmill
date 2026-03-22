-- Execute-to-finalize memo: typed state persisted between phases.
ALTER TABLE tasks ADD COLUMN memo BLOB DEFAULT NULL;
ALTER TABLE task_history ADD COLUMN memo BLOB DEFAULT NULL;
