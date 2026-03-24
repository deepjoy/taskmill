//! Dispatch gate: admission control for task dispatch.
//!
//! The [`DispatchGate`] trait decides whether a popped task should run or be
//! requeued. The built-in [`DefaultDispatchGate`] applies backpressure
//! throttling, IO-budget checks, group/module concurrency limits, and
//! token-bucket rate limiting.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock as StdRwLock};

use crate::backpressure::{CompositePressure, ThrottlePolicy};
use crate::resource::ResourceReader;
use crate::store::{StoreError, TaskStore};
use crate::task::TaskRecord;

use super::rate_limit::RateLimits;

/// Boxed future returned by [`DispatchGate`] methods.
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

// ── Admission ─────────────────────────────────────────────────────

/// Result of a [`DispatchGate::admit`] call.
pub enum Admission {
    /// Task should be dispatched.
    Admit,
    /// Task should be requeued (backpressure, concurrency, etc.).
    Deny,
    /// Task was rate-limited. Contains the `Instant` the next token
    /// will be available — caller should set `run_after` accordingly.
    RateLimited(tokio::time::Instant),
}

// ── Gate Context ───────────────────────────────────────────────────

/// Context provided to a [`DispatchGate`] for admission decisions.
///
/// Built by the scheduler each dispatch cycle so gate implementations
/// can query store state and resource snapshots without owning them.
pub struct GateContext<'a> {
    /// The task store — available for queries like running IO totals.
    pub store: &'a TaskStore,
    /// The current resource reader, if monitoring is enabled.
    pub resource_reader: Option<&'a Arc<dyn ResourceReader>>,
    /// Group concurrency limits (if configured).
    pub group_limits: Option<&'a GroupLimits>,
    /// Per-module concurrency caps (module name → cap).
    pub module_caps: &'a StdRwLock<HashMap<String, usize>>,
    /// Per-module live running counts (module name → AtomicUsize).
    pub module_running: &'a HashMap<String, AtomicUsize>,
    /// Set of currently paused group keys.
    pub paused_groups: &'a HashSet<String>,
    /// Per-task-type rate limits.
    pub type_rate_limits: &'a RateLimits,
    /// Per-group rate limits.
    pub group_rate_limits: &'a RateLimits,
    /// When true, the gate skips the group concurrency check. Used by
    /// `dispatch_fair()` pass 1 where group slot budgets are already
    /// enforced by the allocation algorithm.
    pub skip_group_concurrency: bool,
}

// ── Dispatch Gate ──────────────────────────────────────────────────

/// Decides whether a popped task should be dispatched or requeued.
///
/// The scheduler calls [`admit`](DispatchGate::admit) after popping a
/// task from the store but before spawning the executor. Returning
/// `Ok(Admission::Deny)` causes the task to be requeued for a later cycle.
///
/// The default [`DefaultDispatchGate`] applies backpressure throttling,
/// IO-budget checks, group concurrency, module concurrency, and
/// rate-limit checks.
///
/// # Example
///
/// ```ignore
/// use taskmill::scheduler::gate::{Admission, DispatchGate, GateContext};
/// use taskmill::store::StoreError;
/// use taskmill::task::TaskRecord;
///
/// struct AlwaysAdmit;
///
/// impl DispatchGate for AlwaysAdmit {
///     fn admit<'a>(
///         &'a self,
///         _task: &'a TaskRecord,
///         _ctx: &'a GateContext<'a>,
///     ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Admission, StoreError>> + Send + 'a>> {
///         Box::pin(async { Ok(Admission::Admit) })
///     }
/// }
/// ```
pub trait DispatchGate: Send + Sync + 'static {
    /// Check whether `task` should be dispatched given the current context.
    ///
    /// Return `Ok(Admission::Admit)` to dispatch, `Ok(Admission::Deny)` to
    /// requeue, or `Ok(Admission::RateLimited(next))` to defer until the
    /// given instant.
    fn admit<'a>(
        &'a self,
        task: &'a TaskRecord,
        ctx: &'a GateContext<'a>,
    ) -> BoxFuture<'a, Result<Admission, StoreError>>;

    /// Current aggregate pressure (0.0–1.0). Returns 0.0 by default.
    fn pressure<'a>(&'a self) -> BoxFuture<'a, f32> {
        Box::pin(async { 0.0 })
    }

    /// Per-source pressure breakdown for diagnostics. Empty by default.
    fn pressure_breakdown<'a>(&'a self) -> BoxFuture<'a, Vec<(String, f32)>> {
        Box::pin(async { Vec::new() })
    }
}

// ── Default Gate ───────────────────────────────────────────────────

/// Default gate: backpressure, IO budget, concurrency, and rate limiting.
///
/// This is what the scheduler uses unless you provide a custom gate via
/// [`SchedulerBuilder::dispatch_gate`](super::SchedulerBuilder::dispatch_gate).
pub struct DefaultDispatchGate {
    pub(crate) pressure: tokio::sync::Mutex<CompositePressure>,
    pub(crate) policy: ThrottlePolicy,
}

impl DefaultDispatchGate {
    pub fn new(pressure: CompositePressure, policy: ThrottlePolicy) -> Self {
        Self {
            pressure: tokio::sync::Mutex::new(pressure),
            policy,
        }
    }
}

impl DispatchGate for DefaultDispatchGate {
    fn admit<'a>(
        &'a self,
        task: &'a TaskRecord,
        ctx: &'a GateContext<'a>,
    ) -> BoxFuture<'a, Result<Admission, StoreError>> {
        Box::pin(async move {
            // Backpressure check.
            let current_pressure = self.pressure.lock().await.pressure();
            if self.policy.should_throttle(task.priority, current_pressure) {
                tracing::trace!(
                    priority = task.priority.value(),
                    pressure = current_pressure,
                    "task throttled by backpressure — requeuing"
                );
                return Ok(Admission::Deny);
            }

            // IO budget check (disk).
            if !has_io_headroom(task, ctx).await? {
                tracing::trace!(
                    task_type = task.task_type,
                    expected_read = task.expected_io.disk_read,
                    expected_write = task.expected_io.disk_write,
                    "task deferred — disk IO budget exhausted — requeuing"
                );
                return Ok(Admission::Deny);
            }

            // Network IO budget check.
            if !has_net_io_headroom(task, ctx).await? {
                tracing::trace!(
                    task_type = task.task_type,
                    expected_rx = task.expected_io.net_rx,
                    expected_tx = task.expected_io.net_tx,
                    "task deferred — network IO budget exhausted — requeuing"
                );
                return Ok(Admission::Deny);
            }

            // Group pause check.
            if let Some(group_key) = &task.group_key {
                if ctx.paused_groups.contains(group_key) {
                    tracing::trace!(
                        task_type = task.task_type,
                        group = group_key,
                        "task deferred — group paused — requeuing"
                    );
                    return Ok(Admission::Deny);
                }
            }

            // Group concurrency check (skipped in fair dispatch pass 1
            // where the allocation already enforces group slot budgets).
            if !ctx.skip_group_concurrency {
                if let Some(group_key) = &task.group_key {
                    if let Some(limits) = ctx.group_limits {
                        if let Some(limit) = limits.limit_for(group_key) {
                            let running = ctx.store.running_count_for_group(group_key).await?;
                            if running >= limit as i64 {
                                tracing::trace!(
                                    task_type = task.task_type,
                                    group = group_key,
                                    running,
                                    limit,
                                    "task deferred — group concurrency saturated — requeuing"
                                );
                                return Ok(Admission::Deny);
                            }
                        }
                    }
                }
            }

            // Module concurrency check.
            if let Some(module_name) = task.module_name() {
                let cap = ctx.module_caps.read().unwrap().get(module_name).copied();
                if let Some(cap) = cap {
                    let running = ctx
                        .module_running
                        .get(module_name)
                        .map_or(0, |c| c.load(Ordering::Relaxed));
                    if running >= cap {
                        tracing::trace!(
                            task_type = task.task_type,
                            module = module_name,
                            running,
                            cap,
                            "task deferred — module concurrency saturated — requeuing"
                        );
                        return Ok(Admission::Deny);
                    }
                }
            }

            // Rate limit check — task type (acquire-last: only after all
            // free checks pass, so tokens are never wasted on downstream
            // rejections).
            if let Some(Err(next)) = ctx.type_rate_limits.try_acquire(&task.task_type) {
                tracing::trace!(
                    task_type = task.task_type,
                    "task deferred — task-type rate limit"
                );
                return Ok(Admission::RateLimited(next));
            }

            // Rate limit check — group.
            if let Some(group_key) = &task.group_key {
                if let Some(Err(next)) = ctx.group_rate_limits.try_acquire(group_key) {
                    tracing::trace!(
                        task_type = task.task_type,
                        group = group_key,
                        "task deferred — group rate limit"
                    );
                    return Ok(Admission::RateLimited(next));
                }
            }

            Ok(Admission::Admit)
        })
    }

    fn pressure<'a>(&'a self) -> BoxFuture<'a, f32> {
        Box::pin(async { self.pressure.lock().await.pressure() })
    }

    fn pressure_breakdown<'a>(&'a self) -> BoxFuture<'a, Vec<(String, f32)>> {
        Box::pin(async {
            self.pressure
                .lock()
                .await
                .breakdown()
                .into_iter()
                .map(|(name, val)| (name.to_owned(), val))
                .collect()
        })
    }
}

// ── IO Budget ──────────────────────────────────────────────────────

/// Check if there is IO headroom for a task given current running IO
/// and system capacity.
///
/// This is a utility function that custom [`DispatchGate`] implementations
/// can reuse if they want IO-budget awareness alongside their own logic.
pub async fn has_io_headroom(task: &TaskRecord, ctx: &GateContext<'_>) -> Result<bool, StoreError> {
    let Some(reader) = ctx.resource_reader else {
        // No monitor configured — always allow.
        return Ok(true);
    };

    let snapshot = reader.latest();
    // If we have no IO data yet, allow the task.
    if snapshot.io_read_bytes_per_sec == 0.0 && snapshot.io_write_bytes_per_sec == 0.0 {
        return Ok(true);
    }

    let (running_read, running_write) = ctx.store.running_io_totals().await?;

    // Simple heuristic: if running tasks' expected IO already exceeds
    // 80% of observed system throughput (per second × 2s budget window),
    // defer new work.
    let read_capacity = snapshot.io_read_bytes_per_sec * 2.0;
    let write_capacity = snapshot.io_write_bytes_per_sec * 2.0;

    let read_ok = read_capacity == 0.0
        || (running_read + task.expected_io.disk_read) as f64 <= read_capacity * 0.8;
    let write_ok = write_capacity == 0.0
        || (running_write + task.expected_io.disk_write) as f64 <= write_capacity * 0.8;

    Ok(read_ok && write_ok)
}

// ── Network IO Budget ─────────────────────────────────────────────

/// Check if there is network IO headroom for a task given current running
/// network IO and system capacity.
///
/// Parallel to [`has_io_headroom`] but for network throughput.
pub async fn has_net_io_headroom(
    task: &TaskRecord,
    ctx: &GateContext<'_>,
) -> Result<bool, StoreError> {
    // If the task doesn't declare any network IO, always allow.
    if task.expected_io.net_rx == 0 && task.expected_io.net_tx == 0 {
        return Ok(true);
    }

    let Some(reader) = ctx.resource_reader else {
        return Ok(true);
    };

    let snapshot = reader.latest();
    if snapshot.net_rx_bytes_per_sec == 0.0 && snapshot.net_tx_bytes_per_sec == 0.0 {
        return Ok(true);
    }

    let (running_rx, running_tx) = ctx.store.running_net_io_totals().await?;

    let rx_capacity = snapshot.net_rx_bytes_per_sec * 2.0;
    let tx_capacity = snapshot.net_tx_bytes_per_sec * 2.0;

    let rx_ok =
        rx_capacity == 0.0 || (running_rx + task.expected_io.net_rx) as f64 <= rx_capacity * 0.8;
    let tx_ok =
        tx_capacity == 0.0 || (running_tx + task.expected_io.net_tx) as f64 <= tx_capacity * 0.8;

    Ok(rx_ok && tx_ok)
}

// ── Group Limits ──────────────────────────────────────────────────

/// Per-group concurrency limits for task dispatch.
///
/// Groups allow throttling tasks that target the same resource (e.g. an S3
/// endpoint) independently from global concurrency. Each task can optionally
/// carry a `group_key`; the scheduler checks the running count for that group
/// against the configured limit before dispatching.
///
/// A default limit applies to any group without a specific override. Set
/// the default to `0` (the initial value) to disable group limiting for
/// groups without explicit overrides.
pub struct GroupLimits {
    default: AtomicUsize,
    overrides: StdRwLock<HashMap<String, usize>>,
}

impl Default for GroupLimits {
    fn default() -> Self {
        Self::new()
    }
}

impl GroupLimits {
    /// Create a new `GroupLimits` with no default limit and no overrides.
    pub fn new() -> Self {
        Self {
            default: AtomicUsize::new(0),
            overrides: StdRwLock::new(HashMap::new()),
        }
    }

    /// Look up the effective limit for a group.
    ///
    /// Returns the per-group override if set, otherwise the default limit.
    /// Returns `None` if neither is configured (default is 0 = unlimited).
    pub fn limit_for(&self, group: &str) -> Option<usize> {
        // Fast path: check overrides with a read lock.
        if let Some(&limit) = self.overrides.read().unwrap().get(group) {
            return Some(limit);
        }
        let default = self.default.load(Ordering::Relaxed);
        if default > 0 {
            Some(default)
        } else {
            None
        }
    }

    /// Set a concurrency limit for a specific group.
    pub fn set_limit(&self, group: String, limit: usize) {
        self.overrides.write().unwrap().insert(group, limit);
    }

    /// Remove the per-group override, falling back to the default.
    pub fn remove_limit(&self, group: &str) {
        self.overrides.write().unwrap().remove(group);
    }

    /// Set the default limit applied to groups without a specific override.
    ///
    /// `0` means unlimited (no group limiting for unconfigured groups).
    pub fn set_default(&self, limit: usize) {
        self.default.store(limit, Ordering::Relaxed);
    }

    /// Read the current default limit.
    pub fn default_limit(&self) -> usize {
        self.default.load(Ordering::Relaxed)
    }

    /// Returns true if any per-group overrides are configured.
    pub fn has_overrides(&self) -> bool {
        !self.overrides.read().unwrap().is_empty()
    }
}
