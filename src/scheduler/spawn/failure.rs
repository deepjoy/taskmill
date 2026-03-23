//! Failure path: retry policy, dead-letter, fail-fast cascade, dependency propagation.

use std::sync::Arc;

use crate::store::TaskStore;
use crate::task::{IoBudget, TaskError, TaskRecord};

use super::super::dispatch::ActiveTaskMap;
use super::super::SchedulerEvent;
use super::parent::handle_parent_resolution;

/// Shared dependencies for the failure handler.
pub(crate) struct FailureDeps {
    pub store: TaskStore,
    pub active: ActiveTaskMap,
    pub event_tx: tokio::sync::broadcast::Sender<SchedulerEvent>,
    pub work_notify: Arc<tokio::sync::Notify>,
    pub max_retries: i32,
    pub registry: Arc<crate::registry::TaskTypeRegistry>,
}

/// Handle a failed task execution.
///
/// Resolves retry policy, records the failure, propagates to dependents, and
/// handles fail-fast parent cascading.
pub(crate) async fn handle_failure(
    task: &TaskRecord,
    error: TaskError,
    metrics: &IoBudget,
    deps: &FailureDeps,
    decrement_module: impl FnOnce(),
) {
    let task_id = task.id;

    // Resolve effective retry policy for this task type.
    let policy = deps.registry.type_retry_policy(&task.task_type);
    let effective_max_retries = task
        .max_retries
        .unwrap_or(policy.map(|p| p.max_retries).unwrap_or(deps.max_retries));
    let backoff_strategy = policy.map(|p| &p.strategy);

    let will_retry = error.retryable && task.retry_count < effective_max_retries;

    // Compute retry delay for event reporting.
    let retry_delay = if will_retry {
        if let Some(ms) = error.retry_after_ms {
            Some(std::time::Duration::from_millis(ms))
        } else if let Some(strategy) = backoff_strategy {
            let d = strategy.delay_for(task.retry_count);
            if d.is_zero() {
                None
            } else {
                Some(d)
            }
        } else {
            None
        }
    } else {
        None
    };

    tracing::warn!(
        task_id,
        task_type = task.task_type,
        error = %error.message,
        retryable = error.retryable,
        will_retry,
        "task failed"
    );

    if will_retry {
        // Fast path: single UPDATE without a transaction wrapper.
        // Avoids BEGIN IMMEDIATE + COMMIT round-trips, reducing connection
        // hold time from 3 SQL executions to 1.
        let delay = retry_delay.unwrap_or(std::time::Duration::ZERO);
        if let Err(e) = deps
            .store
            .requeue_for_retry(task.id, &error.message, delay)
            .await
        {
            tracing::error!(task_id, error = %e, "failed to requeue task for retry");
        }
    } else {
        // Terminal failure (permanent or retries exhausted): needs a
        // transaction for the multi-statement INSERT history + DELETE.
        let fail_backoff = crate::store::FailBackoff {
            strategy: backoff_strategy,
            executor_retry_after_ms: error.retry_after_ms,
        };
        if let Err(e) = deps
            .store
            .fail_with_record(
                task,
                &error.message,
                error.retryable,
                effective_max_retries,
                metrics,
                &fail_backoff,
            )
            .await
        {
            tracing::error!(task_id, error = %e, "failed to record task failure");
        }
    }

    // Remove from active tracking AFTER the store write completes.
    decrement_module();
    deps.active.remove(task_id);

    let dead_lettered = error.retryable && !will_retry;
    if dead_lettered {
        let _ = deps.event_tx.send(SchedulerEvent::DeadLettered {
            header: task.event_header(),
            error: error.message.clone(),
            retry_count: task.retry_count + 1,
        });
    } else {
        let _ = deps.event_tx.send(SchedulerEvent::Failed {
            header: task.event_header(),
            error: error.message.clone(),
            will_retry,
            retry_after: retry_delay,
        });
    }
    deps.work_notify.notify_one();

    // If permanent failure, propagate to dependency chain.
    if !will_retry {
        propagate_failure(task, &error, deps).await;
    }
}

/// Propagate a permanent failure to dependents and handle fail-fast parent logic.
async fn propagate_failure(task: &TaskRecord, error: &TaskError, deps: &FailureDeps) {
    let task_id = task.id;

    match deps.store.fail_dependents(task_id).await {
        Ok((failed_ids, unblocked_ids)) => {
            for fid in &failed_ids {
                let _ = deps.event_tx.send(SchedulerEvent::DependencyFailed {
                    task_id: *fid,
                    failed_dependency: task_id,
                });
            }
            for uid in &unblocked_ids {
                let _ = deps
                    .event_tx
                    .send(SchedulerEvent::TaskUnblocked { task_id: *uid });
            }
            if !unblocked_ids.is_empty() {
                deps.work_notify.notify_one();
            }
        }
        Err(e) => {
            tracing::error!(task_id, error = %e, "failed to propagate failure to dependents");
        }
    }

    if let Some(parent_id) = task.parent_id {
        // Check if parent uses fail_fast.
        if let Ok(Some(parent)) = deps.store.task_by_id(parent_id).await {
            if parent.fail_fast {
                // Cancel remaining siblings.
                if let Ok(running_ids) = deps.store.cancel_children(parent_id).await {
                    for rid in &running_ids {
                        if let Some(at) = deps.active.remove(*rid) {
                            at.token.cancel();
                            let _ = deps.store.delete(*rid).await;
                            let _ = deps
                                .event_tx
                                .send(SchedulerEvent::Cancelled(at.record.event_header()));
                        }
                    }
                }
                // Fail the parent.
                let msg = format!("child task {task_id} failed: {}", error.message);
                if let Err(e) = deps
                    .store
                    .fail_with_record(
                        &parent,
                        &msg,
                        false,
                        0,
                        &IoBudget::default(),
                        &Default::default(),
                    )
                    .await
                {
                    tracing::error!(
                        parent_id,
                        error = %e,
                        "failed to record parent failure"
                    );
                }
                let _ = deps.event_tx.send(SchedulerEvent::Failed {
                    header: parent.event_header(),
                    error: msg,
                    will_retry: false,
                    retry_after: None,
                });
            } else {
                // Not fail_fast — check if all children done.
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
    }
}
