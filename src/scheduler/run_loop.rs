//! The main scheduler run loop, dispatch logic, and shutdown.

use std::collections::HashMap;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::scheduler::aging::AgingParams;
use crate::scheduler::fair::{self, GroupDemand};
use crate::store::StoreError;
use crate::task::IoBudget;

use super::{emit_event, SchedulerEvent};

use super::gate::{Admission, GateContext};
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
            aging_config: self.inner.aging_config.clone(),
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

        // Compute aging params once per dispatch attempt.
        let aging = self
            .inner
            .aging_config
            .as_ref()
            .map(|c| AgingParams::from_config(c));

        // Fast path: no gate checks needed, use pop_next() (single SQL)
        // instead of peek_next() + gate.admit() + claim_task() (2 SQL).
        // pop_next() skips expired tasks via its WHERE clause.
        if self.inner.fast_dispatch.load(AtomicOrdering::Relaxed) {
            let Some(mut task) = self.inner.store.pop_next(aging.as_ref()).await? else {
                return Ok(false);
            };
            self.inner
                .store
                .populate_tags(std::slice::from_mut(&mut task))
                .await?;
            return self.spawn_dispatched_task(task).await;
        }

        // Slow path: peek → gate check → claim.
        let Some(mut candidate) = self.inner.store.peek_next(aging.as_ref()).await? else {
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
                    emit_event(
                        &self.inner.event_tx,
                        SchedulerEvent::TaskExpired {
                            header: task.event_header(),
                            age,
                        },
                    );
                }
                return Ok(true);
            }
        }

        // Build gate context from current state.
        let reader_guard = self.inner.resource_reader.lock().await;
        let paused_groups = self.inner.paused_groups.read().unwrap().clone();
        let gate_ctx = GateContext {
            store: &self.inner.store,
            resource_reader: reader_guard.as_ref(),
            group_limits: Some(&self.inner.group_limits),
            module_caps: &self.inner.module_caps,
            module_running: &self.inner.module_running,
            paused_groups: &paused_groups,
            type_rate_limits: &self.inner.type_rate_limits,
            group_rate_limits: &self.inner.group_rate_limits,
            skip_group_concurrency: false,
        };

        // Admission check while the task is still pending — no running
        // window if the gate rejects.
        match self.inner.gate.admit(&candidate, &gate_ctx).await? {
            Admission::Admit => { /* proceed to claim */ }
            Admission::Deny => {
                drop(reader_guard);
                return Ok(false);
            }
            Admission::RateLimited(next) => {
                drop(reader_guard);
                // Set run_after to push the task out of the peek window,
                // preventing head-of-line blocking. Other task types can
                // still dispatch while this one waits for a token.
                let wait = next.duration_since(tokio::time::Instant::now());
                let run_after = chrono::Utc::now()
                    + chrono::Duration::from_std(wait).unwrap_or(chrono::Duration::milliseconds(1));
                self.inner
                    .store
                    .set_run_after(candidate.id, run_after)
                    .await?;
                // Return true to keep looping — another task may be eligible.
                return Ok(true);
            }
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

        // Check if parent's group is paused — defer finalize if so.
        if let Ok(Some(parent_record)) = self.inner.store.task_by_id(parent_id).await {
            if let Some(ref gk) = parent_record.group_key {
                if self.inner.paused_groups.read().unwrap().contains(gk) {
                    // Re-insert into pending_finalizers for later.
                    self.inner
                        .active
                        .pending_finalizers
                        .lock()
                        .unwrap()
                        .insert(parent_id);
                    return Ok(false);
                }
            }
        }

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

    /// Dispatch pending tasks using fair scheduling (if configured), batch
    /// pop on the fast path, or one-at-a-time on the slow path.
    async fn dispatch_pending(&self) -> Result<(), StoreError> {
        if self.inner.group_weights.is_configured() {
            return self.dispatch_fair().await;
        }
        if self.inner.fast_dispatch.load(AtomicOrdering::Relaxed) {
            let aging = self
                .inner
                .aging_config
                .as_ref()
                .map(|c| AgingParams::from_config(c));
            loop {
                let active_count = self.inner.active.count();
                let max = self.inner.max_concurrency.load(AtomicOrdering::Relaxed);
                if active_count >= max {
                    break;
                }
                let available = max - active_count;
                let mut tasks = self
                    .inner
                    .store
                    .pop_next_batch(available, aging.as_ref())
                    .await?;
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

    /// Three-pass fair dispatch loop.
    ///
    /// Pass 1 (fair): Each group gets slots proportional to its weight.
    /// Pass 2 (greedy): Unfilled slots filled by global priority order.
    /// Pass 3 (urgent): Tasks aged past urgent_threshold bypass weights.
    async fn dispatch_fair(&self) -> Result<(), StoreError> {
        let active_count = self.inner.active.count();
        let max = self.inner.max_concurrency.load(AtomicOrdering::Relaxed);
        if active_count >= max {
            return Ok(());
        }

        let now_ms = chrono::Utc::now().timestamp_millis();
        let aging = self
            .inner
            .aging_config
            .as_ref()
            .map(|c| AgingParams::from_config(c));

        // Gather demand.
        let running = self.inner.store.running_counts_per_group().await?;
        let pending = self.inner.store.pending_counts_per_group().await?;
        let paused = self.inner.paused_groups.read().unwrap().clone();

        let running_map: HashMap<Option<String>, usize> = running.iter().cloned().collect();

        let demand = merge_demand(&running, &pending);
        let allocation = fair::compute_allocation(
            max,
            &demand,
            &self.inner.group_weights,
            &self.inner.group_limits,
            &paused,
        );

        let mut dispatched = 0;

        // ── Pass 1: Fair per-group dispatch ────────────────────────
        for (group, total_slots) in &allocation.groups {
            let group_running = running_map.get(group).copied().unwrap_or(0);
            let available = total_slots.saturating_sub(group_running);
            for _ in 0..available {
                if self.inner.active.count() + dispatched >= max {
                    break;
                }
                let candidate = match group {
                    Some(g) => {
                        self.inner
                            .store
                            .peek_next_in_group(g, aging.as_ref())
                            .await?
                    }
                    None => self.inner.store.peek_next_ungrouped(aging.as_ref()).await?,
                };
                let Some(mut candidate) = candidate else {
                    break;
                };

                // Expiry check.
                if let Some(expires_at) = candidate.expires_at {
                    if expires_at.timestamp_millis() <= now_ms {
                        self.expire_task_inline(candidate).await?;
                        continue;
                    }
                }

                self.inner
                    .store
                    .populate_tags(std::slice::from_mut(&mut candidate))
                    .await?;

                // Gate check — skip group concurrency (allocation handles it).
                let reader_guard = self.inner.resource_reader.lock().await;
                let gate_ctx = GateContext {
                    store: &self.inner.store,
                    resource_reader: reader_guard.as_ref(),
                    group_limits: Some(&self.inner.group_limits),
                    module_caps: &self.inner.module_caps,
                    module_running: &self.inner.module_running,
                    paused_groups: &paused,
                    type_rate_limits: &self.inner.type_rate_limits,
                    group_rate_limits: &self.inner.group_rate_limits,
                    skip_group_concurrency: true,
                };

                match self.inner.gate.admit(&candidate, &gate_ctx).await? {
                    Admission::Admit => {
                        drop(reader_guard);
                        if self.inner.store.claim_task(candidate.id).await? {
                            let mut task = candidate;
                            task.status = crate::task::TaskStatus::Running;
                            task.started_at = Some(chrono::Utc::now());
                            if task.ttl_from == crate::task::TtlFrom::FirstAttempt
                                && task.expires_at.is_none()
                            {
                                if let Some(ttl) = task.ttl_seconds {
                                    task.expires_at =
                                        Some(chrono::Utc::now() + chrono::Duration::seconds(ttl));
                                }
                            }
                            self.spawn_dispatched_task(task).await?;
                            dispatched += 1;
                        }
                    }
                    Admission::RateLimited(next) => {
                        drop(reader_guard);
                        let wait = next.duration_since(tokio::time::Instant::now());
                        let run_after = chrono::Utc::now()
                            + chrono::Duration::from_std(wait)
                                .unwrap_or(chrono::Duration::milliseconds(1));
                        self.inner
                            .store
                            .set_run_after(candidate.id, run_after)
                            .await?;
                    }
                    Admission::Deny => {
                        drop(reader_guard);
                        break; // stop this group
                    }
                }
            }
        }

        // ── Pass 2: Greedy fill (work-conserving) ─────────────────
        let remaining = max.saturating_sub(self.inner.active.count() + dispatched);
        if remaining > 0 {
            for _ in 0..remaining {
                let Some(mut candidate) = self.inner.store.peek_next(aging.as_ref()).await? else {
                    break;
                };

                if let Some(expires_at) = candidate.expires_at {
                    if expires_at.timestamp_millis() <= now_ms {
                        self.expire_task_inline(candidate).await?;
                        continue;
                    }
                }

                self.inner
                    .store
                    .populate_tags(std::slice::from_mut(&mut candidate))
                    .await?;

                // Full gate check (group concurrency enforced here).
                let reader_guard = self.inner.resource_reader.lock().await;
                let gate_ctx = GateContext {
                    store: &self.inner.store,
                    resource_reader: reader_guard.as_ref(),
                    group_limits: Some(&self.inner.group_limits),
                    module_caps: &self.inner.module_caps,
                    module_running: &self.inner.module_running,
                    paused_groups: &paused,
                    type_rate_limits: &self.inner.type_rate_limits,
                    group_rate_limits: &self.inner.group_rate_limits,
                    skip_group_concurrency: false,
                };

                match self.inner.gate.admit(&candidate, &gate_ctx).await? {
                    Admission::Admit => {
                        drop(reader_guard);
                        if self.inner.store.claim_task(candidate.id).await? {
                            let mut task = candidate;
                            task.status = crate::task::TaskStatus::Running;
                            task.started_at = Some(chrono::Utc::now());
                            if task.ttl_from == crate::task::TtlFrom::FirstAttempt
                                && task.expires_at.is_none()
                            {
                                if let Some(ttl) = task.ttl_seconds {
                                    task.expires_at =
                                        Some(chrono::Utc::now() + chrono::Duration::seconds(ttl));
                                }
                            }
                            self.spawn_dispatched_task(task).await?;
                        }
                    }
                    Admission::RateLimited(next) => {
                        drop(reader_guard);
                        let wait = next.duration_since(tokio::time::Instant::now());
                        let run_after = chrono::Utc::now()
                            + chrono::Duration::from_std(wait)
                                .unwrap_or(chrono::Duration::milliseconds(1));
                        self.inner
                            .store
                            .set_run_after(candidate.id, run_after)
                            .await?;
                    }
                    Admission::Deny => {
                        drop(reader_guard);
                        break;
                    }
                }
            }
        }

        // ── Pass 3: Urgent threshold override ─────────────────────
        if let Some(config) = &self.inner.aging_config {
            if let Some(urgent) = config.urgent_threshold {
                let remaining = max.saturating_sub(self.inner.active.count());
                if remaining > 0 {
                    self.dispatch_urgent(urgent, remaining, aging.as_ref())
                        .await?;
                }
            }
        }

        Ok(())
    }

    /// Dispatch tasks whose effective priority has aged past the urgent
    /// threshold, regardless of group allocation. Respects max_concurrency.
    async fn dispatch_urgent(
        &self,
        threshold: crate::priority::Priority,
        limit: usize,
        aging: Option<&AgingParams>,
    ) -> Result<(), StoreError> {
        let paused = self.inner.paused_groups.read().unwrap().clone();
        for _ in 0..limit {
            let Some(mut candidate) = self.inner.store.peek_next_urgent(threshold, aging).await?
            else {
                break;
            };

            let now_ms = chrono::Utc::now().timestamp_millis();
            if let Some(expires_at) = candidate.expires_at {
                if expires_at.timestamp_millis() <= now_ms {
                    self.expire_task_inline(candidate).await?;
                    continue;
                }
            }

            self.inner
                .store
                .populate_tags(std::slice::from_mut(&mut candidate))
                .await?;

            // Full gate check (urgent bypasses weights, not concurrency/rate-limits).
            let reader_guard = self.inner.resource_reader.lock().await;
            let gate_ctx = GateContext {
                store: &self.inner.store,
                resource_reader: reader_guard.as_ref(),
                group_limits: Some(&self.inner.group_limits),
                module_caps: &self.inner.module_caps,
                module_running: &self.inner.module_running,
                paused_groups: &paused,
                type_rate_limits: &self.inner.type_rate_limits,
                group_rate_limits: &self.inner.group_rate_limits,
                skip_group_concurrency: false,
            };

            match self.inner.gate.admit(&candidate, &gate_ctx).await? {
                Admission::Admit => {
                    drop(reader_guard);
                    if self.inner.store.claim_task(candidate.id).await? {
                        let mut task = candidate;
                        task.status = crate::task::TaskStatus::Running;
                        task.started_at = Some(chrono::Utc::now());
                        if task.ttl_from == crate::task::TtlFrom::FirstAttempt
                            && task.expires_at.is_none()
                        {
                            if let Some(ttl) = task.ttl_seconds {
                                task.expires_at =
                                    Some(chrono::Utc::now() + chrono::Duration::seconds(ttl));
                            }
                        }
                        self.spawn_dispatched_task(task).await?;
                    }
                }
                Admission::RateLimited(next) => {
                    drop(reader_guard);
                    let wait = next.duration_since(tokio::time::Instant::now());
                    let run_after = chrono::Utc::now()
                        + chrono::Duration::from_std(wait)
                            .unwrap_or(chrono::Duration::milliseconds(1));
                    self.inner
                        .store
                        .set_run_after(candidate.id, run_after)
                        .await?;
                }
                Admission::Deny => {
                    drop(reader_guard);
                    break;
                }
            }
        }
        Ok(())
    }

    /// Expire a single task inline (used by dispatch_fair to avoid code duplication).
    async fn expire_task_inline(
        &self,
        candidate: crate::task::TaskRecord,
    ) -> Result<(), StoreError> {
        if let Ok(Some(task)) = self.inner.store.expire_single(candidate.id).await {
            let age = (chrono::Utc::now() - task.created_at)
                .to_std()
                .unwrap_or_default();
            emit_event(
                &self.inner.event_tx,
                SchedulerEvent::TaskExpired {
                    header: task.event_header(),
                    age,
                },
            );
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
                    emit_event(
                        &self.inner.event_tx,
                        SchedulerEvent::TaskExpired {
                            header: task.event_header(),
                            age,
                        },
                    );
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

    /// Drain the submit coalescing channel and process any stranded messages.
    ///
    /// Most submits are handled inline by the submitter (leader election in
    /// `TaskStore::submit`). This is a safety-net drain for messages that
    /// arrived after the last leader finished processing.
    async fn drain_submits(&self) {
        let mut batch = Vec::new();
        {
            let mut rx = self.inner.store.submit_rx.lock().await;
            while let Ok(msg) = rx.try_recv() {
                batch.push(msg);
            }
        }

        if batch.is_empty() {
            return;
        }

        tracing::debug!(count = batch.len(), "draining submit batch");
        self.inner.store.process_submit_batch(batch).await;
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

        // Drain queued submits, completions, and failures before dispatching.
        self.drain_submits().await;
        self.drain_completions().await;
        self.drain_failures().await;

        // Run expiry sweep before dispatching.
        self.maybe_expire_tasks().await;

        // Resume preemption-paused tasks only if no active preemptors exist.
        // Scoped to the PREEMPTION bit — tasks paused by module/global/group
        // are not auto-resumed here.
        if self.inner.has_paused_tasks.load(AtomicOrdering::Relaxed) {
            if let Ok(paused) = self.inner.store.preemption_paused_tasks().await {
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
                        let _ = self.inner.store.resume_preempted(task.id).await;
                    }
                }
            }
        }

        // Auto-resume time-boxed group pauses (throttled to avoid per-cycle DB queries).
        if !self.inner.paused_groups.read().unwrap().is_empty() {
            let now = tokio::time::Instant::now();
            let should_check = {
                let last = self.inner.last_group_resume_check.lock().unwrap();
                now.duration_since(*last) >= Duration::from_secs(5)
            };
            if should_check {
                *self.inner.last_group_resume_check.lock().unwrap() = now;
                if let Ok(due_groups) = self.inner.store.groups_due_for_resume().await {
                    for group_key in due_groups {
                        if let Err(e) = self.resume_group(&group_key).await {
                            tracing::error!(group = group_key, error = %e, "auto-resume failed");
                        }
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

        // Drain any remaining submits, completions, and failures before closing.
        self.drain_submits().await;
        self.drain_completions().await;
        self.drain_failures().await;

        // Flush WAL and close the database.
        self.inner.store.close().await;
    }
}

/// Merge running and pending counts into a unified demand list.
fn merge_demand(
    running: &[(Option<String>, usize)],
    pending: &[(Option<String>, usize)],
) -> Vec<(Option<String>, GroupDemand)> {
    let mut map: HashMap<Option<String>, GroupDemand> = HashMap::new();
    for (g, count) in running {
        map.entry(g.clone())
            .or_insert(GroupDemand {
                running: 0,
                pending: 0,
            })
            .running = *count;
    }
    for (g, count) in pending {
        map.entry(g.clone())
            .or_insert(GroupDemand {
                running: 0,
                pending: 0,
            })
            .pending = *count;
    }
    map.into_iter().collect()
}
