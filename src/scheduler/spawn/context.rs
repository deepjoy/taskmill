//! Task context construction for spawned tasks.

use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::registry::{ChildSpawner, IoTracker, ParentContext, StateSnapshot, TaskContext};
use crate::store::TaskStore;
use crate::task::TaskRecord;

use super::super::dispatch::ActiveTaskMap;
use super::super::progress::ProgressReporter;
use super::super::SchedulerEvent;

/// Shared scheduler resources passed to each spawned task.
pub(crate) struct SpawnContext {
    pub store: TaskStore,
    pub active: ActiveTaskMap,
    pub event_tx: tokio::sync::broadcast::Sender<SchedulerEvent>,
    pub max_retries: i32,
    pub registry: Arc<crate::registry::TaskTypeRegistry>,
    pub app_state: StateSnapshot,
    pub work_notify: Arc<tokio::sync::Notify>,
    pub scheduler: super::super::WeakScheduler,
    #[allow(dead_code)]
    pub cancel_hook_timeout: tokio::time::Duration,
    /// Per-module live running counts. Incremented on dispatch; decremented on terminal.
    pub module_running: Arc<HashMap<String, AtomicUsize>>,
    /// Pre-snapshotted per-module state (module name → snapshot). Cloned at dispatch time.
    pub module_state: Arc<HashMap<String, StateSnapshot>>,
    /// Registry of all registered modules — shared with spawned tasks so they can
    /// construct [`ModuleHandle`](crate::module::ModuleHandle) instances.
    pub module_registry: Arc<crate::module::ModuleRegistry>,
    /// Completion coalescing channel sender.
    pub completion_tx: tokio::sync::mpsc::UnboundedSender<super::super::CompletionMsg>,
    /// Completion coalescing channel receiver (Arc-wrapped for leader election).
    pub completion_rx: std::sync::Arc<
        tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<super::super::CompletionMsg>>,
    >,
    /// Failure coalescing channel sender.
    pub failure_tx: tokio::sync::mpsc::UnboundedSender<super::super::FailureMsg>,
    /// Failure coalescing channel receiver (Arc-wrapped for leader election).
    pub failure_rx: std::sync::Arc<
        tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<super::super::FailureMsg>>,
    >,
    /// Priority aging configuration. `None` = aging disabled.
    pub aging_config: Option<Arc<crate::scheduler::aging::AgingConfig>>,
    /// Internal atomic counters for throughput metrics.
    pub counters: Arc<crate::scheduler::counters::SchedulerCounters>,
    /// `metrics` crate emitter (feature-gated).
    #[cfg(feature = "metrics")]
    pub emitter: Arc<crate::scheduler::metrics_bridge::MetricsEmitter>,
}

/// Output of task context construction — everything needed to insert into the
/// active map and spawn the executor.
pub(crate) struct PreparedTask {
    pub ctx: TaskContext,
    pub io: Arc<IoTracker>,
    pub token: CancellationToken,
}

/// Build the [`TaskContext`], [`IoTracker`], and [`CancellationToken`] for a task.
pub(crate) fn build_task_context(task: &TaskRecord, spawn_ctx: &SpawnContext) -> PreparedTask {
    let owning_module = task.module_name().unwrap_or_default().to_string();

    // Clone the pre-snapshotted module state — no lock needed, already lock-free.
    let module_state_snapshot: StateSnapshot = task
        .module_name()
        .and_then(|name| spawn_ctx.module_state.get(name).cloned())
        .unwrap_or_default();

    let token = CancellationToken::new();

    let child_spawner = ChildSpawner::new(
        spawn_ctx.store.clone(),
        task.id,
        spawn_ctx.work_notify.clone(),
        ParentContext {
            created_at: task.created_at,
            ttl_seconds: task.ttl_seconds,
            ttl_from: task.ttl_from,
            started_at: task.started_at,
            tags: task.tags.clone(),
        },
    );

    let io = Arc::new(IoTracker::new());

    let ctx = TaskContext {
        record: task.clone(),
        token: token.clone(),
        progress: ProgressReporter::new(
            task.event_header_with_aging(spawn_ctx.aging_config.as_deref()),
            spawn_ctx.event_tx.clone(),
            spawn_ctx.active.clone(),
            io.clone(),
        ),
        scheduler: spawn_ctx.scheduler.clone(),
        app_state: spawn_ctx.app_state.clone(),
        module_state: module_state_snapshot,
        child_spawner: Some(child_spawner),
        io: io.clone(),
        module_registry: spawn_ctx.module_registry.clone(),
        owning_module: owning_module.clone(),
        aging_config: spawn_ctx.aging_config.clone(),
    };

    PreparedTask { ctx, io, token }
}
