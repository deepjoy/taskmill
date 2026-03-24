//! Integration tests for the observability metrics system.

use std::sync::atomic::AtomicI32;
use std::time::Duration;

use taskmill::{Domain, Priority, Scheduler, TaskSubmission};
use tokio_util::sync::CancellationToken;

use super::common::*;

/// Helper: start the scheduler run loop and return the token + join handle.
fn start_scheduler(scheduler: &Scheduler) -> (CancellationToken, tokio::task::JoinHandle<()>) {
    let token = CancellationToken::new();
    let sched = scheduler.clone();
    let t = token.clone();
    let handle = tokio::spawn(async move { sched.run(t).await });
    (token, handle)
}

// ── A. MetricsSnapshot — basic counter lifecycle ──────────────────────

#[tokio::test]
async fn metrics_snapshot_submit_dispatch_complete() {
    let scheduler = Scheduler::builder()
        .store(taskmill::TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new()
                .task::<TestTask>(NoopExecutor)
                .task::<SlowTask>(NoopExecutor),
        )
        .max_concurrency(4)
        .build()
        .await
        .unwrap();

    let (token, run_handle) = start_scheduler(&scheduler);

    let handle = scheduler.domain::<TestDomain>();
    handle.submit(TestTask).await.unwrap();
    handle.submit(SlowTask).await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let snap = scheduler.metrics_snapshot().await;
    assert_eq!(snap.submitted, 2, "should have 2 submitted");
    assert!(snap.dispatched >= 2, "should have dispatched at least 2");
    assert!(snap.completed >= 2, "should have completed at least 2");
    assert_eq!(snap.failed, 0, "no failures");
    assert_eq!(snap.dead_lettered, 0);

    token.cancel();
    let _ = run_handle.await;
}

// ── B. MetricsSnapshot — failure and retry counters ──────────────────

#[tokio::test]
async fn metrics_snapshot_failure_and_retry_counters() {
    let scheduler = Scheduler::builder()
        .store(taskmill::TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new().task::<TestTask>(FailNTimesExecutor {
                failures: AtomicI32::new(0),
                max_failures: 2,
            }),
        )
        .max_concurrency(1)
        .max_retries(3)
        .build()
        .await
        .unwrap();

    let (token, run_handle) = start_scheduler(&scheduler);

    scheduler
        .domain::<TestDomain>()
        .submit(TestTask)
        .await
        .unwrap();

    // Wait for retries and eventual success.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let snap = scheduler.metrics_snapshot().await;
    assert_eq!(snap.submitted, 1);
    assert!(snap.dispatched >= 1);
    assert!(
        snap.failed >= 2,
        "should have at least 2 failures (retryable): got {}",
        snap.failed
    );
    assert!(
        snap.failed_retryable >= 2,
        "retryable failures should be >= 2"
    );
    assert!(
        snap.retried >= 2,
        "should have retried at least 2 times: got {}",
        snap.retried
    );
    assert!(snap.completed >= 1, "should eventually complete");
    assert_eq!(snap.dead_lettered, 0, "should not dead-letter");

    token.cancel();
    let _ = run_handle.await;
}

// ── C. MetricsSnapshot — dead letter counter ─────────────────────────

#[tokio::test]
async fn metrics_snapshot_dead_letter() {
    let scheduler = Scheduler::builder()
        .store(taskmill::TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new().task::<TestTask>(FailNTimesExecutor {
                failures: AtomicI32::new(0),
                max_failures: 100, // always fail retryably
            }),
        )
        .max_concurrency(1)
        .max_retries(1) // only 1 retry allowed
        .build()
        .await
        .unwrap();

    let (token, run_handle) = start_scheduler(&scheduler);

    scheduler
        .domain::<TestDomain>()
        .submit(TestTask)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    let snap = scheduler.metrics_snapshot().await;
    assert!(
        snap.dead_lettered >= 1,
        "should have at least 1 dead letter: got {}",
        snap.dead_lettered
    );

    token.cancel();
    let _ = run_handle.await;
}

// ── D. MetricsSnapshot — batch submission counter ────────────────────

#[tokio::test]
async fn metrics_snapshot_batch_submission() {
    let scheduler = Scheduler::builder()
        .store(taskmill::TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .max_concurrency(4)
        .build()
        .await
        .unwrap();

    let (token, run_handle) = start_scheduler(&scheduler);

    let subs: Vec<_> = (0..5)
        .map(|i| {
            TaskSubmission::new("test::test")
                .key(format!("batch-{i}"))
                .priority(Priority::NORMAL)
        })
        .collect();
    scheduler.submit_batch(&subs).await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let snap = scheduler.metrics_snapshot().await;
    assert_eq!(snap.batches_submitted, 1, "one batch call");
    assert_eq!(snap.submitted, 5, "5 tasks submitted");

    token.cancel();
    let _ = run_handle.await;
}

// ── E. MetricsSnapshot — group pause/resume counters ─────────────────

#[tokio::test]
async fn metrics_snapshot_group_pause_resume() {
    let scheduler = Scheduler::builder()
        .store(taskmill::TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .max_concurrency(4)
        .default_group_concurrency(2)
        .build()
        .await
        .unwrap();

    scheduler.pause_group("g1").await.unwrap();
    scheduler.resume_group("g1").await.unwrap();

    let snap = scheduler.metrics_snapshot().await;
    assert_eq!(snap.group_pauses, 1);
    assert_eq!(snap.group_resumes, 1);
}

// ── F. MetricsSnapshot — gauges reflect current state ────────────────

#[tokio::test]
async fn metrics_snapshot_gauges() {
    let scheduler = Scheduler::builder()
        .store(taskmill::TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new().task::<TestTask>(DelayExecutor(Duration::from_millis(500))),
        )
        .max_concurrency(2)
        .build()
        .await
        .unwrap();

    let (token, run_handle) = start_scheduler(&scheduler);

    let handle = scheduler.domain::<TestDomain>();
    for i in 0..3 {
        handle
            .submit_with(TestTask)
            .key(format!("g-{i}"))
            .await
            .unwrap();
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    let snap = scheduler.metrics_snapshot().await;
    assert_eq!(snap.max_concurrency, 2);
    assert!(
        snap.running <= 2,
        "running should be at most max_concurrency"
    );

    token.cancel();
    let _ = run_handle.await;
}

// ── G. MetricsSnapshot — superseded counter ──────────────────────────

#[tokio::test]
async fn metrics_snapshot_superseded() {
    let scheduler = Scheduler::builder()
        .store(taskmill::TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(DelayExecutor(Duration::from_secs(5))))
        .max_concurrency(1)
        .build()
        .await
        .unwrap();

    let (token, run_handle) = start_scheduler(&scheduler);

    let sub = TaskSubmission::new("test::test")
        .key("same-key")
        .on_duplicate(taskmill::DuplicateStrategy::Supersede);
    scheduler.submit(&sub).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    scheduler.submit(&sub).await.unwrap();

    tokio::time::sleep(Duration::from_millis(200)).await;

    let snap = scheduler.metrics_snapshot().await;
    assert!(
        snap.superseded >= 1,
        "should have superseded at least 1: got {}",
        snap.superseded
    );

    token.cancel();
    let _ = run_handle.await;
}
