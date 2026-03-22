//! Tag-based queries: filtering, aggregation, and prefix discovery by task tags.

use crate::store::row_mapping::row_to_task_record;
use crate::store::{StoreError, TaskStore};
use crate::task::TaskRecord;

/// Escape SQL LIKE wildcards in `prefix` and append `%` for a safe prefix pattern.
///
/// The returned value is intended for use with `LIKE ? ESCAPE '\'`.
fn escape_like_prefix(prefix: &str) -> String {
    let mut escaped = String::with_capacity(prefix.len() + 1);
    for ch in prefix.chars() {
        if ch == '%' || ch == '_' || ch == '\\' {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped.push('%');
    escaped
}

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

    /// Find active tasks matching a `task_type` prefix **and** all tag filters.
    ///
    /// If `filters` is empty, returns all tasks matching the prefix.
    pub async fn tasks_by_tags_with_prefix(
        &self,
        prefix: &str,
        filters: &[(&str, &str)],
        status: Option<crate::task::TaskStatus>,
    ) -> Result<Vec<TaskRecord>, StoreError> {
        let pattern = format!("{prefix}%");
        let mut sql = String::from("SELECT t.* FROM tasks t");
        for (i, _) in filters.iter().enumerate() {
            sql.push_str(&format!(
                " INNER JOIN task_tags tt{i} ON t.id = tt{i}.task_id AND tt{i}.key = ? AND tt{i}.value = ?"
            ));
        }
        sql.push_str(" WHERE t.task_type LIKE ?");
        if let Some(ref s) = status {
            sql.push_str(&format!(" AND t.status = '{}'", s.as_str()));
        }
        sql.push_str(" ORDER BY t.priority ASC, t.id ASC");

        let mut q = sqlx::query(&sql);
        for (key, value) in filters {
            q = q.bind(key).bind(value);
        }
        q = q.bind(&pattern);
        let rows = q.fetch_all(&self.pool).await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// Count active tasks grouped by a tag key's values, filtered by `task_type` prefix.
    pub async fn count_by_tag_with_prefix(
        &self,
        prefix: &str,
        key: &str,
        status: Option<crate::task::TaskStatus>,
    ) -> Result<Vec<(String, i64)>, StoreError> {
        let pattern = format!("{prefix}%");
        let (sql, bind_status) = match status {
            Some(ref s) => (
                "SELECT tt.value, COUNT(*) as cnt FROM task_tags tt \
                 JOIN tasks t ON t.id = tt.task_id \
                 WHERE tt.key = ? AND t.task_type LIKE ? AND t.status = ? \
                 GROUP BY tt.value ORDER BY cnt DESC"
                    .to_string(),
                Some(s.as_str()),
            ),
            None => (
                "SELECT tt.value, COUNT(*) as cnt FROM task_tags tt \
                 JOIN tasks t ON t.id = tt.task_id \
                 WHERE tt.key = ? AND t.task_type LIKE ? \
                 GROUP BY tt.value ORDER BY cnt DESC"
                    .to_string(),
                None,
            ),
        };

        let mut q = sqlx::query_as::<_, (String, i64)>(&sql)
            .bind(key)
            .bind(&pattern);
        if let Some(status_str) = bind_status {
            q = q.bind(status_str);
        }
        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// List distinct tag values for a key, filtered to tasks with `task_type` prefix.
    pub async fn tag_values_with_prefix(
        &self,
        prefix: &str,
        key: &str,
    ) -> Result<Vec<(String, i64)>, StoreError> {
        let pattern = format!("{prefix}%");
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT tt.value, COUNT(*) as cnt FROM task_tags tt \
             JOIN tasks t ON t.id = tt.task_id \
             WHERE tt.key = ? AND t.task_type LIKE ? \
             GROUP BY tt.value ORDER BY cnt DESC",
        )
        .bind(key)
        .bind(&pattern)
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

    // ── Tag key prefix queries ──────────────────────────────────────

    /// Discover distinct tag keys matching a prefix across active tasks.
    ///
    /// Returns keys sorted alphabetically. The prefix is matched literally —
    /// LIKE wildcards (`%`, `_`) in the prefix are escaped.
    pub async fn tag_keys_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<String>, StoreError> {
        let pattern = escape_like_prefix(prefix);
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT key FROM task_tags WHERE key LIKE ? ESCAPE '\\' ORDER BY key",
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(k,)| k).collect())
    }

    /// Discover distinct tag keys matching a key prefix, scoped to a task_type prefix.
    pub async fn tag_keys_by_prefix_with_prefix(
        &self,
        task_type_prefix: &str,
        key_prefix: &str,
    ) -> Result<Vec<String>, StoreError> {
        let key_pattern = escape_like_prefix(key_prefix);
        let type_pattern = format!("{task_type_prefix}%");
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT tt.key FROM task_tags tt \
             JOIN tasks t ON t.id = tt.task_id \
             WHERE tt.key LIKE ? ESCAPE '\\' AND t.task_type LIKE ? \
             ORDER BY tt.key",
        )
        .bind(&key_pattern)
        .bind(&type_pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(k,)| k).collect())
    }

    /// Find active tasks that have any tag key matching the prefix.
    ///
    /// Optionally filter by status. Returns tasks ordered by priority.
    /// LIKE wildcards in the prefix are escaped — only true prefix matching.
    pub async fn tasks_by_tag_key_prefix(
        &self,
        prefix: &str,
        status: Option<crate::task::TaskStatus>,
    ) -> Result<Vec<TaskRecord>, StoreError> {
        let pattern = escape_like_prefix(prefix);
        let mut sql = String::from(
            "SELECT t.* FROM tasks t \
             WHERE EXISTS (SELECT 1 FROM task_tags tt \
             WHERE tt.task_id = t.id AND tt.key LIKE ? ESCAPE '\\')",
        );
        if let Some(ref s) = status {
            sql.push_str(&format!(" AND t.status = '{}'", s.as_str()));
        }
        sql.push_str(" ORDER BY t.priority ASC, t.id ASC");

        let rows = sqlx::query(&sql)
            .bind(&pattern)
            .fetch_all(&self.pool)
            .await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// Find active tasks that have any tag key matching the prefix, scoped to a task_type prefix.
    pub async fn tasks_by_tag_key_prefix_with_prefix(
        &self,
        task_type_prefix: &str,
        prefix: &str,
        status: Option<crate::task::TaskStatus>,
    ) -> Result<Vec<TaskRecord>, StoreError> {
        let pattern = escape_like_prefix(prefix);
        let type_pattern = format!("{task_type_prefix}%");
        let mut sql = String::from(
            "SELECT t.* FROM tasks t \
             WHERE EXISTS (SELECT 1 FROM task_tags tt \
             WHERE tt.task_id = t.id AND tt.key LIKE ? ESCAPE '\\') \
             AND t.task_type LIKE ?",
        );
        if let Some(ref s) = status {
            sql.push_str(&format!(" AND t.status = '{}'", s.as_str()));
        }
        sql.push_str(" ORDER BY t.priority ASC, t.id ASC");

        let rows = sqlx::query(&sql)
            .bind(&pattern)
            .bind(&type_pattern)
            .fetch_all(&self.pool)
            .await?;
        let mut records: Vec<TaskRecord> = rows.iter().map(row_to_task_record).collect();
        self.populate_tags(&mut records).await?;
        Ok(records)
    }

    /// Count active tasks that have any tag key matching the prefix.
    ///
    /// LIKE wildcards in the prefix are escaped — only true prefix matching.
    pub async fn count_by_tag_key_prefix(
        &self,
        prefix: &str,
        status: Option<crate::task::TaskStatus>,
    ) -> Result<i64, StoreError> {
        let pattern = escape_like_prefix(prefix);
        let mut sql = String::from(
            "SELECT COUNT(*) FROM tasks t \
             WHERE EXISTS (SELECT 1 FROM task_tags tt \
             WHERE tt.task_id = t.id AND tt.key LIKE ? ESCAPE '\\')",
        );
        if let Some(ref s) = status {
            sql.push_str(&format!(" AND t.status = '{}'", s.as_str()));
        }

        let (count,) = sqlx::query_as::<_, (i64,)>(&sql)
            .bind(&pattern)
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// Count active tasks that have any tag key matching the prefix, scoped to a task_type prefix.
    pub async fn count_by_tag_key_prefix_with_prefix(
        &self,
        task_type_prefix: &str,
        prefix: &str,
        status: Option<crate::task::TaskStatus>,
    ) -> Result<i64, StoreError> {
        let pattern = escape_like_prefix(prefix);
        let type_pattern = format!("{task_type_prefix}%");
        let mut sql = String::from(
            "SELECT COUNT(*) FROM tasks t \
             WHERE EXISTS (SELECT 1 FROM task_tags tt \
             WHERE tt.task_id = t.id AND tt.key LIKE ? ESCAPE '\\') \
             AND t.task_type LIKE ?",
        );
        if let Some(ref s) = status {
            sql.push_str(&format!(" AND t.status = '{}'", s.as_str()));
        }

        let (count,) = sqlx::query_as::<_, (i64,)>(&sql)
            .bind(&pattern)
            .bind(&type_pattern)
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }
}
