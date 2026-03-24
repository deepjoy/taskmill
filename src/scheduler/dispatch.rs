//! Active-task tracking and preemption.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;

use crate::priority::Priority;
use crate::registry::IoTracker;
use crate::store::TaskStore;
use crate::task::TaskRecord;

use super::{emit_event, SchedulerEvent};

// ── Active Task ────────────────────────────────────────────────────

/// Handle to a running task for preemption and progress tracking.
pub(crate) struct ActiveTask {
    pub record: TaskRecord,
    pub token: CancellationToken,
    /// Last reported progress from the executor (0.0 to 1.0).
    pub reported_progress: Option<f32>,
    /// When the last progress report was received.
    pub reported_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Handle to the spawned tokio task, set after spawn.
    pub handle: Option<tokio::task::JoinHandle<()>>,
    /// Shared IO tracker for byte-level progress reporting.
    pub io: Arc<IoTracker>,
    /// When this task started executing.
    pub started_at: std::time::Instant,
}

/// Snapshot of byte-level progress for a single active task.
pub(crate) type ByteProgressSnapshot = (
    i64,
    String,
    String,
    String,
    u64,
    Option<u64>,
    Option<i64>,
    std::time::Instant,
);

// ── Active Task Map ────────────────────────────────────────────────

/// Thread-safe map of currently running tasks.
///
/// Wraps the active-task bookkeeping that was previously inlined in
/// `Scheduler`, making preemption and progress queries independently
/// testable.
///
/// Uses `std::sync::Mutex` rather than `tokio::Mutex` because most
/// operations do trivial `HashMap` work under the lock with no `.await`.
/// Methods that need async I/O (`preempt_below`, `pause_all`) collect
/// data under the lock and release it before awaiting.
#[derive(Clone)]
pub(crate) struct ActiveTaskMap {
    inner: Arc<Mutex<HashMap<i64, ActiveTask>>>,
    /// IDs of parent tasks ready for finalization. Populated by
    /// `handle_parent_resolution` when all children complete. Uses a
    /// `HashSet` to deduplicate — two children completing simultaneously
    /// may both resolve the parent, but we only finalize once.
    pub(crate) pending_finalizers: Arc<Mutex<HashSet<i64>>>,
}

impl ActiveTaskMap {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            pending_finalizers: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    pub fn insert(&self, id: i64, task: ActiveTask) {
        self.inner.lock().unwrap().insert(id, task);
    }

    pub fn remove(&self, id: i64) -> Option<ActiveTask> {
        self.inner.lock().unwrap().remove(&id)
    }

    /// Snapshot of active task records, optionally filtered to those whose
    /// `task_type` starts with `prefix`.
    pub fn records(&self, prefix: Option<&str>) -> Vec<TaskRecord> {
        let map = self.inner.lock().unwrap();
        map.values()
            .filter(|at| prefix.map_or(true, |p| at.record.task_type.starts_with(p)))
            .map(|at| at.record.clone())
            .collect()
    }

    /// Snapshot of progress data for active tasks, optionally filtered to
    /// those whose `task_type` starts with `prefix`.
    pub fn progress_snapshots(
        &self,
        prefix: Option<&str>,
    ) -> Vec<(
        TaskRecord,
        Option<f32>,
        Option<chrono::DateTime<chrono::Utc>>,
    )> {
        let map = self.inner.lock().unwrap();
        map.values()
            .filter(|at| prefix.map_or(true, |p| at.record.task_type.starts_with(p)))
            .map(|at| (at.record.clone(), at.reported_progress, at.reported_at))
            .collect()
    }

    /// Snapshot of byte-level progress for active tasks, optionally filtered
    /// to those whose `task_type` starts with `prefix`.
    ///
    /// Returns `(task_id, task_type, key, label, bytes_completed, bytes_total, parent_id, started_at)`.
    /// Single lock acquisition — reads atomic counters and copies scalar fields only.
    pub fn byte_progress_snapshots(&self, prefix: Option<&str>) -> Vec<ByteProgressSnapshot> {
        let map = self.inner.lock().unwrap();
        map.values()
            .filter(|at| prefix.map_or(true, |p| at.record.task_type.starts_with(p)))
            .map(|at| {
                let (completed, total) = at.io.progress_snapshot();
                (
                    at.record.id,
                    at.record.task_type.clone(),
                    at.record.key.clone(),
                    at.record.label.clone(),
                    completed,
                    total,
                    at.record.parent_id,
                    at.started_at,
                )
            })
            .collect()
    }

    /// Update reported progress for a specific task.
    pub fn update_progress(&self, task_id: i64, percent: f32) {
        let mut map = self.inner.lock().unwrap();
        if let Some(at) = map.get_mut(&task_id) {
            at.reported_progress = Some(percent);
            at.reported_at = Some(chrono::Utc::now());
        }
    }

    /// Preempt active tasks with priority lower than the incoming priority.
    ///
    /// Cancels their tokens, pauses them in the store, and emits
    /// `SchedulerEvent::Preempted`. Returns the IDs of preempted tasks.
    ///
    /// Collects tasks to preempt under the sync lock, then releases the
    /// lock before performing async store writes.
    pub async fn preempt_below(
        &self,
        incoming_priority: Priority,
        store: &TaskStore,
        event_tx: &tokio::sync::broadcast::Sender<SchedulerEvent>,
    ) -> Vec<i64> {
        let drained = self.drain_where(|at| at.record.priority.value() > incoming_priority.value());
        for (id, at) in &drained {
            tracing::info!(
                task_id = id,
                task_type = at.record.task_type,
                "preempting task for higher-priority work"
            );
        }
        cancel_pause_emit(&drained, store, event_tx).await;
        drained.into_iter().map(|(id, _)| id).collect()
    }

    /// Check whether any active task would preempt work at the given priority.
    pub fn has_preemptors_for(&self, priority: Priority, preempt_threshold: Priority) -> bool {
        let active = self.inner.lock().unwrap();
        active.values().any(|at| {
            at.record.priority.value() <= preempt_threshold.value()
                && at.record.priority.value() < priority.value()
        })
    }

    /// Store the `JoinHandle` for a task that was just spawned.
    pub fn set_handle(&self, id: i64, handle: tokio::task::JoinHandle<()>) {
        let mut map = self.inner.lock().unwrap();
        if let Some(at) = map.get_mut(&id) {
            at.handle = Some(handle);
        }
    }

    /// Cancel all active tasks and abort their handles (hard shutdown).
    pub fn cancel_all(&self) {
        let mut active = self.inner.lock().unwrap();
        for (_, at) in active.drain() {
            at.token.cancel();
            if let Some(h) = at.handle {
                h.abort();
            }
        }
    }

    /// Cancel all tokens, drain the map, and return the join handles
    /// for graceful shutdown (caller can join with a timeout).
    pub fn cancel_and_drain_handles(&self) -> Vec<tokio::task::JoinHandle<()>> {
        let mut active = self.inner.lock().unwrap();
        let mut handles = Vec::with_capacity(active.len());
        for (_, at) in active.drain() {
            at.token.cancel();
            if let Some(h) = at.handle {
                handles.push(h);
            }
        }
        handles
    }

    /// Pause active tasks whose `task_type` starts with `prefix`: cancel their
    /// tokens and move them to paused state in the store. Returns count paused.
    pub async fn pause_module(
        &self,
        prefix: &str,
        store: &TaskStore,
        event_tx: &tokio::sync::broadcast::Sender<SchedulerEvent>,
    ) -> usize {
        let drained = self.drain_where(|at| at.record.task_type.starts_with(prefix));
        cancel_pause_emit(&drained, store, event_tx).await;
        drained.len()
    }

    /// Pause all active tasks: cancel their tokens and move them to paused
    /// state in the store. Returns the number of tasks paused.
    ///
    /// Drains the map under the sync lock, then releases the lock before
    /// performing async store writes.
    pub async fn pause_all(
        &self,
        store: &TaskStore,
        event_tx: &tokio::sync::broadcast::Sender<SchedulerEvent>,
    ) -> usize {
        let drained = self.drain_where(|_| true);
        cancel_pause_emit(&drained, store, event_tx).await;
        drained.len()
    }

    /// Drain tasks matching `predicate` from the active map.
    ///
    /// Collects matching tasks under the sync lock and removes them
    /// atomically. Returns the removed tasks for async follow-up work.
    fn drain_where(&self, predicate: impl Fn(&ActiveTask) -> bool) -> Vec<(i64, ActiveTask)> {
        let mut map = self.inner.lock().unwrap();
        let ids: Vec<i64> = map
            .iter()
            .filter(|(_, at)| predicate(at))
            .map(|(id, _)| *id)
            .collect();
        ids.into_iter()
            .filter_map(|id| map.remove(&id).map(|at| (id, at)))
            .collect()
    }
}

// ── Helpers ────────────────────────────────────────────────────────

/// Cancel tokens, pause in store, and emit `Preempted` events for drained tasks.
async fn cancel_pause_emit(
    drained: &[(i64, ActiveTask)],
    store: &TaskStore,
    event_tx: &tokio::sync::broadcast::Sender<SchedulerEvent>,
) {
    for (id, at) in drained {
        at.token.cancel();
        let _ = store.pause(*id).await;
        emit_event(event_tx, SchedulerEvent::Preempted(at.record.event_header()));
    }
}
