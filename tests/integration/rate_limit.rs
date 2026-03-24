//! Integration tests: Rate limiting per task type / group (Plan 043).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use taskmill::{Domain, RateLimit, Scheduler, SchedulerEvent, TaskStore, TaskSubmission};
use tokio_util::sync::CancellationToken;

use super::common::*;

// ═══════════════════════════════════════════════════════════════════
// rate_limit_caps_dispatch_rate
// ═══════════════════════════════════════════════════════════════════

/// Submit many tasks with a rate limit of 5/sec. Only the first 5 should
/// dispatch in the initial burst; more should follow after time advances.
#[tokio::test]
async fn rate_limit_caps_dispatch_rate() {
    let count = Arc::new(AtomicUsize::new(0));
    let executor = CountingExecutor {
        count: count.clone(),
    };

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<FastTask>(executor))
        .max_concurrency(100) // high concurrency — rate limit is the bottleneck
        .rate_limit("test::fast", RateLimit::per_second(5))
        .build()
        .await
        .unwrap();

    // Submit 20 tasks.
    for i in 0..20 {
        sched
            .submit(&TaskSubmission::new("test::fast").key(format!("t-{i}")))
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched2 = sched.clone();
    let token2 = token.clone();
    let handle = tokio::spawn(async move { sched2.run(token2).await });

    // Give the scheduler time to dispatch the initial burst.
    tokio::time::sleep(Duration::from_millis(300)).await;

    let dispatched = count.load(Ordering::SeqCst);
    // Should have dispatched approximately 5 (the burst), not all 20.
    assert!(
        (3..=8).contains(&dispatched),
        "expected ~5 dispatched in initial burst, got {dispatched}"
    );

    // Wait for more tokens to refill and dispatch more.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let dispatched_later = count.load(Ordering::SeqCst);
    assert!(
        dispatched_later > dispatched,
        "expected more tasks after waiting, was {dispatched} now {dispatched_later}"
    );

    token.cancel();
    let _ = handle.await;
}

// ═══════════════════════════════════════════════════════════════════
// group_rate_limit_caps_group
// ═══════════════════════════════════════════════════════════════════

/// Two groups: one rate-limited to 2/sec, one unlimited. The unlimited
/// group dispatches all tasks quickly while the limited group is throttled.
#[tokio::test]
async fn group_rate_limit_caps_group() {
    let count_limited = Arc::new(AtomicUsize::new(0));
    let count_unlimited = Arc::new(AtomicUsize::new(0));
    let count_limited2 = count_limited.clone();
    let count_unlimited2 = count_unlimited.clone();

    // Use a single executor that routes by group key via tags.
    // We'll use two different task types instead for simplicity.
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new()
                .task::<FastTask>(CountingExecutor {
                    count: count_limited.clone(),
                })
                .task::<WorkerTask>(CountingExecutor {
                    count: count_unlimited.clone(),
                }),
        )
        .max_concurrency(100)
        .group_rate_limit("limited-group", RateLimit::per_second(2))
        .build()
        .await
        .unwrap();

    // Submit 10 tasks to the limited group.
    for i in 0..10 {
        sched
            .submit(
                &TaskSubmission::new("test::fast")
                    .key(format!("lim-{i}"))
                    .group("limited-group"),
            )
            .await
            .unwrap();
    }

    // Submit 10 tasks to an unlimited group.
    for i in 0..10 {
        sched
            .submit(
                &TaskSubmission::new("test::worker")
                    .key(format!("unlim-{i}"))
                    .group("free-group"),
            )
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched2 = sched.clone();
    let token2 = token.clone();
    let handle = tokio::spawn(async move { sched2.run(token2).await });

    tokio::time::sleep(Duration::from_millis(500)).await;

    let limited = count_limited2.load(Ordering::SeqCst);
    let unlimited = count_unlimited2.load(Ordering::SeqCst);

    // Unlimited group should have dispatched all 10.
    assert!(
        unlimited >= 8,
        "unlimited group should dispatch all tasks, got {unlimited}"
    );
    // Limited group should have dispatched only ~2 (2/sec burst).
    assert!(
        limited <= 5,
        "limited group should be throttled, got {limited}"
    );

    token.cancel();
    let _ = handle.await;
}

// ═══════════════════════════════════════════════════════════════════
// dual_scope_both_checked
// ═══════════════════════════════════════════════════════════════════

/// Task with both type and group rate limits — rejected if either is
/// exhausted.
#[tokio::test]
async fn dual_scope_both_checked() {
    let count = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new().task::<FastTask>(CountingExecutor {
                count: count.clone(),
            }),
        )
        .max_concurrency(100)
        // Type limit: 10/sec
        .rate_limit("test::fast", RateLimit::per_second(10))
        // Group limit: 3/sec (stricter)
        .group_rate_limit("strict-group", RateLimit::per_second(3))
        .build()
        .await
        .unwrap();

    for i in 0..20 {
        sched
            .submit(
                &TaskSubmission::new("test::fast")
                    .key(format!("dual-{i}"))
                    .group("strict-group"),
            )
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched2 = sched.clone();
    let token2 = token.clone();
    let handle = tokio::spawn(async move { sched2.run(token2).await });

    tokio::time::sleep(Duration::from_millis(400)).await;

    let dispatched = count.load(Ordering::SeqCst);
    // Group limit of 3 is stricter — should cap dispatch to ~3 initial burst.
    assert!(
        dispatched <= 6,
        "stricter group limit should cap dispatch, got {dispatched}"
    );

    token.cancel();
    let _ = handle.await;
}

// ═══════════════════════════════════════════════════════════════════
// runtime_set_rate_limit
// ═══════════════════════════════════════════════════════════════════

/// Call `set_rate_limit()` at runtime and verify it takes effect.
#[tokio::test]
async fn runtime_set_rate_limit() {
    let count = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new().task::<FastTask>(CountingExecutor {
                count: count.clone(),
            }),
        )
        .max_concurrency(100)
        .build()
        .await
        .unwrap();

    // Submit tasks without rate limit — should all dispatch quickly.
    for i in 0..5 {
        sched
            .submit(&TaskSubmission::new("test::fast").key(format!("pre-{i}")))
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched2 = sched.clone();
    let token2 = token.clone();
    let handle = tokio::spawn(async move { sched2.run(token2).await });

    tokio::time::sleep(Duration::from_millis(300)).await;
    let before_limit = count.load(Ordering::SeqCst);
    assert_eq!(before_limit, 5, "all 5 should dispatch without rate limit");

    // Now set a tight rate limit.
    sched.set_rate_limit("test::fast", RateLimit::per_second(2));

    // Submit 10 more tasks.
    for i in 0..10 {
        sched
            .submit(&TaskSubmission::new("test::fast").key(format!("post-{i}")))
            .await
            .unwrap();
    }

    tokio::time::sleep(Duration::from_millis(500)).await;
    let after_limit = count.load(Ordering::SeqCst);
    // Should have dispatched the original 5 + only ~2-3 more (2/sec rate limit).
    assert!(
        after_limit < 15,
        "rate limit should throttle, got {after_limit} (expected < 15)"
    );

    token.cancel();
    let _ = handle.await;
}

// ═══════════════════════════════════════════════════════════════════
// runtime_remove_rate_limit
// ═══════════════════════════════════════════════════════════════════

/// Remove a rate limit at runtime and verify tasks dispatch freely.
#[tokio::test]
async fn runtime_remove_rate_limit() {
    let count = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new().task::<FastTask>(CountingExecutor {
                count: count.clone(),
            }),
        )
        .max_concurrency(100)
        .rate_limit("test::fast", RateLimit::per_second(2))
        .build()
        .await
        .unwrap();

    for i in 0..10 {
        sched
            .submit(&TaskSubmission::new("test::fast").key(format!("rl-{i}")))
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched2 = sched.clone();
    let token2 = token.clone();
    let handle = tokio::spawn(async move { sched2.run(token2).await });

    tokio::time::sleep(Duration::from_millis(400)).await;
    let throttled = count.load(Ordering::SeqCst);
    assert!(
        throttled <= 5,
        "should be throttled before removal, got {throttled}"
    );

    // Remove the rate limit.
    sched.remove_rate_limit("test::fast");

    // Wait for remaining tasks to dispatch freely.
    tokio::time::sleep(Duration::from_millis(800)).await;
    let after_removal = count.load(Ordering::SeqCst);
    assert_eq!(
        after_removal, 10,
        "all tasks should complete after removing rate limit, got {after_removal}"
    );

    token.cancel();
    let _ = handle.await;
}

// ═══════════════════════════════════════════════════════════════════
// rate_limit_with_concurrency
// ═══════════════════════════════════════════════════════════════════

/// Both rate limit and concurrency limit active — stricter one wins.
#[tokio::test]
async fn rate_limit_with_concurrency() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new().task::<SlowTask>(DelayExecutor(Duration::from_millis(200))),
        )
        .max_concurrency(2) // concurrency limit of 2
        .rate_limit("test::slow", RateLimit::per_second(100)) // generous rate limit
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    for i in 0..6 {
        sched
            .submit(&TaskSubmission::new("test::slow").key(format!("conc-{i}")))
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched2 = sched.clone();
    let token2 = token.clone();
    let handle = tokio::spawn(async move { sched2.run(token2).await });

    // Wait for all 6 tasks to complete (2 at a time, 200ms each = ~600ms).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    let mut completed = 0usize;
    while completed < 6 && tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(SchedulerEvent::Completed(_))) => completed += 1,
            _ => continue,
        }
    }
    assert_eq!(completed, 6, "all tasks should complete, got {completed}");

    token.cancel();
    let _ = handle.await;
}

// ═══════════════════════════════════════════════════════════════════
// builder_configures_rate_limits
// ═══════════════════════════════════════════════════════════════════

/// Builder API sets limits visible in snapshot.
#[tokio::test]
async fn builder_configures_rate_limits() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .rate_limit("test::test", RateLimit::per_second(50))
        .group_rate_limit("my-group", RateLimit::per_minute(100).with_burst(20))
        .build()
        .await
        .unwrap();

    let snap = sched.snapshot().await.unwrap();
    assert_eq!(snap.rate_limits.len(), 2);

    let type_limit = snap
        .rate_limits
        .iter()
        .find(|r| r.scope == "type:test::test")
        .expect("type rate limit should be in snapshot");
    assert_eq!(type_limit.permits, 50);
    assert_eq!(type_limit.burst, 50);
    assert_eq!(type_limit.scope_kind, "type");

    let group_limit = snap
        .rate_limits
        .iter()
        .find(|r| r.scope == "group:my-group")
        .expect("group rate limit should be in snapshot");
    assert_eq!(group_limit.permits, 100);
    assert_eq!(group_limit.burst, 20);
    assert_eq!(group_limit.scope_kind, "group");
}

// ═══════════════════════════════════════════════════════════════════
// rate_limited_task_sets_run_after
// ═══════════════════════════════════════════════════════════════════

/// Rate-limited task gets `run_after` set, pushing it out of the peek
/// window so other tasks can dispatch.
#[tokio::test]
async fn rate_limited_task_sets_run_after() {
    let fast_count = Arc::new(AtomicUsize::new(0));
    let worker_count = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new()
                .task::<FastTask>(CountingExecutor {
                    count: fast_count.clone(),
                })
                .task::<WorkerTask>(CountingExecutor {
                    count: worker_count.clone(),
                }),
        )
        .max_concurrency(100)
        // Tight rate limit on FastTask.
        .rate_limit("test::fast", RateLimit::per_second(1).with_burst(1))
        .build()
        .await
        .unwrap();

    // Submit 5 FastTask (rate limited) and 5 WorkerTask (no limit).
    for i in 0..5 {
        sched
            .submit(&TaskSubmission::new("test::fast").key(format!("fast-{i}")))
            .await
            .unwrap();
    }
    for i in 0..5 {
        sched
            .submit(&TaskSubmission::new("test::worker").key(format!("worker-{i}")))
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched2 = sched.clone();
    let token2 = token.clone();
    let handle = tokio::spawn(async move { sched2.run(token2).await });

    tokio::time::sleep(Duration::from_millis(500)).await;

    let fast = fast_count.load(Ordering::SeqCst);
    let worker = worker_count.load(Ordering::SeqCst);

    // WorkerTask should all be complete (no rate limit).
    assert_eq!(worker, 5, "all WorkerTasks should dispatch, got {worker}");
    // FastTask should be limited (1/sec burst).
    assert!(fast <= 2, "FastTask should be rate-limited, got {fast}");

    token.cancel();
    let _ = handle.await;
}

// ═══════════════════════════════════════════════════════════════════
// no_head_of_line_blocking
// ═══════════════════════════════════════════════════════════════════

/// High-priority rate-limited type does not block lower-priority tasks
/// of a different type.
#[tokio::test]
async fn no_head_of_line_blocking() {
    let fast_count = Arc::new(AtomicUsize::new(0));
    let worker_count = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new()
                .task::<FastTask>(CountingExecutor {
                    count: fast_count.clone(),
                })
                .task::<WorkerTask>(CountingExecutor {
                    count: worker_count.clone(),
                }),
        )
        .max_concurrency(100)
        // Very tight rate limit on FastTask (1 per 10 seconds).
        .rate_limit("test::fast", RateLimit::per_second(1).with_burst(1))
        .build()
        .await
        .unwrap();

    // Submit high-priority rate-limited tasks.
    for i in 0..3 {
        sched
            .submit(
                &TaskSubmission::new("test::fast")
                    .key(format!("hp-{i}"))
                    .priority(taskmill::Priority::HIGH),
            )
            .await
            .unwrap();
    }
    // Submit lower-priority non-rate-limited tasks.
    for i in 0..5 {
        sched
            .submit(
                &TaskSubmission::new("test::worker")
                    .key(format!("lp-{i}"))
                    .priority(taskmill::Priority::NORMAL),
            )
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched2 = sched.clone();
    let token2 = token.clone();
    let handle = tokio::spawn(async move { sched2.run(token2).await });

    tokio::time::sleep(Duration::from_millis(600)).await;

    let worker = worker_count.load(Ordering::SeqCst);
    // Lower-priority WorkerTasks should still dispatch despite
    // high-priority FastTasks being rate-limited.
    assert!(
        worker >= 3,
        "worker tasks should dispatch despite rate-limited high-priority tasks, got {worker}"
    );

    token.cancel();
    let _ = handle.await;
}
