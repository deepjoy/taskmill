//! Task spawning, active-task tracking, preemption, and parent-child resolution.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::priority::Priority;
use crate::registry::{ChildSpawner, IoTracker, TaskContext};
use crate::store::TaskStore;
use crate::task::{ParentResolution, TaskMetrics, TaskRecord};

use super::progress::ProgressReporter;
use super::SchedulerEvent;

// ── Active Task ────────────────────────────────────────────────────

/// Handle to a running task for preemption and progress tracking.
pub(crate) struct ActiveTask {
    pub record: TaskRecord,
    pub token: CancellationToken,
    /// Last reported progress from the executor (0.0 to 1.0).
    pub reported_progress: Option<f32>,
    /// When the last progress report was received.
    pub reported_at: Option<chrono::DateTime<chrono::Utc>>,
}

// ── Active Task Map ────────────────────────────────────────────────

/// Thread-safe map of currently running tasks.
///
/// Wraps the active-task bookkeeping that was previously inlined in
/// `Scheduler`, making preemption and progress queries independently
/// testable.
#[derive(Clone)]
pub(crate) struct ActiveTaskMap {
    inner: Arc<Mutex<HashMap<i64, ActiveTask>>>,
    /// IDs of parent tasks ready for finalization. Populated by
    /// `handle_parent_resolution` when all children complete.
    pub(crate) pending_finalizers: Arc<Mutex<Vec<i64>>>,
}

impl ActiveTaskMap {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            pending_finalizers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn count(&self) -> usize {
        self.inner.lock().await.len()
    }

    pub async fn insert(&self, id: i64, task: ActiveTask) {
        self.inner.lock().await.insert(id, task);
    }

    pub async fn remove(&self, id: i64) -> Option<ActiveTask> {
        self.inner.lock().await.remove(&id)
    }

    /// Snapshot of all active task records.
    pub async fn records(&self) -> Vec<TaskRecord> {
        self.inner
            .lock()
            .await
            .values()
            .map(|at| at.record.clone())
            .collect()
    }

    /// Snapshot of progress data for all active tasks.
    pub async fn progress_snapshots(
        &self,
    ) -> Vec<(
        TaskRecord,
        Option<f32>,
        Option<chrono::DateTime<chrono::Utc>>,
    )> {
        self.inner
            .lock()
            .await
            .values()
            .map(|at| (at.record.clone(), at.reported_progress, at.reported_at))
            .collect()
    }

    /// Update reported progress for a specific task.
    pub async fn update_progress(&self, task_id: i64, percent: f32) {
        let mut map = self.inner.lock().await;
        if let Some(at) = map.get_mut(&task_id) {
            at.reported_progress = Some(percent);
            at.reported_at = Some(chrono::Utc::now());
        }
    }

    /// Preempt active tasks with priority lower than the incoming priority.
    ///
    /// Cancels their tokens, pauses them in the store, and emits
    /// `SchedulerEvent::Preempted`. Returns the IDs of preempted tasks.
    pub async fn preempt_below(
        &self,
        incoming_priority: Priority,
        store: &TaskStore,
        event_tx: &tokio::sync::broadcast::Sender<SchedulerEvent>,
    ) -> Vec<i64> {
        let mut active = self.inner.lock().await;
        let to_preempt: Vec<i64> = active
            .iter()
            .filter(|(_, at)| at.record.priority.value() > incoming_priority.value())
            .map(|(id, _)| *id)
            .collect();

        let mut preempted = Vec::new();
        for id in to_preempt {
            if let Some(at) = active.remove(&id) {
                tracing::info!(
                    task_id = id,
                    task_type = at.record.task_type,
                    "preempting task for higher-priority work"
                );
                at.token.cancel();
                let _ = store.pause(id).await;
                let _ = event_tx.send(SchedulerEvent::Preempted {
                    task_id: id,
                    task_type: at.record.task_type.clone(),
                    key: at.record.key.clone(),
                });
                preempted.push(id);
            }
        }

        preempted
    }

    /// Check whether any active task would preempt work at the given priority.
    pub async fn has_preemptors_for(
        &self,
        priority: Priority,
        preempt_threshold: Priority,
    ) -> bool {
        let active = self.inner.lock().await;
        active.values().any(|at| {
            at.record.priority.value() <= preempt_threshold.value()
                && at.record.priority.value() < priority.value()
        })
    }

    /// Cancel all active tasks (for shutdown).
    pub async fn cancel_all(&self) {
        let mut active = self.inner.lock().await;
        for (_, at) in active.drain() {
            at.token.cancel();
        }
    }

    /// Pause all active tasks: cancel their tokens and move them to paused
    /// state in the store. Returns the number of tasks paused.
    pub async fn pause_all(
        &self,
        store: &TaskStore,
        event_tx: &tokio::sync::broadcast::Sender<SchedulerEvent>,
    ) -> usize {
        let mut active = self.inner.lock().await;
        let count = active.len();
        for (id, at) in active.drain() {
            at.token.cancel();
            let _ = store.pause(id).await;
            let _ = event_tx.send(SchedulerEvent::Preempted {
                task_id: id,
                task_type: at.record.task_type.clone(),
                key: at.record.key.clone(),
            });
        }
        count
    }
}

// ── Spawn ──────────────────────────────────────────────────────────

/// Whether to call `execute` or `finalize` on the executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionPhase {
    Execute,
    Finalize,
}

/// Shared scheduler resources passed to each spawned task.
pub(crate) struct SpawnContext {
    pub store: TaskStore,
    pub active: ActiveTaskMap,
    pub event_tx: tokio::sync::broadcast::Sender<SchedulerEvent>,
    pub max_retries: i32,
    pub app_state: crate::registry::StateSnapshot,
    pub work_notify: Arc<tokio::sync::Notify>,
    pub scheduler: super::Scheduler,
}

/// Spawn a task executor and wire up completion/failure handling.
///
/// Inserts the task into the active map, starts a progress listener,
/// and spawns the executor on a new tokio task.
pub(crate) async fn spawn_task(
    task: TaskRecord,
    executor: Arc<dyn crate::registry::ErasedExecutor>,
    ctx: SpawnContext,
    phase: ExecutionPhase,
) {
    let SpawnContext {
        store,
        active,
        event_tx,
        max_retries,
        app_state,
        work_notify,
        scheduler,
    } = ctx;
    let child_token = CancellationToken::new();

    // Insert into active map before spawning to avoid races.
    active
        .insert(
            task.id,
            ActiveTask {
                record: task.clone(),
                token: child_token.clone(),
                reported_progress: None,
                reported_at: None,
            },
        )
        .await;

    // Build execution context.
    let child_spawner = ChildSpawner::new(store.clone(), task.id, work_notify.clone());
    let io = Arc::new(IoTracker::new());
    let ctx = TaskContext {
        record: task.clone(),
        token: child_token.clone(),
        progress: ProgressReporter::new(
            task.id,
            task.task_type.clone(),
            task.key.clone(),
            event_tx.clone(),
        ),
        scheduler,
        app_state,
        child_spawner: Some(child_spawner),
        io: io.clone(),
    };

    // Emit dispatched event.
    let _ = event_tx.send(SchedulerEvent::Dispatched {
        task_id: task.id,
        task_type: task.task_type.clone(),
        key: task.key.clone(),
    });

    // Spawn progress listener — bridges broadcast events into the active map.
    let active_for_progress = active.clone();
    let mut progress_rx = event_tx.subscribe();
    let progress_task_id = task.id;
    tokio::spawn(async move {
        while let Ok(evt) = progress_rx.recv().await {
            if let SchedulerEvent::Progress {
                task_id, percent, ..
            } = evt
            {
                if task_id == progress_task_id {
                    active_for_progress.update_progress(task_id, percent).await;
                    if percent >= 1.0 {
                        break;
                    }
                }
            }
        }
    });

    // Spawn executor.
    let token_for_spawn = child_token.clone();
    tokio::spawn(async move {
        let task_id = task.id;
        let result = match phase {
            ExecutionPhase::Execute => executor.execute_erased(&ctx).await,
            ExecutionPhase::Finalize => executor.finalize_erased(&ctx).await,
        };

        // Read IO bytes from the context tracker.
        let metrics = io.snapshot();

        // Drop the context (and its progress reporter) — executor is done.
        drop(ctx);

        match result {
            Ok(()) => {
                // For the execute phase, check if the task spawned children.
                // If so, transition to waiting instead of completing.
                if phase == ExecutionPhase::Execute {
                    match store.active_children_count(task_id).await {
                        Ok(count) if count > 0 => {
                            if let Err(e) = store.set_waiting(task_id).await {
                                tracing::error!(task_id, error = %e, "failed to set task to waiting");
                            }
                            active.remove(task_id).await;
                            let _ = event_tx.send(SchedulerEvent::Waiting {
                                task_id,
                                children_count: count,
                            });
                            // Children may have completed before we set waiting.
                            // Re-check to avoid a missed finalization.
                            handle_parent_resolution(
                                task_id,
                                &store,
                                &active,
                                &event_tx,
                                max_retries,
                                &work_notify,
                            )
                            .await;
                            // Wake the scheduler to dispatch children (or finalizer).
                            work_notify.notify_one();
                            return;
                        }
                        Err(e) => {
                            tracing::error!(task_id, error = %e, "failed to check children count");
                            // Fall through to normal completion.
                        }
                        _ => {
                            // No children — complete normally.
                        }
                    }
                }

                if let Err(e) = store.complete(task_id, &metrics).await {
                    tracing::error!(task_id, error = %e, "failed to record task completion");
                }
                // Remove from active tracking AFTER the store write completes.
                active.remove(task_id).await;
                let _ = event_tx.send(SchedulerEvent::Completed {
                    task_id,
                    task_type: task.task_type.clone(),
                    key: task.key.clone(),
                });

                // If this was a child task, check if parent is ready.
                if let Some(parent_id) = task.parent_id {
                    handle_parent_resolution(
                        parent_id,
                        &store,
                        &active,
                        &event_tx,
                        max_retries,
                        &work_notify,
                    )
                    .await;
                }
            }
            Err(te) => {
                // If cancelled (preempted), the scheduler already paused it.
                if token_for_spawn.is_cancelled() {
                    active.remove(task_id).await;
                    return;
                }
                let will_retry = te.retryable && task.retry_count < max_retries;
                tracing::warn!(
                    task_id,
                    task_type = task.task_type,
                    error = %te.message,
                    retryable = te.retryable,
                    will_retry,
                    "task failed"
                );
                if let Err(e) = store
                    .fail(task_id, &te.message, te.retryable, max_retries, &metrics)
                    .await
                {
                    tracing::error!(task_id, error = %e, "failed to record task failure");
                }
                // Remove from active tracking AFTER the store write completes.
                active.remove(task_id).await;
                let _ = event_tx.send(SchedulerEvent::Failed {
                    task_id,
                    task_type: task.task_type.clone(),
                    key: task.key.clone(),
                    error: te.message.clone(),
                    will_retry,
                });

                // If this child failed permanently and parent is fail_fast,
                // cancel siblings and fail the parent.
                if !will_retry {
                    if let Some(parent_id) = task.parent_id {
                        // Check if parent uses fail_fast.
                        if let Ok(Some(parent)) = store.task_by_id(parent_id).await {
                            if parent.fail_fast {
                                // Cancel remaining siblings.
                                if let Ok(running_ids) = store.cancel_children(parent_id).await {
                                    for rid in &running_ids {
                                        if let Some(at) = active.remove(*rid).await {
                                            at.token.cancel();
                                            let _ = store.delete(*rid).await;
                                            let _ = event_tx.send(SchedulerEvent::Cancelled {
                                                task_id: *rid,
                                                task_type: at.record.task_type.clone(),
                                                key: at.record.key.clone(),
                                            });
                                        }
                                    }
                                }
                                // Fail the parent.
                                let msg = format!("child task {task_id} failed: {}", te.message);
                                if let Err(e) = store
                                    .fail(parent_id, &msg, false, 0, &TaskMetrics::default())
                                    .await
                                {
                                    tracing::error!(
                                        parent_id,
                                        error = %e,
                                        "failed to record parent failure"
                                    );
                                }
                                let _ = event_tx.send(SchedulerEvent::Failed {
                                    task_id: parent_id,
                                    task_type: parent.task_type.clone(),
                                    key: parent.key.clone(),
                                    error: msg,
                                    will_retry: false,
                                });
                            } else {
                                // Not fail_fast — check if all children done.
                                handle_parent_resolution(
                                    parent_id,
                                    &store,
                                    &active,
                                    &event_tx,
                                    max_retries,
                                    &work_notify,
                                )
                                .await;
                            }
                        }
                    }
                }
            }
        }
    });
}

/// Check if a waiting parent is ready for finalization or has failed,
/// and dispatch the finalize phase if ready.
async fn handle_parent_resolution(
    parent_id: i64,
    store: &TaskStore,
    active: &ActiveTaskMap,
    event_tx: &tokio::sync::broadcast::Sender<SchedulerEvent>,
    _max_retries: i32,
    work_notify: &Arc<tokio::sync::Notify>,
) {
    match store.try_resolve_parent(parent_id).await {
        Ok(Some(ParentResolution::ReadyToFinalize)) => {
            // Enqueue parent for finalize dispatch.
            active.pending_finalizers.lock().await.push(parent_id);
            // Wake the scheduler to dispatch the finalize phase.
            work_notify.notify_one();
        }
        Ok(Some(ParentResolution::Failed(reason))) => {
            // All children done but some failed — fail the parent.
            if let Ok(Some(parent)) = store.task_by_id(parent_id).await {
                if let Err(e) = store
                    .fail(parent_id, &reason, false, 0, &TaskMetrics::default())
                    .await
                {
                    tracing::error!(parent_id, error = %e, "failed to record parent failure");
                }
                let _ = event_tx.send(SchedulerEvent::Failed {
                    task_id: parent_id,
                    task_type: parent.task_type.clone(),
                    key: parent.key.clone(),
                    error: reason,
                    will_retry: false,
                });
            }
        }
        Ok(Some(ParentResolution::StillWaiting)) | Ok(None) => {
            // Children still active or parent not found — nothing to do.
        }
        Err(e) => {
            tracing::error!(parent_id, error = %e, "failed to resolve parent");
        }
    }
}
