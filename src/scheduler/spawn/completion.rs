//! Success path: children check, completion, recurring re-enqueue, dependency resolution.

use std::sync::Arc;

use crate::store::TaskStore;
use crate::task::{IoBudget, TaskRecord};

use super::super::dispatch::ActiveTaskMap;
use super::super::{emit_event, CompletionMsg, SchedulerEvent};
use super::parent::handle_parent_resolution;
use super::ExecutionPhase;

/// Shared dependencies for the completion handler.
pub(crate) struct CompletionDeps {
    pub store: TaskStore,
    pub active: ActiveTaskMap,
    pub event_tx: tokio::sync::broadcast::Sender<SchedulerEvent>,
    pub work_notify: Arc<tokio::sync::Notify>,
    pub max_retries: i32,
    pub completion_tx: tokio::sync::mpsc::UnboundedSender<CompletionMsg>,
    pub completion_rx: Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<CompletionMsg>>>,
}

/// Handle a successful task execution.
///
/// For the execute phase, checks if the task spawned children (transition to
/// waiting). Otherwise sends a [`CompletionMsg`] to the coalescing channel
/// for batched processing in `drain_completions`.
pub(crate) async fn handle_success(
    task: &TaskRecord,
    phase: ExecutionPhase,
    metrics: &IoBudget,
    memo: Option<Vec<u8>>,
    deps: &CompletionDeps,
    mut decrement_module: impl FnMut(),
) {
    let task_id = task.id;

    // For the execute phase, check if the task spawned children.
    // If so, transition to waiting instead of completing.
    // Skip the DB query entirely when no tasks have been submitted with parent_id.
    if phase == ExecutionPhase::Execute
        && deps
            .store
            .has_hierarchy
            .load(std::sync::atomic::Ordering::Relaxed)
    {
        match deps.store.active_children_count(task_id).await {
            Ok(count) if count > 0 => {
                if let Err(e) = deps.store.set_waiting(task_id, memo.as_deref()).await {
                    tracing::error!(task_id, error = %e, "failed to set task to waiting");
                }
                decrement_module();
                deps.active.remove(task_id);
                emit_event(&deps.event_tx, SchedulerEvent::Waiting {
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

    // Send completion to the coalescing channel.
    let msg = CompletionMsg {
        task: task.clone(),
        metrics: *metrics,
    };

    // Decrement module counter and remove from active map eagerly so that
    // concurrency slots are freed before the batch commits.
    decrement_module();
    deps.active.remove(task_id);

    if deps.completion_tx.send(msg).is_err() {
        tracing::error!(task_id, "completion channel closed — processing inline");
        if let Err(e) = deps
            .store
            .complete_with_record_and_resolve(task, metrics)
            .await
        {
            tracing::error!(task_id, error = %e, "inline completion failed");
        }
        return;
    }

    // Yield before draining to let more completions accumulate in the
    // channel, increasing batch size under high throughput.
    tokio::task::yield_now().await;

    // Leader election: try to grab the rx lock and drain the batch.
    // Under high concurrency, only one task wins — it processes the batch
    // for everyone. Others just wake the scheduler and return.
    if let Ok(mut rx) = deps.completion_rx.try_lock() {
        let mut batch = Vec::new();
        while let Ok(m) = rx.try_recv() {
            batch.push(m);
        }
        drop(rx);

        if !batch.is_empty() {
            process_completion_batch(
                &batch,
                &deps.store,
                &deps.event_tx,
                &deps.active,
                deps.max_retries,
                &deps.work_notify,
            )
            .await;
        }
    }

    // Wake the scheduler to dispatch new work (and drain any stragglers).
    deps.work_notify.notify_one();
}

/// Process a batch of completions: write to store, emit events, resolve parents.
///
/// Called from both the spawned task (leader election) and the run loop
/// (`drain_completions`). Handles parent resolution after the batch commits
/// since child records must be removed from the DB before the parent
/// resolution check.
pub(in crate::scheduler) async fn process_completion_batch(
    batch: &[CompletionMsg],
    store: &TaskStore,
    event_tx: &tokio::sync::broadcast::Sender<SchedulerEvent>,
    active: &ActiveTaskMap,
    max_retries: i32,
    work_notify: &Arc<tokio::sync::Notify>,
) {
    let items: Vec<(&TaskRecord, &IoBudget)> =
        batch.iter().map(|m| (&m.task, &m.metrics)).collect();

    match store.complete_batch_with_resolve(&items).await {
        Ok(results) => {
            for (msg, (_task_id, recurring_info, unblocked)) in batch.iter().zip(results) {
                emit_completion_events(&msg.task, recurring_info, &unblocked, event_tx);
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "batch completion failed");
            return;
        }
    }

    // Parent resolution: now that the batch has committed (child records are
    // removed from `tasks`), check if any parent is ready for finalization.
    for msg in batch {
        if let Some(parent_id) = msg.task.parent_id {
            handle_parent_resolution(parent_id, store, active, event_tx, max_retries, work_notify)
                .await;
        }
    }
}

/// Emit completion, recurring, and unblocked events for a single task.
pub(in crate::scheduler) fn emit_completion_events(
    task: &TaskRecord,
    recurring_info: Option<(chrono::DateTime<chrono::Utc>, i64)>,
    unblocked: &[i64],
    event_tx: &tokio::sync::broadcast::Sender<SchedulerEvent>,
) {
    if task.recurring_interval_secs.is_some() {
        let (next_run, exec_count) = match recurring_info {
            Some((next, count)) => (Some(next), count),
            None => (None, task.recurring_execution_count + 1),
        };
        emit_event(event_tx, SchedulerEvent::RecurringCompleted {
            header: task.event_header(),
            execution_count: exec_count,
            next_run,
        });
    }

    emit_event(event_tx, SchedulerEvent::Completed(task.event_header()));

    for uid in unblocked {
        emit_event(event_tx, SchedulerEvent::TaskUnblocked { task_id: *uid });
    }
}
