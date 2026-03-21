/// One-shot timing breakdown of dep_chain_submit to identify where time is spent.
/// Run with: cargo run --release --example profile_dep_chain
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use taskmill::{
    Domain, DomainKey, DomainTaskContext, Scheduler, TaskError, TaskStore, TaskSubmission,
    TypedExecutor, TypedTask,
};

struct BenchDomain;
impl DomainKey for BenchDomain {
    const NAME: &'static str = "bench";
}

#[derive(Serialize, Deserialize)]
struct BenchTask;
impl TypedTask for BenchTask {
    type Domain = BenchDomain;
    const TASK_TYPE: &'static str = "test";
}

struct NoopExecutor;
impl TypedExecutor<BenchTask> for NoopExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: BenchTask,
        _ctx: DomainTaskContext<'a, BenchDomain>,
    ) -> Result<(), TaskError> {
        Ok(())
    }
}

async fn build_scheduler() -> Scheduler {
    Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<BenchDomain>::new().task::<BenchTask>(NoopExecutor))
        .max_concurrency(8)
        .poll_interval(Duration::from_millis(10))
        .build()
        .await
        .unwrap()
}

async fn run(depth: usize, iters: u32) {
    // Warm up the allocator / SQLite page cache with a throw-away run.
    let _ = build_scheduler().await;

    let mut build_times = Vec::with_capacity(iters as usize);
    let mut first_submit_times = Vec::with_capacity(iters as usize);
    let mut chain_submit_times = Vec::with_capacity(iters as usize);

    for _ in 0..iters {
        let t0 = Instant::now();
        let sched = build_scheduler().await;
        build_times.push(t0.elapsed());

        let t1 = Instant::now();
        let first = sched
            .submit(&TaskSubmission::new("bench::test").key("d-0"))
            .await
            .unwrap()
            .id()
            .unwrap();
        first_submit_times.push(t1.elapsed());

        let t2 = Instant::now();
        let mut prev = first;
        for i in 1..depth {
            prev = sched
                .submit(
                    &TaskSubmission::new("bench::test")
                        .key(format!("d-{i}"))
                        .depends_on(prev),
                )
                .await
                .unwrap()
                .id()
                .unwrap();
        }
        chain_submit_times.push(t2.elapsed());
    }

    let avg = |v: &[Duration]| -> Duration { v.iter().sum::<Duration>() / v.len() as u32 };
    let med = |v: &mut Vec<Duration>| -> Duration {
        v.sort();
        v[v.len() / 2]
    };

    let total_avg = avg(&build_times) + avg(&first_submit_times) + avg(&chain_submit_times);
    let per_chained_submit_avg = avg(&chain_submit_times)
        .checked_div((depth - 1) as u32)
        .unwrap_or_default();

    println!("depth={depth}  iters={iters}");
    println!(
        "  build_scheduler  avg={:>8.3?}  med={:>8.3?}",
        avg(&build_times),
        med(&mut build_times)
    );
    println!(
        "  first submit     avg={:>8.3?}  med={:>8.3?}",
        avg(&first_submit_times),
        med(&mut first_submit_times)
    );
    println!(
        "  chain submits    avg={:>8.3?}  med={:>8.3?}  ({} calls, {:.3?}/call)",
        avg(&chain_submit_times),
        med(&mut chain_submit_times),
        depth - 1,
        per_chained_submit_avg,
    );
    println!("  total            avg={total_avg:>8.3?}");
    println!();
}

#[tokio::main]
async fn main() {
    for depth in [10, 50, 200] {
        run(depth, 20).await;
    }
}
