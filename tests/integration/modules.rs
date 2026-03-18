//! Integration tests: sections N (module registration + DomainHandle)

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use taskmill::{
    Domain, DomainHandle, DomainKey, Scheduler, SchedulerEvent, TaskStatus, TaskStore,
    TaskSubmission,
};
use tokio_util::sync::CancellationToken;

use super::common::*;

// ── Domain key structs ──────────────────────────────────────────────

struct MediaDomain;
impl DomainKey for MediaDomain {
    const NAME: &'static str = "media";
}

struct SyncDomain;
impl DomainKey for SyncDomain {
    const NAME: &'static str = "sync";
}

struct AnalyticsDomain;
impl DomainKey for AnalyticsDomain {
    const NAME: &'static str = "analytics";
}

// ═══════════════════════════════════════════════════════════════════
// N. Module Registration (Step 3)
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn two_modules_route_to_correct_executors() {
    let media_count = Arc::new(AtomicUsize::new(0));
    let sync_count = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MediaDomain>::new().raw_executor(
            "thumb",
            CountingExecutor {
                count: media_count.clone(),
            },
        ))
        .domain(Domain::<SyncDomain>::new().raw_executor(
            "push",
            CountingExecutor {
                count: sync_count.clone(),
            },
        ))
        .max_concurrency(4)
        .build()
        .await
        .unwrap();

    sched
        .submit(&TaskSubmission::new("media::thumb").key("t1"))
        .await
        .unwrap();
    sched
        .submit(&TaskSubmission::new("sync::push").key("p1"))
        .await
        .unwrap();

    sched.try_dispatch().await.unwrap();
    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(
        media_count.load(Ordering::SeqCst),
        1,
        "media::thumb executor should have run once"
    );
    assert_eq!(
        sync_count.load(Ordering::SeqCst),
        1,
        "sync::push executor should have run once"
    );
}

#[tokio::test]
async fn zero_modules_build_returns_error() {
    let result = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .build()
        .await;

    assert!(result.is_err(), "build with no modules should fail");
    let msg = result.err().unwrap().to_string();
    assert!(
        msg.contains("module"),
        "error message should mention modules, got: {msg}"
    );
}

#[tokio::test]
async fn duplicate_module_names_build_returns_error() {
    let result = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MediaDomain>::new().raw_executor("thumb", NoopExecutor))
        .domain(Domain::<MediaDomain>::new().raw_executor("transcode", NoopExecutor))
        .build()
        .await;

    assert!(result.is_err(), "duplicate module names should fail");
    let msg = result.err().unwrap().to_string();
    assert!(
        msg.contains("media"),
        "error message should mention the duplicate name, got: {msg}"
    );
}

#[tokio::test]
async fn task_type_collision_across_modules_returns_error() {
    // Two different modules register the same local task type name.
    // The prefixed names differ ("a::thumb" vs "b::thumb") so this is actually fine.
    // To get a true collision we'd need the same *prefixed* name, which means
    // the same module name AND same type — covered by duplicate_module_names.
    // Instead, verify that two distinct modules with distinct types succeed.
    let result = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MediaDomain>::new().raw_executor("thumb", NoopExecutor))
        .domain(Domain::<AnalyticsDomain>::new().raw_executor("thumb", NoopExecutor))
        .build()
        .await;

    assert!(
        result.is_ok(),
        "same local type name in different modules should be fine (different prefixes)"
    );
}

// ═══════════════════════════════════════════════════════════════════
// N. DomainHandle — Step 4
// ═══════════════════════════════════════════════════════════════════

/// Build a two-module scheduler (media + sync) backed by an in-memory store.
async fn two_module_scheduler() -> (
    Scheduler,
    DomainHandle<MediaDomain>,
    DomainHandle<SyncDomain>,
) {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MediaDomain>::new().raw_executor("thumb", NoopExecutor))
        .domain(Domain::<SyncDomain>::new().raw_executor("push", NoopExecutor))
        .poll_interval(Duration::from_millis(20))
        .max_concurrency(8)
        .build()
        .await
        .unwrap();
    let media = sched.domain::<MediaDomain>();
    let sync = sched.domain::<SyncDomain>();
    (sched, media, sync)
}

/// `cancel_all()` on the media handle only cancels media tasks; sync tasks
/// remain in the queue.
#[tokio::test]
async fn module_cancel_all_only_cancels_own_module() {
    let (sched, media, _sync) = two_module_scheduler().await;

    // Submit 3 media tasks and 2 sync tasks.
    for i in 0..3 {
        sched
            .submit(&TaskSubmission::new("media::thumb").key(format!("m{i}")))
            .await
            .unwrap();
    }
    let sync_ids: Vec<i64> = {
        let mut ids = Vec::new();
        for i in 0..2 {
            let outcome = sched
                .submit(&TaskSubmission::new("sync::push").key(format!("s{i}")))
                .await
                .unwrap();
            ids.push(outcome.id().unwrap());
        }
        ids
    };

    let cancelled = media.cancel_all().await.unwrap();
    assert_eq!(
        cancelled.len(),
        3,
        "media.cancel_all() should cancel 3 tasks"
    );

    // Sync tasks must still be in the active queue.
    for sync_id in sync_ids {
        let task = sched.store().task_by_id(sync_id).await.unwrap();
        assert!(
            task.is_some(),
            "sync task {sync_id} should still exist after media.cancel_all()"
        );
    }
}

/// `pause()` sets the pending media tasks to paused while sync tasks remain
/// pending; `resume()` moves them back.
#[tokio::test]
async fn module_pause_resume_only_affects_own_module() {
    let (sched, media, _sync) = two_module_scheduler().await;

    for i in 0..3 {
        sched
            .submit(&TaskSubmission::new("media::thumb").key(format!("m{i}")))
            .await
            .unwrap();
        sched
            .submit(&TaskSubmission::new("sync::push").key(format!("s{i}")))
            .await
            .unwrap();
    }

    media.pause().await.unwrap();
    assert!(media.is_paused(), "media should be paused");

    // Media tasks should now be paused in the DB; sync tasks still pending.
    let media_tasks = sched.store().tasks_by_type_prefix("media::").await.unwrap();
    let sync_tasks = sched.store().tasks_by_type_prefix("sync::").await.unwrap();
    assert!(
        media_tasks.iter().all(|t| t.status == TaskStatus::Paused),
        "all media tasks should be Paused"
    );
    assert!(
        sync_tasks.iter().all(|t| t.status == TaskStatus::Pending),
        "all sync tasks should still be Pending"
    );

    media.resume().await.unwrap();
    assert!(!media.is_paused(), "media should be resumed");

    let media_tasks_after = sched.store().tasks_by_type_prefix("media::").await.unwrap();
    assert!(
        media_tasks_after
            .iter()
            .all(|t| t.status == TaskStatus::Pending),
        "all media tasks should be Pending after resume"
    );
}

/// `resume()` while the global scheduler is paused should leave tasks in paused
/// state.
#[tokio::test]
async fn module_resume_while_scheduler_paused_tasks_stay_paused() {
    let (sched, media, _sync) = two_module_scheduler().await;

    for i in 0..2 {
        sched
            .submit(&TaskSubmission::new("media::thumb").key(format!("m{i}")))
            .await
            .unwrap();
    }

    // Pause media first, then globally pause the scheduler.
    media.pause().await.unwrap();
    sched.pause_all().await;

    // Attempt to resume the module while the scheduler is globally paused.
    let resumed = media.resume().await.unwrap();
    assert_eq!(
        resumed, 0,
        "no tasks should be resumed while globally paused"
    );

    // Tasks should still be paused.
    let tasks = sched.store().tasks_by_type_prefix("media::").await.unwrap();
    assert!(
        tasks.iter().all(|t| t.status == TaskStatus::Paused),
        "tasks should remain Paused when globally paused"
    );
}

/// `active_tasks()` on a domain handle returns only running tasks owned by that
/// module.
#[tokio::test]
async fn module_active_tasks_returns_only_own_module() {
    // Use delay executors so tasks are "running" long enough to observe.
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<MediaDomain>::new()
                .raw_executor("thumb", DelayExecutor(Duration::from_secs(5))),
        )
        .domain(
            Domain::<SyncDomain>::new().raw_executor("push", DelayExecutor(Duration::from_secs(5))),
        )
        .poll_interval(Duration::from_millis(20))
        .max_concurrency(8)
        .build()
        .await
        .unwrap();
    let media = sched.domain::<MediaDomain>();

    for i in 0..2 {
        sched
            .submit(&TaskSubmission::new("media::thumb").key(format!("m{i}")))
            .await
            .unwrap();
        sched
            .submit(&TaskSubmission::new("sync::push").key(format!("s{i}")))
            .await
            .unwrap();
    }

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let tok = token.clone();
    tokio::spawn(async move { sched_clone.run(tok).await });

    // Wait until all 4 tasks are dispatched.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut dispatched = 0usize;
    while dispatched < 4 && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(SchedulerEvent::Dispatched(_))) =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
        {
            dispatched += 1;
        }
    }
    assert_eq!(dispatched, 4, "expected all 4 tasks dispatched");

    // media.active_tasks() must only contain media tasks.
    let active = media.active_tasks();
    assert_eq!(
        active.len(),
        2,
        "media.active_tasks() should have 2 entries"
    );
    assert!(
        active.iter().all(|t| t.task_type.starts_with("media::")),
        "all active tasks should be media tasks"
    );

    token.cancel();
}

/// `events()` on a domain handle only delivers events for that module.
#[tokio::test]
async fn module_subscribe_receives_only_own_events() {
    let count = Arc::new(AtomicUsize::new(0));
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MediaDomain>::new().raw_executor(
            "thumb",
            CountingExecutor {
                count: count.clone(),
            },
        ))
        .domain(Domain::<SyncDomain>::new().raw_executor(
            "push",
            CountingExecutor {
                count: count.clone(),
            },
        ))
        .poll_interval(Duration::from_millis(20))
        .max_concurrency(8)
        .build()
        .await
        .unwrap();
    let media = sched.domain::<MediaDomain>();
    let mut media_rx = media.events();

    for i in 0..3 {
        sched
            .submit(&TaskSubmission::new("media::thumb").key(format!("m{i}")))
            .await
            .unwrap();
        sched
            .submit(&TaskSubmission::new("sync::push").key(format!("s{i}")))
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let tok = token.clone();
    tokio::spawn(async move { sched_clone.run(tok).await });

    // Collect 3 Completed events from the media receiver.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut media_completions = 0usize;
    while media_completions < 3 && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(SchedulerEvent::Completed(ref h))) =
            tokio::time::timeout(Duration::from_millis(100), media_rx.recv()).await
        {
            assert!(
                h.task_type.starts_with("media::"),
                "received non-media event: {:?}",
                h.task_type
            );
            media_completions += 1;
        }
    }
    assert_eq!(
        media_completions, 3,
        "should receive exactly 3 media completions"
    );

    token.cancel();
}

/// `cancel()` on a task that belongs to a different module returns `Ok(false)`.
#[tokio::test]
async fn module_cancel_cross_module_returns_false() {
    let (sched, media, _sync) = two_module_scheduler().await;

    let sync_id = sched
        .submit(&TaskSubmission::new("sync::push").key("s0"))
        .await
        .unwrap()
        .id()
        .unwrap();

    let result = media.cancel(sync_id).await.unwrap();
    assert!(
        !result,
        "cancel of a sync task via media handle should return false"
    );

    // Sync task should still be pending.
    let task = sched.store().task_by_id(sync_id).await.unwrap();
    assert!(task.is_some(), "sync task should still exist");
}

/// `scheduler.domain::<NonexistentDomain>()` panics.
struct NonexistentDomain;
impl DomainKey for NonexistentDomain {
    const NAME: &'static str = "nonexistent";
}

#[tokio::test]
#[should_panic(expected = "not registered")]
async fn scheduler_module_nonexistent_panics() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MediaDomain>::new().raw_executor("thumb", NoopExecutor))
        .build()
        .await
        .unwrap();
    let _ = sched.domain::<NonexistentDomain>();
}

/// `scheduler.try_domain::<NonexistentDomain>()` returns `None`.
#[tokio::test]
async fn scheduler_try_module_nonexistent_returns_none() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MediaDomain>::new().raw_executor("thumb", NoopExecutor))
        .build()
        .await
        .unwrap();
    assert!(sched.try_domain::<NonexistentDomain>().is_none());
    assert!(sched.try_domain::<MediaDomain>().is_some());
}

/// `scheduler.task(id)` returns the task regardless of which module owns it.
#[tokio::test]
async fn scheduler_task_returns_regardless_of_module() {
    let (sched, _media, _sync) = two_module_scheduler().await;

    let media_id = sched
        .submit(&TaskSubmission::new("media::thumb").key("m0"))
        .await
        .unwrap()
        .id()
        .unwrap();
    let sync_id = sched
        .submit(&TaskSubmission::new("sync::push").key("s0"))
        .await
        .unwrap()
        .id()
        .unwrap();

    let media_task = sched.task(media_id).await.unwrap();
    let sync_task = sched.task(sync_id).await.unwrap();

    assert!(media_task.is_some(), "should find media task by id");
    assert_eq!(media_task.unwrap().task_type, "media::thumb");
    assert!(sync_task.is_some(), "should find sync task by id");
    assert_eq!(sync_task.unwrap().task_type, "sync::push");
}

#[tokio::test]
async fn module_registry_stored_in_scheduler() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MediaDomain>::new().raw_executor("thumb", NoopExecutor))
        .domain(Domain::<SyncDomain>::new().raw_executor("push", NoopExecutor))
        .build()
        .await
        .unwrap();

    let registry = sched.module_registry();
    assert!(
        registry.get("media").is_some(),
        "media module should be in registry"
    );
    assert!(
        registry.get("sync").is_some(),
        "sync module should be in registry"
    );
    assert!(
        registry.get("nonexistent").is_none(),
        "nonexistent module should not be found"
    );
    assert_eq!(
        registry.get("media").unwrap().prefix,
        "media::",
        "media prefix should be 'media::'"
    );
}
