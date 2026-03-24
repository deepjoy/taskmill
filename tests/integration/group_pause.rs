//! Integration tests: Group Pause edge cases (plan 042, step 6).
//!
//! - 6a: Submit to paused group → inserted as paused
//! - 6b: Recurring task in paused group → next instance paused
//! - 6c: Blocked→pending in paused group → downgraded to paused
//! - 6d: Multi-reason pause interaction (module + group)

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use taskmill::{
    Domain, IoBudget, PauseReasons, Scheduler, SchedulerEvent, SubmitOutcome, TaskStatus,
    TaskStore, TaskSubmission,
};
use tokio_util::sync::CancellationToken;

use super::common::*;

// ═══════════════════════════════════════════════════════════════════
// 6a. Submit to paused group
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn submit_to_paused_group_inserts_as_paused() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .build()
        .await
        .unwrap();

    // Pause the group before submitting.
    sched.pause_group("g1").await.unwrap();

    // Submit a task to the paused group.
    let outcome = sched
        .submit(
            &TaskSubmission::new("test::test")
                .key("paused-submit")
                .group("g1"),
        )
        .await
        .unwrap();

    // Outcome should indicate group_paused.
    match outcome {
        SubmitOutcome::Inserted { id, group_paused } => {
            assert!(group_paused, "group_paused flag should be true");
            let task = sched.store().task_by_id(id).await.unwrap().unwrap();
            assert_eq!(task.status, TaskStatus::Paused);
            assert!(task.pause_reasons.contains(PauseReasons::GROUP));
        }
        other => panic!("expected Inserted, got {other:?}"),
    }

    // Submit to a non-paused group — should NOT be paused.
    let outcome2 = sched
        .submit(
            &TaskSubmission::new("test::test")
                .key("normal-submit")
                .group("g2"),
        )
        .await
        .unwrap();

    match outcome2 {
        SubmitOutcome::Inserted { group_paused, .. } => {
            assert!(!group_paused, "group_paused should be false for non-paused group");
        }
        other => panic!("expected Inserted, got {other:?}"),
    }
}

#[tokio::test]
async fn submit_batch_to_paused_group() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .build()
        .await
        .unwrap();

    sched.pause_group("g1").await.unwrap();

    let subs: Vec<_> = (0..3)
        .map(|i| {
            TaskSubmission::new("test::test")
                .key(format!("batch-{i}"))
                .group("g1")
        })
        .collect();

    let outcomes = sched.submit_batch(&subs).await.unwrap();
    for outcome in outcomes {
        match outcome {
            SubmitOutcome::Inserted { id, group_paused } => {
                assert!(group_paused);
                let task = sched.store().task_by_id(id).await.unwrap().unwrap();
                assert_eq!(task.status, TaskStatus::Paused);
            }
            other => panic!("expected Inserted, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn submit_to_paused_group_resumes_on_group_resume() {
    let count = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new().task::<TestTask>(CountingExecutor {
                count: count.clone(),
            }),
        )
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    // Pause group, submit tasks, then resume — they should all complete.
    sched.pause_group("g1").await.unwrap();

    for i in 0..3 {
        sched
            .submit(
                &TaskSubmission::new("test::test")
                    .key(format!("resume-{i}"))
                    .group("g1"),
            )
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move { sched_clone.run(token_clone).await });

    // Brief wait to verify nothing dispatches while paused.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(count.load(Ordering::SeqCst), 0, "no tasks should run while group is paused");

    // Resume the group.
    sched.resume_group("g1").await.unwrap();

    // Wait for all 3 completions.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut completed = 0;
    while tokio::time::Instant::now() < deadline && completed < 3 {
        if let Ok(Ok(SchedulerEvent::Completed(..))) =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
        {
            completed += 1;
        }
    }

    token.cancel();
    let _ = handle.await;

    assert_eq!(completed, 3, "all 3 tasks should complete after group resume");
    assert_eq!(count.load(Ordering::SeqCst), 3);
}

// ═══════════════════════════════════════════════════════════════════
// 6b. Recurring task in paused group
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn recurring_next_instance_paused_in_paused_group() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let store = sched.store();

    // Submit a recurring task in group g1.
    let outcome = sched
        .submit(
            &TaskSubmission::new("test::test")
                .key("recurring-g1")
                .group("g1")
                .recurring(Duration::from_secs(600)),
        )
        .await
        .unwrap();
    let id = outcome.id().unwrap();

    // Pop and run it.
    let task = store.pop_by_id(id).await.unwrap().unwrap();

    // Pause the group while the task is running.
    sched.pause_group("g1").await.unwrap();

    // Complete the task — this should create a next instance.
    store
        .complete_with_record(&task, &IoBudget::default())
        .await
        .unwrap();

    // The next instance should exist and be paused with GROUP bit.
    let key = task.key.clone();
    let next = store.task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(next.status, TaskStatus::Paused);
    assert!(next.pause_reasons.contains(PauseReasons::GROUP));
}

// ═══════════════════════════════════════════════════════════════════
// 6c. Blocked→pending in paused group
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn blocked_task_unblocks_into_paused_group() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .build()
        .await
        .unwrap();

    let store = sched.store();
    let handle = sched.domain::<TestDomain>();

    // Submit dependency task in group g1.
    let dep_outcome = handle
        .submit_with(TestTask)
        .key("dep")
        .group("g1")
        .await
        .unwrap();
    let dep_id = dep_outcome.id().unwrap();

    // Submit blocked task depending on dep, same group.
    let blocked_outcome = handle
        .submit_with(TestTask)
        .key("blocked")
        .group("g1")
        .depends_on(dep_id)
        .await
        .unwrap();
    let blocked_id = blocked_outcome.id().unwrap();

    // Verify blocked.
    let t = store.task_by_id(blocked_id).await.unwrap().unwrap();
    assert_eq!(t.status, TaskStatus::Blocked);

    // Pause the group.
    sched.pause_group("g1").await.unwrap();

    // Complete the dependency.
    let dep = store.pop_by_id(dep_id).await.unwrap();
    // dep may be None if pause_group already paused it — use store directly.
    if let Some(dep) = dep {
        store
            .complete_with_record(&dep, &IoBudget::default())
            .await
            .unwrap();
        store.resolve_dependents(dep_id).await.unwrap();
    } else {
        // The dep was paused by pause_group. Resume it, pop, complete, resolve.
        sched.resume_group("g1").await.unwrap();
        let dep = store.pop_by_id(dep_id).await.unwrap().unwrap();
        // Re-pause before completing so the blocked task unblocks into a paused group.
        sched.pause_group("g1").await.unwrap();
        store
            .complete_with_record(&dep, &IoBudget::default())
            .await
            .unwrap();
        store.resolve_dependents(dep_id).await.unwrap();
    }

    // Blocked task should now be paused (not pending) because its group is paused.
    let t = store.task_by_id(blocked_id).await.unwrap().unwrap();
    assert_eq!(t.status, TaskStatus::Paused);
    assert!(t.pause_reasons.contains(PauseReasons::GROUP));

    // Resume group — task should become pending.
    sched.resume_group("g1").await.unwrap();
    let t = store.task_by_id(blocked_id).await.unwrap().unwrap();
    assert_eq!(t.status, TaskStatus::Pending);
    assert!(t.pause_reasons.is_empty());
}

// ═══════════════════════════════════════════════════════════════════
// 6d. Multi-reason pause interaction
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn multi_reason_pause_no_stranding() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .build()
        .await
        .unwrap();

    let store = sched.store();
    let handle = sched.domain::<TestDomain>();

    // Submit a task in group g1.
    let outcome = handle
        .submit_with(TestTask)
        .key("multi")
        .group("g1")
        .await
        .unwrap();
    let id = outcome.id().unwrap();

    // Pause by module (adds MODULE bit).
    handle.pause().await.unwrap();
    let t = store.task_by_id(id).await.unwrap().unwrap();
    assert_eq!(t.status, TaskStatus::Paused);
    assert!(t.pause_reasons.contains(PauseReasons::MODULE));

    // Pause group (adds GROUP bit to already-paused task).
    sched.pause_group("g1").await.unwrap();
    let t = store.task_by_id(id).await.unwrap().unwrap();
    assert!(t.pause_reasons.contains(PauseReasons::MODULE));
    assert!(t.pause_reasons.contains(PauseReasons::GROUP));

    // Resume module — task stays paused (GROUP bit remains).
    handle.resume().await.unwrap();
    let t = store.task_by_id(id).await.unwrap().unwrap();
    assert_eq!(t.status, TaskStatus::Paused);
    assert!(!t.pause_reasons.contains(PauseReasons::MODULE));
    assert!(t.pause_reasons.contains(PauseReasons::GROUP));

    // Resume group — task finally becomes pending.
    sched.resume_group("g1").await.unwrap();
    let t = store.task_by_id(id).await.unwrap().unwrap();
    assert_eq!(t.status, TaskStatus::Pending);
    assert!(t.pause_reasons.is_empty());
}

#[tokio::test]
async fn group_resume_with_module_still_paused() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .build()
        .await
        .unwrap();

    let store = sched.store();
    let handle = sched.domain::<TestDomain>();

    let outcome = handle
        .submit_with(TestTask)
        .key("dual")
        .group("g1")
        .await
        .unwrap();
    let id = outcome.id().unwrap();

    // Both pauses.
    handle.pause().await.unwrap();
    sched.pause_group("g1").await.unwrap();

    // Resume group first — module still holds it paused.
    sched.resume_group("g1").await.unwrap();
    let t = store.task_by_id(id).await.unwrap().unwrap();
    assert_eq!(t.status, TaskStatus::Paused);
    assert!(t.pause_reasons.contains(PauseReasons::MODULE));
    assert!(!t.pause_reasons.contains(PauseReasons::GROUP));

    // Resume module — now finally pending.
    handle.resume().await.unwrap();
    let t = store.task_by_id(id).await.unwrap().unwrap();
    assert_eq!(t.status, TaskStatus::Pending);
    assert!(t.pause_reasons.is_empty());
}

// ═══════════════════════════════════════════════════════════════════
// 6e. ModuleHandle / DomainHandle delegation
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn domain_handle_group_pause_delegation() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .build()
        .await
        .unwrap();

    let handle = sched.domain::<TestDomain>();

    assert!(!handle.is_group_paused("g1"));
    assert!(handle.paused_groups().is_empty());

    handle.pause_group("g1").await.unwrap();
    assert!(handle.is_group_paused("g1"));
    assert_eq!(handle.paused_groups(), vec!["g1".to_string()]);

    // Submit via domain handle — should be paused.
    let outcome = handle
        .submit_with(TestTask)
        .key("via-handle")
        .group("g1")
        .await
        .unwrap();
    match outcome {
        SubmitOutcome::Inserted { group_paused, .. } => assert!(group_paused),
        other => panic!("expected Inserted, got {other:?}"),
    }

    handle.resume_group("g1").await.unwrap();
    assert!(!handle.is_group_paused("g1"));
    assert!(handle.paused_groups().is_empty());
}
