//! Read-only query methods for the active queue and history tables.

mod active;
mod history;
mod scheduling;
mod tags;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use crate::task::{TaskHistoryRecord, TaskRecord};

use super::{StoreError, TaskStore};

impl TaskStore {
    // ── Tag population ─────────────────────────────────────────────

    /// Populate tags for a slice of task records from the task_tags table.
    pub(crate) async fn populate_tags(&self, records: &mut [TaskRecord]) -> Result<(), StoreError> {
        if records.is_empty() {
            return Ok(());
        }
        let ids: Vec<i64> = records.iter().map(|r| r.id).collect();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query =
            format!("SELECT task_id, key, value FROM task_tags WHERE task_id IN ({placeholders})");
        let mut q = sqlx::query_as::<_, (i64, String, String)>(&query);
        for id in &ids {
            q = q.bind(id);
        }
        let tag_rows = q.fetch_all(&self.pool).await?;

        let mut tag_map: HashMap<i64, HashMap<String, String>> = HashMap::new();
        for (task_id, key, value) in tag_rows {
            tag_map.entry(task_id).or_default().insert(key, value);
        }
        for record in records {
            if let Some(tags) = tag_map.remove(&record.id) {
                record.tags = tags;
            }
        }
        Ok(())
    }

    /// Populate tags for a slice of history records from the task_history_tags table.
    pub(crate) async fn populate_history_tags(
        &self,
        records: &mut [TaskHistoryRecord],
    ) -> Result<(), StoreError> {
        if records.is_empty() {
            return Ok(());
        }
        let ids: Vec<i64> = records.iter().map(|r| r.id).collect();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT history_rowid, key, value FROM task_history_tags WHERE history_rowid IN ({placeholders})"
        );
        let mut q = sqlx::query_as::<_, (i64, String, String)>(&query);
        for id in &ids {
            q = q.bind(id);
        }
        let tag_rows = q.fetch_all(&self.pool).await?;

        let mut tag_map: HashMap<i64, HashMap<String, String>> = HashMap::new();
        for (history_rowid, key, value) in tag_rows {
            tag_map.entry(history_rowid).or_default().insert(key, value);
        }
        for record in records {
            if let Some(tags) = tag_map.remove(&record.id) {
                record.tags = tags;
            }
        }
        Ok(())
    }
}
