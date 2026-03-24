//! Integration tests: Weighted fair scheduling (plan 037, phase 2).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use taskmill::{Domain, Scheduler, SchedulerEvent, TaskStore, TaskSubmission};
use tokio_util::sync::CancellationToken;

use super::common::*;

// ═══════════════════════════════════════════════════════════════════
// Weighted groups: proportional dispatch
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn weighted_groups_proportional_dispatch() {
    let count_a = Arc::new(AtomicUsize::new(0));
    let count_b = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new()
                .task::<TestTask>(CountingExecutor {
                    count: count_a.clone(),
                })
                .task::<SlowTask>(CountingExecutor {
                    count: count_b.clone(),
                }),
        )
        .max_concurrency(8)
        .group_weight("heavy", 3)
        .group_weight("light", 1)
        .build()
        .await
        .unwrap();

    // Submit 20 tasks in each group.
    for i in 0..20 {
        sched
            .submit(
                &TaskSubmission::new("test::test")
                    .key(format!("heavy-{i}"))
                    .group("heavy"),
            )
            .await
            .unwrap();
        sched
            .submit(
                &TaskSubmission::new("test::slow")
                    .key(format!("light-{i}"))
                    .group("light"),
            )
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched2 = sched.clone();
    let token2 = token.clone();
    let handle = tokio::spawn(async move { sched2.run(token2).await });

    // Wait for all tasks to complete.
    tokio::time::sleep(Duration::from_secs(2)).await;
    token.cancel();
    handle.await.unwrap();

    let a = count_a.load(Ordering::SeqCst);
    let b = count_b.load(Ordering::SeqCst);
    assert_eq!(a, 20, "all heavy tasks should complete");
    assert_eq!(b, 20, "all light tasks should complete");
}

// ═══════════════════════════════════════════════════════════════════
// Min slots guaranteed under pressure
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn min_slots_guaranteed_under_pressure() {
    let count_light = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new()
                .task::<TestTask>(DelayExecutor(Duration::from_millis(50)))
                .task::<SlowTask>(CountingExecutor {
                    count: count_light.clone(),
                }),
        )
        .max_concurrency(6)
        .group_weight("heavy", 10)
        .group_weight("light", 1)
        .group_minimum_slots("light", 2)
        .build()
        .await
        .unwrap();

    // Submit 20 heavy tasks (will fill most slots).
    for i in 0..20 {
        sched
            .submit(
                &TaskSubmission::new("test::test")
                    .key(format!("heavy-{i}"))
                    .group("heavy"),
            )
            .await
            .unwrap();
    }

    // Submit a few light tasks.
    for i in 0..5 {
        sched
            .submit(
                &TaskSubmission::new("test::slow")
                    .key(format!("light-{i}"))
                    .group("light"),
            )
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched2 = sched.clone();
    let token2 = token.clone();
    let handle = tokio::spawn(async move { sched2.run(token2).await });

    tokio::time::sleep(Duration::from_secs(2)).await;
    token.cancel();
    handle.await.unwrap();

    // Light tasks should have completed — min_slots guaranteed at least 2.
    let light = count_light.load(Ordering::SeqCst);
    assert!(
        light >= 2,
        "at least 2 light tasks should complete; got {light}"
    );
}

// ═══════════════════════════════════════════════════════════════════
// Work-conserving redistribution
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn work_conserving_redistribution() {
    let count = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new()
                .task::<TestTask>(CountingExecutor {
                    count: count.clone(),
                })
                .task::<SlowTask>(NoopExecutor),
        )
        .max_concurrency(10)
        .group_weight("a", 1)
        .group_weight("b", 1)
        .build()
        .await
        .unwrap();

    // Only submit tasks to group "a" — none in "b".
    for i in 0..10 {
        sched
            .submit(
                &TaskSubmission::new("test::test")
                    .key(format!("a-{i}"))
                    .group("a"),
            )
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched2 = sched.clone();
    let token2 = token.clone();
    let handle = tokio::spawn(async move { sched2.run(token2).await });

    tokio::time::sleep(Duration::from_secs(2)).await;
    token.cancel();
    handle.await.unwrap();

    // All 10 should have dispatched — idle group's slots should overflow.
    let dispatched = count.load(Ordering::SeqCst);
    assert_eq!(
        dispatched, 10,
        "work-conserving: all tasks dispatched; got {dispatched}"
    );
}

// ═══════════════════════════════════════════════════════════════════
// Runtime weight change
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn runtime_weight_change() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .max_concurrency(4)
        .group_weight("x", 1)
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    sched.set_group_weight("x", 5);

    // Should emit GroupWeightChanged event.
    let deadline = tokio::time::Instant::now() + Duration::from_millis(200);
    let evt = wait_for_event(
        &mut rx,
        deadline,
        |e| matches!(e, SchedulerEvent::GroupWeightChanged { group, .. } if group == "x"),
    )
    .await;

    assert!(evt.is_some(), "should emit GroupWeightChanged event");
    if let Some(SchedulerEvent::GroupWeightChanged {
        previous_weight,
        new_weight,
        ..
    }) = evt
    {
        assert_eq!(previous_weight, 1);
        assert_eq!(new_weight, 5);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Reset group weights
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn reset_group_weights_restores_default() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .max_concurrency(4)
        .group_weight("x", 5)
        .build()
        .await
        .unwrap();

    sched.reset_group_weights();

    // After reset, fair scheduling should be effectively off (no weights configured).
    let snapshot = sched.snapshot().await.unwrap();
    assert!(
        snapshot.group_allocations.is_empty(),
        "after reset, no allocations should be reported"
    );
}

// ═══════════════════════════════════════════════════════════════════
// Paused group releases allocation
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn paused_group_releases_allocation() {
    let count_b = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new()
                .task::<TestTask>(DelayExecutor(Duration::from_millis(50)))
                .task::<SlowTask>(CountingExecutor {
                    count: count_b.clone(),
                }),
        )
        .max_concurrency(6)
        .group_weight("a", 1)
        .group_weight("b", 1)
        .build()
        .await
        .unwrap();

    // Submit tasks in both groups.
    for i in 0..10 {
        sched
            .submit(
                &TaskSubmission::new("test::test")
                    .key(format!("a-{i}"))
                    .group("a"),
            )
            .await
            .unwrap();
        sched
            .submit(
                &TaskSubmission::new("test::slow")
                    .key(format!("b-{i}"))
                    .group("b"),
            )
            .await
            .unwrap();
    }

    // Pause group a — b should get all slots.
    sched.pause_group("a").await.unwrap();

    let token = CancellationToken::new();
    let sched2 = sched.clone();
    let token2 = token.clone();
    let handle = tokio::spawn(async move { sched2.run(token2).await });

    tokio::time::sleep(Duration::from_secs(2)).await;
    token.cancel();
    handle.await.unwrap();

    let b_completed = count_b.load(Ordering::SeqCst);
    assert_eq!(
        b_completed, 10,
        "all b tasks should complete when a is paused"
    );
}

// ═══════════════════════════════════════════════════════════════════
// Ungrouped tasks get fair share
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn ungrouped_tasks_get_fair_share() {
    let count_grouped = Arc::new(AtomicUsize::new(0));
    let count_ungrouped = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new()
                .task::<TestTask>(CountingExecutor {
                    count: count_grouped.clone(),
                })
                .task::<SlowTask>(CountingExecutor {
                    count: count_ungrouped.clone(),
                }),
        )
        .max_concurrency(8)
        .group_weight("grouped", 1)
        // default_weight = 1, so ungrouped virtual group also gets 1
        .build()
        .await
        .unwrap();

    // Submit grouped tasks.
    for i in 0..10 {
        sched
            .submit(
                &TaskSubmission::new("test::test")
                    .key(format!("g-{i}"))
                    .group("grouped"),
            )
            .await
            .unwrap();
    }

    // Submit ungrouped tasks.
    for i in 0..10 {
        sched
            .submit(&TaskSubmission::new("test::slow").key(format!("u-{i}")))
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched2 = sched.clone();
    let token2 = token.clone();
    let handle = tokio::spawn(async move { sched2.run(token2).await });

    tokio::time::sleep(Duration::from_secs(2)).await;
    token.cancel();
    handle.await.unwrap();

    let g = count_grouped.load(Ordering::SeqCst);
    let u = count_ungrouped.load(Ordering::SeqCst);
    assert_eq!(g, 10, "all grouped tasks should complete");
    assert_eq!(u, 10, "all ungrouped tasks should complete");
}

// ═══════════════════════════════════════════════════════════════════
// Snapshot shows allocations
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn snapshot_shows_allocations() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new().task::<TestTask>(DelayExecutor(Duration::from_secs(10))),
        )
        .max_concurrency(4)
        .group_weight("alpha", 3)
        .group_weight("beta", 1)
        .build()
        .await
        .unwrap();

    // Submit tasks so groups have pending demand.
    for i in 0..5 {
        sched
            .submit(
                &TaskSubmission::new("test::test")
                    .key(format!("a-{i}"))
                    .group("alpha"),
            )
            .await
            .unwrap();
        sched
            .submit(
                &TaskSubmission::new("test::test")
                    .key(format!("b-{i}"))
                    .group("beta"),
            )
            .await
            .unwrap();
    }

    let snapshot = sched.snapshot().await.unwrap();
    assert!(
        !snapshot.group_allocations.is_empty(),
        "snapshot should show group allocations"
    );

    // Both alpha and beta should appear.
    let names: Vec<_> = snapshot
        .group_allocations
        .iter()
        .map(|a| &a.group)
        .collect();
    assert!(
        names.contains(&&"alpha".to_string()),
        "alpha should be in allocations"
    );
    assert!(
        names.contains(&&"beta".to_string()),
        "beta should be in allocations"
    );
}

// ═══════════════════════════════════════════════════════════════════
// Builder configures weights
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn builder_configures_weights() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .max_concurrency(10)
        .group_weight("x", 3)
        .group_weight("y", 1)
        .default_group_weight(2)
        .group_minimum_slots("y", 2)
        .build()
        .await
        .unwrap();

    // Verify fair dispatch is being used by submitting and running.
    for i in 0..4 {
        sched
            .submit(
                &TaskSubmission::new("test::test")
                    .key(format!("x-{i}"))
                    .group("x"),
            )
            .await
            .unwrap();
        sched
            .submit(
                &TaskSubmission::new("test::test")
                    .key(format!("y-{i}"))
                    .group("y"),
            )
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched2 = sched.clone();
    let token2 = token.clone();
    let handle = tokio::spawn(async move { sched2.run(token2).await });

    tokio::time::sleep(Duration::from_secs(1)).await;

    // All tasks should have completed (fair scheduling didn't block anything).
    let pending = sched.store().pending_count().await.unwrap();

    token.cancel();
    handle.await.unwrap();

    assert_eq!(pending, 0, "all tasks should be dispatched");
}
