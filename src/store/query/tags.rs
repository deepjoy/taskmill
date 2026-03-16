//! Tag-based queries: filtering and aggregation by task tags.

use crate::store::row_mapping::row_to_task_record;
use crate::store::{StoreError, TaskStore};
use crate::task::TaskRecord;

impl TaskStore {
    /// Find active tasks matching all specified tag filters (AND semantics).
    ///
    /// Each `(key, value)` pair adds an INNER JOIN, so only tasks matching
    /// **all** filters are returned. Optionally filter by status.
    pub async fn tasks_by_tags(
        &self,
        filters: &[(&str, &str)],
        status: Option<crate::task::TaskStatus>,
    ) -> Result<Vec<TaskRecord>, StoreError> {
        if filters.is_empty() {
            return Ok(Vec::new());
        }

        let mut sql = String::from("SELECT t.* FROM tasks t");
        for (i, _) in filters.iter().enumerate() {
            sql.push_str(&format!(
                " INNER JOIN task_tags tt{i} ON t.id = tt{i}.task_id AND tt{i}.key = ? AND tt{i}.value = ?"
            ));
        }
        if let Some(ref s) = status {
            sql.push_str(&format!(" WHERE t.status = '{}'", s.as_str()));
        }
        sql.push_str(" ORDER BY t.priority ASC, t.id ASC");

        let mut q = sqlx::query(&sql);
        for (key, value) in filters {
            q = q.bind(key).bind(value);
        }
        let rows = q.fetch_all(&self.pool).await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// Count active tasks matching all specified tag filters (AND semantics).
    pub async fn count_by_tags(
        &self,
        filters: &[(&str, &str)],
        status: Option<crate::task::TaskStatus>,
    ) -> Result<i64, StoreError> {
        if filters.is_empty() {
            return Ok(0);
        }

        let mut sql = String::from("SELECT COUNT(*) FROM tasks t");
        for (i, _) in filters.iter().enumerate() {
            sql.push_str(&format!(
                " INNER JOIN task_tags tt{i} ON t.id = tt{i}.task_id AND tt{i}.key = ? AND tt{i}.value = ?"
            ));
        }
        if let Some(ref s) = status {
            sql.push_str(&format!(" WHERE t.status = '{}'", s.as_str()));
        }

        let mut q = sqlx::query_as::<_, (i64,)>(&sql);
        for (key, value) in filters {
            q = q.bind(key).bind(value);
        }
        let (count,) = q.fetch_one(&self.pool).await?;
        Ok(count)
    }

    /// List distinct values for a tag key across active tasks, with counts.
    ///
    /// Returns `(value, count)` pairs sorted by count descending.
    pub async fn tag_values(&self, key: &str) -> Result<Vec<(String, i64)>, StoreError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT value, COUNT(*) as cnt FROM task_tags WHERE key = ? GROUP BY value ORDER BY cnt DESC",
        )
        .bind(key)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Count active tasks grouped by a tag key's values.
    ///
    /// Returns `(tag_value, count)` pairs sorted by count descending.
    /// Optionally filter by task status.
    pub async fn count_by_tag(
        &self,
        key: &str,
        status: Option<crate::task::TaskStatus>,
    ) -> Result<Vec<(String, i64)>, StoreError> {
        let (sql, bind_status) = match status {
            Some(ref s) => (
                "SELECT tt.value, COUNT(*) as cnt FROM task_tags tt \
                 JOIN tasks t ON t.id = tt.task_id \
                 WHERE tt.key = ? AND t.status = ? \
                 GROUP BY tt.value ORDER BY cnt DESC"
                    .to_string(),
                Some(s.as_str()),
            ),
            None => (
                "SELECT tt.value, COUNT(*) as cnt FROM task_tags tt \
                 JOIN tasks t ON t.id = tt.task_id \
                 WHERE tt.key = ? \
                 GROUP BY tt.value ORDER BY cnt DESC"
                    .to_string(),
                None,
            ),
        };

        let mut q = sqlx::query_as::<_, (String, i64)>(&sql).bind(key);
        if let Some(status_str) = bind_status {
            q = q.bind(status_str);
        }
        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows)
    }
}
