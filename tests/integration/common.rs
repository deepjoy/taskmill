//! Shared test executors, pressure sources, and helpers for integration tests.

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use taskmill::{
    PressureSource, SchedulerEvent, TaskContext, TaskError, TaskExecutor, TaskSubmission,
};

// ── Test Executors ──────────────────────────────────────────────────

/// Completes immediately with no side effects.
pub struct NoopExecutor;

impl TaskExecutor for NoopExecutor {
    async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        Ok(())
    }
}

/// Sleeps for a configurable duration, respecting cancellation.
pub struct DelayExecutor(pub Duration);

impl TaskExecutor for DelayExecutor {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
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

impl TaskExecutor for CountingExecutor {
    async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Fails retryably `max_failures` times, then succeeds.
pub struct FailNTimesExecutor {
    pub failures: AtomicI32,
    pub max_failures: i32,
}

impl TaskExecutor for FailNTimesExecutor {
    async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        let count = self.failures.fetch_add(1, Ordering::SeqCst);
        if count < self.max_failures {
            Err(TaskError::retryable("transient failure"))
        } else {
            Ok(())
        }
    }
}

/// Records IO bytes via TaskContext.
pub struct IoReportingExecutor {
    pub read: i64,
    pub write: i64,
}

impl TaskExecutor for IoReportingExecutor {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
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

impl TaskExecutor for ConcurrencyTrackingExecutor {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
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

/// An executor that spawns N child tasks.
pub struct ChildSpawnerExecutor {
    pub child_type: &'static str,
    pub count: usize,
    pub fail_fast: bool,
}

impl TaskExecutor for ChildSpawnerExecutor {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        for i in 0..self.count {
            let sub = TaskSubmission::new(self.child_type)
                .key(format!("child-{i}"))
                .priority(ctx.record().priority)
                .fail_fast(self.fail_fast);
            ctx.spawn_child(sub).await?;
        }
        Ok(())
    }
}

/// Tracks whether finalize was called.
pub struct FinalizeTracker {
    pub child_count: usize,
    pub finalized: Arc<AtomicBool>,
}

impl TaskExecutor for FinalizeTracker {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        for i in 0..self.child_count {
            let sub = TaskSubmission::new("child")
                .key(format!("ft-child-{i}"))
                .priority(ctx.record().priority);
            ctx.spawn_child(sub).await?;
        }
        Ok(())
    }

    async fn finalize<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        self.finalized.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Fails unconditionally with a non-retryable error.
pub struct AlwaysFailExecutor;

impl TaskExecutor for AlwaysFailExecutor {
    async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
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
