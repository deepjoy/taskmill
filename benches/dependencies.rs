//! Benchmarks for task dependency graph submission and dispatch.
//!
//! Run with: `cargo bench --bench dependencies`

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

async fn build_scheduler(max_concurrency: usize) -> Scheduler {
    Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .module(Module::new("bench").executor("test", Arc::new(NoopExecutor)))
        .max_concurrency(max_concurrency)
        .poll_interval(Duration::from_millis(10))
        .build()
        .await
        .unwrap()
}

// ── Benchmarks ──────────────────────────────────────────────────────

/// Submission cost for a linear dependency chain: t0 → t1 → … → tN.
/// Measures store write cost, edge insertion, and per-submission cycle detection.
fn bench_dep_chain_submit(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("dep_chain_submit");

    for depth in [10usize, 50, 200] {
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, &depth| {
            b.to_async(&rt).iter(|| async move {
                let sched = build_scheduler(8).await;

                let first = sched
                    .submit(&TaskSubmission::new("bench::test").key("d-0"))
                    .await
                    .unwrap()
                    .id()
                    .unwrap();

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
            });
        });
    }

    group.finish();
}

/// End-to-end dispatch of a linear chain at varying depths.
/// Each task is blocked until its predecessor completes — the critical path
/// is fully sequential regardless of concurrency.
fn bench_dep_chain_dispatch(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("dep_chain_dispatch");
    group.sample_size(20);

    for depth in [10usize, 25, 50] {
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, &depth| {
            b.to_async(&rt).iter(|| async move {
                let sched = build_scheduler(8).await;

                let first = sched
                    .submit(&TaskSubmission::new("bench::test").key("c-0"))
                    .await
                    .unwrap()
                    .id()
                    .unwrap();

                let mut prev = first;
                let mut last_id = first;
                for i in 1..depth {
                    let id = sched
                        .submit(
                            &TaskSubmission::new("bench::test")
                                .key(format!("c-{i}"))
                                .depends_on(prev),
                        )
                        .await
                        .unwrap()
                        .id()
                        .unwrap();
                    prev = id;
                    last_id = id;
                }

                let mut rx = sched.subscribe();
                let token = CancellationToken::new();
                let sched_clone = sched.clone();
                let token_clone = token.clone();
                let handle = tokio::spawn(async move { sched_clone.run(token_clone).await });

                // Wait for the final task in the chain to complete.
                while let Ok(event) = rx.recv().await {
                    if let SchedulerEvent::Completed(h) = event {
                        if h.task_id == last_id {
                            break;
                        }
                    }
                }

                token.cancel();
                let _ = handle.await;
            });
        });
    }

    group.finish();
}

/// Fan-in: N independent tasks converging on a single aggregator.
/// Measures the `blocked → pending` transition cost when the last upstream completes.
fn bench_dep_fan_in_dispatch(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("dep_fan_in_dispatch");
    group.sample_size(20);

    for width in [10usize, 50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(width), &width, |b, &width| {
            b.to_async(&rt).iter(|| async move {
                // Concurrency high enough to run all upstreams in parallel.
                let sched = build_scheduler(width + 1).await;

                let mut upstream_ids = Vec::with_capacity(width);
                for i in 0..width {
                    let id = sched
                        .submit(&TaskSubmission::new("bench::test").key(format!("up-{i}")))
                        .await
                        .unwrap()
                        .id()
                        .unwrap();
                    upstream_ids.push(id);
                }

                let aggregator_id = sched
                    .submit(
                        &TaskSubmission::new("bench::test")
                            .key("aggregator")
                            .depends_on_all(upstream_ids),
                    )
                    .await
                    .unwrap()
                    .id()
                    .unwrap();

                let mut rx = sched.subscribe();
                let token = CancellationToken::new();
                let sched_clone = sched.clone();
                let token_clone = token.clone();
                let handle = tokio::spawn(async move { sched_clone.run(token_clone).await });

                while let Ok(event) = rx.recv().await {
                    if let SchedulerEvent::Completed(h) = event {
                        if h.task_id == aggregator_id {
                            break;
                        }
                    }
                }

                token.cancel();
                let _ = handle.await;
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_dep_chain_submit,
    bench_dep_chain_dispatch,
    bench_dep_fan_in_dispatch,
);
criterion_main!(benches);
