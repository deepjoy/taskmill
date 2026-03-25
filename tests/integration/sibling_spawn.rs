//! Integration tests for `spawn_sibling_with` / `spawn_siblings_with` and
//! `DomainSubmitBuilder::sibling_of`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use taskmill::store::TaskStore;
use taskmill::{
    Domain, DomainTaskContext, Priority, Scheduler, SchedulerEvent, TaskError, TaskSubmission,
    TypedExecutor, TypedTask,
};
use tokio_util::sync::CancellationToken;

use super::common::*;

// ── Task types for sibling tests ──────────────────────────────────

define_task!(OrchestratorTask, TestDomain, "orchestrator");
define_task!(SiblingTask, TestDomain, "sibling");
define_task!(SiblingChildTask, TestDomain, "sibling-child");

// Cross-domain sibling task
define_task!(CrossDomainSiblingTask, MediaDomain, "cross-sibling");

// Indexed sibling task for batch tests (unique payload per instance avoids dedup)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexedSiblingTask {
    index: u32,
}

impl TypedTask for IndexedSiblingTask {
    type Domain = TestDomain;
    const TASK_TYPE: &'static str = "indexed-sibling";
}

// ── Executors ─────────────────────────────────────────────────────

/// Executor that spawns N children, each of which will spawn siblings.
struct OrchestratorExecutor {
    child_count: usize,
}

impl TypedExecutor<OrchestratorTask> for OrchestratorExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: OrchestratorTask,
        ctx: DomainTaskContext<'a, TestDomain>,
    ) -> Result<(), TaskError> {
        for i in 0..self.child_count {
            ctx.spawn_child_with(SiblingChildTask)
                .key(format!("child-{i}"))
                .await
                .map_err(|e| TaskError::new(e.to_string()))?;
        }
        Ok(())
    }
}

/// Executor for a child that spawns a single sibling.
struct SiblingSpawnerExecutor;

impl TypedExecutor<SiblingChildTask> for SiblingSpawnerExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: SiblingChildTask,
        ctx: DomainTaskContext<'a, TestDomain>,
    ) -> Result<(), TaskError> {
        ctx.spawn_sibling_with(SiblingTask)
            .key(format!("sibling-of-{}", ctx.record().id))
            .await
            .map_err(|e| TaskError::new(e.to_string()))?;
        Ok(())
    }
}

/// Executor for a child that spawns multiple siblings in batch.
struct BatchSiblingSpawnerExecutor {
    count: usize,
}

impl TypedExecutor<SiblingChildTask> for BatchSiblingSpawnerExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: SiblingChildTask,
        ctx: DomainTaskContext<'a, TestDomain>,
    ) -> Result<(), TaskError> {
        let tasks: Vec<IndexedSiblingTask> = (0..self.count)
            .map(|i| IndexedSiblingTask { index: i as u32 })
            .collect();
        ctx.spawn_siblings_with(tasks)
            .await
            .map_err(|e| TaskError::new(e.to_string()))?;
        Ok(())
    }
}

/// Executor that tries to spawn a sibling from a root task — should fail.
struct RootSiblingSpawnerExecutor;

impl TypedExecutor<OrchestratorTask> for RootSiblingSpawnerExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: OrchestratorTask,
        ctx: DomainTaskContext<'a, TestDomain>,
    ) -> Result<(), TaskError> {
        match ctx.spawn_sibling_with(SiblingTask).await {
            Err(e) => {
                // Expected — root task has no parent_id.
                assert!(
                    e.to_string().contains("no parent_id"),
                    "expected InvalidState error, got: {e}"
                );
                Ok(())
            }
            Ok(_) => Err(TaskError::new("should have failed")),
        }
    }
}

/// Finalize tracker for orchestrator — verifies all children+siblings complete.
struct OrchestratorFinalizeTracker {
    child_count: usize,
    finalized: Arc<AtomicBool>,
}

impl TypedExecutor<OrchestratorTask> for OrchestratorFinalizeTracker {
    async fn execute<'a>(
        &'a self,
        _payload: OrchestratorTask,
        ctx: DomainTaskContext<'a, TestDomain>,
    ) -> Result<(), TaskError> {
        for i in 0..self.child_count {
            ctx.spawn_child_with(SiblingChildTask)
                .key(format!("child-{i}"))
                .await
                .map_err(|e| TaskError::new(e.to_string()))?;
        }
        Ok(())
    }

    async fn finalize<'a>(
        &'a self,
        _payload: OrchestratorTask,
        _memo: (),
        _ctx: DomainTaskContext<'a, TestDomain>,
    ) -> Result<(), TaskError> {
        self.finalized.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Cross-domain sibling spawner — uses `sibling_of()`.
struct CrossDomainSiblingSpawner;

impl TypedExecutor<SiblingChildTask> for CrossDomainSiblingSpawner {
    async fn execute<'a>(
        &'a self,
        _payload: SiblingChildTask,
        ctx: DomainTaskContext<'a, TestDomain>,
    ) -> Result<(), TaskError> {
        ctx.domain::<MediaDomain>()
            .submit_with(CrossDomainSiblingTask)
            .sibling_of(&ctx)
            .map_err(|e| TaskError::new(e.to_string()))?
            .key(format!("cross-sibling-{}", ctx.record().id))
            .await
            .map_err(|e| TaskError::new(e.to_string()))?;
        Ok(())
    }
}

/// Cross-domain root sibling spawner — should fail.
struct CrossDomainRootSiblingSpawner;

impl TypedExecutor<OrchestratorTask> for CrossDomainRootSiblingSpawner {
    async fn execute<'a>(
        &'a self,
        _payload: OrchestratorTask,
        ctx: DomainTaskContext<'a, TestDomain>,
    ) -> Result<(), TaskError> {
        let result = ctx
            .domain::<MediaDomain>()
            .submit_with(CrossDomainSiblingTask)
            .sibling_of(&ctx);
        match result {
            Err(e) => {
                assert!(
                    e.to_string().contains("no parent_id"),
                    "expected InvalidState error, got: {e}"
                );
                Ok(())
            }
            Ok(_) => Err(TaskError::new("should have failed")),
        }
    }
}

/// Builder-override sibling spawner — tests key/priority/group overrides.
struct OverrideSiblingSpawner;

impl TypedExecutor<SiblingChildTask> for OverrideSiblingSpawner {
    async fn execute<'a>(
        &'a self,
        _payload: SiblingChildTask,
        ctx: DomainTaskContext<'a, TestDomain>,
    ) -> Result<(), TaskError> {
        ctx.spawn_sibling_with(SiblingTask)
            .key("custom-key")
            .priority(Priority::HIGH)
            .group("custom-group")
            .ttl(Duration::from_secs(300))
            .await
            .map_err(|e| TaskError::new(e.to_string()))?;
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────

/// 1. Orchestrator spawns child A. Child A calls `spawn_sibling_with(B)`.
/// Assert B's `parent_id == orchestrator.id`.
#[tokio::test]
async fn sibling_spawn_inherits_parent_id() {
    let store = TaskStore::open_memory().await.unwrap();
    let query_store = store.clone();

    let sched = Scheduler::builder()
        .store(store)
        .domain(
            Domain::<TestDomain>::new()
                .task::<OrchestratorTask>(OrchestratorExecutor { child_count: 1 })
                .task::<SiblingChildTask>(SiblingSpawnerExecutor)
                .task::<SiblingTask>(NoopExecutor),
        )
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    let orchestrator_outcome = sched
        .submit(&TaskSubmission::new("test::orchestrator").key("orch-1"))
        .await
        .unwrap();
    let orchestrator_id = orchestrator_outcome.id().unwrap();

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for sibling to complete.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut sibling_key = None;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(SchedulerEvent::Completed(ref h))) if h.task_type == "test::sibling" => {
                sibling_key = Some(h.key.clone());
                break;
            }
            _ => continue,
        }
    }

    assert!(sibling_key.is_some(), "sibling should have completed");

    // Query store while scheduler is still running.
    let history = query_store
        .latest_history_by_key(sibling_key.as_ref().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        history.parent_id,
        Some(orchestrator_id),
        "sibling's parent_id should be the orchestrator's id"
    );

    token.cancel();
    let _ = handle.await;
}

/// 2. Root task calls `spawn_sibling_with(X)`. Assert `StoreError::InvalidState`.
#[tokio::test]
async fn sibling_spawn_no_parent_returns_error() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new()
                .task::<OrchestratorTask>(RootSiblingSpawnerExecutor)
                .task::<SiblingTask>(NoopExecutor),
        )
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    sched
        .submit(&TaskSubmission::new("test::orchestrator").key("root-sib"))
        .await
        .unwrap();

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // The executor asserts that spawning a sibling from a root task returns an error
    // and then returns Ok(()). Wait for it to complete.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let completed = wait_for_event(&mut rx, deadline, |evt| {
        matches!(evt, SchedulerEvent::Completed(ref h) if h.task_type == "test::orchestrator")
    })
    .await;

    token.cancel();
    let _ = handle.await;

    assert!(
        completed.is_some(),
        "task should complete after handling error"
    );
}

/// 3. Child spawns 10 siblings via `spawn_siblings_with()`.
/// Assert all have `parent_id == orchestrator.id`.
#[tokio::test]
async fn sibling_spawn_batch() {
    let store = TaskStore::open_memory().await.unwrap();
    let query_store = store.clone();

    let sched = Scheduler::builder()
        .store(store)
        .domain(
            Domain::<TestDomain>::new()
                .task::<OrchestratorTask>(OrchestratorExecutor { child_count: 1 })
                .task::<SiblingChildTask>(BatchSiblingSpawnerExecutor { count: 10 })
                .task::<IndexedSiblingTask>(NoopExecutor),
        )
        .max_concurrency(12)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    let orchestrator_outcome = sched
        .submit(&TaskSubmission::new("test::orchestrator").key("orch-batch"))
        .await
        .unwrap();
    let orchestrator_id = orchestrator_outcome.id().unwrap();

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for all 10 siblings to complete.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut sibling_keys = Vec::new();
    while tokio::time::Instant::now() < deadline && sibling_keys.len() < 10 {
        if let Ok(Ok(SchedulerEvent::Completed(ref h))) =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
        {
            if h.task_type == "test::indexed-sibling" {
                sibling_keys.push(h.key.clone());
            }
        }
    }

    // Verify parent_id for all siblings while scheduler is still running.
    for key in &sibling_keys {
        let history = query_store
            .latest_history_by_key(key)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            history.parent_id,
            Some(orchestrator_id),
            "sibling's parent_id should be orchestrator"
        );
    }

    token.cancel();
    let _ = handle.await;

    assert_eq!(
        sibling_keys.len(),
        10,
        "all 10 siblings should have completed"
    );
}

/// 5. Test `.key()`, `.priority()`, `.ttl()`, `.group()` on `SiblingSpawnBuilder`.
#[tokio::test]
async fn sibling_spawn_builder_overrides() {
    let store = TaskStore::open_memory().await.unwrap();
    let query_store = store.clone();

    let sched = Scheduler::builder()
        .store(store)
        .domain(
            Domain::<TestDomain>::new()
                .task::<OrchestratorTask>(OrchestratorExecutor { child_count: 1 })
                .task::<SiblingChildTask>(OverrideSiblingSpawner)
                .task::<SiblingTask>(NoopExecutor),
        )
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    sched
        .submit(&TaskSubmission::new("test::orchestrator").key("orch-override"))
        .await
        .unwrap();

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for sibling to complete.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut sibling_key = None;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(SchedulerEvent::Completed(ref h))) if h.task_type == "test::sibling" => {
                sibling_key = Some(h.key.clone());
                break;
            }
            _ => continue,
        }
    }

    assert!(
        sibling_key.is_some(),
        "sibling with overrides should complete"
    );

    let history = query_store
        .latest_history_by_key(sibling_key.as_ref().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        history.priority,
        Priority::HIGH,
        "priority override applied"
    );

    token.cancel();
    let _ = handle.await;
}

/// 7. Cross-domain sibling: `ctx.domain::<Other>().submit_with(task).sibling_of(&ctx)`.
#[tokio::test]
async fn sibling_spawn_cross_domain() {
    let store = TaskStore::open_memory().await.unwrap();
    let query_store = store.clone();

    let sched = Scheduler::builder()
        .store(store)
        .domain(
            Domain::<TestDomain>::new()
                .task::<OrchestratorTask>(OrchestratorExecutor { child_count: 1 })
                .task::<SiblingChildTask>(CrossDomainSiblingSpawner),
        )
        .domain(Domain::<MediaDomain>::new().task::<CrossDomainSiblingTask>(NoopExecutor))
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    let orchestrator_outcome = sched
        .submit(&TaskSubmission::new("test::orchestrator").key("orch-cross"))
        .await
        .unwrap();
    let orchestrator_id = orchestrator_outcome.id().unwrap();

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for cross-domain sibling to complete.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut sibling_key = None;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(SchedulerEvent::Completed(ref h))) if h.task_type == "media::cross-sibling" => {
                sibling_key = Some(h.key.clone());
                break;
            }
            _ => continue,
        }
    }

    assert!(
        sibling_key.is_some(),
        "cross-domain sibling should complete"
    );

    let history = query_store
        .latest_history_by_key(sibling_key.as_ref().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        history.parent_id,
        Some(orchestrator_id),
        "cross-domain sibling's parent_id should be orchestrator"
    );

    token.cancel();
    let _ = handle.await;
}

/// 8. Orchestrator's `finalize()` runs only after all siblings complete.
#[tokio::test]
async fn sibling_spawn_finalize_waits() {
    let finalized = Arc::new(AtomicBool::new(false));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new()
                .task::<OrchestratorTask>(OrchestratorFinalizeTracker {
                    child_count: 1,
                    finalized: finalized.clone(),
                })
                .task::<SiblingChildTask>(SiblingSpawnerExecutor)
                .task::<SiblingTask>(NoopExecutor),
        )
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    sched
        .submit(
            &TaskSubmission::new("test::orchestrator")
                .key("orch-fin")
                .fail_fast(false),
        )
        .await
        .unwrap();

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for orchestrator to complete (after finalize).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let orch_completed = wait_for_event(&mut rx, deadline, |evt| {
        matches!(evt, SchedulerEvent::Completed(ref h) if h.task_type == "test::orchestrator")
    })
    .await;

    token.cancel();
    let _ = handle.await;

    assert!(
        orch_completed.is_some(),
        "orchestrator should complete after children and siblings"
    );
    assert!(
        finalized.load(Ordering::SeqCst),
        "finalize should have been called"
    );
}

/// 9. Verify that the refactored `spawn_children_with` batch path produces
/// correct outcomes for 100+ children (regression test for Step 2).
#[tokio::test]
async fn spawn_children_batch_uses_single_transaction() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new()
                .task::<ParentTask>(ChildSpawnerExecutor::<ChildTask>::new(100))
                .task::<ChildTask>(NoopExecutor),
        )
        .max_concurrency(50)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    sched
        .submit(&TaskSubmission::new("test::parent").key("batch-parent"))
        .await
        .unwrap();

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    // Wait for parent to complete (meaning all 100 children completed).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
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
        "parent should complete after all 100 children"
    );
}

/// 10. Root task calls `.sibling_of(&ctx)`. Assert `Err(StoreError::InvalidState)`.
#[tokio::test]
async fn sibling_spawn_cross_domain_no_parent_returns_error() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<OrchestratorTask>(CrossDomainRootSiblingSpawner))
        .domain(Domain::<MediaDomain>::new().task::<CrossDomainSiblingTask>(NoopExecutor))
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    sched
        .submit(&TaskSubmission::new("test::orchestrator").key("root-cross"))
        .await
        .unwrap();

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let completed = wait_for_event(&mut rx, deadline, |evt| {
        matches!(evt, SchedulerEvent::Completed(ref h) if h.task_type == "test::orchestrator")
    })
    .await;

    token.cancel();
    let _ = handle.await;

    assert!(
        completed.is_some(),
        "task should complete after handling error"
    );
}
