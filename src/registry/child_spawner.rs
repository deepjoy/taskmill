//! Child task spawning from within an executor.

use std::sync::Arc;

use crate::store::{StoreError, TaskStore};
use crate::task::{SubmitOutcome, TaskSubmission};

/// Handle for spawning child tasks from within an executor.
///
/// Wraps a [`TaskStore`] reference and the parent task ID so that
/// child submissions automatically inherit the parent relationship.
/// Holds a `Notify` reference to wake the scheduler run loop after
/// spawning, so children are dispatched promptly.
#[derive(Clone)]
pub(crate) struct ChildSpawner {
    store: TaskStore,
    parent_id: i64,
    work_notify: Arc<tokio::sync::Notify>,
}

impl ChildSpawner {
    pub(crate) fn new(
        store: TaskStore,
        parent_id: i64,
        work_notify: Arc<tokio::sync::Notify>,
    ) -> Self {
        Self {
            store,
            parent_id,
            work_notify,
        }
    }

    /// Submit a single child task. Sets `parent_id` automatically.
    pub async fn spawn(&self, mut sub: TaskSubmission) -> Result<SubmitOutcome, StoreError> {
        sub.parent_id = Some(self.parent_id);
        let outcome = self.store.submit(&sub).await?;
        self.work_notify.notify_one();
        Ok(outcome)
    }

    /// Submit multiple child tasks in a single transaction.
    pub async fn spawn_batch(
        &self,
        submissions: &mut [TaskSubmission],
    ) -> Result<Vec<SubmitOutcome>, StoreError> {
        for sub in submissions.iter_mut() {
            sub.parent_id = Some(self.parent_id);
        }
        let outcomes = self.store.submit_batch(submissions).await?;
        self.work_notify.notify_one();
        Ok(outcomes)
    }
}
