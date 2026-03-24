//! Runtime control: pause/resume, concurrency limits, and group limits.

use std::sync::atomic::Ordering as AtomicOrdering;

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
    /// its next poll tick. Tasks that were paused in the store will be
    /// picked up automatically.
    pub async fn resume_all(&self) {
        self.inner.paused.store(false, AtomicOrdering::Release);
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
}
