use serde::{Deserialize, Serialize};
use std::time::Duration;
use taskmill::store::TaskStore;
use taskmill::{
    Domain, DomainKey, DomainTaskContext, Scheduler, SchedulerEvent, TaskError, TaskSubmission,
    TypedExecutor, TypedTask,
};
use tokio_util::sync::CancellationToken;

struct TestDomain;
impl DomainKey for TestDomain {
    const NAME: &'static str = "test";
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct OrcTask;
impl TypedTask for OrcTask {
    type Domain = TestDomain;
    const TASK_TYPE: &'static str = "orc";
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct ChildT;
impl TypedTask for ChildT {
    type Domain = TestDomain;
    const TASK_TYPE: &'static str = "child";
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct SibT;
impl TypedTask for SibT {
    type Domain = TestDomain;
    const TASK_TYPE: &'static str = "sib";
}

struct OrcExec;
impl TypedExecutor<OrcTask> for OrcExec {
    async fn execute<'a>(
        &'a self,
        _: OrcTask,
        ctx: DomainTaskContext<'a, TestDomain>,
    ) -> Result<(), TaskError> {
        ctx.spawn_child_with(ChildT)
            .key("c0")
            .await
            .map_err(|e| TaskError::new(e.to_string()))?;
        Ok(())
    }
}

struct ChildExec;
impl TypedExecutor<ChildT> for ChildExec {
    async fn execute<'a>(
        &'a self,
        _: ChildT,
        ctx: DomainTaskContext<'a, TestDomain>,
    ) -> Result<(), TaskError> {
        eprintln!(
            "  ChildExec: spawning sibling (my parent_id={:?})",
            ctx.record().parent_id
        );
        let outcome = ctx
            .spawn_sibling_with(SibT)
            .key("s0")
            .await
            .map_err(|e| TaskError::new(e.to_string()))?;
        eprintln!("  ChildExec: sibling spawned, outcome={:?}", outcome);
        Ok(())
    }
}

struct SibExec;
impl TypedExecutor<SibT> for SibExec {
    async fn execute<'a>(
        &'a self,
        _: SibT,
        ctx: DomainTaskContext<'a, TestDomain>,
    ) -> Result<(), TaskError> {
        eprintln!(
            "  SibExec: running (parent_id={:?})",
            ctx.record().parent_id
        );
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let store = TaskStore::open_memory().await.unwrap();
    let qs = store.clone();

    let sched = Scheduler::builder()
        .store(store)
        .domain(
            Domain::<TestDomain>::new()
                .task::<OrcTask>(OrcExec)
                .task::<ChildT>(ChildExec)
                .task::<SibT>(SibExec),
        )
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    let orc = sched
        .submit(&TaskSubmission::new("test::orc").key("o0"))
        .await
        .unwrap();
    eprintln!("Orchestrator submitted: {:?}", orc);

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let sc = sched.clone();
    let tc = token.clone();
    let h = tokio::spawn(async move {
        sc.run(tc).await;
    });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(evt)) => {
                if let Some(hdr) = evt.header() {
                    eprintln!(
                        "Event: {:?} type={} id={}",
                        std::mem::discriminant(&evt),
                        hdr.task_type,
                        hdr.task_id
                    );
                } else {
                    eprintln!("Event: {:?}", evt);
                }
                if matches!(&evt, SchedulerEvent::Completed(h) if h.task_type == "test::orc") {
                    break;
                }
            }
            _ => {}
        }
    }

    // Check history
    for id in 1..=10 {
        if let Ok(Some(h)) = qs.history_by_id(id).await {
            eprintln!(
                "History id={}: type={} parent_id={:?}",
                h.id, h.task_type, h.parent_id
            );
        }
    }

    token.cancel();
    let _ = h.await;
}
