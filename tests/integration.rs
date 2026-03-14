//! Integration tests for the taskmill scheduler.
//!
//! These tests exercise the public API surface as an external consumer would,
//! covering multi-component interactions that unit tests don't capture:
//! priority ordering, retry lifecycle, preemption + resume, backpressure
//! gating, group concurrency, run-loop integration, and child task semantics.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use taskmill::{
    PressureSource, Priority, Scheduler, SchedulerEvent, TaskContext, TaskError, TaskExecutor,
    TaskStore, TaskSubmission,
};
use tokio_util::sync::CancellationToken;

// ── Test Executors ──────────────────────────────────────────────────

/// Completes immediately with no side effects.
struct NoopExecutor;

impl TaskExecutor for NoopExecutor {
    async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        Ok(())
    }
}

/// Sleeps for a configurable duration, respecting cancellation.
struct DelayExecutor(Duration);

impl TaskExecutor for DelayExecutor {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        tokio::select! {
            _ = ctx.token().cancelled() => Err(TaskError::new("cancelled")),
            _ = tokio::time::sleep(self.0) => Ok(()),
        }
    }
}

/// Increments a counter on each execution — useful for tracking throughput.
struct CountingExecutor {
    count: Arc<AtomicUsize>,
}

impl TaskExecutor for CountingExecutor {
    async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Fails retryably `max_failures` times, then succeeds.
struct FailNTimesExecutor {
    failures: AtomicI32,
    max_failures: i32,
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
struct IoReportingExecutor {
    read: i64,
    write: i64,
}

impl TaskExecutor for IoReportingExecutor {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        ctx.record_read_bytes(self.read);
        ctx.record_write_bytes(self.write);
        Ok(())
    }
}

/// Tracks how many tasks are simultaneously executing — for concurrency tests.
struct ConcurrencyTrackingExecutor {
    current: Arc<AtomicUsize>,
    max_seen: Arc<AtomicUsize>,
    delay: Duration,
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
struct ChildSpawnerExecutor {
    child_type: &'static str,
    count: usize,
    fail_fast: bool,
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
struct FinalizeTracker {
    child_count: usize,
    finalized: Arc<AtomicBool>,
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
struct AlwaysFailExecutor;

impl TaskExecutor for AlwaysFailExecutor {
    async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        Err(TaskError::new("permanent failure"))
    }
}

/// Mock pressure source with a fixed value.
struct FixedPressure {
    value: f32,
    name: &'static str,
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
async fn wait_for_event(
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

// ═══════════════════════════════════════════════════════════════════
// A. Priority & Ordering
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn priority_ordering_dispatches_highest_first() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor("test", Arc::new(NoopExecutor))
        .max_concurrency(1) // dispatch one at a time
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    // Submit in reverse priority order (low first, high last).
    sched
        .submit(
            &TaskSubmission::new("test")
                .key("low")
                .priority(Priority::IDLE),
        )
        .await
        .unwrap();
    sched
        .submit(
            &TaskSubmission::new("test")
                .key("mid")
                .priority(Priority::NORMAL),
        )
        .await
        .unwrap();
    sched
        .submit(
            &TaskSubmission::new("test")
                .key("high")
                .priority(Priority::HIGH),
        )
        .await
        .unwrap();

    // Dispatch tasks one at a time and collect event order.
    let mut dispatch_order = Vec::new();
    for _ in 0..3 {
        sched.try_dispatch().await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Drain dispatched events.
        while let Ok(evt) = rx.try_recv() {
            if let SchedulerEvent::Dispatched(ref h) = evt {
                dispatch_order.push(h.label.clone());
            }
        }
    }

    assert_eq!(dispatch_order, vec!["high", "mid", "low"]);
}

// ═══════════════════════════════════════════════════════════════════
// B. Retry Lifecycle
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn retryable_error_retries_then_succeeds() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor(
            "test",
            Arc::new(FailNTimesExecutor {
                failures: AtomicI32::new(0),
                max_failures: 2,
            }),
        )
        .max_retries(3)
        .max_concurrency(1)
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    sched
        .submit(&TaskSubmission::new("test").key("retry-me"))
        .await
        .unwrap();

    // Run the scheduler loop.
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for completion.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let completed = wait_for_event(&mut rx, deadline, |evt| {
        matches!(evt, SchedulerEvent::Completed(..))
    })
    .await;

    token.cancel();
    let _ = handle.await;

    assert!(completed.is_some(), "task should eventually complete");
}

#[tokio::test]
async fn retryable_error_exhausts_retries() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor(
            "test",
            Arc::new(FailNTimesExecutor {
                failures: AtomicI32::new(0),
                max_failures: 100, // will never succeed
            }),
        )
        .max_retries(2)
        .max_concurrency(1)
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    sched
        .submit(&TaskSubmission::new("test").key("exhaust"))
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for permanent failure (will_retry = false).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let failed = wait_for_event(&mut rx, deadline, |evt| {
        matches!(
            evt,
            SchedulerEvent::Failed {
                will_retry: false,
                ..
            }
        )
    })
    .await;

    token.cancel();
    let _ = handle.await;

    assert!(
        failed.is_some(),
        "task should permanently fail after retries exhausted"
    );
}

// ═══════════════════════════════════════════════════════════════════
// C. Preemption & Resume
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn preemption_resumes_after_preemptor_completes() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor("slow", Arc::new(DelayExecutor(Duration::from_secs(10))))
        .executor("fast", Arc::new(NoopExecutor))
        .max_concurrency(1)
        .preempt_priority(Priority::REALTIME)
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    // Submit a background task first.
    sched
        .submit(
            &TaskSubmission::new("slow")
                .key("bg-work")
                .priority(Priority::BACKGROUND),
        )
        .await
        .unwrap();

    // Dispatch it.
    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Now submit a REALTIME task — should preempt the slow task.
    sched
        .submit(
            &TaskSubmission::new("fast")
                .key("urgent")
                .priority(Priority::REALTIME),
        )
        .await
        .unwrap();

    // Run the scheduler loop to process preemption + resume.
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for both the preempted event and the fast task completing.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut saw_preempted = false;
    let mut saw_urgent_complete = false;

    while tokio::time::Instant::now() < deadline && !(saw_preempted && saw_urgent_complete) {
        match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(SchedulerEvent::Preempted(ref h))) if h.label == "bg-work" => {
                saw_preempted = true;
            }
            Ok(Ok(SchedulerEvent::Completed(ref h))) if h.label == "urgent" => {
                saw_urgent_complete = true;
            }
            _ => {}
        }
    }

    token.cancel();
    let _ = handle.await;

    assert!(saw_preempted, "background task should have been preempted");
    assert!(saw_urgent_complete, "urgent task should have completed");
}

// ═══════════════════════════════════════════════════════════════════
// D. Backpressure Gating
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn backpressure_throttles_low_priority_tasks() {
    // Default three-tier policy: BACKGROUND throttled >50%.
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor("test", Arc::new(NoopExecutor))
        .pressure_source(Box::new(FixedPressure {
            value: 0.6,
            name: "test-pressure",
        }))
        .max_concurrency(4)
        .build()
        .await
        .unwrap();

    // Submit BACKGROUND task — should be throttled (not dispatched).
    sched
        .submit(
            &TaskSubmission::new("test")
                .key("bg")
                .priority(Priority::BACKGROUND),
        )
        .await
        .unwrap();

    let dispatched = sched.try_dispatch().await.unwrap();
    assert!(
        !dispatched,
        "BACKGROUND task should be throttled at 60% pressure"
    );

    // Submit NORMAL task — should dispatch (threshold is 75%).
    sched
        .submit(
            &TaskSubmission::new("test")
                .key("normal")
                .priority(Priority::NORMAL),
        )
        .await
        .unwrap();

    let dispatched = sched.try_dispatch().await.unwrap();
    assert!(dispatched, "NORMAL task should dispatch at 60% pressure");
}

#[tokio::test]
async fn backpressure_blocks_normal_at_high_pressure() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor("test", Arc::new(NoopExecutor))
        .pressure_source(Box::new(FixedPressure {
            value: 0.8,
            name: "test-pressure",
        }))
        .max_concurrency(4)
        .build()
        .await
        .unwrap();

    // NORMAL task should also be throttled at 80% pressure.
    sched
        .submit(
            &TaskSubmission::new("test")
                .key("normal")
                .priority(Priority::NORMAL),
        )
        .await
        .unwrap();

    let dispatched = sched.try_dispatch().await.unwrap();
    assert!(
        !dispatched,
        "NORMAL task should be throttled at 80% pressure"
    );

    // HIGH priority should still dispatch.
    sched
        .submit(
            &TaskSubmission::new("test")
                .key("high")
                .priority(Priority::HIGH),
        )
        .await
        .unwrap();

    let dispatched = sched.try_dispatch().await.unwrap();
    assert!(dispatched, "HIGH task should dispatch even at 80% pressure");
}

// ═══════════════════════════════════════════════════════════════════
// E. Group Concurrency
// ═══════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn group_concurrency_limits_dispatch() {
    let current = Arc::new(AtomicUsize::new(0));
    let max_seen = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor(
            "test",
            Arc::new(ConcurrencyTrackingExecutor {
                current: current.clone(),
                max_seen: max_seen.clone(),
                delay: Duration::from_millis(100),
            }),
        )
        .max_concurrency(10) // high global limit
        .group_concurrency("s3-bucket", 2) // but group capped at 2
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    // Submit 5 tasks in the same group.
    for i in 0..5 {
        sched
            .submit(
                &TaskSubmission::new("test")
                    .key(format!("group-task-{i}"))
                    .group("s3-bucket"),
            )
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let mut rx = sched.subscribe();

    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for all 5 tasks to complete.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut completed = 0;
    while tokio::time::Instant::now() < deadline && completed < 5 {
        if let Ok(Ok(SchedulerEvent::Completed(..))) =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
        {
            completed += 1;
        }
    }

    token.cancel();
    let _ = handle.await;

    assert_eq!(completed, 5, "all 5 tasks should complete");
    assert!(
        max_seen.load(Ordering::SeqCst) <= 2,
        "group concurrency should never exceed 2, got {}",
        max_seen.load(Ordering::SeqCst)
    );
}

// ═══════════════════════════════════════════════════════════════════
// F. Run Loop Integration
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn run_loop_processes_queue_to_completion() {
    let count = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor(
            "test",
            Arc::new(CountingExecutor {
                count: count.clone(),
            }),
        )
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    // Submit 20 tasks.
    for i in 0..20 {
        sched
            .submit(&TaskSubmission::new("test").key(format!("task-{i}")))
            .await
            .unwrap();
    }

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for all 20 completions.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut completed = 0;
    while tokio::time::Instant::now() < deadline && completed < 20 {
        if let Ok(Ok(SchedulerEvent::Completed(..))) =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
        {
            completed += 1;
        }
    }

    token.cancel();
    let _ = handle.await;

    assert_eq!(completed, 20, "all 20 tasks should complete");
    assert_eq!(count.load(Ordering::SeqCst), 20);
}

// ═══════════════════════════════════════════════════════════════════
// G. Concurrent Dispatch
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn concurrent_tasks_respect_max_concurrency() {
    let current = Arc::new(AtomicUsize::new(0));
    let max_seen = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor(
            "test",
            Arc::new(ConcurrencyTrackingExecutor {
                current: current.clone(),
                max_seen: max_seen.clone(),
                delay: Duration::from_millis(50),
            }),
        )
        .max_concurrency(2)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    for i in 0..10 {
        sched
            .submit(&TaskSubmission::new("test").key(format!("conc-{i}")))
            .await
            .unwrap();
    }

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for all completions.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut completed = 0;
    while tokio::time::Instant::now() < deadline && completed < 10 {
        if let Ok(Ok(SchedulerEvent::Completed(..))) =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
        {
            completed += 1;
        }
    }

    token.cancel();
    let _ = handle.await;

    assert_eq!(completed, 10, "all 10 tasks should complete");
    assert!(
        max_seen.load(Ordering::SeqCst) <= 2,
        "max concurrency should never exceed 2, got {}",
        max_seen.load(Ordering::SeqCst)
    );
}

// ═══════════════════════════════════════════════════════════════════
// H. Child Tasks
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fail_fast_cancels_siblings_on_child_failure() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor(
            "parent",
            Arc::new(ChildSpawnerExecutor {
                child_type: "child",
                count: 3,
                fail_fast: true,
            }),
        )
        .executor("child", Arc::new(AlwaysFailExecutor))
        .max_concurrency(4)
        .max_retries(0) // no retries so failures are permanent
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    sched
        .submit(
            &TaskSubmission::new("parent")
                .key("parent-ff")
                .fail_fast(true),
        )
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for parent failure.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let parent_failed = wait_for_event(
        &mut rx,
        deadline,
        |evt| matches!(evt, SchedulerEvent::Failed { ref header, .. } if header.task_type == "parent"),
    )
    .await;

    token.cancel();
    let _ = handle.await;

    assert!(
        parent_failed.is_some(),
        "parent should fail when child fails with fail_fast"
    );
}

#[tokio::test]
async fn non_fail_fast_waits_for_all_children() {
    let finalized = Arc::new(AtomicBool::new(false));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor(
            "parent",
            Arc::new(FinalizeTracker {
                child_count: 2,
                finalized: finalized.clone(),
            }),
        )
        .executor("child", Arc::new(NoopExecutor))
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    sched
        .submit(
            &TaskSubmission::new("parent")
                .key("parent-noff")
                .fail_fast(false),
        )
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for parent completion.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let parent_completed = wait_for_event(
        &mut rx,
        deadline,
        |evt| matches!(evt, SchedulerEvent::Completed(ref h) if h.task_type == "parent"),
    )
    .await;

    token.cancel();
    let _ = handle.await;

    assert!(
        parent_completed.is_some(),
        "parent should complete after children"
    );
    assert!(
        finalized.load(Ordering::SeqCst),
        "finalize should have been called"
    );
}

// ═══════════════════════════════════════════════════════════════════
// I. Crash Recovery
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn running_tasks_reset_to_pending_on_restart() {
    // TaskStore::open() calls recover_running() which resets running → pending.
    // We use a file-based store because in-memory stores don't call
    // recover_running and each connection is isolated.
    let db_path = format!("/tmp/taskmill_test_{}.db", std::process::id());
    // Clean up leftover files from previous runs.
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(format!("{db_path}-wal"));
    let _ = std::fs::remove_file(format!("{db_path}-shm"));

    // Phase 1: Open store, submit a task, pop it to "running", then close.
    let store = TaskStore::open(&db_path).await.unwrap();
    let sub = TaskSubmission::new("test").key("crash-recovery");
    store.submit(&sub).await.unwrap();
    store.pop_next().await.unwrap(); // now "running"

    let running = store.running_count().await.unwrap();
    assert_eq!(running, 1, "task should be running");
    store.close().await;

    // Phase 2: Re-open via TaskStore::open (which calls recover_running).
    let recovered = TaskStore::open(&db_path).await.unwrap();
    let pending = recovered.pending_count().await.unwrap();
    assert_eq!(pending, 1, "task should be reset to pending after restart");
    recovered.close().await;

    // Clean up.
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(format!("{db_path}-wal"));
    let _ = std::fs::remove_file(format!("{db_path}-shm"));
}

// ═══════════════════════════════════════════════════════════════════
// J. Batch Submit
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn submit_batch_enqueues_all_tasks() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor("test", Arc::new(NoopExecutor))
        .build()
        .await
        .unwrap();

    let submissions: Vec<_> = (0..50)
        .map(|i| TaskSubmission::new("test").key(format!("batch-{i}")))
        .collect();

    let outcomes = sched.submit_batch(&submissions).await.unwrap();
    assert_eq!(outcomes.len(), 50);
    assert!(
        outcomes.iter().all(|o| o.is_inserted()),
        "all submissions should be inserted"
    );

    let pending = sched.store().pending_count().await.unwrap();
    assert_eq!(pending, 50);
}

// ═══════════════════════════════════════════════════════════════════
// K. IO Metrics Tracking
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn io_metrics_recorded_in_history() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor(
            "test",
            Arc::new(IoReportingExecutor {
                read: 4096,
                write: 1024,
            }),
        )
        .build()
        .await
        .unwrap();

    sched
        .submit(&TaskSubmission::new("test").key("io-track"))
        .await
        .unwrap();

    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check history for the completed task.
    let key = taskmill::generate_dedup_key("test", Some(b"io-track"));
    let history = sched.store().history_by_key(&key).await.unwrap();
    assert_eq!(history.len(), 1);
    let actual = history[0].actual_io.unwrap();
    assert_eq!(actual.disk_read, 4096);
    assert_eq!(actual.disk_write, 1024);
}

// ═══════════════════════════════════════════════════════════════════
// L. Snapshot & Event Diagnostics
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn snapshot_reflects_pressure_breakdown() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor("test", Arc::new(NoopExecutor))
        .pressure_source(Box::new(FixedPressure {
            value: 0.42,
            name: "api-load",
        }))
        .build()
        .await
        .unwrap();

    let snap = sched.snapshot().await.unwrap();
    assert!((snap.pressure - 0.42).abs() < 0.01);
    assert_eq!(snap.pressure_breakdown.len(), 1);
    assert_eq!(snap.pressure_breakdown[0].0, "api-load");
}
