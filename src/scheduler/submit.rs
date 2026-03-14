//! Task submission (single, batch, typed), lookup, and cancellation.

use crate::priority::Priority;
use crate::store::StoreError;
use crate::task::{
    generate_dedup_key, BatchOutcome, BatchSubmission, SubmitOutcome, TaskLookup, TaskSubmission,
    TypedTask,
};

use super::{Scheduler, SchedulerEvent};

impl Scheduler {
    /// Submit a task.
    ///
    /// If the task's priority meets the preemption threshold, running tasks
    /// with lower priority are preempted (their cancellation tokens are cancelled
    /// and they are paused in the store).
    pub async fn submit(&self, sub: &TaskSubmission) -> Result<SubmitOutcome, StoreError> {
        let outcome = self.inner.store.submit(sub).await?;

        if !matches!(outcome, SubmitOutcome::Duplicate) {
            // Preempt if this is a high-priority task.
            if sub.priority.value() <= self.inner.preempt_priority.value() {
                self.inner
                    .active
                    .preempt_below(sub.priority, &self.inner.store, &self.inner.event_tx)
                    .await;
            }

            // Wake the scheduler loop so it picks up the new/upgraded task.
            self.inner.work_notify.notify_one();
        }

        Ok(outcome)
    }

    /// Submit multiple tasks in a single SQLite transaction.
    ///
    /// Returns a [`BatchOutcome`] with per-task results and convenience
    /// accessors. Preemption is triggered once at the end if any inserted
    /// or upgraded task has high enough priority. A [`SchedulerEvent::BatchSubmitted`]
    /// event is emitted when at least one task was changed.
    pub async fn submit_batch(
        &self,
        submissions: &[TaskSubmission],
    ) -> Result<BatchOutcome, StoreError> {
        let results = self.inner.store.submit_batch(submissions).await?;

        // Find the highest (lowest numeric value) priority among tasks that
        // were inserted or had their priority upgraded.
        let best_priority = submissions
            .iter()
            .zip(results.iter())
            .filter(|(_, outcome)| !matches!(outcome, SubmitOutcome::Duplicate))
            .map(|(sub, _)| sub.priority)
            .min_by_key(|p| p.value());

        let any_changed = results
            .iter()
            .any(|o| !matches!(o, SubmitOutcome::Duplicate));

        if let Some(priority) = best_priority {
            if priority.value() <= self.inner.preempt_priority.value() {
                self.inner
                    .active
                    .preempt_below(priority, &self.inner.store, &self.inner.event_tx)
                    .await;
            }
        }

        let outcome = BatchOutcome { outcomes: results };

        if any_changed {
            let inserted_ids = outcome.inserted();
            let _ = self
                .inner
                .event_tx
                .send(SchedulerEvent::BatchSubmitted {
                    count: submissions.len(),
                    inserted_ids,
                });

            self.inner.work_notify.notify_one();
        }

        Ok(outcome)
    }

    /// Submit a batch built with [`BatchSubmission`].
    ///
    /// Applies the builder's defaults and delegates to [`submit_batch`](Self::submit_batch).
    pub async fn submit_built(
        &self,
        batch: BatchSubmission,
    ) -> Result<BatchOutcome, StoreError> {
        let submissions = batch.build();
        self.submit_batch(&submissions).await
    }

    /// Submit a [`TypedTask`], handling serialization automatically.
    ///
    /// Uses the priority from [`TypedTask::priority()`].
    pub async fn submit_typed<T: TypedTask>(&self, task: &T) -> Result<SubmitOutcome, StoreError> {
        let sub = TaskSubmission::from_typed(task);
        self.submit(&sub).await
    }

    /// Submit a [`TypedTask`] with an explicit priority override.
    ///
    /// The provided `priority` replaces whatever [`TypedTask::priority()`]
    /// would return, keeping priority out of the serialized payload.
    pub async fn submit_typed_at<T: TypedTask>(
        &self,
        task: &T,
        priority: Priority,
    ) -> Result<SubmitOutcome, StoreError> {
        let mut sub = TaskSubmission::from_typed(task);
        sub.priority = priority;
        self.submit(&sub).await
    }

    /// Look up a task by the same inputs used during submission.
    ///
    /// Computes the dedup key from `task_type` and `dedup_input` (the
    /// explicit key string or payload bytes — whichever was used when
    /// submitting), then checks the active queue and history in one call.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Using an explicit key (same as TaskSubmission.key = Some("my-file.jpg"))
    /// let result = scheduler.task_lookup("thumbnail", Some(b"my-file.jpg")).await?;
    ///
    /// // Using payload-based dedup (same as TaskSubmission.key = None, payload = ...)
    /// let result = scheduler.task_lookup("ingest", Some(&payload_bytes)).await?;
    /// ```
    pub async fn task_lookup(
        &self,
        task_type: &str,
        dedup_input: Option<&[u8]>,
    ) -> Result<TaskLookup, StoreError> {
        let key = generate_dedup_key(task_type, dedup_input);
        self.inner.store.task_lookup(&key).await
    }

    /// Look up a [`TypedTask`] by value, using its serialized form as the
    /// dedup input.
    ///
    /// This mirrors [`submit_typed`](Self::submit_typed) — pass the same
    /// struct you would submit and get back its current status.
    pub async fn lookup_typed<T: TypedTask>(&self, task: &T) -> Result<TaskLookup, StoreError> {
        let payload = serde_json::to_vec(task)?;
        let key = generate_dedup_key(T::TASK_TYPE, Some(&payload));
        self.inner.store.task_lookup(&key).await
    }

    /// Cancel a task by id.
    ///
    /// If the task is currently running, its cancellation token is triggered
    /// and it is removed from the active map. If it is pending or paused,
    /// it is deleted from the store. Returns `true` if the task was found
    /// and cancelled.
    pub async fn cancel(&self, task_id: i64) -> Result<bool, StoreError> {
        // Cancel children first (cascade).
        let running_child_ids = self.inner.store.cancel_children(task_id).await?;
        for child_id in &running_child_ids {
            if let Some(at) = self.inner.active.remove(*child_id) {
                at.token.cancel();
                let _ = self.inner.store.delete(*child_id).await;
                let _ = self
                    .inner
                    .event_tx
                    .send(SchedulerEvent::Cancelled(at.record.event_header()));
            }
        }

        // Check if it's an active (running) task first.
        if let Some(at) = self.inner.active.remove(task_id) {
            at.token.cancel();
            self.inner.store.delete(task_id).await?;
            let _ = self
                .inner
                .event_tx
                .send(SchedulerEvent::Cancelled(at.record.event_header()));
            return Ok(true);
        }

        // Not active — try to delete from the queue (pending/paused/waiting).
        let deleted = self.inner.store.delete(task_id).await?;
        Ok(deleted)
    }
}
