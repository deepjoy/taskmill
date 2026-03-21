use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::domain::{Domain, DomainKey, TypedExecutor};
use crate::priority::Priority;
use crate::registry::TaskContext;
use crate::store::TaskStore;
use crate::task::{
    DuplicateStrategy, HistoryStatus, SubmitOutcome, TaskError, TaskSubmission, TypedTask,
};

use super::{Scheduler, SchedulerEvent, TaskProgress};

// ── Domain & task type definitions ──────────────────────────────────

struct TestDomain;
impl DomainKey for TestDomain {
    const NAME: &'static str = "test";
}

struct ParentDomain;
impl DomainKey for ParentDomain {
    const NAME: &'static str = "parent";
}

// ChildTask lives in ParentDomain so spawn_child (which prefixes with
// the owning module) produces "parent::child" — matching the registration.

struct ByteTestDomain;
impl DomainKey for ByteTestDomain {
    const NAME: &'static str = "byte-test";
}

struct AlphaDomain;
impl DomainKey for AlphaDomain {
    const NAME: &'static str = "alpha";
}

struct BetaDomain;
impl DomainKey for BetaDomain {
    const NAME: &'static str = "beta";
}

struct UnknownDomain;
impl DomainKey for UnknownDomain {
    const NAME: &'static str = "unknown";
}

#[derive(Serialize, Deserialize)]
struct TestTask;
impl TypedTask for TestTask {
    type Domain = TestDomain;
    const TASK_TYPE: &'static str = "test";
}

#[derive(Serialize, Deserialize)]
struct ParentTask;
impl TypedTask for ParentTask {
    type Domain = ParentDomain;
    const TASK_TYPE: &'static str = "parent";
}

#[derive(Serialize, Deserialize)]
struct ChildTask;
impl TypedTask for ChildTask {
    type Domain = ParentDomain;
    const TASK_TYPE: &'static str = "child";
}

#[derive(Serialize, Deserialize)]
struct ByteTestTask;
impl TypedTask for ByteTestTask {
    type Domain = ByteTestDomain;
    const TASK_TYPE: &'static str = "byte-test";
}

#[derive(Serialize, Deserialize)]
struct AlphaTask;
impl TypedTask for AlphaTask {
    type Domain = AlphaDomain;
    const TASK_TYPE: &'static str = "alpha";
}

#[derive(Serialize, Deserialize)]
struct BetaTask;
impl TypedTask for BetaTask {
    type Domain = BetaDomain;
    const TASK_TYPE: &'static str = "beta";
}

// ── Executors ───────────────────────────────────────────────────────

struct InstantExecutor;

impl<T: TypedTask> TypedExecutor<T> for InstantExecutor {
    async fn execute<'a>(&'a self, _payload: T, ctx: &'a TaskContext) -> Result<(), TaskError> {
        ctx.record_read_bytes(100);
        ctx.record_write_bytes(50);
        Ok(())
    }
}

struct SlowExecutor;

impl<T: TypedTask> TypedExecutor<T> for SlowExecutor {
    async fn execute<'a>(&'a self, _payload: T, ctx: &'a TaskContext) -> Result<(), TaskError> {
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

/// An executor that tracks whether its on_cancel hook was called.
struct CancelHookExecutor {
    cancel_called: Arc<std::sync::atomic::AtomicBool>,
}

impl TypedExecutor<TestTask> for CancelHookExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: TestTask,
        ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        tokio::select! {
            _ = ctx.token().cancelled() => {
                Err(TaskError::new("cancelled"))
            }
            _ = tokio::time::sleep(Duration::from_secs(60)) => {
                Ok(())
            }
        }
    }

    async fn on_cancel<'a>(
        &'a self,
        _payload: TestTask,
        _ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        self.cancel_called
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[allow(dead_code)]
struct FailingExecutor;

impl TypedExecutor<TestTask> for FailingExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: TestTask,
        _ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        Err(TaskError::retryable("boom"))
    }
}

async fn setup(executor: impl TypedExecutor<TestTask>) -> Scheduler {
    Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(executor))
        .build()
        .await
        .unwrap()
}

/// Helper: create a test-domain submission with a serialized `TestTask` payload.
fn test_sub(key: &str) -> TaskSubmission {
    TaskSubmission::new("test::test")
        .key(key)
        .payload_json(&TestTask)
}

#[tokio::test]
async fn dispatch_executes_task() {
    let sched = setup(InstantExecutor).await;

    sched.submit(&test_sub("k1")).await.unwrap();

    let dispatched = sched.try_dispatch().await.unwrap();
    assert!(dispatched);

    // Give spawned task time to complete.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Task should be completed and in history.
    let k1 = crate::task::generate_dedup_key("test::test", Some(b"k1"));
    assert!(sched.store().task_by_key(&k1).await.unwrap().is_none());
    let hist = sched.store().history_by_key(&k1).await.unwrap();
    assert_eq!(hist.len(), 1);
}

#[tokio::test]
async fn dispatch_returns_false_when_empty() {
    let sched = setup(InstantExecutor).await;
    let dispatched = sched.try_dispatch().await.unwrap();
    assert!(!dispatched);
}

#[tokio::test]
async fn unregistered_type_fails_task() {
    let store = TaskStore::open_memory().await.unwrap();
    // Empty domain — no executors registered for "unknown::unknown".
    let sched = Scheduler::builder()
        .store(store)
        .domain(Domain::<UnknownDomain>::new())
        .build()
        .await
        .unwrap();

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
    let sched = setup(InstantExecutor).await;

    let sub = test_sub("dup");

    let first = sched.submit(&sub).await.unwrap();
    let second = sched.submit(&sub).await.unwrap();
    assert!(first.is_inserted());
    assert_eq!(second, SubmitOutcome::Duplicate);
}

#[tokio::test]
async fn set_max_concurrency_works() {
    let sched = setup(InstantExecutor).await;
    assert_eq!(sched.max_concurrency(), 4);
    sched.set_max_concurrency(8);
    assert_eq!(sched.max_concurrency(), 8);
}

#[tokio::test]
async fn cancel_pending_task() {
    let sched = setup(InstantExecutor).await;

    let id = sched
        .submit(&test_sub("cancel-me"))
        .await
        .unwrap()
        .id()
        .unwrap();

    let cancelled = sched.cancel(id).await.unwrap();
    assert!(cancelled);

    // Task should be gone.
    let cancel_key = crate::task::generate_dedup_key("test::test", Some(b"cancel-me"));
    assert!(sched
        .store()
        .task_by_key(&cancel_key)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn cancel_running_task() {
    let sched = setup(SlowExecutor).await;

    let id = sched
        .submit(&test_sub("cancel-running"))
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
    let sched = setup(InstantExecutor).await;
    let mut rx = sched.subscribe();

    sched.submit(&test_sub("evt")).await.unwrap();

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
    let sched = setup(InstantExecutor).await;
    let sched2 = sched.clone();

    // Both should share the same store.
    sched.submit(&test_sub("shared")).await.unwrap();

    // The clone can see the task.
    let shared_key = crate::task::generate_dedup_key("test::test", Some(b"shared"));
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

    struct TestDomain2;
    impl crate::domain::DomainKey for TestDomain2 {
        const NAME: &'static str = "test";
    }

    impl crate::task::TypedTask for Thumb {
        type Domain = TestDomain2;
        const TASK_TYPE: &'static str = "test";

        fn config() -> crate::domain::TaskTypeConfig {
            crate::domain::TaskTypeConfig::new().expected_io(crate::task::IoBudget::disk(4096, 512))
        }
    }

    let sched = setup(InstantExecutor).await;

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
    let sched = setup(SlowExecutor).await;

    // Submit two tasks.
    for key in &["snap-a", "snap-b"] {
        sched.submit(&test_sub(key)).await.unwrap();
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
    let sched = setup(SlowExecutor).await;

    // Submit two tasks.
    for key in &["pa-1", "pa-2"] {
        sched.submit(&test_sub(key)).await.unwrap();
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
    let sched = setup(InstantExecutor).await;
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

    impl TypedExecutor<TestTask> for StateCheckExecutor {
        async fn execute<'a>(
            &'a self,
            _payload: TestTask,
            ctx: &'a TaskContext,
        ) -> Result<(), TaskError> {
            let state = ctx.state::<MyState>().expect("state should be set");
            state.flag.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    let flag = Arc::new(AtomicBool::new(false));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(StateCheckExecutor))
        .app_state(MyState { flag: flag.clone() })
        .build()
        .await
        .unwrap();

    sched.submit(&test_sub("state-test")).await.unwrap();

    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(flag.load(Ordering::SeqCst));
}

#[tokio::test]
async fn task_lookup_pending() {
    let sched = setup(InstantExecutor).await;

    sched.submit(&test_sub("lookup-1")).await.unwrap();

    let result = sched
        .task_lookup("test::test", Some(b"lookup-1"))
        .await
        .unwrap();
    assert!(matches!(
        result,
        crate::task::TaskLookup::Active(ref r) if r.status == crate::task::TaskStatus::Pending
    ));
}

#[tokio::test]
async fn task_lookup_completed() {
    let sched = setup(InstantExecutor).await;

    sched.submit(&test_sub("lookup-done")).await.unwrap();

    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = sched
        .task_lookup("test::test", Some(b"lookup-done"))
        .await
        .unwrap();
    assert!(matches!(result, crate::task::TaskLookup::History(_)));
}

#[tokio::test]
async fn task_lookup_not_found() {
    let sched = setup(InstantExecutor).await;
    let result = sched
        .task_lookup("test::test", Some(b"does-not-exist"))
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

    struct TestDomain2;
    impl crate::domain::DomainKey for TestDomain2 {
        const NAME: &'static str = "test";
    }

    impl crate::task::TypedTask for Thumb {
        type Domain = TestDomain2;
        const TASK_TYPE: &'static str = "test";
    }

    let sched = setup(InstantExecutor).await;

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

impl TypedExecutor<ParentTask> for SpawningExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: ParentTask,
        ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        for i in 0..self.num_children {
            let sub = TaskSubmission::new("child")
                .key(format!("child-{i}"))
                .priority(ctx.record().priority)
                .payload_json(&ChildTask);
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

impl TypedExecutor<ParentTask> for FinalizeTrackingExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: ParentTask,
        ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        for i in 0..self.children {
            let sub = TaskSubmission::new("child")
                .key(format!("ft-child-{i}"))
                .priority(ctx.record().priority)
                .payload_json(&ChildTask);
            ctx.spawn_child(sub).await?;
        }
        Ok(())
    }

    async fn finalize<'a>(
        &'a self,
        _payload: ParentTask,
        _ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        self.finalized
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn parent_enters_waiting_when_children_spawned() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<ParentDomain>::new()
                .task::<ParentTask>(SpawningExecutor { num_children: 2 })
                .task::<ChildTask>(InstantExecutor),
        )
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    // Submit parent task.
    sched
        .submit(
            &TaskSubmission::new("parent::parent")
                .key("p1")
                .payload_json(&ParentTask),
        )
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
    let parent_key = crate::task::generate_dedup_key("parent::parent", Some(b"p1"));
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
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<ParentDomain>::new()
                .task::<ParentTask>(SpawningExecutor { num_children: 2 })
                .task::<ChildTask>(InstantExecutor),
        )
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    sched
        .submit(
            &TaskSubmission::new("parent::parent")
                .key("p-complete")
                .payload_json(&ParentTask),
        )
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
    let parent_key = crate::task::generate_dedup_key("parent::parent", Some(b"p-complete"));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut parent_completed = false;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(SchedulerEvent::Completed(ref h))) if h.task_type == "parent::parent" => {
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

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<ParentDomain>::new()
                .task::<ParentTask>(FinalizeTrackingExecutor {
                    children: 1,
                    finalized: finalized.clone(),
                })
                .task::<ChildTask>(InstantExecutor),
        )
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    sched
        .submit(
            &TaskSubmission::new("parent::parent")
                .key("p-finalize")
                .payload_json(&ParentTask),
        )
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
            Ok(Ok(SchedulerEvent::Completed(ref h))) if h.task_type == "parent::parent" => {
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
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<ParentDomain>::new()
                .task::<ParentTask>(SpawningExecutor { num_children: 3 })
                .task::<ChildTask>(SlowExecutor),
        )
        .build()
        .await
        .unwrap();

    let parent_id = sched
        .submit(
            &TaskSubmission::new("parent::parent")
                .key("p-cancel")
                .payload_json(&ParentTask),
        )
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
    let sched = setup(InstantExecutor).await;

    sched.submit(&test_sub("no-kids")).await.unwrap();

    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let key = crate::task::generate_dedup_key("test::test", Some(b"no-kids"));
    let lookup = sched.store().task_lookup(&key).await.unwrap();
    assert!(matches!(lookup, crate::task::TaskLookup::History(_)));
}

// ── Byte-level progress tests ─────────────────────────────────────

/// An executor that reports byte-level progress incrementally.
struct ByteProgressExecutor;

impl TypedExecutor<ByteTestTask> for ByteProgressExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: ByteTestTask,
        ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        ctx.set_bytes_total(1_048_576);
        for _ in 0..1024 {
            ctx.add_bytes(1024);
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        Ok(())
    }
}

/// Helper: create a byte-test submission with a serialized payload.
fn byte_test_sub(key: &str) -> TaskSubmission {
    TaskSubmission::new("byte-test::byte-test")
        .key(key)
        .payload_json(&ByteTestTask)
}

#[tokio::test]
async fn byte_progress_events_received() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<ByteTestDomain>::new().task::<ByteTestTask>(ByteProgressExecutor))
        .progress_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let mut progress_rx = sched.subscribe_progress();

    sched.submit(&byte_test_sub("bp1")).await.unwrap();

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
        if let Ok(Ok(evt)) =
            tokio::time::timeout(Duration::from_millis(200), progress_rx.recv()).await
        {
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
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<ByteTestDomain>::new().task::<ByteTestTask>(ByteProgressExecutor))
        .progress_interval(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let mut lifecycle_rx = sched.subscribe();

    sched.submit(&byte_test_sub("bp-lifecycle")).await.unwrap();

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
        if let Ok(Ok(evt)) =
            tokio::time::timeout(Duration::from_millis(200), lifecycle_rx.recv()).await
        {
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
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<ByteTestDomain>::new().task::<ByteTestTask>(ByteProgressExecutor))
        .build()
        .await
        .unwrap();

    sched.submit(&byte_test_sub("bp-snap")).await.unwrap();

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

#[tokio::test]
async fn batch_submitted_event() {
    let sched = setup(InstantExecutor).await;
    let mut rx = sched.subscribe();

    let subs: Vec<_> = (0..3)
        .map(|i| {
            TaskSubmission::new("test::test")
                .key(format!("ev-{i}"))
                .payload_json(&TestTask)
        })
        .collect();

    let outcome = sched.submit_batch(&subs).await.unwrap();
    assert_eq!(outcome.len(), 3);
    assert_eq!(outcome.inserted().len(), 3);
    assert_eq!(outcome.duplicated_count(), 0);

    // The BatchSubmitted event should be receivable.
    let event = rx.try_recv().unwrap();
    match event {
        SchedulerEvent::BatchSubmitted {
            count,
            inserted_ids,
        } => {
            assert_eq!(count, 3);
            assert_eq!(inserted_ids.len(), 3);
        }
        other => panic!("expected BatchSubmitted, got {other:?}"),
    }
}

#[tokio::test]
async fn batch_outcome_convenience_methods() {
    let sched = setup(InstantExecutor).await;

    // Submit one task first so re-submitting it produces a Duplicate.
    sched.submit(&test_sub("existing")).await.unwrap();

    let subs = vec![test_sub("new-1"), test_sub("existing"), test_sub("new-2")];

    let outcome = sched.submit_batch(&subs).await.unwrap();
    assert_eq!(outcome.len(), 3);
    assert_eq!(outcome.inserted().len(), 2);
    assert_eq!(outcome.duplicated_count(), 1);
    assert!(outcome.upgraded().is_empty());
    assert!(outcome.requeued().is_empty());
}

#[tokio::test]
async fn submit_built_applies_defaults() {
    use crate::task::BatchSubmission;

    let sched = setup(InstantExecutor).await;

    let batch = BatchSubmission::new()
        .default_group("g1")
        .default_priority(Priority::HIGH)
        .task(test_sub("built-1"))
        .task(test_sub("built-2"));

    let outcome = sched.submit_built(batch).await.unwrap();
    assert_eq!(outcome.inserted().len(), 2);
}

// ── Cancellation with history tests ──────────────────────────────

#[tokio::test]
async fn cancel_pending_records_history() {
    let sched = setup(InstantExecutor).await;

    let id = sched
        .submit(&test_sub("cancel-hist"))
        .await
        .unwrap()
        .id()
        .unwrap();

    let cancelled = sched.cancel(id).await.unwrap();
    assert!(cancelled);

    // Task should be gone from active queue.
    let key = crate::task::generate_dedup_key("test::test", Some(b"cancel-hist"));
    assert!(sched.store().task_by_key(&key).await.unwrap().is_none());

    // History should have a cancelled entry.
    let hist = sched.store().history_by_key(&key).await.unwrap();
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].status, crate::task::HistoryStatus::Cancelled);
}

#[tokio::test]
async fn cancel_running_records_history_and_fires_hook() {
    let cancel_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let executor = CancelHookExecutor {
        cancel_called: cancel_called.clone(),
    };

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(executor))
        .build()
        .await
        .unwrap();

    let id = sched
        .submit(&test_sub("cancel-running-hist"))
        .await
        .unwrap()
        .id()
        .unwrap();

    // Dispatch it so it's running.
    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let cancelled = sched.cancel(id).await.unwrap();
    assert!(cancelled);

    // Give the on_cancel hook time to fire.
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        cancel_called.load(std::sync::atomic::Ordering::SeqCst),
        "on_cancel hook should have been called"
    );

    // History should have a cancelled entry.
    let key = crate::task::generate_dedup_key("test::test", Some(b"cancel-running-hist"));
    let hist = sched.store().history_by_key(&key).await.unwrap();
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].status, crate::task::HistoryStatus::Cancelled);
}

#[tokio::test]
async fn cancel_parent_cascade_records_history() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<ParentDomain>::new()
                .task::<ParentTask>(SpawningExecutor { num_children: 2 })
                .task::<ChildTask>(SlowExecutor),
        )
        .build()
        .await
        .unwrap();

    let parent_id = sched
        .submit(
            &TaskSubmission::new("parent::parent")
                .key("p-cancel-hist")
                .payload_json(&ParentTask),
        )
        .await
        .unwrap()
        .id()
        .unwrap();

    // Dispatch parent (which spawns children).
    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Cancel parent — should cascade to children.
    sched.cancel(parent_id).await.unwrap();

    // All tasks should be recorded as cancelled in history.
    let hist = sched.store().history(100, 0).await.unwrap();
    assert!(
        !hist.is_empty(),
        "expected at least parent in history, got {}",
        hist.len()
    );
    for h in &hist {
        assert_eq!(
            h.status,
            crate::task::HistoryStatus::Cancelled,
            "expected cancelled status for task {}, got {:?}",
            h.task_type,
            h.status
        );
    }
}

#[tokio::test]
async fn check_cancelled_returns_error() {
    use crate::task::TaskError;
    let err = TaskError::cancelled();
    assert!(err.is_cancelled());
    assert!(!err.retryable);
}

#[tokio::test]
async fn cancel_group_cancels_matching_tasks() {
    let sched = setup(InstantExecutor).await;

    // Submit tasks in different groups.
    sched
        .submit(&test_sub("g-a1").group("group-a"))
        .await
        .unwrap();
    sched
        .submit(&test_sub("g-a2").group("group-a"))
        .await
        .unwrap();
    sched
        .submit(&test_sub("g-b1").group("group-b"))
        .await
        .unwrap();

    let cancelled = sched.cancel_group("group-a").await.unwrap();
    assert_eq!(cancelled.len(), 2);

    // group-b task should still exist.
    assert_eq!(sched.store().pending_count().await.unwrap(), 1);
}

#[tokio::test]
async fn cancel_type_cancels_matching_tasks() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<AlphaDomain>::new().task::<AlphaTask>(InstantExecutor))
        .domain(Domain::<BetaDomain>::new().task::<BetaTask>(InstantExecutor))
        .build()
        .await
        .unwrap();

    sched
        .submit(
            &TaskSubmission::new("alpha::alpha")
                .key("a1")
                .payload_json(&AlphaTask),
        )
        .await
        .unwrap();
    sched
        .submit(
            &TaskSubmission::new("alpha::alpha")
                .key("a2")
                .payload_json(&AlphaTask),
        )
        .await
        .unwrap();
    sched
        .submit(
            &TaskSubmission::new("beta::beta")
                .key("b1")
                .payload_json(&BetaTask),
        )
        .await
        .unwrap();

    let cancelled = sched.cancel_type("alpha::alpha").await.unwrap();
    assert_eq!(cancelled.len(), 2);

    // beta task should still exist.
    assert_eq!(sched.store().pending_count().await.unwrap(), 1);
}

#[tokio::test]
async fn cancel_where_filters_correctly() {
    let sched = setup(InstantExecutor).await;

    for i in 0..5 {
        sched.submit(&test_sub(&format!("cw-{i}"))).await.unwrap();
    }

    // Cancel only tasks whose key contains "cw-3" or "cw-4".
    let cancelled = sched
        .cancel_where(|r| r.label == "cw-3" || r.label == "cw-4")
        .await
        .unwrap();
    assert_eq!(cancelled.len(), 2);
    assert_eq!(sched.store().pending_count().await.unwrap(), 3);
}

#[tokio::test]
async fn on_cancel_hook_timeout_does_not_block() {
    struct SlowCancelExecutor;

    impl TypedExecutor<TestTask> for SlowCancelExecutor {
        async fn execute<'a>(
            &'a self,
            _payload: TestTask,
            ctx: &'a TaskContext,
        ) -> Result<(), TaskError> {
            tokio::select! {
                _ = ctx.token().cancelled() => Err(TaskError::new("cancelled")),
                _ = tokio::time::sleep(Duration::from_secs(60)) => Ok(()),
            }
        }

        async fn on_cancel<'a>(
            &'a self,
            _payload: TestTask,
            _ctx: &'a TaskContext,
        ) -> Result<(), TaskError> {
            // Simulate a very slow cancel hook.
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok(())
        }
    }

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(SlowCancelExecutor))
        .cancel_hook_timeout(Duration::from_millis(50))
        .build()
        .await
        .unwrap();

    let id = sched
        .submit(&test_sub("timeout-hook"))
        .await
        .unwrap()
        .id()
        .unwrap();

    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Cancel should return quickly even though the hook is slow.
    let start = std::time::Instant::now();
    sched.cancel(id).await.unwrap();
    let elapsed = start.elapsed();

    // The cancel itself should be fast (hook is fire-and-forget).
    assert!(
        elapsed < Duration::from_secs(1),
        "cancel took too long: {elapsed:?}"
    );

    // Give the hook time to timeout.
    tokio::time::sleep(Duration::from_millis(100)).await;
}

// ── Superseding tests ───────────────────────────────────────────────

#[tokio::test]
async fn reject_returns_rejected() {
    let sched = setup(InstantExecutor).await;

    let sub = test_sub("dup").on_duplicate(DuplicateStrategy::Reject);
    let first = sched.submit(&sub).await.unwrap();
    assert!(first.is_inserted());

    let second = sched.submit(&sub).await.unwrap();
    assert_eq!(second, SubmitOutcome::Rejected);
}

#[tokio::test]
async fn supersede_pending_replaces_in_place() {
    let sched = setup(InstantExecutor).await;

    // Submit initial task.
    let sub1 = TaskSubmission::new("test::test")
        .key("replace-me")
        .priority(Priority::NORMAL)
        .payload_raw(b"old".to_vec());
    let first = sched.submit(&sub1).await.unwrap();
    let first_id = first.id().unwrap();

    // Supersede with new payload and higher priority.
    let sub2 = TaskSubmission::new("test::test")
        .key("replace-me")
        .priority(Priority::HIGH)
        .payload_raw(b"new".to_vec())
        .on_duplicate(DuplicateStrategy::Supersede);
    let outcome = sched.submit(&sub2).await.unwrap();

    // Pending supersede uses in-place update — same row ID.
    assert!(
        matches!(outcome, SubmitOutcome::Superseded { new_task_id, replaced_task_id } if new_task_id == first_id && replaced_task_id == first_id)
    );

    // Old task should be in history as superseded.
    let key = sub1.effective_key();
    let history = sched.store().history_by_key(&key).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, HistoryStatus::Superseded);

    // Active task should have new payload and priority.
    let task = sched.store().task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(task.priority, Priority::HIGH);
    assert_eq!(task.payload.as_deref(), Some(b"new".as_slice()));
}

#[tokio::test]
async fn supersede_running_cancels_and_inserts_new() {
    let cancel_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let executor = CancelHookExecutor {
        cancel_called: cancel_called.clone(),
    };
    let sched = setup(executor).await;

    // Submit and dispatch (now running).
    let sub1 = test_sub("running-sup");
    sched.submit(&sub1).await.unwrap();
    sched.try_dispatch().await.unwrap();

    // Give task time to start.
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Supersede the running task.
    let sub2 = TaskSubmission::new("test::test")
        .key("running-sup")
        .payload_raw(b"replacement".to_vec())
        .on_duplicate(DuplicateStrategy::Supersede);
    let outcome = sched.submit(&sub2).await.unwrap();

    assert!(
        matches!(outcome, SubmitOutcome::Superseded { .. }),
        "expected Superseded, got: {outcome:?}"
    );

    // on_cancel hook should have been fired.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        cancel_called.load(std::sync::atomic::Ordering::SeqCst),
        "on_cancel hook should fire on supersede"
    );

    // Old task should be in history as superseded.
    let key = sub1.effective_key();
    let history = sched.store().history_by_key(&key).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, HistoryStatus::Superseded);

    // New task should be pending in the queue.
    let task = sched.store().task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(task.status, crate::task::TaskStatus::Pending);
    assert_eq!(task.payload.as_deref(), Some(b"replacement".as_slice()));
}

#[tokio::test]
async fn supersede_emits_event() {
    let sched = setup(InstantExecutor).await;
    let mut rx = sched.subscribe();

    let sub1 = test_sub("evt");
    sched.submit(&sub1).await.unwrap();

    let sub2 = test_sub("evt").on_duplicate(DuplicateStrategy::Supersede);
    sched.submit(&sub2).await.unwrap();

    // Drain events and look for Superseded.
    let mut found = false;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, SchedulerEvent::Superseded { .. }) {
            found = true;
        }
    }
    assert!(found, "expected Superseded event");
}

#[tokio::test]
async fn supersede_in_batch() {
    let sched = setup(InstantExecutor).await;

    // Pre-submit a task.
    let sub1 = test_sub("batch-sup");
    sched.submit(&sub1).await.unwrap();

    // Batch supersede it.
    let sub2 = TaskSubmission::new("test::test")
        .key("batch-sup")
        .payload_raw(b"batch-new".to_vec())
        .on_duplicate(DuplicateStrategy::Supersede);
    let outcome = sched.submit_batch(&[sub2]).await.unwrap();

    assert!(matches!(
        outcome.outcomes[0],
        SubmitOutcome::Superseded { .. }
    ));

    let key = sub1.effective_key();
    let history = sched.store().history_by_key(&key).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, HistoryStatus::Superseded);
}

#[tokio::test]
async fn chain_of_supersedes() {
    let sched = setup(InstantExecutor).await;

    // A supersedes nothing (fresh insert).
    let sub_a = TaskSubmission::new("test::test")
        .key("chain")
        .payload_raw(b"A".to_vec());
    let out_a = sched.submit(&sub_a).await.unwrap();
    assert!(matches!(out_a, SubmitOutcome::Inserted(_)));

    // B supersedes A.
    let sub_b = TaskSubmission::new("test::test")
        .key("chain")
        .payload_raw(b"B".to_vec())
        .on_duplicate(DuplicateStrategy::Supersede);
    let out_b = sched.submit(&sub_b).await.unwrap();
    assert!(matches!(out_b, SubmitOutcome::Superseded { .. }));

    // C supersedes B.
    let sub_c = TaskSubmission::new("test::test")
        .key("chain")
        .payload_raw(b"C".to_vec())
        .on_duplicate(DuplicateStrategy::Supersede);
    let out_c = sched.submit(&sub_c).await.unwrap();
    assert!(matches!(out_c, SubmitOutcome::Superseded { .. }));

    // History should have 2 superseded entries (A and B).
    let key = sub_a.effective_key();
    let history = sched.store().history_by_key(&key).await.unwrap();
    assert_eq!(history.len(), 2);
    assert!(history
        .iter()
        .all(|h| h.status == HistoryStatus::Superseded));

    // Active queue should have the final task with payload C.
    let task = sched.store().task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(task.payload.as_deref(), Some(b"C".as_slice()));
}

// ── Phase 5: Dead-letter integration tests ──────────────────────

#[tokio::test]
async fn retry_dead_letter_resubmits_with_reset_retry_count() {
    // Use max_retries=0 so a retryable failure immediately dead-letters.
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(FailingExecutor))
        .max_retries(0)
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    // Submit and dispatch — will fail with retryable error and dead-letter.
    sched.submit(&test_sub("dl-retry")).await.unwrap();

    sched.try_dispatch().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Should have emitted a DeadLettered event.
    let mut got_dead_lettered = false;
    while let Ok(evt) = rx.try_recv() {
        if matches!(evt, SchedulerEvent::DeadLettered { .. }) {
            got_dead_lettered = true;
        }
    }
    assert!(got_dead_lettered, "expected DeadLettered event");

    // Should be in dead_letter_tasks.
    let dl = sched.dead_letter_tasks(10, 0).await.unwrap();
    assert_eq!(dl.len(), 1);
    assert_eq!(dl[0].status, HistoryStatus::DeadLetter);
    let history_id = dl[0].id;
    assert_eq!(dl[0].retry_count, 1); // retry_count was incremented

    // Now re-submit from dead-letter. Replace the executor with one that
    // succeeds so the re-submitted task can complete.
    // (retry_dead_letter only re-submits — it doesn't dispatch.)
    let outcome = sched.retry_dead_letter(history_id).await.unwrap();
    assert!(outcome.is_inserted());

    // Dead-letter history row should be removed.
    let dl_after = sched.dead_letter_tasks(10, 0).await.unwrap();
    assert!(dl_after.is_empty());

    // New task should be in the active queue with retry_count=0.
    let key = crate::task::generate_dedup_key("test::test", Some(b"dl-retry"));
    let task = sched.store().task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(task.retry_count, 0);
}
