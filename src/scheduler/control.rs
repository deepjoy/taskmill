//! Runtime control: pause/resume, concurrency limits, and group limits.

use std::sync::atomic::Ordering as AtomicOrdering;

use chrono::{DateTime, Utc};

use crate::store::StoreError;

use super::rate_limit::RateLimit;
use super::{emit_event, Scheduler, SchedulerEvent};

impl Scheduler {
    /// Update max concurrency at runtime (e.g., from adaptive controller or
    /// in response to battery/thermal state).
    pub fn set_max_concurrency(&self, limit: usize) {
        self.inner
            .max_concurrency
            .store(limit, AtomicOrdering::Relaxed);
        tracing::info!(new_limit = limit, "concurrency limit updated");
    }

    /// Read current max concurrency setting.
    pub fn max_concurrency(&self) -> usize {
        self.inner.max_concurrency.load(AtomicOrdering::Relaxed)
    }

    /// Pause the entire scheduler.
    ///
    /// Stops the run loop from dispatching new tasks and pauses all
    /// currently running tasks (their cancellation tokens are triggered
    /// and they are moved back to the `paused` state in the store so
    /// they will be re-dispatched on resume).
    ///
    /// Useful when the app is backgrounded, the laptop goes to sleep,
    /// or the user clicks "pause all" in the UI.
    pub async fn pause_all(&self) {
        self.inner.paused.store(true, AtomicOrdering::Release);
        let count = self
            .inner
            .active
            .pause_all(&self.inner.store, &self.inner.event_tx)
            .await;
        emit_event(&self.inner.event_tx, SchedulerEvent::Paused);
        tracing::info!(paused_tasks = count, "scheduler paused");
    }

    /// Resume the scheduler after a [`pause_all`](Self::pause_all).
    ///
    /// Clears the pause flag so the run loop will resume dispatching on
    /// its next poll tick. Also clears the GLOBAL bit from all paused tasks
    /// in the store — tasks with no remaining pause reasons transition back
    /// to pending.
    pub async fn resume_all(&self) {
        self.inner.paused.store(false, AtomicOrdering::Release);
        let _ = self
            .inner
            .store
            .clear_pause_bit(crate::task::PauseReasons::GLOBAL)
            .await;
        self.inner.work_notify.notify_one();
        emit_event(&self.inner.event_tx, SchedulerEvent::Resumed);
        tracing::info!("scheduler resumed");
    }

    /// Returns `true` if the scheduler is globally paused.
    pub fn is_paused(&self) -> bool {
        self.inner.paused.load(AtomicOrdering::Acquire)
    }

    /// Set the concurrency limit for a specific task group.
    ///
    /// Tasks with a matching `group_key` will be throttled so that at most
    /// `limit` run concurrently, independent of the global concurrency cap.
    pub fn set_group_limit(&self, group: impl Into<String>, limit: usize) {
        self.inner.group_limits.set_limit(group.into(), limit);
    }

    /// Remove a per-group concurrency override, falling back to the default.
    pub fn remove_group_limit(&self, group: &str) {
        self.inner.group_limits.remove_limit(group);
    }

    /// Set the default concurrency limit for any grouped task without a
    /// specific override. `0` means unlimited.
    pub fn set_default_group_concurrency(&self, limit: usize) {
        self.inner.group_limits.set_default(limit);
    }

    // ── Group Pause / Resume ───────────────────────────────────────

    /// Shared implementation for `pause_group` and `pause_group_until`.
    /// Persists state, updates in-memory set, pauses pending + running tasks.
    ///
    /// **Ordering is load-bearing**: Step 1 (persist) MUST happen before step 4
    /// (pause pending) and step 5 (cancel running). If a running task completes
    /// between steps 1 and 5, it may trigger recurring instance creation or
    /// dependency resolution. Both code paths check the `paused_groups` table —
    /// which is already populated by step 1 — so the new instance/unblocked
    /// task will be correctly inserted as `'paused'`.
    async fn pause_group_inner(
        &self,
        group_key: &str,
        resume_at: Option<i64>,
    ) -> Result<Option<u64>, StoreError> {
        // 1. Persist pause state (idempotent).
        let newly_paused = self
            .inner
            .store
            .pause_group_state(group_key, resume_at)
            .await?;
        if !newly_paused {
            return Ok(None); // Already paused — no-op.
        }

        // 2. Add to in-memory set.
        self.inner
            .paused_groups
            .write()
            .unwrap()
            .insert(group_key.to_string());

        // 3. Disable fast dispatch (gate checks now needed).
        self.inner
            .fast_dispatch
            .store(false, AtomicOrdering::Relaxed);

        // 4. Pause pending tasks in the group (pending → paused, already-paused get GROUP bit).
        let pending_paused = self.inner.store.pause_tasks_in_group(group_key).await?;

        // 5. Cancel and pause running tasks in the group.
        // Note: does NOT set has_paused_tasks — the auto-resume path only handles
        // PREEMPTION-paused tasks, so setting the flag would cause an unnecessary
        // query that returns empty.
        let running_paused = self
            .inner
            .active
            .pause_group(group_key, &self.inner.store, &self.inner.event_tx)
            .await;

        let _ = self.inner.event_tx.send(SchedulerEvent::GroupPaused {
            group: group_key.to_string(),
            pending_count: pending_paused as usize,
            running_count: running_paused,
        });

        Ok(Some(pending_paused))
    }

    /// Pause all tasks in a group.
    ///
    /// - Persists the pause state in SQLite (survives restarts).
    /// - Pending tasks in the group are set to `paused` status with GROUP bit.
    /// - Already-paused tasks get the GROUP bit added (prevents premature resume).
    /// - Running tasks are cancelled and moved to `paused` with GROUP bit.
    ///   They will resume from the beginning on group resume.
    /// - New submissions to the group are accepted but paused immediately.
    /// - TTL is NOT frozen — `expires_at` remains a hard deadline.
    /// - Idempotent: pausing an already-paused group is a no-op.
    pub async fn pause_group(&self, group_key: &str) -> Result<(), StoreError> {
        let Some(pending_paused) = self.pause_group_inner(group_key, None).await? else {
            return Ok(());
        };
        tracing::info!(group = group_key, pending_paused, "group paused");
        Ok(())
    }

    /// Resume a paused group.
    ///
    /// - Clears the GROUP bit. Tasks with no remaining bits become `pending`.
    ///   Tasks with other bits (e.g., MODULE) stay paused under those reasons.
    /// - Removes the group from the persisted pause state.
    /// - Idempotent: resuming a non-paused group is a no-op.
    pub async fn resume_group(&self, group_key: &str) -> Result<(), StoreError> {
        if !self.inner.paused_groups.read().unwrap().contains(group_key) {
            return Ok(());
        }

        // Resume tasks (clear GROUP bit; sole-group tasks become pending).
        let resumed_count = self.inner.store.resume_paused_by_group(group_key).await?;

        self.inner.store.resume_group_state(group_key).await?;

        // Drop the write lock BEFORE calling maybe_restore_fast_dispatch
        // to avoid deadlock (RwLock is not reentrant).
        let is_empty = {
            let mut set = self.inner.paused_groups.write().unwrap();
            set.remove(group_key);
            set.is_empty()
        };
        if is_empty {
            self.maybe_restore_fast_dispatch();
        }

        if resumed_count > 0 {
            self.inner.work_notify.notify_one();
        }

        let _ = self.inner.event_tx.send(SchedulerEvent::GroupResumed {
            group: group_key.to_string(),
            resumed_count: resumed_count as usize,
        });

        tracing::info!(group = group_key, resumed_count, "group resumed");
        Ok(())
    }

    /// Time-boxed pause: pause a group until a deadline, then auto-resume.
    /// Cancels running tasks immediately (same as `pause_group`).
    ///
    /// **Latency**: Auto-resume is checked every ~5 seconds (throttled to avoid
    /// per-cycle DB queries), so the group may remain paused up to ~5 seconds
    /// past the deadline. For sub-second precision, use an external timer that
    /// calls `resume_group` directly.
    pub async fn pause_group_until(
        &self,
        group_key: &str,
        deadline: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        let Some(pending_paused) = self
            .pause_group_inner(group_key, Some(deadline.timestamp_millis()))
            .await?
        else {
            return Ok(());
        };
        tracing::info!(group = group_key, pending_paused, resume_at = %deadline, "group paused until deadline");
        Ok(())
    }

    /// List currently paused groups (synchronous — reads in-memory set).
    pub fn paused_groups(&self) -> Vec<String> {
        self.inner
            .paused_groups
            .read()
            .unwrap()
            .iter()
            .cloned()
            .collect()
    }

    /// Check if a specific group is paused.
    pub fn is_group_paused(&self, group_key: &str) -> bool {
        self.inner.paused_groups.read().unwrap().contains(group_key)
    }

    // ── Rate Limiting ──────────────────────────────────────────────

    /// Set or update the rate limit for a task type at runtime.
    ///
    /// If a bucket already exists, reconfigures in-place (preserving current
    /// token count, clamped to new burst). If not, creates a new bucket.
    pub fn set_rate_limit(&self, task_type: impl Into<String>, limit: RateLimit) {
        self.inner.type_rate_limits.set(task_type.into(), limit);
        self.inner
            .fast_dispatch
            .store(false, AtomicOrdering::Relaxed);
    }

    /// Remove the task-type rate limit, falling back to unlimited.
    pub fn remove_rate_limit(&self, task_type: &str) {
        self.inner.type_rate_limits.remove(task_type);
        self.maybe_restore_fast_dispatch();
    }

    /// Set or update the rate limit for a task group at runtime.
    pub fn set_group_rate_limit(&self, group: impl Into<String>, limit: RateLimit) {
        self.inner.group_rate_limits.set(group.into(), limit);
        self.inner
            .fast_dispatch
            .store(false, AtomicOrdering::Relaxed);
    }

    /// Remove the group rate limit, falling back to unlimited.
    pub fn remove_group_rate_limit(&self, group: &str) {
        self.inner.group_rate_limits.remove(group);
        self.maybe_restore_fast_dispatch();
    }

    /// Re-evaluate whether fast dispatch can be re-enabled.
    ///
    /// Must mirror the conditions in `SchedulerBuilder::build()`:
    /// no paused groups, no group limits (default or overrides), no resource
    /// monitoring, no pressure sources, no module concurrency caps, no rate limits.
    fn maybe_restore_fast_dispatch(&self) {
        let has_groups = self.inner.group_limits.default_limit() > 0
            || self.inner.group_limits.has_overrides()
            || !self.inner.paused_groups.read().unwrap().is_empty();
        let has_module_caps = !self.inner.module_caps.read().unwrap().is_empty();
        let has_rate_limits =
            !self.inner.type_rate_limits.is_empty() || !self.inner.group_rate_limits.is_empty();

        if !has_groups
            && !self
                .inner
                .has_resource_monitoring
                .load(AtomicOrdering::Relaxed)
            && !has_module_caps
            && !self
                .inner
                .has_pressure_sources
                .load(AtomicOrdering::Relaxed)
            && !has_rate_limits
        {
            self.inner
                .fast_dispatch
                .store(true, AtomicOrdering::Relaxed);
        }
    }
}
