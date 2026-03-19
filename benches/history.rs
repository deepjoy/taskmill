//! Benchmarks for history table queries and aggregate stats.
//!
//! Run with: `cargo bench --bench history`

use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use taskmill::{
    Domain, DomainKey, Scheduler, SchedulerEvent, TaskContext, TaskError, TaskExecutor, TaskStore,
    TaskSubmission,
};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

struct BenchDomain;
impl DomainKey for BenchDomain {
    const NAME: &'static str = "bench";
}

struct NoopExecutor;

impl TaskExecutor for NoopExecutor {
    async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        Ok(())
    }
}

/// Build a scheduler and run `n` noop tasks to completion, populating history.
async fn build_scheduler_with_history(n: usize) -> Scheduler {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<BenchDomain>::new().raw_executor("test", NoopExecutor))
        .max_concurrency(32)
        .poll_interval(Duration::from_millis(10))
        .build()
        .await
        .unwrap();

    for i in 0..n {
        sched
            .submit(&TaskSubmission::new("bench::test").key(format!("h-{i}")))
            .await
            .unwrap();
    }

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    tokio::spawn(async move { sched_clone.run(token_clone).await });

    let mut completed = 0;
    while completed < n {
        if let Ok(SchedulerEvent::Completed { .. }) = rx.recv().await {
            completed += 1;
        }
    }

    sched
}

// ── Benchmarks ──────────────────────────────────────────────────────

/// Paginated recent-history query at varying history table sizes.
fn bench_history_query(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("history_query");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(30));

    for history_size in [100usize, 1000, 5000] {
        let sched = rt.block_on(build_scheduler_with_history(history_size));
        let store = sched.store().clone();
        group.bench_with_input(
            BenchmarkId::from_parameter(history_size),
            &history_size,
            |b, _| {
                let store = store.clone();
                b.to_async(&rt).iter_custom(|iters| {
                    let store = store.clone();
                    async move {
                        let start = std::time::Instant::now();
                        for _ in 0..iters {
                            let _ = store.history(50, 0).await.unwrap();
                        }
                        start.elapsed()
                    }
                });
            },
        );
    }

    group.finish();
}

/// Aggregate stats query (`COUNT`, `AVG` duration and IO) at varying history sizes.
fn bench_history_stats(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("history_stats");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(30));

    for history_size in [100usize, 1000, 5000] {
        let sched = rt.block_on(build_scheduler_with_history(history_size));
        let store = sched.store().clone();
        group.bench_with_input(
            BenchmarkId::from_parameter(history_size),
            &history_size,
            |b, _| {
                let store = store.clone();
                b.to_async(&rt).iter_custom(|iters| {
                    let store = store.clone();
                    async move {
                        let start = std::time::Instant::now();
                        for _ in 0..iters {
                            let _ = store.history_stats("bench::test").await.unwrap();
                        }
                        start.elapsed()
                    }
                });
            },
        );
    }

    group.finish();
}

/// Filter-by-type history query at varying history sizes.
fn bench_history_by_type(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("history_by_type");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(30));

    for history_size in [100usize, 1000, 5000] {
        let sched = rt.block_on(build_scheduler_with_history(history_size));
        let store = sched.store().clone();
        group.bench_with_input(
            BenchmarkId::from_parameter(history_size),
            &history_size,
            |b, _| {
                let store = store.clone();
                b.to_async(&rt).iter_custom(|iters| {
                    let store = store.clone();
                    async move {
                        let start = std::time::Instant::now();
                        for _ in 0..iters {
                            let _ = store.history_by_type("bench::test", 100).await.unwrap();
                        }
                        start.elapsed()
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_history_query,
    bench_history_stats,
    bench_history_by_type,
);
criterion_main!(benches);
