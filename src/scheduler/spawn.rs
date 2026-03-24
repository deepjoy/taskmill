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
use super::SchedulerEvent;

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
    let _ = ctx
        .event_tx
        .send(SchedulerEvent::Dispatched(task.event_header()));

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
        store: ctx.store,
        active: ctx.active.clone(),
        event_tx: ctx.event_tx,
        work_notify: ctx.work_notify,
        max_retries: ctx.max_retries,
        registry: ctx.registry,
        failure_tx: ctx.failure_tx,
        failure_rx: ctx.failure_rx,
    };

    let task_id_for_handle = task.id;
    let active_for_handle = ctx.active;
    let token_for_spawn = prepared.token.clone();
    let module_running = ctx.module_running;
    let io = prepared.io;

    let handle = tokio::spawn(async move {
        let task_id = task.id;

        // Helper: decrement the module running counter when this task leaves "running".
        let decrement_module = || {
            if let Some(name) = task.module_name() {
                if let Some(counter) = module_running.get(name) {
                    counter.fetch_sub(1, AtomicOrdering::Relaxed);
                }
            }
        };

        let result = match phase {
            ExecutionPhase::Execute => executor.execute_erased(&prepared.ctx).await,
            ExecutionPhase::Finalize => {
                executor.finalize_erased(&prepared.ctx).await.map(|()| None)
            } // finalize doesn't produce a memo
        };

        // Read IO bytes from the context tracker.
        let metrics = io.snapshot();

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
                    decrement_module,
                )
                .await;
            }
            Err(te) => {
                // If cancelled (preempted), the scheduler already paused it.
                if token_for_spawn.is_cancelled() {
                    decrement_module();
                    failure_deps.active.remove(task_id);
                    return;
                }

                failure::handle_failure(&task, te, &metrics, &failure_deps, decrement_module).await;
            }
        }
    });

    // Store the handle so shutdown can join it.
    active_for_handle.set_handle(task_id_for_handle, handle);
}
