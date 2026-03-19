//! Parent-child task hierarchy: waiting state, child queries, and resolution.

use sqlx::Row;

use crate::task::{ParentResolution, TaskRecord};

use super::row_mapping::row_to_task_record;
use super::{StoreError, TaskStore};

impl TaskStore {
    // ── Hierarchy ───────────────────────────────────────────────────

    /// Transition a running parent task to `waiting` status.
    ///
    /// Called after the parent's executor returns when it has spawned children.
    pub async fn set_waiting(&self, id: i64) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE tasks SET status = 'waiting', started_at = NULL WHERE id = ? AND status = 'running'",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Transition a waiting parent task back to `running` for finalization.
    pub async fn set_running_for_finalize(&self, id: i64) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE tasks SET status = 'running', started_at = datetime('now') WHERE id = ? AND status = 'waiting'",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Count of active (non-terminal) children for a parent task.
    pub async fn active_children_count(&self, parent_id: i64) -> Result<i64, StoreError> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE parent_id = ?")
            .bind(parent_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(count.0)
    }

    /// List active children of a parent task.
    pub async fn children(&self, parent_id: i64) -> Result<Vec<TaskRecord>, StoreError> {
        let rows = sqlx::query("SELECT * FROM tasks WHERE parent_id = ? ORDER BY id ASC")
            .bind(parent_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(row_to_task_record).collect())
    }

    /// Count of children that completed successfully in history.
    pub async fn completed_children_count(&self, parent_id: i64) -> Result<i64, StoreError> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM task_history WHERE parent_id = ? AND status = 'completed'",
        )
        .bind(parent_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0)
    }

    /// Count of children that failed permanently in history.
    pub async fn failed_children_count(&self, parent_id: i64) -> Result<i64, StoreError> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM task_history WHERE parent_id = ? AND status = 'failed'",
        )
        .bind(parent_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count.0)
    }

    /// Cancel all active children of a parent task, recording each in history.
    ///
    /// Pending/paused children are moved to history as `cancelled` and deleted
    /// from the active queue. Returns the IDs of running/waiting children
    /// (whose cancellation tokens the scheduler must cancel separately).
    pub async fn cancel_children(&self, parent_id: i64) -> Result<Vec<i64>, StoreError> {
        // Collect running/waiting children — scheduler handles these.
        let running_rows = sqlx::query(
            "SELECT id FROM tasks WHERE parent_id = ? AND status IN ('running', 'waiting')",
        )
        .bind(parent_id)
        .fetch_all(&self.pool)
        .await?;
        let running_ids: Vec<i64> = running_rows.iter().map(|r| r.get("id")).collect();

        // Move pending/paused children to history as cancelled.
        let pending_rows = sqlx::query(
            "SELECT * FROM tasks WHERE parent_id = ? AND status IN ('pending', 'paused')",
        )
        .bind(parent_id)
        .fetch_all(&self.pool)
        .await?;

        if !pending_rows.is_empty() {
            let mut conn = self.begin_write().await?;
            for row in &pending_rows {
                let task = row_to_task_record(row);
                super::lifecycle::insert_history(
                    &mut conn,
                    &task,
                    super::lifecycle::HistoryStatus::Cancelled,
                    &crate::task::IoBudget::default(),
                    None,
                    None,
                )
                .await?;
            }
            sqlx::query(
                "DELETE FROM tasks WHERE parent_id = ? AND status IN ('pending', 'paused')",
            )
            .bind(parent_id)
            .execute(&mut *conn)
            .await?;
            sqlx::query("COMMIT").execute(&mut *conn).await?;
        }

        Ok(running_ids)
    }

    /// Atomically check whether a waiting parent is ready to finalize or has failed.
    ///
    /// Called after a child completes or fails. The parent must be in `waiting`
    /// status for resolution to proceed.
    pub async fn try_resolve_parent(
        &self,
        parent_id: i64,
    ) -> Result<Option<ParentResolution>, StoreError> {
        let mut conn = self.begin_write().await?;

        let parent_row = sqlx::query("SELECT status FROM tasks WHERE id = ?")
            .bind(parent_id)
            .fetch_optional(&mut *conn)
            .await?;

        let Some(parent_row) = parent_row else {
            sqlx::query("COMMIT").execute(&mut *conn).await?;
            return Ok(None);
        };

        let status_str: String = parent_row.get("status");
        if status_str != "waiting" {
            sqlx::query("COMMIT").execute(&mut *conn).await?;
            return Ok(None);
        }

        let (active_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE parent_id = ?")
                .bind(parent_id)
                .fetch_one(&mut *conn)
                .await?;

        if active_count > 0 {
            sqlx::query("COMMIT").execute(&mut *conn).await?;
            return Ok(Some(ParentResolution::StillWaiting));
        }

        let (failed_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM task_history WHERE parent_id = ? AND status = 'failed'",
        )
        .bind(parent_id)
        .fetch_one(&mut *conn)
        .await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;

        if failed_count > 0 {
            return Ok(Some(ParentResolution::Failed(format!(
                "{failed_count} child task(s) failed"
            ))));
        }

        Ok(Some(ParentResolution::ReadyToFinalize))
    }
}

#[cfg(test)]
mod tests {
    use crate::priority::Priority;
    use crate::task::{IoBudget, ParentResolution, TaskStatus, TaskSubmission};

    use super::super::TaskStore;

    async fn test_store() -> TaskStore {
        TaskStore::open_memory().await.unwrap()
    }

    fn make_submission(key: &str, priority: Priority) -> TaskSubmission {
        TaskSubmission::new("test")
            .key(key)
            .priority(priority)
            .payload_raw(b"hello".to_vec())
            .expected_io(IoBudget::disk(1000, 500))
    }

    #[tokio::test]
    async fn parent_child_relationship_persisted() {
        let store = test_store().await;

        let parent_sub = make_submission("parent", Priority::NORMAL);
        let parent_id = store.submit(&parent_sub).await.unwrap().id().unwrap();

        let mut child_sub = make_submission("child-1", Priority::NORMAL);
        child_sub.parent_id = Some(parent_id);
        let child_id = store.submit(&child_sub).await.unwrap().id().unwrap();

        let child = store.task_by_id(child_id).await.unwrap().unwrap();
        assert_eq!(child.parent_id, Some(parent_id));

        let parent = store.task_by_id(parent_id).await.unwrap().unwrap();
        assert_eq!(parent.parent_id, None);
    }

    #[tokio::test]
    async fn active_children_count_tracks_children() {
        let store = test_store().await;

        let parent_sub = make_submission("parent", Priority::NORMAL);
        let parent_id = store.submit(&parent_sub).await.unwrap().id().unwrap();

        assert_eq!(store.active_children_count(parent_id).await.unwrap(), 0);

        for i in 0..2 {
            let mut sub = make_submission(&format!("child-{i}"), Priority::NORMAL);
            sub.parent_id = Some(parent_id);
            store.submit(&sub).await.unwrap();
        }
        assert_eq!(store.active_children_count(parent_id).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn set_waiting_and_waiting_tasks() {
        let store = test_store().await;

        let sub = make_submission("waiter", Priority::NORMAL);
        store.submit(&sub).await.unwrap();
        let task = store.pop_next().await.unwrap().unwrap();

        store.set_waiting(task.id).await.unwrap();

        let t = store.task_by_id(task.id).await.unwrap().unwrap();
        assert_eq!(t.status, TaskStatus::Waiting);

        let waiting = store.waiting_tasks().await.unwrap();
        assert_eq!(waiting.len(), 1);
        assert_eq!(store.waiting_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn try_resolve_parent_ready_to_finalize() {
        let store = test_store().await;

        let parent_sub = make_submission("parent", Priority::NORMAL);
        let parent_id = store.submit(&parent_sub).await.unwrap().id().unwrap();
        store.pop_next().await.unwrap();
        store.set_waiting(parent_id).await.unwrap();

        let mut child_sub = make_submission("child", Priority::NORMAL);
        child_sub.parent_id = Some(parent_id);
        store.submit(&child_sub).await.unwrap();
        let child = store.pop_next().await.unwrap().unwrap();
        store
            .complete(child.id, &IoBudget::default())
            .await
            .unwrap();

        let resolution = store.try_resolve_parent(parent_id).await.unwrap();
        assert_eq!(resolution, Some(ParentResolution::ReadyToFinalize));
    }

    #[tokio::test]
    async fn try_resolve_parent_still_waiting() {
        let store = test_store().await;

        let parent_sub = make_submission("parent", Priority::NORMAL);
        let parent_id = store.submit(&parent_sub).await.unwrap().id().unwrap();
        store.pop_next().await.unwrap();
        store.set_waiting(parent_id).await.unwrap();

        for i in 0..2 {
            let mut sub = make_submission(&format!("child-{i}"), Priority::NORMAL);
            sub.parent_id = Some(parent_id);
            store.submit(&sub).await.unwrap();
        }
        let child = store.pop_next().await.unwrap().unwrap();
        store
            .complete(child.id, &IoBudget::default())
            .await
            .unwrap();

        let resolution = store.try_resolve_parent(parent_id).await.unwrap();
        assert_eq!(resolution, Some(ParentResolution::StillWaiting));
    }

    #[tokio::test]
    async fn try_resolve_parent_failed() {
        let store = test_store().await;

        let parent_sub = make_submission("parent", Priority::NORMAL);
        let parent_id = store.submit(&parent_sub).await.unwrap().id().unwrap();
        store.pop_next().await.unwrap();
        store.set_waiting(parent_id).await.unwrap();

        let mut child_sub = make_submission("child", Priority::NORMAL);
        child_sub.parent_id = Some(parent_id);
        store.submit(&child_sub).await.unwrap();
        let child = store.pop_next().await.unwrap().unwrap();
        store
            .fail(
                child.id,
                "boom",
                false,
                0,
                &IoBudget::default(),
                &Default::default(),
            )
            .await
            .unwrap();

        let resolution = store.try_resolve_parent(parent_id).await.unwrap();
        assert_eq!(
            resolution,
            Some(ParentResolution::Failed("1 child task(s) failed".into()))
        );
    }

    #[tokio::test]
    async fn cancel_children_removes_pending_returns_running() {
        let store = test_store().await;

        let parent_sub = make_submission("parent", Priority::NORMAL);
        let parent_id = store.submit(&parent_sub).await.unwrap().id().unwrap();

        let _parent = store.pop_next().await.unwrap().unwrap();

        for i in 0..3 {
            let mut sub = make_submission(&format!("child-{i}"), Priority::NORMAL);
            sub.parent_id = Some(parent_id);
            store.submit(&sub).await.unwrap();
        }

        let running_child = store.pop_next().await.unwrap().unwrap();

        let running_ids = store.cancel_children(parent_id).await.unwrap();
        assert_eq!(running_ids.len(), 1);
        assert_eq!(running_ids[0], running_child.id);

        assert_eq!(store.active_children_count(parent_id).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn parent_id_persisted_in_history() {
        let store = test_store().await;

        let parent_sub = make_submission("parent", Priority::NORMAL);
        let parent_id = store.submit(&parent_sub).await.unwrap().id().unwrap();

        let _parent = store.pop_next().await.unwrap().unwrap();

        let mut child_sub = make_submission("child", Priority::NORMAL);
        child_sub.parent_id = Some(parent_id);
        store.submit(&child_sub).await.unwrap();
        let child = store.pop_next().await.unwrap().unwrap();

        store
            .complete(child.id, &IoBudget::default())
            .await
            .unwrap();

        let hist = store.history(10, 0).await.unwrap();
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].parent_id, Some(parent_id));
    }

    #[tokio::test]
    async fn fail_fast_field_persisted() {
        let store = test_store().await;

        let mut sub = make_submission("ff", Priority::NORMAL);
        sub.fail_fast = false;
        store.submit(&sub).await.unwrap();

        let task = store.pop_next().await.unwrap().unwrap();
        assert!(!task.fail_fast);

        store.complete(task.id, &IoBudget::default()).await.unwrap();

        let hist = store.history(10, 0).await.unwrap();
        assert!(!hist[0].fail_fast);
    }

    #[tokio::test]
    async fn set_running_for_finalize() {
        let store = test_store().await;

        let sub = make_submission("fin", Priority::NORMAL);
        store.submit(&sub).await.unwrap();
        let task = store.pop_next().await.unwrap().unwrap();
        store.set_waiting(task.id).await.unwrap();

        store.set_running_for_finalize(task.id).await.unwrap();
        let t = store.task_by_id(task.id).await.unwrap().unwrap();
        assert_eq!(t.status, TaskStatus::Running);
        assert!(t.started_at.is_some());
    }

    #[tokio::test]
    async fn child_inherits_parent_tags() {
        use crate::registry::child_spawner::{ChildSpawner, ParentContext};
        use std::sync::Arc;

        let store = test_store().await;
        let notify = Arc::new(tokio::sync::Notify::new());

        // Submit a parent with tags.
        let parent_sub = TaskSubmission::new("test")
            .key("tagged-parent")
            .tag("env", "prod")
            .tag("region", "us-east");
        let parent_id = store.submit(&parent_sub).await.unwrap().id().unwrap();
        let parent = store.pop_next().await.unwrap().unwrap();

        let ctx = ParentContext {
            created_at: parent.created_at,
            ttl_seconds: None,
            ttl_from: crate::task::TtlFrom::Submission,
            started_at: parent.started_at,
            tags: parent.tags.clone(),
        };
        let spawner = ChildSpawner::new(store.clone(), parent_id, notify, ctx);

        // Spawn a child without tags — should inherit parent tags.
        let child_sub = TaskSubmission::new("test").key("child-no-tags");
        let outcome = spawner.spawn(child_sub).await.unwrap();
        let child_id = outcome.id().unwrap();

        let child = store.task_by_id(child_id).await.unwrap().unwrap();
        assert_eq!(child.tags.get("env").unwrap(), "prod");
        assert_eq!(child.tags.get("region").unwrap(), "us-east");
    }

    #[tokio::test]
    async fn child_overrides_parent_tag() {
        use crate::registry::child_spawner::{ChildSpawner, ParentContext};
        use std::sync::Arc;

        let store = test_store().await;
        let notify = Arc::new(tokio::sync::Notify::new());

        let parent_sub = TaskSubmission::new("test")
            .key("tagged-parent-2")
            .tag("env", "prod")
            .tag("region", "us-east");
        let parent_id = store.submit(&parent_sub).await.unwrap().id().unwrap();
        let parent = store.pop_next().await.unwrap().unwrap();

        let ctx = ParentContext {
            created_at: parent.created_at,
            ttl_seconds: None,
            ttl_from: crate::task::TtlFrom::Submission,
            started_at: parent.started_at,
            tags: parent.tags.clone(),
        };
        let spawner = ChildSpawner::new(store.clone(), parent_id, notify, ctx);

        // Spawn a child that overrides "region" but inherits "env".
        let child_sub = TaskSubmission::new("test")
            .key("child-override")
            .tag("region", "eu-west")
            .tag("extra", "yes");
        let outcome = spawner.spawn(child_sub).await.unwrap();
        let child_id = outcome.id().unwrap();

        let child = store.task_by_id(child_id).await.unwrap().unwrap();
        assert_eq!(child.tags.get("env").unwrap(), "prod"); // Inherited.
        assert_eq!(child.tags.get("region").unwrap(), "eu-west"); // Overridden.
        assert_eq!(child.tags.get("extra").unwrap(), "yes"); // Child's own.
    }

    #[tokio::test]
    async fn recover_preserves_waiting_parents() {
        let store = test_store().await;

        let parent_sub = make_submission("parent", Priority::NORMAL);
        let parent_id = store.submit(&parent_sub).await.unwrap().id().unwrap();
        store.pop_next().await.unwrap();
        store.set_waiting(parent_id).await.unwrap();

        let mut child_sub = make_submission("child", Priority::NORMAL);
        child_sub.parent_id = Some(parent_id);
        store.submit(&child_sub).await.unwrap();
        let child = store.pop_next().await.unwrap().unwrap();
        assert_eq!(child.status, TaskStatus::Running);

        store.recover_running().await.unwrap();

        let parent = store.task_by_id(parent_id).await.unwrap().unwrap();
        assert_eq!(parent.status, TaskStatus::Waiting);

        let child = store.task_by_id(child.id).await.unwrap().unwrap();
        assert_eq!(child.status, TaskStatus::Pending);
    }
}
