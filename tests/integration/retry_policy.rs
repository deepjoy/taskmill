//! Integration tests: Phase 6 — Dispatch Loop / Adaptive Retry Integration

use std::sync::Arc;
use std::time::Duration;

use taskmill::{
    Module, Scheduler, SchedulerEvent, TaskContext, TaskError, TaskExecutor, TaskStore,
    TaskSubmission,
};
use tokio_util::sync::CancellationToken;

// ── Local Executors ──────────────────────────────────────────────────

/// Always fails with a retryable error.
struct AlwaysRetryableExecutor;

impl TaskExecutor for AlwaysRetryableExecutor {
    async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        Err(TaskError::retryable("transient"))
    }
}

/// Fails with a retryable error and requests a specific retry delay.
struct RetryAfterExecutor(Duration);

impl TaskExecutor for RetryAfterExecutor {
    async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        Err(TaskError::retryable("rate limited").retry_after(self.0))
    }
}

// ═══════════════════════════════════════════════════════════════════
// Phase 6: Dispatch Loop — Adaptive Retry Integration
// ═══════════════════════════════════════════════════════════════════

/// 6.5: Per-type retry policy overrides global default.
///
/// Type A has a per-type policy with max_retries=5. Type B uses the global
/// default (max_retries=3). Both fail retryably. A should exhaust 5 retries,
/// B should exhaust 3 retries.
#[tokio::test]
async fn per_type_retry_policy_overrides_global_default() {
    use taskmill::{BackoffStrategy, RetryPolicy};

    let policy_a = RetryPolicy {
        strategy: BackoffStrategy::Constant {
            delay: Duration::ZERO,
        },
        max_retries: 5,
    };

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .module(
            Module::new("test")
                .executor_with_retry_policy("type-a", Arc::new(AlwaysRetryableExecutor), policy_a)
                .executor("type-b", Arc::new(AlwaysRetryableExecutor)),
        )
        .max_retries(3)
        .max_concurrency(2)
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let handle = tokio::spawn({
        let s = sched.clone();
        let t = token.clone();
        async move { s.run(t).await }
    });

    sched
        .submit(&TaskSubmission::new("test::type-a").key("a1"))
        .await
        .unwrap();
    sched
        .submit(&TaskSubmission::new("test::type-b").key("b1"))
        .await
        .unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut dead_a = false;
    let mut dead_b = false;
    let mut a_retry_count = 0i32;
    let mut b_retry_count = 0i32;

    while tokio::time::Instant::now() < deadline && !(dead_a && dead_b) {
        match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(SchedulerEvent::DeadLettered {
                header,
                retry_count,
                ..
            })) => {
                if header.task_type == "test::type-a" {
                    dead_a = true;
                    a_retry_count = retry_count;
                } else if header.task_type == "test::type-b" {
                    dead_b = true;
                    b_retry_count = retry_count;
                }
            }
            _ => continue,
        }
    }

    token.cancel();
    let _ = handle.await;

    assert!(dead_a, "type-a should be dead-lettered");
    assert!(dead_b, "type-b should be dead-lettered");
    // The DeadLettered event reports task.retry_count + 1 where task.retry_count
    // is the value when the task was popped for its final (failing) attempt.
    // max_retries=5: retries at counts 0..4, dead-letters when popped at count=5.
    // Event: 5 + 1 = 6.
    assert_eq!(
        a_retry_count, 6,
        "type-a: 5 retries + final attempt = retry_count 6"
    );
    // max_retries=3: retries at counts 0..2, dead-letters when popped at count=3.
    // Event: 3 + 1 = 4.
    assert_eq!(
        b_retry_count, 4,
        "type-b: 3 retries + final attempt = retry_count 4"
    );
}

/// 6.6: Exponential backoff delays task re-dispatch.
///
/// A task with exponential backoff (initial=200ms, multiplier=2) should not be
/// re-dispatched until the delay elapses. We verify that the gaps between
/// dispatches grow according to the backoff schedule.
#[tokio::test]
async fn exponential_backoff_delays_redispatch() {
    use taskmill::{BackoffStrategy, RetryPolicy};

    let policy = RetryPolicy {
        strategy: BackoffStrategy::Exponential {
            initial: Duration::from_millis(200),
            max: Duration::from_secs(10),
            multiplier: 2.0,
        },
        max_retries: 3,
    };

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .module(Module::new("test").executor_with_retry_policy(
            "backoff-test",
            Arc::new(AlwaysRetryableExecutor),
            policy,
        ))
        .max_concurrency(1)
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let handle = tokio::spawn({
        let s = sched.clone();
        let t = token.clone();
        async move { s.run(t).await }
    });

    sched
        .submit(&TaskSubmission::new("test::backoff-test").key("bk1"))
        .await
        .unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut dispatch_times: Vec<tokio::time::Instant> = Vec::new();
    let mut done = false;

    while tokio::time::Instant::now() < deadline && !done {
        match tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
            Ok(Ok(SchedulerEvent::Dispatched(_))) => {
                dispatch_times.push(tokio::time::Instant::now());
            }
            Ok(Ok(SchedulerEvent::DeadLettered { .. })) => {
                done = true;
            }
            _ => continue,
        }
    }

    token.cancel();
    let _ = handle.await;

    assert!(done, "task should eventually dead-letter");
    // 4 dispatches: initial + 3 retries.
    assert!(
        dispatch_times.len() >= 3,
        "expected at least 3 dispatches, got {}",
        dispatch_times.len()
    );

    // Gap between dispatch 1→2 should be ≥150ms (backoff=200ms, allow some slack).
    if dispatch_times.len() >= 2 {
        let gap = dispatch_times[1] - dispatch_times[0];
        assert!(
            gap >= Duration::from_millis(150),
            "first retry gap should be >=150ms (backoff 200ms), got {:?}",
            gap
        );
    }
    // Gap between dispatch 2→3 should be ≥300ms (backoff=400ms=200*2^1).
    if dispatch_times.len() >= 3 {
        let gap = dispatch_times[2] - dispatch_times[1];
        assert!(
            gap >= Duration::from_millis(300),
            "second retry gap should be >=300ms (backoff 400ms), got {:?}",
            gap
        );
    }
}

/// 6.7: `SchedulerEvent::Failed` includes correct `retry_after` duration.
#[tokio::test]
async fn failed_event_includes_retry_after_duration() {
    use taskmill::{BackoffStrategy, RetryPolicy};

    let policy = RetryPolicy {
        strategy: BackoffStrategy::Constant {
            delay: Duration::from_secs(5),
        },
        max_retries: 2,
    };

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .module(Module::new("test").executor_with_retry_policy(
            "retry-event",
            Arc::new(AlwaysRetryableExecutor),
            policy,
        ))
        .max_concurrency(1)
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let handle = tokio::spawn({
        let s = sched.clone();
        let t = token.clone();
        async move { s.run(t).await }
    });

    sched
        .submit(&TaskSubmission::new("test::retry-event").key("re1"))
        .await
        .unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut found_retry_after = None;

    while tokio::time::Instant::now() < deadline && found_retry_after.is_none() {
        match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(SchedulerEvent::Failed {
                will_retry: true,
                retry_after,
                ..
            })) => {
                found_retry_after = Some(retry_after);
            }
            _ => continue,
        }
    }

    token.cancel();
    let _ = handle.await;

    let retry_after =
        found_retry_after.expect("should receive a Failed event with will_retry=true");
    let delay = retry_after.expect("retry_after should be Some for constant 5s backoff");
    assert_eq!(delay, Duration::from_secs(5));
}

/// 6.7b: Executor `retry_after` override appears in the Failed event.
#[tokio::test]
async fn failed_event_includes_executor_retry_after_override() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .module(Module::new("test").executor(
            "retry-override",
            Arc::new(RetryAfterExecutor(Duration::from_secs(42))),
        ))
        .max_retries(3)
        .max_concurrency(1)
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let handle = tokio::spawn({
        let s = sched.clone();
        let t = token.clone();
        async move { s.run(t).await }
    });

    sched
        .submit(&TaskSubmission::new("test::retry-override").key("ro1"))
        .await
        .unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut found_retry_after = None;

    while tokio::time::Instant::now() < deadline && found_retry_after.is_none() {
        match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(SchedulerEvent::Failed {
                will_retry: true,
                retry_after,
                ..
            })) => {
                found_retry_after = Some(retry_after);
            }
            _ => continue,
        }
    }

    token.cancel();
    let _ = handle.await;

    let retry_after =
        found_retry_after.expect("should receive a Failed event with will_retry=true");
    let delay = retry_after.expect("retry_after should be Some with executor override");
    assert_eq!(delay, Duration::from_secs(42));
}

/// 6.8: Backward compat — tasks with NULL `max_retries` use global default.
///
/// Tasks submitted without a per-type policy get NULL max_retries in the DB.
/// The dispatch loop should fall back to the global `SchedulerConfig::max_retries`.
#[tokio::test]
async fn null_max_retries_uses_global_default() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .module(Module::new("test").executor("legacy", Arc::new(AlwaysRetryableExecutor)))
        .max_retries(2)
        .max_concurrency(1)
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let handle = tokio::spawn({
        let s = sched.clone();
        let t = token.clone();
        async move { s.run(t).await }
    });

    sched
        .submit(&TaskSubmission::new("test::legacy").key("leg1"))
        .await
        .unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut dead_letter_retry_count = None;

    while tokio::time::Instant::now() < deadline && dead_letter_retry_count.is_none() {
        match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(SchedulerEvent::DeadLettered { retry_count, .. })) => {
                dead_letter_retry_count = Some(retry_count);
            }
            _ => continue,
        }
    }

    token.cancel();
    let _ = handle.await;

    let count = dead_letter_retry_count.expect("task should be dead-lettered");
    // max_retries=2: retries at counts 0,1, dead-letters at count=2.
    // Event: 2 + 1 = 3.
    assert_eq!(
        count, 3,
        "dead-letter should report retry_count=3 (2 retries + final attempt)"
    );
}
