//! The main scheduler run loop, dispatch logic, and shutdown.

use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::store::StoreError;
use crate::task::IoBudget;

use super::SchedulerEvent;

use super::gate::GateContext;
use super::spawn::{self, SpawnContext};
use super::{Scheduler, ShutdownMode};

impl Scheduler {
    /// Build a [`SpawnContext`] from current scheduler state.
    async fn build_spawn_context(&self) -> SpawnContext {
        SpawnContext {
            store: self.inner.store.clone(),
            active: self.inner.active.clone(),
            event_tx: self.inner.event_tx.clone(),
            max_retries: self.inner.max_retries,
            registry: Arc::clone(&self.inner.registry),
            app_state: self.inner.app_state.snapshot().await,
            work_notify: Arc::clone(&self.inner.work_notify),
            scheduler: self.downgrade(),
            cancel_hook_timeout: self.inner.cancel_hook_timeout,
            module_running: Arc::clone(&self.inner.module_running),
            module_state: Arc::clone(&self.inner.module_state),
            module_registry: Arc::clone(&self.inner.module_registry),
            completion_tx: self.inner.completion_tx.clone(),
            completion_rx: self.inner.completion_rx.clone(),
            failure_tx: self.inner.failure_tx.clone(),
            failure_rx: self.inner.failure_rx.clone(),
        }
    }

    /// Try to pop and execute the next task.
    ///
    /// Returns `true` if a task was dispatched, `false` if no work was available
    /// (empty queue, concurrency limit, IO budget exhausted, or throttled).
    pub async fn try_dispatch(&self) -> Result<bool, StoreError> {
        // Check concurrency limit.
        let active_count = self.inner.active.count();
        let max = self.inner.max_concurrency.load(AtomicOrdering::Relaxed);
        if active_count >= max {
            return Ok(false);
        }

        // Fast path: no gate checks needed, use pop_next() (single SQL)
        // instead of peek_next() + gate.admit() + claim_task() (2 SQL).
        // pop_next() skips expired tasks via its WHERE clause.
        if self.inner.fast_dispatch.load(AtomicOrdering::Relaxed) {
            let Some(mut task) = self.inner.store.pop_next().await? else {
                return Ok(false);
            };
            self.inner
                .store
                .populate_tags(std::slice::from_mut(&mut task))
                .await?;
            return self.spawn_dispatched_task(task).await;
        }

        // Slow path: peek → gate check → claim.
        let Some(mut candidate) = self.inner.store.peek_next().await? else {
            return Ok(false);
        };
        self.inner
            .store
            .populate_tags(std::slice::from_mut(&mut candidate))
            .await?;

        // Dispatch-time expiry check: if the candidate has expired, expire
        // it and retry (return Ok(true) to loop again).
        if let Some(expires_at) = candidate.expires_at {
            if expires_at <= chrono::Utc::now() {
                if let Ok(Some(task)) = self.inner.store.expire_single(candidate.id).await {
                    let age = (chrono::Utc::now() - task.created_at)
                        .to_std()
                        .unwrap_or_default();
                    let _ = self.inner.event_tx.send(SchedulerEvent::TaskExpired {
                        header: task.event_header(),
                        age,
                    });
                }
                return Ok(true);
            }
        }

        // Build gate context from current state.
        let reader_guard = self.inner.resource_reader.lock().await;
        let gate_ctx = GateContext {
            store: &self.inner.store,
            resource_reader: reader_guard.as_ref(),
            group_limits: Some(&self.inner.group_limits),
            module_caps: &self.inner.module_caps,
            module_running: &self.inner.module_running,
        };

        // Admission check while the task is still pending — no running
        // window if the gate rejects.
        if !self.inner.gate.admit(&candidate, &gate_ctx).await? {
            drop(reader_guard);
            return Ok(false);
        }
        drop(reader_guard);

        // Atomically claim the task. We already have the full record from
        // peek_next, so use claim_task (no RETURNING *) and patch in-memory.
        if !self.inner.store.claim_task(candidate.id).await? {
            return Ok(false);
        }
        let mut task = candidate;
        task.status = crate::task::TaskStatus::Running;
        task.started_at = Some(chrono::Utc::now());
        // Mirror the SQL TTL logic for first-attempt tasks.
        if task.ttl_from == crate::task::TtlFrom::FirstAttempt && task.expires_at.is_none() {
            if let Some(ttl) = task.ttl_seconds {
                task.expires_at = Some(chrono::Utc::now() + chrono::Duration::seconds(ttl));
            }
        }

        self.spawn_dispatched_task(task).await
    }

    /// Look up executor and spawn a task that is already in the `running` state.
    async fn spawn_dispatched_task(
        &self,
        task: crate::task::TaskRecord,
    ) -> Result<bool, StoreError> {
        // Look up executor.
        let Some(executor) = self.inner.registry.get(&task.task_type) else {
            tracing::error!(
                task_type = task.task_type,
                "no executor registered — failing task"
            );
            self.inner
                .store
                .fail_with_record(
                    &task,
                    &format!("no executor registered for type '{}'", task.task_type),
                    false,
                    0,
                    &IoBudget::default(),
                    &Default::default(),
                )
                .await?;
            return Ok(true);
        };
        let executor = Arc::clone(executor);

        // Spawn the task — this inserts into the active map, builds the
        // context, emits Dispatched, and wires up completion handling.
        spawn::spawn_task(
            task,
            executor,
            self.build_spawn_context().await,
            spawn::ExecutionPhase::Execute,
        )
        .await;

        Ok(true)
    }

    /// Try to dispatch a parent task for its finalize phase.
    ///
    /// Returns `true` if a finalizer was dispatched.
    async fn try_dispatch_finalizer(&self) -> Result<bool, StoreError> {
        // Pop the next pending finalizer.
        let parent_id = {
            let mut finalizers = self.inner.active.pending_finalizers.lock().unwrap();
            let Some(&id) = finalizers.iter().next() else {
                return Ok(false);
            };
            finalizers.remove(&id);
            id
        };

        // Transition the parent from waiting to running for finalize.
        self.inner.store.set_running_for_finalize(parent_id).await?;

        // Fetch the parent record (now running).
        let Some(task) = self.inner.store.task_by_id(parent_id).await? else {
            return Ok(false);
        };

        // Look up executor.
        let Some(executor) = self.inner.registry.get(&task.task_type) else {
            tracing::error!(
                task_type = task.task_type,
                "no executor registered for finalize — failing parent"
            );
            self.inner
                .store
                .fail(
                    parent_id,
                    "no executor for finalize",
                    false,
                    0,
                    &IoBudget::default(),
                    &Default::default(),
                )
                .await?;
            return Ok(true);
        };
        let executor = Arc::clone(executor);

        spawn::spawn_task(
            task,
            executor,
            self.build_spawn_context().await,
            spawn::ExecutionPhase::Finalize,
        )
        .await;

        Ok(true)
    }

    /// Dispatch pending tasks using batch pop on the fast path (no gate
    /// checks), falling back to one-at-a-time dispatch on the slow path.
    async fn dispatch_pending(&self) -> Result<(), StoreError> {
        if self.inner.fast_dispatch.load(AtomicOrdering::Relaxed) {
            loop {
                let active_count = self.inner.active.count();
                let max = self.inner.max_concurrency.load(AtomicOrdering::Relaxed);
                if active_count >= max {
                    break;
                }
                let available = max - active_count;
                let mut tasks = self.inner.store.pop_next_batch(available).await?;
                if tasks.is_empty() {
                    break;
                }
                // Populate tags in one batch query (lazy — not done at pop time).
                self.inner.store.populate_tags(&mut tasks).await?;
                for task in tasks {
                    self.spawn_dispatched_task(task).await?;
                }
            }
        } else {
            loop {
                match self.try_dispatch().await {
                    Ok(true) => continue,
                    Ok(false) => break,
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(())
    }

    /// Run the scheduler loop until the cancellation token is triggered.
    ///
    /// This is the main entry point. The loop wakes on three conditions:
    /// 1. Cancellation — triggers shutdown.
    /// 2. Notification — a task was submitted or the scheduler was resumed.
    /// 3. Poll interval — periodic housekeeping (e.g. resuming paused tasks).
    ///
    /// On mobile targets (iOS/Android), the notify-based wake avoids the
    /// constant 500ms polling that would otherwise prevent the CPU from sleeping.
    pub async fn run(&self, token: CancellationToken) {
        tracing::info!(
            max_concurrency = self.inner.max_concurrency.load(AtomicOrdering::Relaxed),
            "taskmill scheduler started"
        );

        // Spawn the progress ticker if configured.
        if let Some(interval) = self.inner.progress_interval {
            let ticker_token = self.inner.progress_ticker_token.clone();
            tokio::spawn(super::progress::run_progress_ticker(
                self.inner.active.clone(),
                self.inner.progress_tx.clone(),
                interval,
                ticker_token,
            ));
        }

        // Track whether we need to query next_run_after. Starts true
        // (conservative). Cleared when the query returns None (no scheduled
        // tasks). Re-set when a notification arrives (new submit may have
        // set run_after) or when we previously found scheduled tasks.
        let mut check_scheduled = true;

        loop {
            let sleep_dur = if check_scheduled {
                match self.inner.store.next_run_after().await {
                    Ok(Some(next)) => {
                        let until_next = (next - chrono::Utc::now())
                            .to_std()
                            .unwrap_or(std::time::Duration::ZERO);
                        std::cmp::min(self.inner.poll_interval, until_next)
                    }
                    _ => {
                        check_scheduled = false;
                        self.inner.poll_interval
                    }
                }
            } else {
                self.inner.poll_interval
            };

            tokio::select! {
                _ = token.cancelled() => {
                    tracing::info!("taskmill scheduler shutting down");
                    self.shutdown().await;
                    break;
                }
                _ = self.inner.work_notify.notified() => {
                    // New work submitted — may include run_after tasks.
                    check_scheduled = true;
                    self.poll_and_dispatch().await;
                }
                _ = tokio::time::sleep(sleep_dur) => {
                    self.poll_and_dispatch().await;
                }
            }
        }
    }

    /// Run the periodic expiry sweep if the interval has elapsed.
    async fn maybe_expire_tasks(&self) {
        let Some(interval) = self.inner.expiry_sweep_interval else {
            return;
        };
        let should_sweep = {
            let last = self.inner.last_expiry_sweep.lock().unwrap();
            last.elapsed() >= interval
        };
        if !should_sweep {
            return;
        }
        *self.inner.last_expiry_sweep.lock().unwrap() = tokio::time::Instant::now();

        match self.inner.store.expire_tasks().await {
            Ok(expired) => {
                for task in &expired {
                    let age = (chrono::Utc::now() - task.created_at)
                        .to_std()
                        .unwrap_or_default();
                    let _ = self.inner.event_tx.send(SchedulerEvent::TaskExpired {
                        header: task.event_header(),
                        age,
                    });
                }
                if !expired.is_empty() {
                    tracing::info!(count = expired.len(), "expired stale tasks");
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "expiry sweep failed");
            }
        }
    }

    /// Drain the completion channel and process all queued completions in a
    /// single batched transaction.
    async fn drain_completions(&self) {
        let mut batch = Vec::new();
        {
            let mut rx = self.inner.completion_rx.lock().await;
            while let Ok(msg) = rx.try_recv() {
                batch.push(msg);
            }
        }

        if batch.is_empty() {
            return;
        }

        tracing::debug!(count = batch.len(), "draining completion batch");
        spawn::process_completion_batch(
            &batch,
            &self.inner.store,
            &self.inner.event_tx,
            &self.inner.active,
            self.inner.max_retries,
            &self.inner.work_notify,
        )
        .await;
    }

    /// Drain the failure channel and process all queued terminal failures
    /// in a single batched transaction.
    async fn drain_failures(&self) {
        let mut batch = Vec::new();
        {
            let mut rx = self.inner.failure_rx.lock().await;
            while let Ok(msg) = rx.try_recv() {
                batch.push(msg);
            }
        }

        if batch.is_empty() {
            return;
        }

        tracing::debug!(count = batch.len(), "draining failure batch");
        spawn::process_failure_batch(&batch, &self.inner.store, &self.inner.event_tx).await;
    }

    /// Resume paused tasks, dispatch finalizers, and dispatch pending work.
    async fn poll_and_dispatch(&self) {
        if self.is_paused() {
            return;
        }

        // Drain queued completions and failures before dispatching new work.
        self.drain_completions().await;
        self.drain_failures().await;

        // Run expiry sweep before dispatching.
        self.maybe_expire_tasks().await;

        // Resume paused tasks only if no active preemptors exist.
        // Skip the query entirely when no tasks have been paused.
        if self.inner.has_paused_tasks.load(AtomicOrdering::Relaxed) {
            if let Ok(paused) = self.inner.store.paused_tasks().await {
                if paused.is_empty() {
                    self.inner
                        .has_paused_tasks
                        .store(false, AtomicOrdering::Relaxed);
                }
                for task in paused {
                    if !self
                        .inner
                        .active
                        .has_preemptors_for(task.priority, self.inner.preempt_priority)
                    {
                        let _ = self.inner.store.resume(task.id).await;
                    }
                }
            }
        }

        // Dispatch any pending finalizers (parent tasks ready for finalize phase).
        loop {
            match self.try_dispatch_finalizer().await {
                Ok(true) => continue,
                Ok(false) => break,
                Err(e) => {
                    tracing::error!(error = %e, "scheduler finalizer dispatch error");
                    break;
                }
            }
        }

        // Dispatch pending tasks — batch pop on the fast path, one-at-a-time
        // with gate checks on the slow path.
        if let Err(e) = self.dispatch_pending().await {
            tracing::error!(error = %e, "scheduler dispatch error");
        }
    }

    /// Perform shutdown according to the configured `ShutdownMode`.
    async fn shutdown(&self) {
        // Stop the resource sampler and progress ticker.
        self.inner.sampler_token.cancel();
        self.inner.progress_ticker_token.cancel();

        match self.inner.shutdown_mode {
            ShutdownMode::Hard => {
                self.inner.active.cancel_all();
            }
            ShutdownMode::Graceful(timeout) => {
                tracing::info!(
                    timeout_ms = timeout.as_millis() as u64,
                    "graceful shutdown — waiting for running tasks"
                );

                // Cancel all tokens and collect handles for joining.
                let handles = self.inner.active.cancel_and_drain_handles();
                let deadline = tokio::time::Instant::now() + timeout;

                for handle in handles {
                    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                    if remaining.is_zero() {
                        tracing::warn!("graceful shutdown timeout — aborting remaining tasks");
                        handle.abort();
                        continue;
                    }
                    if tokio::time::timeout(remaining, handle).await.is_err() {
                        tracing::warn!("task did not finish within graceful shutdown timeout");
                    }
                }

                tracing::info!("graceful shutdown complete");
            }
        }

        // Drain any remaining completions and failures before closing the store.
        self.drain_completions().await;
        self.drain_failures().await;

        // Flush WAL and close the database.
        self.inner.store.close().await;
    }
}
