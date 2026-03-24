//! Task spawning — orchestrator and focused submodules.
//!
//! This module decomposes the former monolithic `spawn_task` function into
//! focused, testable units:
//!
//! - [`context`] — `SpawnContext` and `TaskContext` construction
//! - [`completion`] — success path (children check, completion, recurring)
//! - [`failure`] — failure path (retry, dead-letter, fail-fast cascade)
//! - [`parent`] — parent-child resolution after task completion

mod completion;
mod context;
mod failure;
mod parent;

use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::Arc;

use crate::registry::ErasedExecutor;
use crate::task::TaskRecord;

use super::dispatch::ActiveTask;
use super::{emit_event, SchedulerEvent};

pub(in crate::scheduler) use completion::process_completion_batch;
pub(crate) use context::SpawnContext;
pub(in crate::scheduler) use failure::process_failure_batch;

/// Whether to call `execute` or `finalize` on the executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionPhase {
    Execute,
    Finalize,
}

/// Spawn a task executor and wire up completion/failure handling.
///
/// Inserts the task into the active map, starts a progress listener,
/// and spawns the executor on a new tokio task. The actual success and
/// failure handling is delegated to [`completion::handle_success`] and
/// [`failure::handle_failure`].
///
/// For zero-delay retries, the executor is re-invoked inline (without
/// requeueing to pending) to avoid the pop_next SQL round-trip.
pub(crate) async fn spawn_task(
    task: TaskRecord,
    executor: Arc<dyn ErasedExecutor>,
    ctx: SpawnContext,
    phase: ExecutionPhase,
) {
    let prepared = context::build_task_context(&task, &ctx);

    // Insert into active map before spawning to avoid races.
    ctx.active.insert(
        task.id,
        ActiveTask {
            record: task.clone(),
            token: prepared.token.clone(),
            reported_progress: None,
            reported_at: None,
            handle: None,
            io: prepared.io.clone(),
            started_at: std::time::Instant::now(),
        },
    );

    // Increment the module running counter for this task.
    if let Some(module_name) = task.module_name() {
        if let Some(counter) = ctx.module_running.get(module_name) {
            counter.fetch_add(1, AtomicOrdering::Relaxed);
        }
    }

    // Emit dispatched event.
    emit_event(
        &ctx.event_tx,
        SchedulerEvent::Dispatched(task.event_header()),
    );

    // Build deps for handlers (cloned from SpawnContext since they move into the spawned future).
    let completion_deps = completion::CompletionDeps {
        store: ctx.store.clone(),
        active: ctx.active.clone(),
        event_tx: ctx.event_tx.clone(),
        work_notify: ctx.work_notify.clone(),
        max_retries: ctx.max_retries,
        completion_tx: ctx.completion_tx.clone(),
        completion_rx: ctx.completion_rx.clone(),
    };
    let failure_deps = failure::FailureDeps {
        store: ctx.store.clone(),
        active: ctx.active.clone(),
        event_tx: ctx.event_tx.clone(),
        work_notify: ctx.work_notify.clone(),
        max_retries: ctx.max_retries,
        registry: ctx.registry.clone(),
        failure_tx: ctx.failure_tx.clone(),
        failure_rx: ctx.failure_rx.clone(),
    };

    // Keep SpawnContext alive for inline retry context rebuilds.
    let spawn_ctx = ctx;

    let task_id_for_handle = task.id;
    let active_for_handle = spawn_ctx.active.clone();
    let token_for_spawn = prepared.token.clone();
    let module_running = spawn_ctx.module_running.clone();

    let handle = tokio::spawn(async move {
        let task_id = task.id;
        let mut task = task;
        let mut prepared = prepared;

        // Helper: decrement the module running counter when this task leaves "running".
        // Capture module name upfront to avoid borrowing `task` in the closure.
        let module_name_owned = task.module_name().map(|s| s.to_string());
        let module_running_for_dec = module_running.clone();
        let mut decremented = false;
        let mut decrement_module_once = || {
            if !decremented {
                decremented = true;
                if let Some(ref name) = module_name_owned {
                    if let Some(counter) = module_running_for_dec.get(name.as_str()) {
                        counter.fetch_sub(1, AtomicOrdering::Relaxed);
                    }
                }
            }
        };

        loop {
            let result = match phase {
                ExecutionPhase::Execute => executor.execute_erased(&prepared.ctx).await,
                ExecutionPhase::Finalize => {
                    executor.finalize_erased(&prepared.ctx).await.map(|()| None)
                }
            };

            // Read IO bytes from the context tracker.
            let metrics = prepared.io.snapshot();

            // Drop the context (and its progress reporter) — executor is done.
            drop(prepared.ctx);

            match result {
                Ok(memo) => {
                    completion::handle_success(
                        &task,
                        phase,
                        &metrics,
                        memo,
                        &completion_deps,
                        &mut decrement_module_once,
                    )
                    .await;
                    break;
                }
                Err(te) => {
                    // If cancelled (preempted), the scheduler already paused it.
                    if token_for_spawn.is_cancelled() {
                        decrement_module_once();
                        failure_deps.active.remove(task_id);
                        break;
                    }

                    // Check for inline immediate retry: retryable, under max,
                    // zero delay, execute phase only.
                    if phase == ExecutionPhase::Execute && te.retryable {
                        let policy = failure_deps.registry.type_retry_policy(&task.task_type);
                        let effective_max = task.max_retries.unwrap_or(
                            policy
                                .map(|p| p.max_retries)
                                .unwrap_or(failure_deps.max_retries),
                        );
                        let will_retry = task.retry_count < effective_max;

                        if will_retry {
                            let delay = if let Some(ms) = te.retry_after_ms {
                                std::time::Duration::from_millis(ms)
                            } else if let Some(strategy) = policy.map(|p| &p.strategy) {
                                strategy.delay_for(task.retry_count)
                            } else {
                                std::time::Duration::ZERO
                            };

                            if delay.is_zero() {
                                // Inline retry: persist retry_count, re-execute.
                                let _ = failure_deps
                                    .store
                                    .increment_retry(task.id, &te.message)
                                    .await;
                                task.retry_count += 1;

                                // Emit retry event.
                                emit_event(
                                    &failure_deps.event_tx,
                                    SchedulerEvent::Failed {
                                        header: task.event_header(),
                                        error: te.message,
                                        will_retry: true,
                                        retry_after: None,
                                    },
                                );

                                // Rebuild context for next attempt.
                                prepared = context::build_task_context(&task, &spawn_ctx);
                                continue;
                            }
                        }
                    }

                    // Not inline-retryable — use normal failure path.
                    failure::handle_failure(
                        &task,
                        te,
                        &metrics,
                        &failure_deps,
                        &mut decrement_module_once,
                    )
                    .await;
                    break;
                }
            }
        }
    });

    // Store the handle so shutdown can join it.
    active_for_handle.set_handle(task_id_for_handle, handle);
}
