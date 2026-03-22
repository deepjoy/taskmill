//! Shared test executors, pressure sources, and helpers for integration tests.

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use std::marker::PhantomData;

use serde::{Deserialize, Serialize};
use taskmill::{
    DomainKey, DomainTaskContext, PressureSource, SchedulerEvent, TaskError, TypedExecutor,
    TypedTask,
};

// ── Domain Keys ────────────────────────────────────────────────────

pub struct TestDomain;
impl DomainKey for TestDomain {
    const NAME: &'static str = "test";
}

pub struct MediaDomain;
impl DomainKey for MediaDomain {
    const NAME: &'static str = "media";
}

pub struct SyncDomain;
impl DomainKey for SyncDomain {
    const NAME: &'static str = "sync";
}

pub struct AnalyticsDomain;
impl DomainKey for AnalyticsDomain {
    const NAME: &'static str = "analytics";
}

pub struct DomainA;
impl DomainKey for DomainA {
    const NAME: &'static str = "a";
}

pub struct DomainB;
impl DomainKey for DomainB {
    const NAME: &'static str = "b";
}

pub struct AlphaDomain;
impl DomainKey for AlphaDomain {
    const NAME: &'static str = "alpha";
}

pub struct BetaDomain;
impl DomainKey for BetaDomain {
    const NAME: &'static str = "beta";
}

pub struct GammaDomain;
impl DomainKey for GammaDomain {
    const NAME: &'static str = "gamma";
}

// ── Helper macro for defining no-payload TypedTask types ───────────

/// Define a zero-payload [`TypedTask`] type in one line.
///
/// ```ignore
/// define_task!(TestTask, TestDomain, "test");
/// ```
macro_rules! define_task {
    ($name:ident, $domain:ty, $task_type:expr) => {
        #[derive(Debug, Default, Clone, Serialize, Deserialize)]
        pub struct $name;
        impl TypedTask for $name {
            type Domain = $domain;
            const TASK_TYPE: &'static str = $task_type;
        }
    };
}

pub(crate) use define_task;

// ── Shared TypedTask types ─────────────────────────────────────────

// TestDomain tasks
define_task!(TestTask, TestDomain, "test");
define_task!(SlowTask, TestDomain, "slow");
define_task!(FastTask, TestDomain, "fast");
define_task!(ParentTask, TestDomain, "parent");
define_task!(ChildTask, TestDomain, "child");
define_task!(WorkerTask, TestDomain, "worker");
define_task!(SpawnerTask, TestDomain, "spawner");
define_task!(ProbeTask, TestDomain, "probe");
define_task!(StepTask, TestDomain, "step");

// MediaDomain tasks
define_task!(ThumbTask, MediaDomain, "thumb");
define_task!(TranscodeTask, MediaDomain, "transcode");
define_task!(MediaWorkTask, MediaDomain, "work");
define_task!(MediaLeaderTask, MediaDomain, "leader");
define_task!(MediaFollowerTask, MediaDomain, "follower");
define_task!(MediaParentTask, MediaDomain, "parent");
define_task!(MediaLayeredTask, MediaDomain, "layered");

// SyncDomain tasks
define_task!(PushTask, SyncDomain, "push");
define_task!(SyncWorkTask, SyncDomain, "work");

// AnalyticsDomain tasks
define_task!(AnalyticsWorkTask, AnalyticsDomain, "work");

// Multi-domain "work" tasks
define_task!(AlphaWorkTask, AlphaDomain, "work");
define_task!(BetaWorkTask, BetaDomain, "work");
define_task!(GammaWorkTask, GammaDomain, "work");

// DomainA / DomainB tasks
define_task!(TriggerTask, DomainA, "trigger");
define_task!(DomainATask, DomainA, "task");
define_task!(DomainBTask, DomainB, "task");

// ── Test Executors ──────────────────────────────────────────────────

/// Completes immediately with no side effects.
pub struct NoopExecutor;

impl<T: TypedTask> TypedExecutor<T> for NoopExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: T,
        _ctx: DomainTaskContext<'a, T::Domain>,
    ) -> Result<(), TaskError> {
        Ok(())
    }
}

/// Sleeps for a configurable duration, respecting cancellation.
pub struct DelayExecutor(pub Duration);

impl<T: TypedTask> TypedExecutor<T> for DelayExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: T,
        ctx: DomainTaskContext<'a, T::Domain>,
    ) -> Result<(), TaskError> {
        tokio::select! {
            _ = ctx.token().cancelled() => Err(TaskError::new("cancelled")),
            _ = tokio::time::sleep(self.0) => Ok(()),
        }
    }
}

/// Increments a counter on each execution — useful for tracking throughput.
pub struct CountingExecutor {
    pub count: Arc<AtomicUsize>,
}

impl<T: TypedTask> TypedExecutor<T> for CountingExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: T,
        _ctx: DomainTaskContext<'a, T::Domain>,
    ) -> Result<(), TaskError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Fails retryably `max_failures` times, then succeeds.
pub struct FailNTimesExecutor {
    pub failures: AtomicI32,
    pub max_failures: i32,
}

impl<T: TypedTask> TypedExecutor<T> for FailNTimesExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: T,
        _ctx: DomainTaskContext<'a, T::Domain>,
    ) -> Result<(), TaskError> {
        let count = self.failures.fetch_add(1, Ordering::SeqCst);
        if count < self.max_failures {
            Err(TaskError::retryable("transient failure"))
        } else {
            Ok(())
        }
    }
}

/// Records IO bytes via DomainTaskContext.
pub struct IoReportingExecutor {
    pub read: i64,
    pub write: i64,
}

impl<T: TypedTask> TypedExecutor<T> for IoReportingExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: T,
        ctx: DomainTaskContext<'a, T::Domain>,
    ) -> Result<(), TaskError> {
        ctx.record_read_bytes(self.read);
        ctx.record_write_bytes(self.write);
        Ok(())
    }
}

/// Tracks how many tasks are simultaneously executing — for concurrency tests.
pub struct ConcurrencyTrackingExecutor {
    pub current: Arc<AtomicUsize>,
    pub max_seen: Arc<AtomicUsize>,
    pub delay: Duration,
}

impl<T: TypedTask> TypedExecutor<T> for ConcurrencyTrackingExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: T,
        ctx: DomainTaskContext<'a, T::Domain>,
    ) -> Result<(), TaskError> {
        let prev = self.current.fetch_add(1, Ordering::SeqCst);
        self.max_seen.fetch_max(prev + 1, Ordering::SeqCst);
        tokio::select! {
            _ = ctx.token().cancelled() => {},
            _ = tokio::time::sleep(self.delay) => {},
        }
        self.current.fetch_sub(1, Ordering::SeqCst);
        Ok(())
    }
}

/// An executor that spawns N child tasks of type `C` in the same domain.
pub struct ChildSpawnerExecutor<C> {
    pub count: usize,
    pub _child: PhantomData<C>,
}

impl<C> ChildSpawnerExecutor<C> {
    pub fn new(count: usize) -> Self {
        Self {
            count,
            _child: PhantomData,
        }
    }
}

impl<T, C> TypedExecutor<T> for ChildSpawnerExecutor<C>
where
    T: TypedTask,
    C: TypedTask<Domain = T::Domain> + Default + Send + Sync + 'static,
{
    async fn execute<'a>(
        &'a self,
        _payload: T,
        ctx: DomainTaskContext<'a, T::Domain>,
    ) -> Result<(), TaskError> {
        for i in 0..self.count {
            ctx.spawn_child_with(C::default())
                .key(format!("child-{i}"))
                .priority(ctx.record().priority)
                .await
                .map_err(|e| TaskError::new(e.to_string()))?;
        }
        Ok(())
    }
}

/// Tracks whether finalize was called. Generic over child type `C`.
pub struct FinalizeTracker<C> {
    pub child_count: usize,
    pub finalized: Arc<AtomicBool>,
    pub _child: PhantomData<C>,
}

impl<C> FinalizeTracker<C> {
    pub fn new(child_count: usize, finalized: Arc<AtomicBool>) -> Self {
        Self {
            child_count,
            finalized,
            _child: PhantomData,
        }
    }
}

impl<T, C> TypedExecutor<T> for FinalizeTracker<C>
where
    T: TypedTask,
    C: TypedTask<Domain = T::Domain> + Default + Send + Sync + 'static,
{
    async fn execute<'a>(
        &'a self,
        _payload: T,
        ctx: DomainTaskContext<'a, T::Domain>,
    ) -> Result<(), TaskError> {
        for i in 0..self.child_count {
            ctx.spawn_child_with(C::default())
                .key(format!("ft-child-{i}"))
                .priority(ctx.record().priority)
                .await
                .map_err(|e| TaskError::new(e.to_string()))?;
        }
        Ok(())
    }

    async fn finalize<'a>(
        &'a self,
        _payload: T,
        _memo: (),
        _ctx: DomainTaskContext<'a, T::Domain>,
    ) -> Result<(), TaskError> {
        self.finalized.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Fails unconditionally with a non-retryable error.
pub struct AlwaysFailExecutor;

impl<T: TypedTask> TypedExecutor<T> for AlwaysFailExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: T,
        _ctx: DomainTaskContext<'a, T::Domain>,
    ) -> Result<(), TaskError> {
        Err(TaskError::new("permanent failure"))
    }
}

/// Mock pressure source with a fixed value.
pub struct FixedPressure {
    pub value: f32,
    pub name: &'static str,
}

impl PressureSource for FixedPressure {
    fn pressure(&self) -> f32 {
        self.value
    }
    fn name(&self) -> &str {
        self.name
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Wait for a specific event type with a deadline.
pub async fn wait_for_event(
    rx: &mut tokio::sync::broadcast::Receiver<SchedulerEvent>,
    deadline: tokio::time::Instant,
    mut predicate: impl FnMut(&SchedulerEvent) -> bool,
) -> Option<SchedulerEvent> {
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(evt)) if predicate(&evt) => return Some(evt),
            Ok(Ok(_)) => continue,
            _ => continue,
        }
    }
    None
}
