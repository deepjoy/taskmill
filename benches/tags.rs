//! Benchmarks for task metadata tags: submission cost and query performance.
//!
//! Run with: `cargo bench --bench tags`

use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use serde::{Deserialize, Serialize};
use taskmill::{
    Domain, DomainKey, DomainTaskContext, Scheduler, TaskError, TaskStore, TaskSubmission,
    TypedExecutor, TypedTask,
};
use tokio::runtime::Runtime;

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

/// Pre-populate a store with `n` tasks, each carrying `("bucket", "b0")`.
async fn store_with_tagged_tasks(n: usize) -> TaskStore {
    let store = TaskStore::open_memory().await.unwrap();
    for i in 0..n {
        store
            .submit(
                &TaskSubmission::new("bench::test")
                    .key(format!("tq-{i}"))
                    .tag("bucket", "b0"),
            )
            .await
            .unwrap();
    }
    store
}

// ── Benchmarks ──────────────────────────────────────────────────────

/// Submission cost at varying tag counts per task.
/// Measures the write overhead of tag insertion and validation.
fn bench_submit_with_tags(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("submit_with_tags");

    for tag_count in [0usize, 5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::from_parameter(tag_count),
            &tag_count,
            |b, &tag_count| {
                b.to_async(&rt).iter_custom(|iters| async move {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        let sched = Scheduler::builder()
                            .store(TaskStore::open_memory().await.unwrap())
                            .domain(Domain::<BenchDomain>::new().task::<BenchTask>(NoopExecutor))
                            .max_concurrency(4)
                            .poll_interval(Duration::from_millis(10))
                            .build()
                            .await
                            .unwrap();
                        let start = Instant::now();
                        for i in 0..500 {
                            let mut sub = TaskSubmission::new("bench::test").key(format!("st-{i}"));
                            for t in 0..tag_count {
                                sub = sub.tag(format!("key-{t}"), format!("val-{i}-{t}"));
                            }
                            sched.submit(&sub).await.unwrap();
                        }
                        total += start.elapsed();
                    }
                    total
                });
            },
        );
    }

    group.finish();
}

/// `tasks_by_tags` with a single filter at varying queue depths.
/// All tasks match the filter, so result size equals queue depth.
fn bench_query_by_tags(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("query_by_tags");

    for queue_depth in [100usize, 1000, 5000] {
        let store = rt.block_on(store_with_tagged_tasks(queue_depth));
        group.bench_with_input(
            BenchmarkId::from_parameter(queue_depth),
            &queue_depth,
            |b, _| {
                let store = store.clone();
                b.to_async(&rt).iter_custom(|iters| {
                    let store = store.clone();
                    async move {
                        let start = Instant::now();
                        for _ in 0..iters {
                            let _ = store
                                .tasks_by_tags(&[("bucket", "b0")], None)
                                .await
                                .unwrap();
                        }
                        start.elapsed()
                    }
                });
            },
        );
    }

    group.finish();
}

/// `count_by_tags` with a single filter at varying queue depths.
/// Aggregation query — does not fetch rows, only counts.
fn bench_count_by_tags(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("count_by_tags");

    for queue_depth in [100usize, 1000, 5000] {
        let store = rt.block_on(store_with_tagged_tasks(queue_depth));
        group.bench_with_input(
            BenchmarkId::from_parameter(queue_depth),
            &queue_depth,
            |b, _| {
                let store = store.clone();
                b.to_async(&rt).iter_custom(|iters| {
                    let store = store.clone();
                    async move {
                        let start = Instant::now();
                        for _ in 0..iters {
                            let _ = store
                                .count_by_tags(&[("bucket", "b0")], None)
                                .await
                                .unwrap();
                        }
                        start.elapsed()
                    }
                });
            },
        );
    }

    group.finish();
}

/// `tag_values` distinct-value scan at varying queue depths.
/// Tasks are spread across 10 bucket values to exercise the GROUP BY path.
fn bench_tag_values_scan(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("tag_values");

    for queue_depth in [100usize, 1000, 5000] {
        let store = rt.block_on(async {
            let store = TaskStore::open_memory().await.unwrap();
            for i in 0..queue_depth {
                store
                    .submit(
                        &TaskSubmission::new("bench::test")
                            .key(format!("tv-{i}"))
                            .tag("bucket", format!("b{}", i % 10)),
                    )
                    .await
                    .unwrap();
            }
            store
        });
        group.bench_with_input(
            BenchmarkId::from_parameter(queue_depth),
            &queue_depth,
            |b, _| {
                let store = store.clone();
                b.to_async(&rt).iter_custom(|iters| {
                    let store = store.clone();
                    async move {
                        let start = Instant::now();
                        for _ in 0..iters {
                            let _ = store.tag_values("bucket").await.unwrap();
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
    bench_submit_with_tags,
    bench_query_by_tags,
    bench_count_by_tags,
    bench_tag_values_scan,
);
criterion_main!(benches);
