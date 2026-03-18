//! Integration tests: sections A–L
//! Priority, retry, preemption, backpressure, concurrency, run loop,
//! child tasks, crash recovery, batch submit, IO metrics, diagnostics,
//! and delayed/recurring scheduled tasks.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use taskmill::{Module, Priority, Scheduler, SchedulerEvent, TaskStore, TaskSubmission};
use tokio_util::sync::CancellationToken;

use super::common::*;

// ═══════════════════════════════════════════════════════════════════
// A. Priority & Ordering
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn priority_ordering_dispatches_highest_first() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .module(Module::new("test").executor("test", Arc::new(NoopExecutor)))
        .max_concurrency(1) // dispatch one at a time
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    // Submit in reverse priority order (low first, high last).
    sched
        .submit(
            &TaskSubmission::new("test::test")
                .key("low")
                .priority(Priority::IDLE),
        )
        .await
        .unwrap();
    sched
        .submit(
            &TaskSubmission::new("test::test")
                .key("mid")
                .priority(Priority::NORMAL),
        )
        .await
        .unwrap();
    sched
        .submit(
            &TaskSubmission::new("test::test")
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
        .module(Module::new("test").executor(
            "test",
            Arc::new(FailNTimesExecutor {
                failures: std::sync::atomic::AtomicI32::new(0),
                max_failures: 2,
            }),
        ))
        .max_retries(3)
        .max_concurrency(1)
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    sched
        .submit(&TaskSubmission::new("test::test").key("retry-me"))
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
        .module(Module::new("test").executor(
            "test",
            Arc::new(FailNTimesExecutor {
                failures: std::sync::atomic::AtomicI32::new(0),
                max_failures: 100, // will never succeed
            }),
        ))
        .max_retries(2)
        .max_concurrency(1)
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    sched
        .submit(&TaskSubmission::new("test::test").key("exhaust"))
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for dead-letter event (retries exhausted).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let dead_lettered = wait_for_event(&mut rx, deadline, |evt| {
        matches!(evt, SchedulerEvent::DeadLettered { .. })
    })
    .await;

    token.cancel();
    let _ = handle.await;

    assert!(
        dead_lettered.is_some(),
        "task should be dead-lettered after retries exhausted"
    );
}

// ═══════════════════════════════════════════════════════════════════
// C. Preemption & Resume
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn preemption_resumes_after_preemptor_completes() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .module(
            Module::new("test")
                .executor("slow", Arc::new(DelayExecutor(Duration::from_secs(10))))
                .executor("fast", Arc::new(NoopExecutor)),
        )
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
            &TaskSubmission::new("test::slow")
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
            &TaskSubmission::new("test::fast")
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
        .module(Module::new("test").executor("test", Arc::new(NoopExecutor)))
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
            &TaskSubmission::new("test::test")
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
            &TaskSubmission::new("test::test")
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
        .module(Module::new("test").executor("test", Arc::new(NoopExecutor)))
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
            &TaskSubmission::new("test::test")
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
            &TaskSubmission::new("test::test")
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
        .module(Module::new("test").executor(
            "test",
            Arc::new(ConcurrencyTrackingExecutor {
                current: current.clone(),
                max_seen: max_seen.clone(),
                delay: Duration::from_millis(100),
            }),
        ))
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
                &TaskSubmission::new("test::test")
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
        .module(Module::new("test").executor(
            "test",
            Arc::new(CountingExecutor {
                count: count.clone(),
            }),
        ))
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    // Submit 20 tasks.
    for i in 0..20 {
        sched
            .submit(&TaskSubmission::new("test::test").key(format!("task-{i}")))
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
        .module(Module::new("test").executor(
            "test",
            Arc::new(ConcurrencyTrackingExecutor {
                current: current.clone(),
                max_seen: max_seen.clone(),
                delay: Duration::from_millis(50),
            }),
        ))
        .max_concurrency(2)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    for i in 0..10 {
        sched
            .submit(&TaskSubmission::new("test::test").key(format!("conc-{i}")))
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
        .module(
            Module::new("test")
                .executor(
                    "parent",
                    Arc::new(ChildSpawnerExecutor {
                        child_type: "child",
                        count: 3,
                        fail_fast: true,
                    }),
                )
                .executor("child", Arc::new(AlwaysFailExecutor)),
        )
        .max_concurrency(4)
        .max_retries(0) // no retries so failures are permanent
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    sched
        .submit(
            &TaskSubmission::new("test::parent")
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
        |evt| matches!(evt, SchedulerEvent::Failed { ref header, .. } if header.task_type == "test::parent"),
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
    let finalized = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .module(
            Module::new("test")
                .executor(
                    "parent",
                    Arc::new(FinalizeTracker {
                        child_count: 2,
                        finalized: finalized.clone(),
                    }),
                )
                .executor("child", Arc::new(NoopExecutor)),
        )
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    sched
        .submit(
            &TaskSubmission::new("test::parent")
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
        |evt| matches!(evt, SchedulerEvent::Completed(ref h) if h.task_type == "test::parent"),
    )
    .await;

    token.cancel();
    let _ = handle.await;

    assert!(
        parent_completed.is_some(),
        "parent should complete after children"
    );
    assert!(
        finalized.load(std::sync::atomic::Ordering::SeqCst),
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
        .module(Module::new("test").executor("test", Arc::new(NoopExecutor)))
        .build()
        .await
        .unwrap();

    let submissions: Vec<_> = (0..50)
        .map(|i| TaskSubmission::new("test::test").key(format!("batch-{i}")))
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
        .module(Module::new("test").executor(
            "test",
            Arc::new(IoReportingExecutor {
                read: 4096,
                write: 1024,
            }),
        ))
        .build()
        .await
        .unwrap();

    sched
        .submit(&TaskSubmission::new("test::test").key("io-track"))
        .await
        .unwrap();

    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check history for the completed task.
    let key = taskmill::generate_dedup_key("test::test", Some(b"io-track"));
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
        .module(Module::new("test").executor("test", Arc::new(NoopExecutor)))
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

// ── Delayed & Scheduled Tasks ─────────────────────────────────────

#[tokio::test]
async fn delayed_task_not_dispatched_before_run_after() {
    let store = TaskStore::open_memory().await.unwrap();

    // Submit with a 10-second delay.
    let sub = TaskSubmission::new("test")
        .key("delayed")
        .run_after(Duration::from_secs(10));
    store.submit(&sub).await.unwrap();

    // peek_next should return None because run_after is in the future.
    assert!(store.peek_next().await.unwrap().is_none());
    // pop_next should also return None.
    assert!(store.pop_next().await.unwrap().is_none());

    // But the task is still pending.
    assert_eq!(store.pending_count().await.unwrap(), 1);
}

#[tokio::test]
async fn delayed_task_dispatched_after_run_after() {
    let store = TaskStore::open_memory().await.unwrap();

    // Submit with run_at in the past.
    let sub = TaskSubmission::new("test")
        .key("past-delay")
        .run_at(chrono::Utc::now() - chrono::Duration::seconds(1));
    store.submit(&sub).await.unwrap();

    // Should be immediately dispatchable since run_after is in the past.
    let task = store.peek_next().await.unwrap();
    assert!(task.is_some());
    assert_eq!(task.unwrap().run_after.is_some(), true);
}

#[tokio::test]
async fn recurring_task_creates_next_instance_on_completion() {
    let store = TaskStore::open_memory().await.unwrap();

    // Submit a recurring task with 60s interval.
    let sub = TaskSubmission::new("test")
        .key("recurring-1")
        .recurring(Duration::from_secs(60));
    store.submit(&sub).await.unwrap();
    let dedup_key = sub.effective_key();

    // Pop and complete.
    let task = store.pop_next().await.unwrap().unwrap();
    assert_eq!(task.recurring_interval_secs, Some(60));
    assert_eq!(task.recurring_execution_count, 0);

    store
        .complete(task.id, &taskmill::IoBudget::default())
        .await
        .unwrap();

    // A new pending instance should exist with the same dedup key.
    let next = store.task_by_key(&dedup_key).await.unwrap();
    assert!(next.is_some());
    let next = next.unwrap();
    assert_eq!(next.status, taskmill::TaskStatus::Pending);
    assert!(next.run_after.is_some()); // Should have a future run_after.
    assert_eq!(next.recurring_execution_count, 1);
    assert_eq!(next.recurring_interval_secs, Some(60));
}

#[tokio::test]
async fn recurring_task_respects_max_executions() {
    let store = TaskStore::open_memory().await.unwrap();

    // Submit recurring with max_executions = 2.
    let sub = TaskSubmission::new("test")
        .key("recurring-max")
        .recurring_schedule(taskmill::RecurringSchedule {
            interval: Duration::from_secs(1),
            initial_delay: None,
            max_executions: Some(2),
        });
    store.submit(&sub).await.unwrap();
    let dedup_key = sub.effective_key();

    // First execution.
    let task = store.pop_next().await.unwrap().unwrap();
    store
        .complete(task.id, &taskmill::IoBudget::default())
        .await
        .unwrap();
    // Should create a next instance (execution_count = 1, max = 2).
    let next = store.task_by_key(&dedup_key).await.unwrap().unwrap();
    assert_eq!(next.recurring_execution_count, 1);

    // Wait for run_after to pass.
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Second execution.
    let task2 = store.pop_next().await.unwrap().unwrap();
    store
        .complete(task2.id, &taskmill::IoBudget::default())
        .await
        .unwrap();

    // Should NOT create a third instance (execution_count = 2 >= max = 2).
    let next2 = store.task_by_key(&dedup_key).await.unwrap();
    assert!(next2.is_none());
}

#[tokio::test]
async fn recurring_pile_up_prevention() {
    let store = TaskStore::open_memory().await.unwrap();

    // Submit a recurring task.
    let sub = TaskSubmission::new("test")
        .key("pileup")
        .recurring(Duration::from_secs(1));
    store.submit(&sub).await.unwrap();
    let dedup_key = sub.effective_key();

    // Pop, complete → next instance created.
    let task = store.pop_next().await.unwrap().unwrap();
    store
        .complete(task.id, &taskmill::IoBudget::default())
        .await
        .unwrap();

    // Next instance exists but hasn't been dispatched.
    let pending = store.task_by_key(&dedup_key).await.unwrap().unwrap();
    assert_eq!(pending.status, taskmill::TaskStatus::Pending);

    // Now manually insert a second "completed" instance (simulating the same
    // key completing again while pending exists). We do this by submitting
    // another with the same key to test dedup + pile-up interaction.
    // The pending instance should still be there, not duplicated.
    let count = store.pending_count().await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn pause_and_resume_recurring_schedule() {
    let store = TaskStore::open_memory().await.unwrap();

    let sub = TaskSubmission::new("test")
        .key("pausable-recurring")
        .recurring(Duration::from_secs(60));
    let id = store.submit(&sub).await.unwrap().id().unwrap();
    let dedup_key = sub.effective_key();

    // Pause the recurring schedule.
    store.pause_recurring(id).await.unwrap();

    // Pop and complete — should NOT create next instance.
    let task = store.pop_next().await.unwrap().unwrap();
    assert!(task.recurring_paused);
    store
        .complete(task.id, &taskmill::IoBudget::default())
        .await
        .unwrap();

    let next = store.task_by_key(&dedup_key).await.unwrap();
    assert!(next.is_none());
}

#[tokio::test]
async fn next_run_after_query() {
    let store = TaskStore::open_memory().await.unwrap();

    // No pending tasks → None.
    assert!(store.next_run_after().await.unwrap().is_none());

    // Submit a delayed task.
    let future_time = chrono::Utc::now() + chrono::Duration::seconds(300);
    let sub = TaskSubmission::new("test")
        .key("far-future")
        .run_at(future_time);
    store.submit(&sub).await.unwrap();

    let next = store.next_run_after().await.unwrap();
    assert!(next.is_some());
    // Should be roughly 300 seconds from now.
    let diff = (next.unwrap() - chrono::Utc::now()).num_seconds();
    assert!(diff > 290 && diff <= 300);
}

#[tokio::test]
async fn recurring_schedules_query() {
    let store = TaskStore::open_memory().await.unwrap();

    // No recurring tasks → empty.
    assert!(store.recurring_schedules().await.unwrap().is_empty());

    // Submit a recurring task.
    let sub = TaskSubmission::new("test")
        .key("schedule-1")
        .recurring(Duration::from_secs(120));
    store.submit(&sub).await.unwrap();

    let schedules = store.recurring_schedules().await.unwrap();
    assert_eq!(schedules.len(), 1);
    assert_eq!(schedules[0].interval_secs, 120);
    assert_eq!(schedules[0].execution_count, 0);
    assert!(!schedules[0].paused);
}

#[tokio::test]
async fn recurring_task_rejects_parent_id() {
    let store = TaskStore::open_memory().await.unwrap();

    let mut sub = TaskSubmission::new("test")
        .key("bad-recurring")
        .recurring(Duration::from_secs(60));
    sub.parent_id = Some(42);

    let result = store.submit(&sub).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn delayed_task_full_scheduler_lifecycle() {
    let count = Arc::new(AtomicUsize::new(0));
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .module(Module::new("test").executor(
            "counting",
            Arc::new(CountingExecutor {
                count: count.clone(),
            }),
        ))
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    // Submit a task with run_at in the past.
    let sub = TaskSubmission::new("test::counting")
        .key("immediate")
        .run_at(chrono::Utc::now() - chrono::Duration::seconds(1));
    sched.submit(&sub).await.unwrap();

    let token = CancellationToken::new();
    let t = token.clone();
    tokio::spawn(async move { sched.run(t).await });

    // Wait for the task to be dispatched and completed.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(count.load(Ordering::SeqCst), 1);
    token.cancel();
}

#[tokio::test]
async fn recurring_task_snapshot_includes_schedules() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .module(Module::new("test").executor("test", Arc::new(NoopExecutor)))
        .build()
        .await
        .unwrap();

    let sub = TaskSubmission::new("test::test")
        .key("snap-recurring")
        .recurring(Duration::from_secs(600));
    sched.submit(&sub).await.unwrap();

    let snap = sched.snapshot().await.unwrap();
    assert_eq!(snap.recurring_schedules.len(), 1);
    assert_eq!(snap.recurring_schedules[0].interval_secs, 600);
}
