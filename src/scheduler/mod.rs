//! The scheduler: core types, configuration, and the main run loop.
//!
//! [`Scheduler`] coordinates task execution — popping from the
//! [`TaskStore`], applying [backpressure](crate::backpressure),
//! IO-budget checks, [group concurrency](crate::GroupLimits) limits,
//! and [token-bucket rate limiting](crate::RateLimit) per task type and group,
//! preempting lower-priority work, and emitting [`SchedulerEvent`]s for UI
//! integration. Use [`SchedulerBuilder`] for ergonomic construction.
//!
//! The `Scheduler` implementation is split across focused submodules:
//! - `submit` — task submission, lookup, cancellation, and superseding
//! - `run_loop` — the main event loop, dispatch, and shutdown
//! - `control` — pause/resume, concurrency limits, group limits, and rate limits
//! - `queries` — read-only queries (active tasks, progress, snapshots)
//! - `builder` — ergonomic construction via [`SchedulerBuilder`]
//! - `dispatch` — task spawning, active-task tracking, and preemption
//! - `gate` — admission control (IO budget, backpressure, group limits, rate limits)
//! - `event` — event types and scheduler configuration
//! - [`progress`] — progress reporting, byte-level tracking, and extrapolation
//!
//! See the [crate-level docs](crate) for a full walkthrough of the task
//! lifecycle, common patterns, and how the dispatch loop works.

pub mod aging;
mod builder;
mod control;
pub(crate) mod dispatch;
pub(crate) mod event;
pub mod fair;
pub(crate) mod gate;
pub mod progress;
mod queries;
pub(crate) mod rate_limit;
mod run_loop;
pub(crate) mod spawn;
mod submit;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, RwLock};

use tokio::sync::{Mutex, Notify};
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::task::{IoBudget, TaskRecord};

use crate::backpressure::{CompositePressure, ThrottlePolicy};
use crate::priority::Priority;
use crate::registry::TaskTypeRegistry;
use crate::resource::ResourceReader;
use crate::store::TaskStore;

use dispatch::ActiveTaskMap;

pub use builder::SchedulerBuilder;

// ── Completion coalescing ──────────────────────────────────────────

/// Message sent from spawned tasks to the scheduler's completion channel.
///
/// Batched by `drain_completions` to reduce per-completion transaction overhead.
pub(crate) struct CompletionMsg {
    pub task: TaskRecord,
    pub metrics: IoBudget,
}

// ── Failure coalescing ───────────────────────────────────────────

/// Message sent from spawned tasks for terminal failures (dead-letter / permanent).
///
/// Batched by `drain_failures` to reduce per-failure transaction overhead,
/// mirroring the completion coalescing pattern.
pub(crate) struct FailureMsg {
    pub task: TaskRecord,
    pub error: String,
    pub retryable: bool,
    pub metrics: IoBudget,
}
pub use aging::AgingConfig;
pub use event::{
    PausedGroupInfo, SchedulerConfig, SchedulerEvent, SchedulerSnapshot, ShutdownMode,
    TaskEventHeader,
};
pub use fair::GroupAllocationInfo;
pub use gate::GroupLimits;
pub use progress::{EstimatedProgress, ProgressReporter, TaskProgress};
pub use rate_limit::{RateLimit, RateLimitInfo, RateLimits};

/// Emit a scheduler event only when at least one subscriber is listening.
///
/// Avoids the broadcast channel's internal locking and allocation overhead
/// when no subscribers exist (common in benchmarks and headless operation).
#[inline]
pub(crate) fn emit_event(
    tx: &tokio::sync::broadcast::Sender<SchedulerEvent>,
    event: SchedulerEvent,
) {
    if tx.receiver_count() > 0 {
        let _ = tx.send(event);
    }
}

// ── Scheduler ───────────────────────────────────────────────────────

/// Shared inner state behind `Arc` so `Scheduler` can be `Clone`.
#[allow(dead_code)]
pub(crate) struct SchedulerInner {
    pub(crate) store: TaskStore,
    pub(crate) max_concurrency: AtomicUsize,
    pub(crate) max_retries: i32,
    pub(crate) preempt_priority: Priority,
    pub(crate) poll_interval: Duration,
    pub(crate) throughput_sample_size: i32,
    pub(crate) shutdown_mode: ShutdownMode,
    pub(crate) registry: Arc<TaskTypeRegistry>,
    pub(crate) gate: Box<dyn gate::DispatchGate>,
    pub(crate) resource_reader: Mutex<Option<Arc<dyn ResourceReader>>>,
    /// In-memory tracking of active tasks and their cancellation tokens.
    pub(crate) active: ActiveTaskMap,
    /// Broadcast channel for lifecycle events.
    pub(crate) event_tx: tokio::sync::broadcast::Sender<SchedulerEvent>,
    /// Token to cancel the background resource sampler (if started).
    pub(crate) sampler_token: CancellationToken,
    /// Token to cancel the background progress ticker (if started).
    pub(crate) progress_ticker_token: CancellationToken,
    /// Configured interval for byte-level progress polling.
    pub(crate) progress_interval: Option<Duration>,
    /// Broadcast channel for byte-level progress events.
    pub(crate) progress_tx: tokio::sync::broadcast::Sender<TaskProgress>,
    /// Type-keyed application state passed to every executor via [`TaskContext::state`].
    pub(crate) app_state: Arc<crate::registry::StateMap>,
    /// Global pause flag — when `true`, the run loop skips dispatching.
    pub(crate) paused: AtomicBool,
    /// Wakes the run loop when new work is submitted or the scheduler is resumed.
    pub(crate) work_notify: Arc<Notify>,
    /// Per-group concurrency limits.
    pub(crate) group_limits: GroupLimits,
    /// Per-task-type rate limits (e.g. "media::upload" → 100/sec).
    pub(crate) type_rate_limits: rate_limit::RateLimits,
    /// Per-group rate limits (e.g. "s3://b2-us-west" → 200/sec).
    pub(crate) group_rate_limits: rate_limit::RateLimits,
    /// Timeout for on_cancel hooks.
    pub(crate) cancel_hook_timeout: Duration,
    /// Default TTL for tasks without an explicit TTL.
    pub(crate) default_ttl: Option<Duration>,
    /// How often to sweep for expired tasks.
    pub(crate) expiry_sweep_interval: Option<Duration>,
    /// Last time the expiry sweep ran.
    pub(crate) last_expiry_sweep: std::sync::Mutex<tokio::time::Instant>,
    /// Registry of all registered modules (empty for schedulers built without the module API).
    pub(crate) module_registry: Arc<crate::module::ModuleRegistry>,
    /// Per-module app state (module name → state map). Populated at build time from
    /// each module's `.app_state()` calls. Executors access it via
    /// [`TaskContext::state`], which checks module state before falling back to global.
    pub(crate) module_state: Arc<HashMap<String, crate::registry::StateSnapshot>>,
    /// Per-module pause flags. Keys are module names; values are `true` when that
    /// module has been explicitly paused via [`ModuleHandle::pause`].
    /// Initialized to `false` for every module at build time.
    pub(crate) module_paused: HashMap<String, AtomicBool>,
    /// Per-module concurrency caps (module name → cap).
    /// Initialized from `Module::max_concurrency` at build time.
    /// Updated at runtime by `ModuleHandle::set_max_concurrency`.
    pub(crate) module_caps: RwLock<HashMap<String, usize>>,
    /// Per-module live running counts (module name → count).
    /// Incremented when a task is dispatched; decremented on every terminal transition.
    /// Shared with spawned tasks via `Arc` so they can decrement on completion.
    pub(crate) module_running: Arc<HashMap<String, AtomicUsize>>,
    /// Fast-path flag: set to `true` when a task is paused (preempted).
    /// Cleared when `paused_tasks()` returns empty. Avoids a SQL round-trip
    /// per dispatch cycle when no tasks are paused.
    pub(crate) has_paused_tasks: AtomicBool,
    /// Fast-dispatch mode: when `true`, `try_dispatch` uses `pop_next()`
    /// (single SQL) instead of `peek_next()` + gate + `claim_task()` (2 SQL).
    /// Computed at build time: `true` when no groups, no resource monitoring,
    /// and no module concurrency caps are configured.
    pub(crate) fast_dispatch: AtomicBool,
    /// Set of currently paused group keys. Mirrors the `paused_groups` SQLite
    /// table for fast in-memory lookups during gate checks and submit.
    /// Uses `std::sync::RwLock` since reads vastly outnumber writes.
    pub(crate) paused_groups: std::sync::RwLock<HashSet<String>>,
    /// Builder-computed: were pressure sources configured? Stored here because
    /// the `CompositePressure` is boxed inside `DefaultDispatchGate` and cannot
    /// be introspected through `Box<dyn DispatchGate>`.
    pub(crate) has_pressure_sources: AtomicBool,
    /// Builder-computed: was resource monitoring enabled?
    pub(crate) has_resource_monitoring: AtomicBool,
    /// Last time the auto-resume check ran for time-boxed group pauses.
    pub(crate) last_group_resume_check: std::sync::Mutex<tokio::time::Instant>,
    /// Send side of the completion coalescing channel.
    pub(crate) completion_tx: tokio::sync::mpsc::UnboundedSender<CompletionMsg>,
    /// Receive side, `Arc`-wrapped so spawned tasks can try to drain the batch
    /// (leader election pattern) in addition to the run loop.
    pub(crate) completion_rx:
        std::sync::Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<CompletionMsg>>>,
    /// Send side of the failure coalescing channel.
    pub(crate) failure_tx: tokio::sync::mpsc::UnboundedSender<FailureMsg>,
    /// Receive side (leader election + run loop drain).
    pub(crate) failure_rx: std::sync::Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<FailureMsg>>>,
    /// Priority aging configuration. `None` = aging disabled.
    pub(crate) aging_config: Option<Arc<aging::AgingConfig>>,
    /// Per-group scheduling weights for weighted fair dispatch.
    pub(crate) group_weights: fair::GroupWeights,
}

/// IO-aware priority scheduler.
///
/// Coordinates task execution by:
/// 1. Popping highest-priority pending tasks from the SQLite store
/// 2. Checking IO budget against running task estimates and system capacity
/// 3. Applying backpressure throttling based on external pressure sources
/// 4. Enforcing token-bucket rate limits per task type and group
/// 5. Preempting lower-priority tasks when high-priority work arrives
/// 6. Managing retries and failure recording
/// 7. Emitting lifecycle events for UI integration
///
/// `Scheduler` is `Clone` — each clone shares the same underlying state.
/// This makes it easy to hold in `tauri::State<Scheduler>` or share across
/// async tasks.
#[derive(Clone)]
pub struct Scheduler {
    pub(crate) inner: Arc<SchedulerInner>,
}

/// Weak handle to a [`Scheduler`] that does not prevent shutdown.
///
/// Used inside [`TaskContext`](crate::TaskContext) to avoid keeping the
/// scheduler alive via a strong `Arc` cycle. Upgrade to a full `Scheduler`
/// before use — the upgrade fails if the scheduler has already been dropped.
#[derive(Clone)]
pub(crate) struct WeakScheduler {
    inner: std::sync::Weak<SchedulerInner>,
}

impl WeakScheduler {
    /// Attempt to upgrade to a full [`Scheduler`].
    ///
    /// Returns `None` if the scheduler has been dropped.
    pub fn upgrade(&self) -> Option<Scheduler> {
        self.inner.upgrade().map(|inner| Scheduler { inner })
    }
}

impl Scheduler {
    /// Create a weak handle that does not prevent scheduler shutdown.
    pub(crate) fn downgrade(&self) -> WeakScheduler {
        WeakScheduler {
            inner: Arc::downgrade(&self.inner),
        }
    }

    pub fn new(
        store: TaskStore,
        config: SchedulerConfig,
        registry: Arc<TaskTypeRegistry>,
        pressure: CompositePressure,
        policy: ThrottlePolicy,
    ) -> Self {
        let gate = Box::new(gate::DefaultDispatchGate::new(pressure, policy));
        Self::with_gate(
            store,
            config,
            registry,
            gate,
            Arc::new(crate::registry::StateMap::new()),
            Arc::new(crate::module::ModuleRegistry::empty()),
            Arc::new(HashMap::new()),
        )
    }

    /// Create a scheduler with a custom dispatch gate.
    pub(crate) fn with_gate(
        store: TaskStore,
        config: SchedulerConfig,
        registry: Arc<TaskTypeRegistry>,
        gate: Box<dyn gate::DispatchGate>,
        app_state: Arc<crate::registry::StateMap>,
        module_registry: Arc<crate::module::ModuleRegistry>,
        module_state: Arc<HashMap<String, crate::registry::StateSnapshot>>,
    ) -> Self {
        let module_paused: HashMap<String, AtomicBool> = module_registry
            .entries()
            .iter()
            .map(|e| (e.name.clone(), AtomicBool::new(false)))
            .collect();
        let module_caps: HashMap<String, usize> = module_registry
            .entries()
            .iter()
            .filter_map(|e| e.max_concurrency.map(|cap| (e.name.clone(), cap)))
            .collect();
        let module_running: Arc<HashMap<String, AtomicUsize>> = Arc::new(
            module_registry
                .entries()
                .iter()
                .map(|e| (e.name.clone(), AtomicUsize::new(0)))
                .collect(),
        );
        let (event_tx, _) = tokio::sync::broadcast::channel(256);
        let (progress_tx, _) = tokio::sync::broadcast::channel(64);
        let (completion_tx, completion_rx) = tokio::sync::mpsc::unbounded_channel();
        let (failure_tx, failure_rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            inner: Arc::new(SchedulerInner {
                store,
                max_concurrency: AtomicUsize::new(config.max_concurrency),
                max_retries: config.max_retries,
                preempt_priority: config.preempt_priority,
                poll_interval: config.poll_interval,
                throughput_sample_size: config.throughput_sample_size,
                shutdown_mode: config.shutdown_mode,
                progress_interval: config.progress_interval,
                registry,
                gate,
                resource_reader: Mutex::new(None),
                active: ActiveTaskMap::new(),
                event_tx,
                progress_tx,
                sampler_token: CancellationToken::new(),
                progress_ticker_token: CancellationToken::new(),
                app_state,
                paused: AtomicBool::new(false),
                work_notify: Arc::new(Notify::new()),
                group_limits: GroupLimits::new(),
                type_rate_limits: rate_limit::RateLimits::new(),
                group_rate_limits: rate_limit::RateLimits::new(),
                cancel_hook_timeout: config.cancel_hook_timeout,
                default_ttl: config.default_ttl,
                expiry_sweep_interval: config.expiry_sweep_interval,
                last_expiry_sweep: std::sync::Mutex::new(tokio::time::Instant::now()),
                module_registry,
                module_state,
                module_paused,
                module_caps: RwLock::new(module_caps),
                module_running,
                // Start false — set to true only when preemption pauses a task.
                // For persistent stores the builder checks for pre-existing
                // paused tasks after construction (see SchedulerBuilder::build).
                has_paused_tasks: AtomicBool::new(false),
                // Default to false; builder sets true when safe.
                fast_dispatch: AtomicBool::new(false),
                // Empty by default; builder loads from DB.
                paused_groups: std::sync::RwLock::new(HashSet::new()),
                // Conservative defaults; builder overrides.
                has_pressure_sources: AtomicBool::new(false),
                has_resource_monitoring: AtomicBool::new(false),
                last_group_resume_check: std::sync::Mutex::new(tokio::time::Instant::now()),
                completion_tx,
                completion_rx: std::sync::Arc::new(Mutex::new(completion_rx)),
                failure_tx,
                failure_rx: std::sync::Arc::new(Mutex::new(failure_rx)),
                aging_config: config.aging_config.map(Arc::new),
                group_weights: fair::GroupWeights::new(),
            }),
        }
    }

    /// Create a [`SchedulerBuilder`] for ergonomic construction.
    pub fn builder() -> SchedulerBuilder {
        SchedulerBuilder::new()
    }

    /// Returns the module registry for this scheduler.
    ///
    /// Contains metadata for all modules registered at build time.
    pub fn module_registry(&self) -> &crate::module::ModuleRegistry {
        &self.inner.module_registry
    }

    /// Get a typed domain handle.
    ///
    /// The handle exposes submission, cancellation, pause/resume, and query
    /// methods that are automatically scoped to this domain's task type prefix.
    ///
    /// # Panics
    ///
    /// Panics if `D` was not registered with [`SchedulerBuilder::domain`].
    /// For fallible lookup, use [`try_domain`](Self::try_domain) instead.
    pub fn domain<D: crate::domain::DomainKey>(&self) -> crate::domain::DomainHandle<D> {
        self.try_domain::<D>()
            .unwrap_or_else(|| panic!("domain '{}' is not registered — did you forget to add .domain(...) to the SchedulerBuilder?", D::NAME))
    }

    /// Get a typed domain handle, returning `None` if the domain is not registered.
    pub fn try_domain<D: crate::domain::DomainKey>(
        &self,
    ) -> Option<crate::domain::DomainHandle<D>> {
        let handle = self.try_module(D::NAME)?;
        Some(crate::domain::DomainHandle::new(handle))
    }

    /// Get an untyped module handle by name (internal).
    #[allow(dead_code)]
    pub(crate) fn module(&self, name: &str) -> crate::module::ModuleHandle {
        self.try_module(name)
            .unwrap_or_else(|| panic!("module '{name}' is not registered — did you forget to add .domain(...) to the SchedulerBuilder?"))
    }

    /// Get an untyped module handle, returning `None` if not registered (internal).
    pub(crate) fn try_module(&self, name: &str) -> Option<crate::module::ModuleHandle> {
        let entry = self.inner.module_registry.get(name)?;
        Some(crate::module::ModuleHandle::new(self.clone(), entry))
    }

    /// All registered module handles, in registration order (internal).
    #[allow(dead_code)]
    pub(crate) fn modules(&self) -> Vec<crate::module::ModuleHandle> {
        self.inner
            .module_registry
            .entries()
            .iter()
            .map(|e| crate::module::ModuleHandle::new(self.clone(), e))
            .collect()
    }

    /// Look up an active task by ID, regardless of which module owns it.
    ///
    /// Returns `None` if no active task with that ID exists.
    pub async fn task(
        &self,
        task_id: i64,
    ) -> Result<Option<crate::task::TaskRecord>, crate::store::StoreError> {
        self.inner.store.task_by_id(task_id).await
    }

    /// Subscribe to scheduler lifecycle events.
    ///
    /// Returns a broadcast receiver. Events are emitted on task dispatch,
    /// completion, failure, preemption, cancellation, and progress. Useful for
    /// bridging to a Tauri frontend or updating UI state.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<SchedulerEvent> {
        self.inner.event_tx.subscribe()
    }

    /// Subscribe to byte-level progress events.
    ///
    /// Returns a broadcast receiver for [`TaskProgress`] events emitted at
    /// the configured `progress_interval`. These are separate from lifecycle
    /// events to avoid flooding the main event stream.
    ///
    /// Requires `progress_interval` to be set via the builder; otherwise the
    /// ticker is not spawned and no events will be emitted.
    pub fn subscribe_progress(&self) -> tokio::sync::broadcast::Receiver<TaskProgress> {
        self.inner.progress_tx.subscribe()
    }

    /// Set the resource reader for IO-aware scheduling.
    pub async fn set_resource_reader(&self, reader: Arc<dyn ResourceReader>) {
        *self.inner.resource_reader.lock().await = Some(reader);
    }

    /// Get a reference to the underlying store for direct queries.
    pub fn store(&self) -> &TaskStore {
        &self.inner.store
    }

    /// Register shared application state after the scheduler has been built.
    ///
    /// This is useful when library code (e.g. shoebox) needs to inject its
    /// own state into a scheduler that was constructed by a parent
    /// application. Multiple types can coexist — each is keyed by `TypeId`.
    pub async fn register_state<T: Send + Sync + 'static>(&self, state: Arc<T>) {
        self.inner.app_state.insert(state).await;
    }
}
