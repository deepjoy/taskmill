//! Failure path: retry policy, dead-letter, fail-fast cascade, dependency propagation.

use std::sync::Arc;

use crate::store::TaskStore;
use crate::task::{IoBudget, TaskError, TaskRecord};

use super::super::dispatch::ActiveTaskMap;
use super::super::{emit_event, FailureMsg, SchedulerEvent};
use super::parent::handle_parent_resolution;

/// Shared dependencies for the failure handler.
pub(crate) struct FailureDeps {
    pub store: TaskStore,
    pub active: ActiveTaskMap,
    pub event_tx: tokio::sync::broadcast::Sender<SchedulerEvent>,
    pub work_notify: Arc<tokio::sync::Notify>,
    pub max_retries: i32,
    pub registry: Arc<crate::registry::TaskTypeRegistry>,
    pub failure_tx: tokio::sync::mpsc::UnboundedSender<FailureMsg>,
    pub failure_rx:
        std::sync::Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<FailureMsg>>>,
    pub counters: Arc<crate::scheduler::counters::SchedulerCounters>,
    #[cfg(feature = "metrics")]
    pub emitter: Arc<crate::scheduler::metrics_bridge::MetricsEmitter>,
}

/// Handle a failed task execution.
///
/// Resolves retry policy, records the failure, propagates to dependents, and
/// handles fail-fast parent cascading.
pub(crate) async fn handle_failure(
    task: &TaskRecord,
    error: TaskError,
    metrics: &IoBudget,
    duration: std::time::Duration,
    deps: &FailureDeps,
    mut decrement_module: impl FnMut(),
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

    // Increment failure counters.
    deps.counters
        .failed
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if error.retryable {
        deps.counters
            .failed_retryable
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    if will_retry {
        deps.counters
            .retried
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    #[cfg(feature = "metrics")]
    {
        let module = task.module_name().unwrap_or_default();
        let retryable_label = if error.retryable { "true" } else { "false" };
        deps.emitter.record_failed(
            &task.task_type,
            module,
            task.group_key.as_deref(),
            retryable_label,
        );
        deps.emitter
            .record_duration(duration, &task.task_type, module, "failed");
        if will_retry {
            deps.emitter
                .record_retried(&task.task_type, module, task.group_key.as_deref());
        }
    }

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
    } else if task.parent_id.is_some() {
        // Terminal failure with parent — must process inline for fail-fast
        // cascade and parent resolution ordering.
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
    } else {
        // Terminal failure without parent — batch via channel.
        let msg = FailureMsg {
            task: task.clone(),
            error: error.message.clone(),
            retryable: error.retryable,
            metrics: *metrics,
            duration,
        };

        if deps.failure_tx.send(msg).is_err() {
            // Channel closed — fall back to inline.
            tracing::error!(task_id, "failure channel closed — processing inline");
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
                tracing::error!(task_id, error = %e, "inline failure recording failed");
            }
        } else {
            // Leader election: try to grab the rx lock and drain the batch.
            if let Ok(mut rx) = deps.failure_rx.try_lock() {
                let mut batch = Vec::new();
                while let Ok(m) = rx.try_recv() {
                    batch.push(m);
                }
                drop(rx);

                if !batch.is_empty() {
                    process_failure_batch(&batch, &deps.store, &deps.event_tx).await;
                }
            }
        }
    }

    // Remove from active tracking AFTER the store write completes.
    decrement_module();
    deps.active.remove(task_id);

    let dead_lettered = error.retryable && !will_retry;
    if dead_lettered {
        deps.counters
            .dead_lettered
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        #[cfg(feature = "metrics")]
        {
            let module = task.module_name().unwrap_or_default();
            deps.emitter
                .record_dead_lettered(&task.task_type, module, task.group_key.as_deref());
        }
        emit_event(
            &deps.event_tx,
            SchedulerEvent::DeadLettered {
                header: task.event_header(),
                error: error.message.clone(),
                retry_count: task.retry_count + 1,
            },
        );
    } else {
        emit_event(
            &deps.event_tx,
            SchedulerEvent::Failed {
                header: task.event_header(),
                error: error.message.clone(),
                will_retry,
                retry_after: retry_delay,
            },
        );
    }
    deps.work_notify.notify_one();

    // If permanent failure, propagate to dependency chain.
    if !will_retry {
        propagate_failure(task, &error, deps).await;
    }
}

/// Process a batch of terminal failures: write to store and emit events.
///
/// Called from both the spawned task (leader election) and the run loop
/// (`drain_failures`).
pub(in crate::scheduler) async fn process_failure_batch(
    batch: &[FailureMsg],
    store: &TaskStore,
    _event_tx: &tokio::sync::broadcast::Sender<SchedulerEvent>,
) {
    let items: Vec<(&TaskRecord, &str, bool, &IoBudget)> = batch
        .iter()
        .map(|m| (&m.task, m.error.as_str(), m.retryable, &m.metrics))
        .collect();

    if let Err(e) = store.fail_batch(&items).await {
        tracing::error!(error = %e, "batch failure recording failed");
        // Events are emitted by handle_failure directly, so nothing else needed.
    }
}

/// Propagate a permanent failure to dependents and handle fail-fast parent logic.
async fn propagate_failure(task: &TaskRecord, error: &TaskError, deps: &FailureDeps) {
    let task_id = task.id;

    match deps.store.fail_dependents(task_id).await {
        Ok((failed_ids, unblocked_ids)) => {
            if !failed_ids.is_empty() {
                deps.counters.dependency_failures.fetch_add(
                    failed_ids.len() as u64,
                    std::sync::atomic::Ordering::Relaxed,
                );
            }
            for fid in &failed_ids {
                emit_event(
                    &deps.event_tx,
                    SchedulerEvent::DependencyFailed {
                        task_id: *fid,
                        failed_dependency: task_id,
                    },
                );
            }
            for uid in &unblocked_ids {
                emit_event(
                    &deps.event_tx,
                    SchedulerEvent::TaskUnblocked { task_id: *uid },
                );
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
                            emit_event(
                                &deps.event_tx,
                                SchedulerEvent::Cancelled(at.record.event_header()),
                            );
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
                emit_event(
                    &deps.event_tx,
                    SchedulerEvent::Failed {
                        header: parent.event_header(),
                        error: msg,
                        will_retry: false,
                        retry_after: None,
                    },
                );
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
