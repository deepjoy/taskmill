use std::sync::Arc;

use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::backpressure::{CompositePressure, ThrottlePolicy};
use crate::registry::{TaskContext, TaskExecutor, TaskTypeRegistry};
use crate::store::TaskStore;
use crate::task::{SubmitOutcome, TaskError, TaskSubmission};

use super::{Scheduler, SchedulerConfig, SchedulerEvent, TaskProgress};

struct InstantExecutor;

impl TaskExecutor for InstantExecutor {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        ctx.record_read_bytes(100);
        ctx.record_write_bytes(50);
        Ok(())
    }
}

struct SlowExecutor;

impl TaskExecutor for SlowExecutor {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        tokio::select! {
            _ = ctx.token().cancelled() => {
                Err(TaskError::new("cancelled"))
            }
            _ = tokio::time::sleep(Duration::from_secs(60)) => {
                ctx.record_read_bytes(100);
                ctx.record_write_bytes(50);
                Ok(())
            }
        }
    }
}

#[allow(dead_code)]
struct FailingExecutor;

impl TaskExecutor for FailingExecutor {
    async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        Err(TaskError::retryable("boom"))
    }
}

async fn setup(executor: Arc<dyn crate::registry::ErasedExecutor>) -> Scheduler {
    let store = TaskStore::open_memory().await.unwrap();
    let mut registry = TaskTypeRegistry::new();
    registry.register_erased("test", executor);

    Scheduler::new(
        store,
        SchedulerConfig::default(),
        Arc::new(registry),
        CompositePressure::new(),
        ThrottlePolicy::default_three_tier(),
    )
}

fn arc_erased<E: TaskExecutor>(e: E) -> Arc<dyn crate::registry::ErasedExecutor> {
    Arc::new(e) as Arc<dyn crate::registry::ErasedExecutor>
}

#[tokio::test]
async fn dispatch_executes_task() {
    let sched = setup(arc_erased(InstantExecutor)).await;

    sched
        .submit(&TaskSubmission::new("test").key("k1"))
        .await
        .unwrap();

    let dispatched = sched.try_dispatch().await.unwrap();
    assert!(dispatched);

    // Give spawned task time to complete.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Task should be completed and in history.
    let k1 = crate::task::generate_dedup_key("test", Some(b"k1"));
    assert!(sched.store().task_by_key(&k1).await.unwrap().is_none());
    let hist = sched.store().history_by_key(&k1).await.unwrap();
    assert_eq!(hist.len(), 1);
}

#[tokio::test]
async fn dispatch_returns_false_when_empty() {
    let sched = setup(arc_erased(InstantExecutor)).await;
    let dispatched = sched.try_dispatch().await.unwrap();
    assert!(!dispatched);
}

#[tokio::test]
async fn unregistered_type_fails_task() {
    let store = TaskStore::open_memory().await.unwrap();
    let registry = TaskTypeRegistry::new(); // empty — no executors

    let sched = Scheduler::new(
        store,
        SchedulerConfig::default(),
        Arc::new(registry),
        CompositePressure::new(),
        ThrottlePolicy::default_three_tier(),
    );

    sched
        .submit(&TaskSubmission::new("unknown").key("k"))
        .await
        .unwrap();

    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let failed = sched.store().failed_tasks(10).await.unwrap();
    assert_eq!(failed.len(), 1);
}

#[tokio::test]
async fn dedup_via_scheduler() {
    let sched = setup(arc_erased(InstantExecutor)).await;

    let sub = TaskSubmission::new("test").key("dup");

    let first = sched.submit(&sub).await.unwrap();
    let second = sched.submit(&sub).await.unwrap();
    assert!(first.is_inserted());
    assert_eq!(second, SubmitOutcome::Duplicate);
}

#[tokio::test]
async fn set_max_concurrency_works() {
    let sched = setup(arc_erased(InstantExecutor)).await;
    assert_eq!(sched.max_concurrency(), 4);
    sched.set_max_concurrency(8);
    assert_eq!(sched.max_concurrency(), 8);
}

#[tokio::test]
async fn cancel_pending_task() {
    let sched = setup(arc_erased(InstantExecutor)).await;

    let id = sched
        .submit(&TaskSubmission::new("test").key("cancel-me"))
        .await
        .unwrap()
        .id()
        .unwrap();

    let cancelled = sched.cancel(id).await.unwrap();
    assert!(cancelled);

    // Task should be gone.
    let cancel_key = crate::task::generate_dedup_key("test", Some(b"cancel-me"));
    assert!(sched
        .store()
        .task_by_key(&cancel_key)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn cancel_running_task() {
    let sched = setup(arc_erased(SlowExecutor)).await;

    let id = sched
        .submit(&TaskSubmission::new("test").key("cancel-running"))
        .await
        .unwrap()
        .id()
        .unwrap();

    // Dispatch it so it's running.
    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;

    let cancelled = sched.cancel(id).await.unwrap();
    assert!(cancelled);
}

#[tokio::test]
async fn event_emitted_on_complete() {
    let sched = setup(arc_erased(InstantExecutor)).await;
    let mut rx = sched.subscribe();

    sched
        .submit(&TaskSubmission::new("test").key("evt"))
        .await
        .unwrap();

    sched.try_dispatch().await.unwrap();

    // Should get Dispatched event.
    let evt = rx.recv().await.unwrap();
    assert!(matches!(evt, SchedulerEvent::Dispatched(..)));

    // Wait for completion.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let evt = rx.recv().await.unwrap();
    assert!(matches!(evt, SchedulerEvent::Completed(..)));
}

#[tokio::test]
async fn scheduler_is_clone() {
    let sched = setup(arc_erased(InstantExecutor)).await;
    let sched2 = sched.clone();

    // Both should share the same store.
    sched
        .submit(&TaskSubmission::new("test").key("shared"))
        .await
        .unwrap();

    // The clone can see the task.
    let shared_key = crate::task::generate_dedup_key("test", Some(b"shared"));
    let task = sched2.store().task_by_key(&shared_key).await.unwrap();
    assert!(task.is_some());
}

#[tokio::test]
async fn submit_typed_enqueues_task() {
    use serde::{Deserialize as De, Serialize as Ser};

    #[derive(Ser, De, Debug, PartialEq)]
    struct Thumb {
        path: String,
    }

    impl crate::task::TypedTask for Thumb {
        const TASK_TYPE: &'static str = "test";

        fn expected_io(&self) -> crate::task::IoBudget {
            crate::task::IoBudget::disk(4096, 512)
        }
    }

    let sched = setup(arc_erased(InstantExecutor)).await;

    let task = Thumb {
        path: "/a.jpg".into(),
    };
    let outcome = sched.submit_typed(&task).await.unwrap();
    assert!(outcome.is_inserted());

    // Verify the stored record has correct metadata.
    let record = sched
        .store()
        .task_by_id(outcome.id().unwrap())
        .await
        .unwrap()
        .expect("task should exist");
    assert_eq!(record.task_type, "test");
    assert_eq!(record.expected_io.disk_read, 4096);
    assert_eq!(record.expected_io.disk_write, 512);

    // Payload round-trips.
    let recovered: Thumb = record.deserialize_payload().unwrap().unwrap();
    assert_eq!(recovered, task);
}

#[tokio::test]
async fn snapshot_returns_dashboard_state() {
    let sched = setup(arc_erased(SlowExecutor)).await;

    // Submit two tasks.
    for key in &["snap-a", "snap-b"] {
        sched
            .submit(&TaskSubmission::new("test").key(*key))
            .await
            .unwrap();
    }

    // Dispatch one so it becomes running.
    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;

    let snap = sched.snapshot().await.unwrap();

    assert_eq!(snap.running.len(), 1);
    assert_eq!(snap.pending_count, 1);
    assert_eq!(snap.paused_count, 0);
    assert_eq!(snap.progress.len(), 1);
    assert_eq!(snap.pressure, 0.0); // no pressure sources
    assert!(snap.pressure_breakdown.is_empty());
    assert_eq!(snap.max_concurrency, 4);
}

#[tokio::test]
async fn pause_all_stops_dispatching() {
    let sched = setup(arc_erased(SlowExecutor)).await;

    // Submit two tasks.
    for key in &["pa-1", "pa-2"] {
        sched
            .submit(&TaskSubmission::new("test").key(*key))
            .await
            .unwrap();
    }

    // Dispatch one so it's running.
    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(sched.active_tasks().await.len(), 1);

    // Pause — running task should be cancelled and moved to paused in store.
    sched.pause_all().await;
    assert!(sched.is_paused());
    assert_eq!(sched.active_tasks().await.len(), 0);

    // try_dispatch should still work at the store level (it doesn't check
    // the pause flag itself — the run loop does), but we can verify that
    // the snapshot shows is_paused.
    let snap = sched.snapshot().await.unwrap();
    assert!(snap.is_paused);

    // Resume — flag should clear.
    sched.resume_all().await;
    assert!(!sched.is_paused());
    let snap = sched.snapshot().await.unwrap();
    assert!(!snap.is_paused);
}

#[tokio::test]
async fn pause_resume_events_emitted() {
    let sched = setup(arc_erased(InstantExecutor)).await;
    let mut rx = sched.subscribe();

    sched.pause_all().await;
    let evt = rx.recv().await.unwrap();
    assert!(matches!(evt, SchedulerEvent::Paused));

    sched.resume_all().await;
    let evt = rx.recv().await.unwrap();
    assert!(matches!(evt, SchedulerEvent::Resumed));
}

#[tokio::test]
async fn app_state_accessible_from_executor() {
    use std::sync::atomic::{AtomicBool, Ordering};

    struct MyState {
        flag: Arc<AtomicBool>,
    }

    struct StateCheckExecutor;

    impl TaskExecutor for StateCheckExecutor {
        async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
            let state = ctx.state::<MyState>().expect("state should be set");
            state.flag.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    let flag = Arc::new(AtomicBool::new(false));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor("test", Arc::new(StateCheckExecutor))
        .app_state(MyState { flag: flag.clone() })
        .build()
        .await
        .unwrap();

    sched
        .submit(&TaskSubmission::new("test").key("state-test"))
        .await
        .unwrap();

    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(flag.load(Ordering::SeqCst));
}

#[tokio::test]
async fn task_lookup_pending() {
    let sched = setup(arc_erased(InstantExecutor)).await;

    sched
        .submit(&TaskSubmission::new("test").key("lookup-1"))
        .await
        .unwrap();

    let result = sched.task_lookup("test", Some(b"lookup-1")).await.unwrap();
    assert!(matches!(
        result,
        crate::task::TaskLookup::Active(ref r) if r.status == crate::task::TaskStatus::Pending
    ));
}

#[tokio::test]
async fn task_lookup_completed() {
    let sched = setup(arc_erased(InstantExecutor)).await;

    sched
        .submit(&TaskSubmission::new("test").key("lookup-done"))
        .await
        .unwrap();

    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = sched
        .task_lookup("test", Some(b"lookup-done"))
        .await
        .unwrap();
    assert!(matches!(result, crate::task::TaskLookup::History(_)));
}

#[tokio::test]
async fn task_lookup_not_found() {
    let sched = setup(arc_erased(InstantExecutor)).await;
    let result = sched
        .task_lookup("test", Some(b"does-not-exist"))
        .await
        .unwrap();
    assert!(matches!(result, crate::task::TaskLookup::NotFound));
}

#[tokio::test]
async fn lookup_typed_works() {
    use serde::{Deserialize as De, Serialize as Ser};

    #[derive(Ser, De, Debug, PartialEq)]
    struct Thumb {
        path: String,
    }

    impl crate::task::TypedTask for Thumb {
        const TASK_TYPE: &'static str = "test";
    }

    let sched = setup(arc_erased(InstantExecutor)).await;

    let task = Thumb {
        path: "/a.jpg".into(),
    };
    sched.submit_typed(&task).await.unwrap();

    let result = sched.lookup_typed(&task).await.unwrap();
    assert!(matches!(result, crate::task::TaskLookup::Active(_)));
}

// ── Hierarchy tests ─────────────────────────────────────────────

/// An executor that spawns N child tasks during execution.
struct SpawningExecutor {
    num_children: usize,
}

impl TaskExecutor for SpawningExecutor {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        for i in 0..self.num_children {
            let sub = TaskSubmission::new("child")
                .key(format!("child-{i}"))
                .priority(ctx.record().priority);
            ctx.spawn_child(sub).await?;
        }
        Ok(())
    }
}

/// An executor that records whether finalize was called.
struct FinalizeTrackingExecutor {
    children: usize,
    finalized: Arc<std::sync::atomic::AtomicBool>,
}

impl TaskExecutor for FinalizeTrackingExecutor {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        for i in 0..self.children {
            let sub = TaskSubmission::new("child")
                .key(format!("ft-child-{i}"))
                .priority(ctx.record().priority);
            ctx.spawn_child(sub).await?;
        }
        Ok(())
    }

    async fn finalize<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        self.finalized
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn parent_enters_waiting_when_children_spawned() {
    let store = TaskStore::open_memory().await.unwrap();
    let mut registry = TaskTypeRegistry::new();
    registry.register_erased("parent", arc_erased(SpawningExecutor { num_children: 2 }));
    registry.register_erased("child", arc_erased(InstantExecutor));

    let sched = Scheduler::new(
        store,
        SchedulerConfig::default(),
        Arc::new(registry),
        CompositePressure::new(),
        ThrottlePolicy::default_three_tier(),
    );
    let mut rx = sched.subscribe();

    // Submit parent task.
    sched
        .submit(&TaskSubmission::new("parent").key("p1"))
        .await
        .unwrap();

    // Dispatch parent.
    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Should get Dispatched, then Waiting events for the parent.
    let mut saw_waiting = false;
    for _ in 0..10 {
        if let Ok(evt) = rx.try_recv() {
            if matches!(evt, SchedulerEvent::Waiting { .. }) {
                saw_waiting = true;
                break;
            }
        }
    }
    assert!(saw_waiting, "expected Waiting event for parent");

    // Parent should be in waiting status in the store.
    let parent_key = crate::task::generate_dedup_key("parent", Some(b"p1"));
    let parent = sched
        .store()
        .task_by_key(&parent_key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(parent.status, crate::task::TaskStatus::Waiting);

    // Two children should be pending.
    assert_eq!(sched.store().pending_count().await.unwrap(), 2);
}

#[tokio::test]
async fn parent_auto_completes_after_children_finish() {
    let store = TaskStore::open_memory().await.unwrap();
    let mut registry = TaskTypeRegistry::new();
    registry.register_erased("parent", arc_erased(SpawningExecutor { num_children: 2 }));
    registry.register_erased("child", arc_erased(InstantExecutor));

    let sched = Scheduler::new(
        store,
        SchedulerConfig::default(),
        Arc::new(registry),
        CompositePressure::new(),
        ThrottlePolicy::default_three_tier(),
    );
    let mut rx = sched.subscribe();

    sched
        .submit(&TaskSubmission::new("parent").key("p-complete"))
        .await
        .unwrap();

    // Run scheduler loop.
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for parent Completed event.
    let parent_key = crate::task::generate_dedup_key("parent", Some(b"p-complete"));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut parent_completed = false;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(SchedulerEvent::Completed(ref h))) if h.task_type == "parent" => {
                parent_completed = true;
                break;
            }
            _ => {}
        }
    }

    // Check before shutdown closes the pool.
    let lookup = sched.store().task_lookup(&parent_key).await.unwrap();

    token.cancel();
    let _ = handle.await;

    assert!(parent_completed, "expected parent Completed event");
    assert!(
        matches!(lookup, crate::task::TaskLookup::History(ref h) if h.status == crate::task::HistoryStatus::Completed),
        "expected parent in history as completed, got: {lookup:?}"
    );
}

#[tokio::test]
async fn finalize_called_after_children_complete() {
    let finalized = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let store = TaskStore::open_memory().await.unwrap();
    let mut registry = TaskTypeRegistry::new();
    registry.register_erased(
        "parent",
        arc_erased(FinalizeTrackingExecutor {
            children: 1,
            finalized: finalized.clone(),
        }),
    );
    registry.register_erased("child", arc_erased(InstantExecutor));

    let sched = Scheduler::new(
        store,
        SchedulerConfig::default(),
        Arc::new(registry),
        CompositePressure::new(),
        ThrottlePolicy::default_three_tier(),
    );
    let mut rx = sched.subscribe();

    sched
        .submit(&TaskSubmission::new("parent").key("p-finalize"))
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for parent Completed event rather than a fixed sleep.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(SchedulerEvent::Completed(ref h))) if h.task_type == "parent" => {
                break;
            }
            _ => {}
        }
    }

    token.cancel();
    let _ = handle.await;

    assert!(
        finalized.load(std::sync::atomic::Ordering::SeqCst),
        "finalize() should have been called"
    );
}

#[tokio::test]
async fn cancel_parent_cascades_to_children() {
    let store = TaskStore::open_memory().await.unwrap();
    let mut registry = TaskTypeRegistry::new();
    registry.register_erased("parent", arc_erased(SpawningExecutor { num_children: 3 }));
    registry.register_erased("child", arc_erased(SlowExecutor));

    let sched = Scheduler::new(
        store,
        SchedulerConfig::default(),
        Arc::new(registry),
        CompositePressure::new(),
        ThrottlePolicy::default_three_tier(),
    );

    let parent_id = sched
        .submit(&TaskSubmission::new("parent").key("p-cancel"))
        .await
        .unwrap()
        .id()
        .unwrap();

    // Dispatch parent (which spawns children).
    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Cancel parent — should cascade to children.
    let cancelled = sched.cancel(parent_id).await.unwrap();
    assert!(cancelled);

    // All children should be gone.
    assert_eq!(sched.store().pending_count().await.unwrap(), 0);
    assert_eq!(sched.store().running_count().await.unwrap(), 0);
}

#[tokio::test]
async fn no_children_completes_normally() {
    // Task without children should complete as before (backward compat).
    let sched = setup(arc_erased(InstantExecutor)).await;

    sched
        .submit(&TaskSubmission::new("test").key("no-kids"))
        .await
        .unwrap();

    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let key = crate::task::generate_dedup_key("test", Some(b"no-kids"));
    let lookup = sched.store().task_lookup(&key).await.unwrap();
    assert!(matches!(lookup, crate::task::TaskLookup::History(_)));
}

// ── Byte-level progress tests ─────────────────────────────────────

/// An executor that reports byte-level progress incrementally.
struct ByteProgressExecutor;

impl TaskExecutor for ByteProgressExecutor {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        ctx.set_bytes_total(1_048_576);
        for _ in 0..1024 {
            ctx.add_bytes(1024);
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        Ok(())
    }
}

#[tokio::test]
async fn byte_progress_events_received() {
    let store = TaskStore::open_memory().await.unwrap();
    let mut registry = TaskTypeRegistry::new();
    registry.register_erased("byte-test", arc_erased(ByteProgressExecutor));

    let config = SchedulerConfig {
        progress_interval: Some(Duration::from_millis(50)),
        ..Default::default()
    };

    let sched = Scheduler::new(
        store,
        config,
        Arc::new(registry),
        CompositePressure::new(),
        ThrottlePolicy::default_three_tier(),
    );

    let mut progress_rx = sched.subscribe_progress();

    sched
        .submit(&TaskSubmission::new("byte-test").key("bp1"))
        .await
        .unwrap();

    // Run the scheduler.
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Collect progress events.
    let mut events: Vec<TaskProgress> = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Ok(evt)) = tokio::time::timeout(Duration::from_millis(200), progress_rx.recv()).await {
            events.push(evt);
            if events.len() >= 3 {
                break;
            }
        }
    }

    token.cancel();
    let _ = handle.await;

    // Should have received at least a few events.
    assert!(
        events.len() >= 2,
        "expected at least 2 progress events, got {}",
        events.len()
    );

    // bytes_completed should be increasing.
    for window in events.windows(2) {
        assert!(window[1].bytes_completed >= window[0].bytes_completed);
    }

    // bytes_total should be set.
    assert_eq!(events[0].bytes_total, Some(1_048_576));

    // Later events should have non-zero throughput.
    let last = events.last().unwrap();
    assert!(last.throughput_bps > 0.0, "expected non-zero throughput");
}

#[tokio::test]
async fn lifecycle_events_not_polluted_by_byte_progress() {
    let store = TaskStore::open_memory().await.unwrap();
    let mut registry = TaskTypeRegistry::new();
    registry.register_erased("byte-test", arc_erased(ByteProgressExecutor));

    let config = SchedulerConfig {
        progress_interval: Some(Duration::from_millis(50)),
        ..Default::default()
    };

    let sched = Scheduler::new(
        store,
        config,
        Arc::new(registry),
        CompositePressure::new(),
        ThrottlePolicy::default_three_tier(),
    );

    let mut lifecycle_rx = sched.subscribe();

    sched
        .submit(&TaskSubmission::new("byte-test").key("bp-lifecycle"))
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Collect lifecycle events until Completed.
    let mut lifecycle_events = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Ok(evt)) = tokio::time::timeout(Duration::from_millis(200), lifecycle_rx.recv()).await {
            let is_completed = matches!(evt, SchedulerEvent::Completed(..));
            lifecycle_events.push(evt);
            if is_completed {
                break;
            }
        }
    }

    token.cancel();
    let _ = handle.await;

    // Lifecycle events should only be Dispatched + Completed (no byte-level progress).
    // There may be percent-based Progress events too, but no TaskProgress type in
    // the lifecycle channel.
    for evt in &lifecycle_events {
        assert!(
            matches!(
                evt,
                SchedulerEvent::Dispatched(..)
                    | SchedulerEvent::Completed(..)
                    | SchedulerEvent::Progress { .. }
            ),
            "unexpected lifecycle event: {evt:?}"
        );
    }
}

#[tokio::test]
async fn byte_progress_in_snapshot() {
    let store = TaskStore::open_memory().await.unwrap();
    let mut registry = TaskTypeRegistry::new();
    registry.register_erased("byte-test", arc_erased(ByteProgressExecutor));

    let sched = Scheduler::new(
        store,
        SchedulerConfig::default(),
        Arc::new(registry),
        CompositePressure::new(),
        ThrottlePolicy::default_three_tier(),
    );

    sched
        .submit(&TaskSubmission::new("byte-test").key("bp-snap"))
        .await
        .unwrap();

    sched.try_dispatch().await.unwrap();
    // Let the executor run a bit so bytes are reported.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let snap = sched.snapshot().await.unwrap();
    assert!(
        !snap.byte_progress.is_empty(),
        "expected byte_progress in snapshot"
    );
    assert_eq!(snap.byte_progress[0].bytes_total, Some(1_048_576));
    assert!(snap.byte_progress[0].bytes_completed > 0);
}
