//! Benchmarks for per-group concurrency gate checks.
//!
//! Run with: `cargo bench --bench groups`

use std::sync::Arc;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use taskmill::{
    Module, Scheduler, SchedulerEvent, TaskContext, TaskError, TaskExecutor, TaskStore,
    TaskSubmission,
};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

struct NoopExecutor;

impl TaskExecutor for NoopExecutor {
    async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        Ok(())
    }
}

async fn dispatch_all(sched: &Scheduler, expected: usize) {
    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let handle = tokio::spawn(async move { sched_clone.run(token_clone).await });

    let mut completed = 0;
    while completed < expected {
        if let Ok(SchedulerEvent::Completed { .. }) = rx.recv().await {
            completed += 1;
        }
    }

    token.cancel();
    let _ = handle.await;
}

// ── Benchmarks ──────────────────────────────────────────────────────

/// Baseline: 500 tasks with no group assignment.
/// The gate skips the group check entirely.
fn bench_dispatch_no_groups(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("dispatch_no_groups_500", |b| {
        b.to_async(&rt).iter(|| async {
            let sched = Scheduler::builder()
                .store(TaskStore::open_memory().await.unwrap())
                .module(Module::new("bench").executor("test", Arc::new(NoopExecutor)))
                .max_concurrency(8)
                .poll_interval(Duration::from_millis(10))
                .build()
                .await
                .unwrap();

            for i in 0..500usize {
                sched
                    .submit(&TaskSubmission::new("bench::test").key(format!("ng-{i}")))
                    .await
                    .unwrap();
            }

            dispatch_all(&sched, 500).await;
        });
    });
}

/// 500 tasks all in a single group with a high limit (no throttling).
/// The gate performs a group-map lookup on every dispatch.
fn bench_dispatch_one_group(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("dispatch_one_group_500", |b| {
        b.to_async(&rt).iter(|| async {
            let sched = Scheduler::builder()
                .store(TaskStore::open_memory().await.unwrap())
                .module(Module::new("bench").executor("test", Arc::new(NoopExecutor)))
                .max_concurrency(8)
                .group_concurrency("g0", 500) // high limit — no artificial throttling
                .poll_interval(Duration::from_millis(10))
                .build()
                .await
                .unwrap();

            for i in 0..500usize {
                sched
                    .submit(
                        &TaskSubmission::new("bench::test")
                            .key(format!("og-{i}"))
                            .group("g0"),
                    )
                    .await
                    .unwrap();
            }

            dispatch_all(&sched, 500).await;
        });
    });
}

/// Gate check overhead as the number of tracked groups grows.
/// 500 tasks spread evenly across N groups, each with a high per-group limit.
/// Measures how group-map size affects per-dispatch gate latency.
fn bench_dispatch_group_scaling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("dispatch_group_scaling");

    for n_groups in [1usize, 10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::from_parameter(n_groups),
            &n_groups,
            |b, &n_groups| {
                b.to_async(&rt).iter(|| async move {
                    let tasks_per_group = 500 / n_groups;
                    let total = tasks_per_group * n_groups;

                    let mut builder = Scheduler::builder()
                        .store(TaskStore::open_memory().await.unwrap())
                        .module(Module::new("bench").executor("test", Arc::new(NoopExecutor)))
                        .max_concurrency(8)
                        .poll_interval(Duration::from_millis(10));

                    // Register all groups up front so the gate map is fully populated.
                    for g in 0..n_groups {
                        builder = builder.group_concurrency(format!("grp-{g}"), 500);
                    }

                    let sched = builder.build().await.unwrap();

                    for g in 0..n_groups {
                        for t in 0..tasks_per_group {
                            sched
                                .submit(
                                    &TaskSubmission::new("bench::test")
                                        .key(format!("mg-{g}-{t}"))
                                        .group(format!("grp-{g}")),
                                )
                                .await
                                .unwrap();
                        }
                    }

                    dispatch_all(&sched, total).await;
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_dispatch_no_groups,
    bench_dispatch_one_group,
    bench_dispatch_group_scaling,
);
criterion_main!(benches);
