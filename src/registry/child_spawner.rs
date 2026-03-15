//! Child task spawning from within an executor.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::store::{StoreError, TaskStore};
use crate::task::{SubmitOutcome, TaskSubmission, TtlFrom};

/// Inherited parent context for child spawning: TTL and tags.
#[derive(Clone)]
pub(crate) struct ParentContext {
    pub created_at: DateTime<Utc>,
    pub ttl_seconds: Option<i64>,
    pub ttl_from: TtlFrom,
    pub started_at: Option<DateTime<Utc>>,
    pub tags: HashMap<String, String>,
}

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
    parent: ParentContext,
}

impl ChildSpawner {
    pub(crate) fn new(
        store: TaskStore,
        parent_id: i64,
        work_notify: Arc<tokio::sync::Notify>,
        parent: ParentContext,
    ) -> Self {
        Self {
            store,
            parent_id,
            work_notify,
            parent,
        }
    }

    /// Compute the remaining parent TTL and apply it to a child submission
    /// if the child doesn't have an explicit TTL.
    fn inherit_ttl(&self, sub: &mut TaskSubmission) {
        if sub.ttl.is_some() {
            return; // Child has explicit TTL, don't override.
        }
        let Some(parent_ttl_secs) = self.parent.ttl_seconds else {
            return; // Parent has no TTL.
        };
        let parent_ttl = Duration::from_secs(parent_ttl_secs as u64);

        // Determine when the parent's TTL started.
        let ttl_start = match self.parent.ttl_from {
            TtlFrom::Submission => self.parent.created_at,
            TtlFrom::FirstAttempt => match self.parent.started_at {
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

    /// Inherit parent tags into a child submission. Child tags take precedence.
    fn inherit_tags(&self, sub: &mut TaskSubmission) {
        for (k, v) in &self.parent.tags {
            sub.tags.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }

    /// Submit a single child task. Sets `parent_id` automatically.
    pub async fn spawn(&self, mut sub: TaskSubmission) -> Result<SubmitOutcome, StoreError> {
        sub.parent_id = Some(self.parent_id);
        self.inherit_ttl(&mut sub);
        self.inherit_tags(&mut sub);
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
            self.inherit_tags(sub);
        }
        let outcomes = self.store.submit_batch(submissions).await?;
        self.work_notify.notify_one();
        Ok(outcomes)
    }
}
