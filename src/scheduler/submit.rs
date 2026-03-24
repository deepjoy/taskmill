//! Task submission (single, batch, typed), lookup, and cancellation.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::priority::Priority;
use crate::registry::{IoTracker, StateSnapshot, TaskContext};
use crate::store::StoreError;
use crate::task::{
    generate_dedup_key, BatchOutcome, BatchSubmission, HistoryStatus, SubmitOutcome, TaskLookup,
    TaskRecord, TaskSubmission, TypedTask,
};

use super::progress::ProgressReporter;
use super::{emit_event, Scheduler, SchedulerEvent};

impl Scheduler {
    /// Resolve the effective TTL for a submission.
    ///
    /// Priority: per-task TTL > per-type TTL > global default TTL > None
    fn resolve_ttl(&self, sub: &mut TaskSubmission) {
        if sub.ttl.is_some() {
            return; // per-task TTL takes precedence
        }
        if let Some(type_ttl) = self.inner.registry.type_ttl(&sub.task_type) {
            sub.ttl = Some(*type_ttl);
            return;
        }
        if let Some(default_ttl) = self.inner.default_ttl {
            sub.ttl = Some(default_ttl);
        }
    }

    /// Submit a task.
    ///
    /// If the task's priority meets the preemption threshold, running tasks
    /// with lower priority are preempted (their cancellation tokens are cancelled
    /// and they are paused in the store).
    pub async fn submit(&self, sub: &TaskSubmission) -> Result<SubmitOutcome, StoreError> {
        let mut sub = sub.clone();
        self.resolve_ttl(&mut sub);
        let outcome = self.inner.store.submit(&sub).await?;

        // Handle superseded tasks.
        if let SubmitOutcome::Superseded {
            new_task_id,
            replaced_task_id,
        } = &outcome
        {
            self.handle_superseded_active(*replaced_task_id).await;
            // Emit superseded event — we need the old record's header.
            // The old task is already in history, so build header from
            // submission info.
            let priority = sub.priority;
            let old_header = super::event::TaskEventHeader {
                task_id: *replaced_task_id,
                module: sub
                    .task_type
                    .split_once("::")
                    .map(|(n, _)| n.to_string())
                    .unwrap_or_default(),
                task_type: sub.task_type.clone(),
                key: sub.effective_key(),
                label: sub.label.clone(),
                tags: sub.tags.clone(),
                base_priority: priority,
                effective_priority: priority,
            };
            emit_event(
                &self.inner.event_tx,
                SchedulerEvent::Superseded {
                    old: old_header,
                    new_task_id: *new_task_id,
                },
            );
        }

        if !matches!(outcome, SubmitOutcome::Duplicate | SubmitOutcome::Rejected) {
            // Preempt if this is a high-priority task.
            if sub.priority.value() <= self.inner.preempt_priority.value() {
                let preempted = self
                    .inner
                    .active
                    .preempt_below(sub.priority, &self.inner.store, &self.inner.event_tx)
                    .await;
                if !preempted.is_empty() {
                    self.inner
                        .has_paused_tasks
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                }
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
        // Resolve TTLs for all submissions.
        let mut resolved: Vec<TaskSubmission> = submissions.to_vec();
        for sub in &mut resolved {
            self.resolve_ttl(sub);
        }
        let results = self.inner.store.submit_batch(&resolved).await?;

        // Handle superseded tasks.
        for (sub, outcome) in resolved.iter().zip(results.iter()) {
            if let SubmitOutcome::Superseded {
                new_task_id,
                replaced_task_id,
            } = outcome
            {
                self.handle_superseded_active(*replaced_task_id).await;
                let priority = sub.priority;
                let old_header = super::event::TaskEventHeader {
                    task_id: *replaced_task_id,
                    module: sub
                        .task_type
                        .split_once("::")
                        .map(|(n, _)| n.to_string())
                        .unwrap_or_default(),
                    task_type: sub.task_type.clone(),
                    key: sub.effective_key(),
                    label: sub.label.clone(),
                    tags: sub.tags.clone(),
                    base_priority: priority,
                    effective_priority: priority,
                };
                emit_event(
                    &self.inner.event_tx,
                    SchedulerEvent::Superseded {
                        old: old_header,
                        new_task_id: *new_task_id,
                    },
                );
            }
        }

        // Find the highest (lowest numeric value) priority among tasks that
        // were inserted or had their priority upgraded.
        let best_priority = resolved
            .iter()
            .zip(results.iter())
            .filter(|(_, outcome)| {
                !matches!(outcome, SubmitOutcome::Duplicate | SubmitOutcome::Rejected)
            })
            .map(|(sub, _)| sub.priority)
            .min_by_key(|p| p.value());

        let any_changed = results
            .iter()
            .any(|o| !matches!(o, SubmitOutcome::Duplicate | SubmitOutcome::Rejected));

        if let Some(priority) = best_priority {
            if priority.value() <= self.inner.preempt_priority.value() {
                let preempted = self
                    .inner
                    .active
                    .preempt_below(priority, &self.inner.store, &self.inner.event_tx)
                    .await;
                if !preempted.is_empty() {
                    self.inner
                        .has_paused_tasks
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }

        let outcome = BatchOutcome { outcomes: results };

        if any_changed {
            let inserted_ids = outcome.inserted();
            emit_event(
                &self.inner.event_tx,
                SchedulerEvent::BatchSubmitted {
                    count: resolved.len(),
                    inserted_ids,
                },
            );

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
    /// Uses the priority from [`TypedTask::config()`].
    pub async fn submit_typed<T: TypedTask>(&self, task: &T) -> Result<SubmitOutcome, StoreError> {
        let sub = TaskSubmission::from_typed(task);
        self.submit(&sub).await
    }

    /// Submit a [`TypedTask`] with an explicit priority override.
    ///
    /// The provided `priority` replaces whatever [`TypedTask::config()`]
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
    /// token, fires the [`on_cancel`](crate::TypedExecutor::on_cancel) hook,
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
                emit_event(
                    &self.inner.event_tx,
                    SchedulerEvent::Cancelled(at.record.event_header()),
                );
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
            emit_event(
                &self.inner.event_tx,
                SchedulerEvent::Cancelled(at.record.event_header()),
            );
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

    /// Cancel all active tasks matching a tag key-value pair.
    ///
    /// Uses [`TaskStore::task_ids_by_tags`](crate::TaskStore::task_ids_by_tags) (ID-only query)
    /// to avoid full record deserialization and tag population overhead.
    /// Returns the ids of tasks that were successfully cancelled.
    pub async fn cancel_by_tag(&self, key: &str, value: &str) -> Result<Vec<i64>, StoreError> {
        let ids = self
            .inner
            .store
            .task_ids_by_tags(&[(key, value)], None)
            .await?;
        let mut cancelled = Vec::new();
        for id in ids {
            if self.cancel(id).await? {
                cancelled.push(id);
            }
        }
        Ok(cancelled)
    }

    /// Cancel all active tasks that have any tag key matching the given prefix.
    ///
    /// Uses [`crate::TaskStore::task_ids_by_tag_key_prefix`] for ID-only lookup.
    /// Returns the ids of tasks that were successfully cancelled.
    pub async fn cancel_by_tag_key_prefix(&self, prefix: &str) -> Result<Vec<i64>, StoreError> {
        let ids = self
            .inner
            .store
            .task_ids_by_tag_key_prefix(prefix, None)
            .await?;
        let mut cancelled = Vec::new();
        for id in ids {
            if self.cancel(id).await? {
                cancelled.push(id);
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

    /// Pause a recurring schedule. The current instance (if running) is
    /// not affected, but no new instances will be created on completion.
    pub async fn pause_recurring(&self, task_id: i64) -> Result<(), StoreError> {
        self.inner.store.pause_recurring(task_id).await
    }

    /// Resume a paused recurring schedule.
    pub async fn resume_recurring(&self, task_id: i64) -> Result<(), StoreError> {
        self.inner.store.resume_recurring(task_id).await
    }

    /// Cancel a recurring schedule entirely. Cancels any pending instance
    /// and prevents future ones.
    pub async fn cancel_recurring(&self, task_id: i64) -> Result<bool, StoreError> {
        // If it's active (running), cancel via the normal cancel path.
        if let Some(at) = self.inner.active.remove(task_id) {
            at.token.cancel();
            self.inner
                .store
                .cancel_to_history_with_record(&at.record)
                .await?;
            self.fire_on_cancel(&at.record).await;
            emit_event(
                &self.inner.event_tx,
                SchedulerEvent::Cancelled(at.record.event_header()),
            );
            return Ok(true);
        }
        self.inner.store.cancel_recurring(task_id).await
    }

    /// Handle the scheduler-side effects of a superseded task.
    ///
    /// If the replaced task was running (in the active map), cancel its token,
    /// fire the on_cancel hook, cascade-cancel its children, and emit events.
    async fn handle_superseded_active(&self, replaced_task_id: i64) {
        // Cancel children of the replaced task.
        if let Ok(running_child_ids) = self.inner.store.cancel_children(replaced_task_id).await {
            for child_id in &running_child_ids {
                if let Some(at) = self.inner.active.remove(*child_id) {
                    at.token.cancel();
                    let _ = self
                        .inner
                        .store
                        .cancel_to_history_with_record(&at.record)
                        .await;
                    self.fire_on_cancel(&at.record).await;
                    let _ = self
                        .inner
                        .event_tx
                        .send(SchedulerEvent::Cancelled(at.record.event_header()));
                }
            }
        }

        // Cancel the replaced task itself if it's running.
        if let Some(at) = self.inner.active.remove(replaced_task_id) {
            at.token.cancel();
            self.fire_on_cancel(&at.record).await;
            emit_event(
                &self.inner.event_tx,
                SchedulerEvent::Cancelled(at.record.event_header()),
            );
        }
    }

    /// Re-submit a dead-lettered task.
    ///
    /// Loads the history record, verifies it has `dead_letter` status, constructs
    /// a new [`TaskSubmission`] from its fields, submits it (going through normal
    /// dedup and validation), and removes the history row on success.
    ///
    /// The re-submitted task starts with `retry_count = 0`.
    pub async fn retry_dead_letter(&self, history_id: i64) -> Result<SubmitOutcome, StoreError> {
        let record = self
            .inner
            .store
            .history_by_id(history_id)
            .await?
            .ok_or_else(|| StoreError::NotFound(format!("history record {history_id}")))?;

        if record.status != HistoryStatus::DeadLetter {
            return Err(StoreError::InvalidState(format!(
                "history record {history_id} has status {:?}, expected DeadLetter",
                record.status
            )));
        }

        // Reconstruct a submission from the history record.
        // If label != task_type, the original submission had an explicit dedup
        // key (label preserves the original key string). Otherwise, the key
        // was derived from the payload hash.
        let mut sub = TaskSubmission::new(&record.task_type)
            .priority(record.priority)
            .expected_io(record.expected_io)
            .tags(record.tags.clone());

        if record.label != record.task_type {
            sub = sub.key(&record.label);
        }

        if let Some(payload) = &record.payload {
            sub = sub.payload_raw(payload.clone());
        }
        if let Some(parent_id) = record.parent_id {
            sub.parent_id = Some(parent_id);
        }
        sub.fail_fast = record.fail_fast;
        if let Some(ref gk) = record.group_key {
            sub.group_key = Some(gk.clone());
        }
        if let Some(mr) = record.max_retries {
            sub.max_retries = Some(mr);
        }

        let outcome = self.submit(&sub).await?;

        // Remove the dead-letter history row on successful submission.
        if outcome.is_inserted() {
            self.inner.store.delete_history(history_id).await?;
        }

        Ok(outcome)
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
        let module_registry = Arc::clone(&self.inner.module_registry);
        let owning_module: String = record
            .task_type
            .split_once("::")
            .map(|(n, _)| n.to_string())
            .unwrap_or_default();

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
                module_state: StateSnapshot::default(),
                child_spawner: None,
                io,
                module_registry,
                owning_module,
                aging_config: None,
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
