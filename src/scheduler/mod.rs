//! The scheduler: core types, configuration, and the main run loop.
//!
//! [`Scheduler`] coordinates task execution — popping from the
//! [`TaskStore`], applying [backpressure](crate::backpressure),
//! IO-budget checks, and [group concurrency](crate::GroupLimits) limits,
//! preempting lower-priority work, and emitting [`SchedulerEvent`]s for UI
//! integration. Use [`SchedulerBuilder`] for ergonomic construction.
//!
//! The `Scheduler` implementation is split across focused submodules:
//! - [`submit`] — task submission, lookup, and cancellation
//! - [`run_loop`] — the main event loop, dispatch, and shutdown
//! - [`control`] — pause/resume, concurrency limits, and group limits
//! - [`queries`] — read-only queries (active tasks, progress, snapshots)
//! - [`builder`] — ergonomic construction via [`SchedulerBuilder`]
//! - [`dispatch`] — task spawning, active-task tracking, and preemption
//! - [`gate`] — admission control (IO budget, backpressure, group limits)
//! - [`event`] — event types and scheduler configuration
//! - [`progress`] — progress reporting and extrapolation
//!
//! See the [crate-level docs](crate) for a full walkthrough of the task
//! lifecycle, common patterns, and how the dispatch loop works.

mod builder;
mod control;
pub(crate) mod dispatch;
pub(crate) mod event;
pub(crate) mod gate;
pub mod progress;
mod queries;
mod run_loop;
mod submit;
#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Arc;

use tokio::sync::{Mutex, Notify};
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::backpressure::{CompositePressure, ThrottlePolicy};
use crate::priority::Priority;
use crate::registry::TaskTypeRegistry;
use crate::resource::ResourceReader;
use crate::store::TaskStore;

use dispatch::ActiveTaskMap;

pub use builder::SchedulerBuilder;
pub use event::{SchedulerConfig, SchedulerEvent, SchedulerSnapshot, ShutdownMode};
pub use gate::GroupLimits;
pub use progress::{EstimatedProgress, ProgressReporter};

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
    /// Type-keyed application state passed to every executor via [`TaskContext::state`].
    pub(crate) app_state: Arc<crate::registry::StateMap>,
    /// Global pause flag — when `true`, the run loop skips dispatching.
    pub(crate) paused: AtomicBool,
    /// Wakes the run loop when new work is submitted or the scheduler is resumed.
    pub(crate) work_notify: Arc<Notify>,
    /// Per-group concurrency limits.
    pub(crate) group_limits: GroupLimits,
}

/// IO-aware priority scheduler.
///
/// Coordinates task execution by:
/// 1. Popping highest-priority pending tasks from the SQLite store
/// 2. Checking IO budget against running task estimates and system capacity
/// 3. Applying backpressure throttling based on external pressure sources
/// 4. Preempting lower-priority tasks when high-priority work arrives
/// 5. Managing retries and failure recording
/// 6. Emitting lifecycle events for UI integration
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
        )
    }

    /// Create a scheduler with a custom dispatch gate.
    pub(crate) fn with_gate(
        store: TaskStore,
        config: SchedulerConfig,
        registry: Arc<TaskTypeRegistry>,
        gate: Box<dyn gate::DispatchGate>,
        app_state: Arc<crate::registry::StateMap>,
    ) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(256);
        Self {
            inner: Arc::new(SchedulerInner {
                store,
                max_concurrency: AtomicUsize::new(config.max_concurrency),
                max_retries: config.max_retries,
                preempt_priority: config.preempt_priority,
                poll_interval: config.poll_interval,
                throughput_sample_size: config.throughput_sample_size,
                shutdown_mode: config.shutdown_mode,
                registry,
                gate,
                resource_reader: Mutex::new(None),
                active: ActiveTaskMap::new(),
                event_tx,
                sampler_token: CancellationToken::new(),
                app_state,
                paused: AtomicBool::new(false),
                work_notify: Arc::new(Notify::new()),
                group_limits: GroupLimits::new(),
            }),
        }
    }

    /// Create a [`SchedulerBuilder`] for ergonomic construction.
    pub fn builder() -> SchedulerBuilder {
        SchedulerBuilder::new()
    }

    /// Subscribe to scheduler lifecycle events.
    ///
    /// Returns a broadcast receiver. Events are emitted on task dispatch,
    /// completion, failure, preemption, cancellation, and progress. Useful for
    /// bridging to a Tauri frontend or updating UI state.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<SchedulerEvent> {
        self.inner.event_tx.subscribe()
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
