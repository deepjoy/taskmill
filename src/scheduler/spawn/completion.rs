//! Success path: children check, completion, recurring re-enqueue, dependency resolution.

use std::sync::Arc;

use crate::store::TaskStore;
use crate::task::{IoBudget, TaskRecord};

use super::super::dispatch::ActiveTaskMap;
use super::super::SchedulerEvent;
use super::parent::handle_parent_resolution;
use super::ExecutionPhase;

/// Shared dependencies for the completion handler.
pub(crate) struct CompletionDeps {
    pub store: TaskStore,
    pub active: ActiveTaskMap,
    pub event_tx: tokio::sync::broadcast::Sender<SchedulerEvent>,
    pub work_notify: Arc<tokio::sync::Notify>,
    pub max_retries: i32,
}

/// Handle a successful task execution.
///
/// For the execute phase, checks if the task spawned children (transition to
/// waiting). Otherwise records completion, resolves dependents, and handles
/// recurring re-enqueue.
pub(crate) async fn handle_success(
    task: &TaskRecord,
    phase: ExecutionPhase,
    metrics: &IoBudget,
    deps: &CompletionDeps,
    decrement_module: impl FnOnce(),
) {
    let task_id = task.id;

    // For the execute phase, check if the task spawned children.
    // If so, transition to waiting instead of completing.
    if phase == ExecutionPhase::Execute {
        match deps.store.active_children_count(task_id).await {
            Ok(count) if count > 0 => {
                if let Err(e) = deps.store.set_waiting(task_id).await {
                    tracing::error!(task_id, error = %e, "failed to set task to waiting");
                }
                decrement_module();
                deps.active.remove(task_id);
                let _ = deps.event_tx.send(SchedulerEvent::Waiting {
                    task_id,
                    children_count: count,
                });
                // Children may have completed before we set waiting.
                // Re-check to avoid a missed finalization.
                handle_parent_resolution(
                    task_id,
                    &deps.store,
                    &deps.active,
                    &deps.event_tx,
                    deps.max_retries,
                    &deps.work_notify,
                )
                .await;
                // Wake the scheduler to dispatch children (or finalizer).
                deps.work_notify.notify_one();
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

    match deps
        .store
        .complete_with_record_and_resolve(task, metrics)
        .await
    {
        Ok((recurring_info, unblocked)) => {
            // Emit recurring event if this was a recurring task.
            if task.recurring_interval_secs.is_some() {
                let (next_run, exec_count) = match recurring_info {
                    Some((next, count)) => (Some(next), count),
                    None => (None, task.recurring_execution_count + 1),
                };
                let _ = deps.event_tx.send(SchedulerEvent::RecurringCompleted {
                    header: task.event_header(),
                    execution_count: exec_count,
                    next_run,
                });
            }

            // Remove from active tracking AFTER the store write completes.
            decrement_module();
            deps.active.remove(task_id);
            let _ = deps
                .event_tx
                .send(SchedulerEvent::Completed(task.event_header()));

            // Emit unblocked events for resolved dependents.
            for uid in &unblocked {
                let _ = deps
                    .event_tx
                    .send(SchedulerEvent::TaskUnblocked { task_id: *uid });
            }
        }
        Err(e) => {
            tracing::error!(task_id, error = %e, "failed to complete task and resolve dependents");
            decrement_module();
            deps.active.remove(task_id);
        }
    }

    deps.work_notify.notify_one();

    // If this was a child task, check if parent is ready.
    if let Some(parent_id) = task.parent_id {
        handle_parent_resolution(
            parent_id,
            &deps.store,
            &deps.active,
            &deps.event_tx,
            deps.max_retries,
            &deps.work_notify,
        )
        .await;
    }
}
