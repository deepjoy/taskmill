//! Child task spawning from within an executor.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::store::{StoreError, TaskStore};
use crate::task::{SubmitOutcome, TaskSubmission, TtlFrom};

/// Handle for spawning child tasks from within an executor.
///
/// Wraps a [`TaskStore`] reference and the parent task ID so that
/// child submissions automatically inherit the parent relationship.
/// Holds a `Notify` reference to wake the scheduler run loop after
/// spawning, so children are dispatched promptly.
///
/// If the parent has a TTL, children without an explicit TTL inherit
/// the remaining parent TTL (with `TtlFrom::Submission`).
#[derive(Clone)]
pub(crate) struct ChildSpawner {
    store: TaskStore,
    parent_id: i64,
    work_notify: Arc<tokio::sync::Notify>,
    parent_created_at: DateTime<Utc>,
    parent_ttl_seconds: Option<i64>,
    parent_ttl_from: TtlFrom,
    parent_started_at: Option<DateTime<Utc>>,
}

impl ChildSpawner {
    pub(crate) fn new(
        store: TaskStore,
        parent_id: i64,
        work_notify: Arc<tokio::sync::Notify>,
        parent_created_at: DateTime<Utc>,
        parent_ttl_seconds: Option<i64>,
        parent_ttl_from: TtlFrom,
        parent_started_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            store,
            parent_id,
            work_notify,
            parent_created_at,
            parent_ttl_seconds,
            parent_ttl_from,
            parent_started_at,
        }
    }

    /// Compute the remaining parent TTL and apply it to a child submission
    /// if the child doesn't have an explicit TTL.
    fn inherit_ttl(&self, sub: &mut TaskSubmission) {
        if sub.ttl.is_some() {
            return; // Child has explicit TTL, don't override.
        }
        let Some(parent_ttl_secs) = self.parent_ttl_seconds else {
            return; // Parent has no TTL.
        };
        let parent_ttl = Duration::from_secs(parent_ttl_secs as u64);

        // Determine when the parent's TTL started.
        let ttl_start = match self.parent_ttl_from {
            TtlFrom::Submission => self.parent_created_at,
            TtlFrom::FirstAttempt => match self.parent_started_at {
                Some(started) => started,
                None => return, // Parent hasn't started yet, can't compute remaining.
            },
        };

        let elapsed = Utc::now() - ttl_start;
        let elapsed_std = elapsed.to_std().unwrap_or_default();

        if let Some(remaining) = parent_ttl.checked_sub(elapsed_std) {
            if remaining > Duration::ZERO {
                sub.ttl = Some(remaining);
                sub.ttl_from = TtlFrom::Submission;
            }
        }
    }

    /// Submit a single child task. Sets `parent_id` automatically.
    pub async fn spawn(&self, mut sub: TaskSubmission) -> Result<SubmitOutcome, StoreError> {
        sub.parent_id = Some(self.parent_id);
        self.inherit_ttl(&mut sub);
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
            self.inherit_ttl(sub);
        }
        let outcomes = self.store.submit_batch(submissions).await?;
        self.work_notify.notify_one();
        Ok(outcomes)
    }
}
