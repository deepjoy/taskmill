//! Integration tests for the execute-to-finalize memo feature.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use taskmill::{
    Domain, DomainKey, DomainTaskContext, Scheduler, SchedulerEvent, TaskError, TaskStore,
    TypedExecutor, TypedTask,
};
use tokio_util::sync::CancellationToken;

use super::common::*;

// ── Domain and task types ───────────────────────────────────────────

struct MemoDomain;
impl DomainKey for MemoDomain {
    const NAME: &'static str = "memo";
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct MemoParent;
impl TypedTask for MemoParent {
    type Domain = MemoDomain;
    const TASK_TYPE: &'static str = "parent";
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct MemoChild;
impl TypedTask for MemoChild {
    type Domain = MemoDomain;
    const TASK_TYPE: &'static str = "child";
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct MemoLeaf;
impl TypedTask for MemoLeaf {
    type Domain = MemoDomain;
    const TASK_TYPE: &'static str = "leaf";
}

// ── Memo type ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ScanMemo {
    scan_start_ns: i64,
    batch_id: u64,
}

// ── Executors ───────────────────────────────────────────────────────

/// Executor that returns a ScanMemo from execute() and verifies it in finalize().
struct MemoRoundTripExecutor {
    finalized: Arc<AtomicBool>,
    memo_matched: Arc<AtomicBool>,
}

impl TypedExecutor<MemoParent, ScanMemo> for MemoRoundTripExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: MemoParent,
        ctx: DomainTaskContext<'a, MemoDomain>,
    ) -> Result<ScanMemo, TaskError> {
        ctx.spawn_child_with(MemoChild)
            .key("memo-child-0")
            .await
            .map_err(|e| TaskError::new(e.to_string()))?;

        Ok(ScanMemo {
            scan_start_ns: 42_000_000,
            batch_id: 99,
        })
    }

    async fn finalize<'a>(
        &'a self,
        _payload: MemoParent,
        memo: ScanMemo,
        _ctx: DomainTaskContext<'a, MemoDomain>,
    ) -> Result<(), TaskError> {
        self.finalized.store(true, Ordering::SeqCst);
        if memo.scan_start_ns == 42_000_000 && memo.batch_id == 99 {
            self.memo_matched.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

/// Executor without memo — verifies backward compatibility.
struct NoMemoExecutor {
    finalized: Arc<AtomicBool>,
}

impl TypedExecutor<MemoParent> for NoMemoExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: MemoParent,
        ctx: DomainTaskContext<'a, MemoDomain>,
    ) -> Result<(), TaskError> {
        ctx.spawn_child_with(MemoChild)
            .key("no-memo-child-0")
            .await
            .map_err(|e| TaskError::new(e.to_string()))?;
        Ok(())
    }

    async fn finalize<'a>(
        &'a self,
        _payload: MemoParent,
        _memo: (),
        _ctx: DomainTaskContext<'a, MemoDomain>,
    ) -> Result<(), TaskError> {
        self.finalized.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Executor that produces a memo but spawns no children (leaf task).
struct LeafWithMemoExecutor;

impl TypedExecutor<MemoLeaf, ScanMemo> for LeafWithMemoExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: MemoLeaf,
        _ctx: DomainTaskContext<'a, MemoDomain>,
    ) -> Result<ScanMemo, TaskError> {
        Ok(ScanMemo {
            scan_start_ns: 1,
            batch_id: 2,
        })
    }
}

/// Executor whose memo fails to serialize.
struct BadMemoExecutor;

#[derive(Debug, Clone, Deserialize)]
struct BadMemo;

impl Serialize for BadMemo {
    fn serialize<S: serde::Serializer>(&self, _s: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom(
            "intentional serialization failure",
        ))
    }
}

impl TypedExecutor<MemoLeaf, BadMemo> for BadMemoExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: MemoLeaf,
        _ctx: DomainTaskContext<'a, MemoDomain>,
    ) -> Result<BadMemo, TaskError> {
        Ok(BadMemo)
    }
}

// ── Helper ──────────────────────────────────────────────────────────

fn start_run_loop(sched: &Scheduler) -> (tokio::task::JoinHandle<()>, CancellationToken) {
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move {
        sched_clone.run(token_clone).await;
    });
    (handle, token)
}

// ── Tests ───────────────────────────────────────────────────────────

/// 1. Basic round-trip: execute() returns ScanMemo, finalize() receives it.
#[tokio::test]
async fn memo_round_trip() {
    let finalized = Arc::new(AtomicBool::new(false));
    let memo_matched = Arc::new(AtomicBool::new(false));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<MemoDomain>::new()
                .task_memo(MemoRoundTripExecutor {
                    finalized: finalized.clone(),
                    memo_matched: memo_matched.clone(),
                })
                .task::<MemoChild>(NoopExecutor),
        )
        .build()
        .await
        .unwrap();

    let handle = sched.domain::<MemoDomain>();
    let mut rx = sched.subscribe();

    handle.submit(MemoParent).await.unwrap();

    let (run_handle, token) = start_run_loop(&sched);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let completed = wait_for_event(
        &mut rx,
        deadline,
        |evt| matches!(evt, SchedulerEvent::Completed(h) if h.task_type == "memo::parent"),
    )
    .await;

    token.cancel();
    let _ = run_handle.await;

    assert!(completed.is_some(), "parent should complete");
    assert!(
        finalized.load(Ordering::SeqCst),
        "finalize should be called"
    );
    assert!(
        memo_matched.load(Ordering::SeqCst),
        "memo values should match in finalize"
    );
}

/// 2. Executor with Memo = () works unchanged — no memo written to DB.
#[tokio::test]
async fn unit_memo_works_unchanged() {
    let finalized = Arc::new(AtomicBool::new(false));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<MemoDomain>::new()
                .task::<MemoParent>(NoMemoExecutor {
                    finalized: finalized.clone(),
                })
                .task::<MemoChild>(NoopExecutor),
        )
        .build()
        .await
        .unwrap();

    let handle = sched.domain::<MemoDomain>();
    let mut rx = sched.subscribe();

    handle.submit(MemoParent).await.unwrap();

    let (run_handle, token) = start_run_loop(&sched);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let completed = wait_for_event(
        &mut rx,
        deadline,
        |evt| matches!(evt, SchedulerEvent::Completed(h) if h.task_type == "memo::parent"),
    )
    .await;

    assert!(completed.is_some(), "parent should complete");
    assert!(
        finalized.load(Ordering::SeqCst),
        "finalize should be called for unit-memo executor"
    );

    // Verify no memo was persisted in history (query before shutdown closes pool).
    let history = sched.store().history(10, 0).await.unwrap();
    let parent_hist = history
        .iter()
        .find(|h| h.task_type == "memo::parent")
        .expect("parent should be in history");
    assert!(
        parent_hist.memo.is_none(),
        "unit memo should not be persisted"
    );

    token.cancel();
    let _ = run_handle.await;
}

/// 3. Memo is preserved in history after task completes.
#[tokio::test]
async fn memo_preserved_in_history() {
    let finalized = Arc::new(AtomicBool::new(false));
    let memo_matched = Arc::new(AtomicBool::new(false));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<MemoDomain>::new()
                .task_memo(MemoRoundTripExecutor {
                    finalized: finalized.clone(),
                    memo_matched: memo_matched.clone(),
                })
                .task::<MemoChild>(NoopExecutor),
        )
        .build()
        .await
        .unwrap();

    let handle = sched.domain::<MemoDomain>();
    let mut rx = sched.subscribe();

    handle.submit(MemoParent).await.unwrap();

    let (run_handle, token) = start_run_loop(&sched);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let _ = wait_for_event(
        &mut rx,
        deadline,
        |evt| matches!(evt, SchedulerEvent::Completed(h) if h.task_type == "memo::parent"),
    )
    .await;

    // Query before shutdown closes the pool.
    let history = sched.store().history(10, 0).await.unwrap();
    let parent_hist = history
        .iter()
        .find(|h| h.task_type == "memo::parent")
        .expect("parent should be in history");

    assert!(
        parent_hist.memo.is_some(),
        "memo should be preserved in history"
    );
    let memo: ScanMemo = serde_json::from_slice(parent_hist.memo.as_ref().unwrap()).unwrap();
    assert_eq!(memo.scan_start_ns, 42_000_000);
    assert_eq!(memo.batch_id, 99);

    token.cancel();
    let _ = run_handle.await;
}

/// 4. Leaf task (no children, no finalize): execute returns memo but task
/// completes normally — memo is not written since there's no set_waiting.
#[tokio::test]
async fn leaf_task_memo_not_persisted() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MemoDomain>::new().task_memo(LeafWithMemoExecutor))
        .build()
        .await
        .unwrap();

    let handle = sched.domain::<MemoDomain>();
    let mut rx = sched.subscribe();

    handle.submit(MemoLeaf).await.unwrap();

    let (run_handle, token) = start_run_loop(&sched);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let _ = wait_for_event(
        &mut rx,
        deadline,
        |evt| matches!(evt, SchedulerEvent::Completed(h) if h.task_type == "memo::leaf"),
    )
    .await;

    let history = sched.store().history(10, 0).await.unwrap();
    let leaf_hist = history
        .iter()
        .find(|h| h.task_type == "memo::leaf")
        .expect("leaf should be in history");
    assert!(
        leaf_hist.memo.is_none(),
        "leaf task memo should not be persisted (no waiting transition)"
    );

    token.cancel();
    let _ = run_handle.await;
}

/// 5. Serialization failure: executor returns a type that fails to serialize →
/// execute returns TaskError::permanent, task is not left in a broken state.
#[tokio::test]
async fn serialization_failure_produces_permanent_error() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MemoDomain>::new().task_memo(BadMemoExecutor))
        .build()
        .await
        .unwrap();

    let handle = sched.domain::<MemoDomain>();
    let mut rx = sched.subscribe();

    handle.submit(MemoLeaf).await.unwrap();

    let (run_handle, token) = start_run_loop(&sched);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let failed = wait_for_event(&mut rx, deadline, |evt| {
        matches!(
            evt,
            SchedulerEvent::Failed {
                header,
                will_retry: false,
                ..
            } if header.task_type == "memo::leaf"
        )
    })
    .await;

    token.cancel();
    let _ = run_handle.await;

    assert!(
        failed.is_some(),
        "task should fail with serialization error"
    );

    if let Some(SchedulerEvent::Failed { error, .. }) = failed {
        assert!(
            error.contains("memo serialization"),
            "error should mention memo serialization: {error}"
        );
    }
}

/// 6. Memo survives restart: persist memo → stop scheduler → reopen →
/// children complete → finalize receives correct memo.
#[tokio::test]
async fn memo_survives_restart() {
    let dir = std::env::temp_dir().join(format!("taskmill-memo-restart-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let db_path = dir.join("test.db");
    let db_str = db_path.to_str().unwrap();

    let finalized = Arc::new(AtomicBool::new(false));
    let memo_matched = Arc::new(AtomicBool::new(false));

    // Phase 1: submit parent, let it execute (spawns child), enter waiting.
    {
        let sched = Scheduler::builder()
            .store(TaskStore::open(db_str).await.unwrap())
            .domain(
                Domain::<MemoDomain>::new()
                    .task_memo(MemoRoundTripExecutor {
                        finalized: finalized.clone(),
                        memo_matched: memo_matched.clone(),
                    })
                    .task::<MemoChild>(DelayExecutor(Duration::from_secs(60))),
            )
            .build()
            .await
            .unwrap();

        let handle = sched.domain::<MemoDomain>();
        let mut rx = sched.subscribe();

        handle.submit(MemoParent).await.unwrap();

        let (run_handle, token) = start_run_loop(&sched);

        // Wait for parent to enter waiting.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let _ = wait_for_event(&mut rx, deadline, |evt| {
            matches!(evt, SchedulerEvent::Waiting { .. })
        })
        .await;

        // Verify memo is persisted on the task record.
        let tasks = sched.store().waiting_tasks().await.unwrap();
        assert_eq!(tasks.len(), 1, "parent should be in waiting state");
        assert!(tasks[0].memo.is_some(), "memo should be persisted");
        let memo: ScanMemo = serde_json::from_slice(tasks[0].memo.as_ref().unwrap()).unwrap();
        assert_eq!(memo.scan_start_ns, 42_000_000);
        assert_eq!(memo.batch_id, 99);

        // Stop the scheduler (simulates restart).
        token.cancel();
        let _ = run_handle.await;
        sched.store().close().await;
    }

    // Phase 2: reopen from same DB — child re-runs and completes, parent finalizes.
    {
        let sched = Scheduler::builder()
            .store(TaskStore::open(db_str).await.unwrap())
            .domain(
                Domain::<MemoDomain>::new()
                    .task_memo(MemoRoundTripExecutor {
                        finalized: finalized.clone(),
                        memo_matched: memo_matched.clone(),
                    })
                    .task::<MemoChild>(NoopExecutor),
            )
            .build()
            .await
            .unwrap();

        let mut rx = sched.subscribe();

        let (run_handle, token) = start_run_loop(&sched);

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let _ = wait_for_event(
            &mut rx,
            deadline,
            |evt| matches!(evt, SchedulerEvent::Completed(h) if h.task_type == "memo::parent"),
        )
        .await;

        token.cancel();
        let _ = run_handle.await;

        assert!(
            finalized.load(Ordering::SeqCst),
            "finalize should run after restart"
        );
        assert!(
            memo_matched.load(Ordering::SeqCst),
            "memo should survive restart and match in finalize"
        );

        sched.store().close().await;
    }

    // Cleanup.
    let _ = std::fs::remove_dir_all(&dir);
}
