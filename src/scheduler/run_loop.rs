//! The main scheduler run loop, dispatch logic, and shutdown.

use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::store::StoreError;
use crate::task::IoBudget;

use super::dispatch::{self, SpawnContext};
use super::gate::GateContext;
use super::{Scheduler, ShutdownMode};

impl Scheduler {
    /// Build a [`SpawnContext`] from current scheduler state.
    async fn build_spawn_context(&self) -> SpawnContext {
        SpawnContext {
            store: self.inner.store.clone(),
            active: self.inner.active.clone(),
            event_tx: self.inner.event_tx.clone(),
            max_retries: self.inner.max_retries,
            app_state: self.inner.app_state.snapshot().await,
            work_notify: Arc::clone(&self.inner.work_notify),
            scheduler: self.downgrade(),
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

        // Peek at the next candidate without changing its status.
        let Some(candidate) = self.inner.store.peek_next().await? else {
            return Ok(false);
        };

        // Build gate context from current state.
        let reader_guard = self.inner.resource_reader.lock().await;
        let gate_ctx = GateContext {
            store: &self.inner.store,
            resource_reader: reader_guard.as_ref(),
            group_limits: Some(&self.inner.group_limits),
        };

        // Admission check while the task is still pending — no running
        // window if the gate rejects.
        if !self.inner.gate.admit(&candidate, &gate_ctx).await? {
            drop(reader_guard);
            return Ok(false);
        }
        drop(reader_guard);

        // Atomically claim the task. Returns None if another dispatcher
        // claimed it (or it was cancelled) between peek and now.
        let Some(task) = self.inner.store.pop_by_id(candidate.id).await? else {
            return Ok(false);
        };

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
                )
                .await?;
            return Ok(true);
        };
        let executor = Arc::clone(executor);

        // Spawn the task — this inserts into the active map, builds the
        // context, emits Dispatched, and wires up completion handling.
        dispatch::spawn_task(
            task,
            executor,
            self.build_spawn_context().await,
            dispatch::ExecutionPhase::Execute,
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
                )
                .await?;
            return Ok(true);
        };
        let executor = Arc::clone(executor);

        dispatch::spawn_task(
            task,
            executor,
            self.build_spawn_context().await,
            dispatch::ExecutionPhase::Finalize,
        )
        .await;

        Ok(true)
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

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    tracing::info!("taskmill scheduler shutting down");
                    self.shutdown().await;
                    break;
                }
                _ = self.inner.work_notify.notified() => {
                    self.poll_and_dispatch().await;
                }
                _ = tokio::time::sleep(self.inner.poll_interval) => {
                    self.poll_and_dispatch().await;
                }
            }
        }
    }

    /// Resume paused tasks, dispatch finalizers, and dispatch pending work.
    async fn poll_and_dispatch(&self) {
        if self.is_paused() {
            return;
        }

        // Resume paused tasks only if no active preemptors exist.
        if let Ok(paused) = self.inner.store.paused_tasks().await {
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

        // Try to dispatch tasks until we can't.
        loop {
            match self.try_dispatch().await {
                Ok(true) => continue,
                Ok(false) => break,
                Err(e) => {
                    tracing::error!(error = %e, "scheduler dispatch error");
                    break;
                }
            }
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

        // Flush WAL and close the database.
        self.inner.store.close().await;
    }
}
