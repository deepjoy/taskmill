//! Task submission (single, batch, typed), lookup, and cancellation.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::priority::Priority;
use crate::registry::{IoTracker, TaskContext};
use crate::store::StoreError;
use crate::task::{
    generate_dedup_key, BatchOutcome, BatchSubmission, SubmitOutcome, TaskLookup, TaskRecord,
    TaskSubmission, TypedTask,
};

use super::progress::ProgressReporter;
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
            let _ = self.inner.event_tx.send(SchedulerEvent::BatchSubmitted {
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
    pub async fn submit_built(&self, batch: BatchSubmission) -> Result<BatchOutcome, StoreError> {
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
    /// Records the task in history as `cancelled` (instead of silently
    /// deleting). For running/waiting tasks, triggers the cancellation
    /// token, fires the [`on_cancel`](crate::TaskExecutor::on_cancel) hook,
    /// and emits a [`SchedulerEvent::Cancelled`] event. Returns `true` if
    /// the task was found and cancelled.
    pub async fn cancel(&self, task_id: i64) -> Result<bool, StoreError> {
        // Cancel children first (cascade). cancel_children now records
        // pending/paused children in history. Running children are returned
        // for us to handle.
        let running_child_ids = self.inner.store.cancel_children(task_id).await?;
        for child_id in &running_child_ids {
            if let Some(at) = self.inner.active.remove(*child_id) {
                at.token.cancel();
                self.inner
                    .store
                    .cancel_to_history_with_record(&at.record)
                    .await?;
                self.fire_on_cancel(&at.record).await;
                let _ = self
                    .inner
                    .event_tx
                    .send(SchedulerEvent::Cancelled(at.record.event_header()));
            }
        }

        // Check if it's an active (running/waiting) task first.
        if let Some(at) = self.inner.active.remove(task_id) {
            at.token.cancel();
            self.inner
                .store
                .cancel_to_history_with_record(&at.record)
                .await?;
            self.fire_on_cancel(&at.record).await;
            let _ = self
                .inner
                .event_tx
                .send(SchedulerEvent::Cancelled(at.record.event_header()));
            return Ok(true);
        }

        // Not active in memory — try to cancel from the queue
        // (pending/paused/waiting in DB only).
        let found = self.inner.store.cancel_to_history(task_id).await?;
        Ok(found)
    }

    /// Cancel all tasks in a group.
    pub async fn cancel_group(&self, group_key: &str) -> Result<Vec<i64>, StoreError> {
        let tasks = self.inner.store.tasks_by_group(group_key).await?;
        let mut cancelled = Vec::new();
        for task in &tasks {
            if self.cancel(task.id).await? {
                cancelled.push(task.id);
            }
        }
        Ok(cancelled)
    }

    /// Cancel all tasks of a given type.
    pub async fn cancel_type(&self, task_type: &str) -> Result<Vec<i64>, StoreError> {
        let tasks = self.inner.store.tasks_by_type(task_type).await?;
        let mut cancelled = Vec::new();
        for task in &tasks {
            if self.cancel(task.id).await? {
                cancelled.push(task.id);
            }
        }
        Ok(cancelled)
    }

    /// Cancel all tasks matching a predicate.
    pub async fn cancel_where(
        &self,
        predicate: impl Fn(&TaskRecord) -> bool,
    ) -> Result<Vec<i64>, StoreError> {
        let tasks = self.inner.store.all_active_tasks().await?;
        let mut cancelled = Vec::new();
        for task in &tasks {
            if predicate(task) && self.cancel(task.id).await? {
                cancelled.push(task.id);
            }
        }
        Ok(cancelled)
    }

    /// Fire the `on_cancel` hook for a task (fire-and-forget).
    ///
    /// Takes a snapshot of the app state and spawns a tokio task that runs
    /// the executor's `on_cancel` method with a timeout. Errors and timeouts
    /// are logged but not propagated.
    async fn fire_on_cancel(&self, record: &TaskRecord) {
        let Some(executor) = self.inner.registry.get(&record.task_type) else {
            return;
        };
        let executor = executor.clone();
        let record = record.clone();
        let timeout = self.inner.cancel_hook_timeout;
        let app_state = self.inner.app_state.snapshot().await;
        let scheduler = self.downgrade();
        let event_tx = self.inner.event_tx.clone();
        let active = self.inner.active.clone();

        tokio::spawn(async move {
            let fresh_token = CancellationToken::new();
            let io = Arc::new(IoTracker::new());
            let ctx = TaskContext {
                record: record.clone(),
                token: fresh_token,
                progress: ProgressReporter::new(
                    record.event_header(),
                    event_tx,
                    active,
                    io.clone(),
                ),
                scheduler,
                app_state,
                child_spawner: None,
                io,
            };

            match tokio::time::timeout(timeout, executor.on_cancel_erased(&ctx)).await {
                Ok(Ok(())) => {
                    tracing::debug!(
                        task_id = record.id,
                        task_type = record.task_type,
                        "on_cancel hook completed"
                    );
                }
                Ok(Err(e)) => {
                    tracing::warn!(
                        task_id = record.id,
                        task_type = record.task_type,
                        error = %e,
                        "on_cancel hook failed"
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        task_id = record.id,
                        task_type = record.task_type,
                        "on_cancel hook timed out"
                    );
                }
            }
        });
    }
}
