//! Benchmarks for history table queries and aggregate stats.
//!
//! Run with: `cargo bench --bench history`

use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use taskmill::{IoBudget, TaskStore, TaskSubmission};
use tokio::runtime::Runtime;

/// Task types used to create a realistic multi-type history table.
const TASK_TYPES: &[&str] = &[
    "media::thumbnail",
    "media::transcode",
    "billing::charge",
    "billing::refund",
    "email::send",
];

/// Populate history directly via store (no scheduler overhead).
/// Distributes tasks across multiple types for realistic selectivity.
async fn store_with_history(n: usize) -> TaskStore {
    let store = TaskStore::open_memory().await.unwrap();
    let budget = IoBudget::default();
    for i in 0..n {
        let task_type = TASK_TYPES[i % TASK_TYPES.len()];
        store
            .submit(&TaskSubmission::new(task_type).key(format!("h-{i}")))
            .await
            .unwrap();
        let task = store.pop_next().await.unwrap().unwrap();
        store.complete(task.id, &budget).await.unwrap();
    }
    store
}

// ── Benchmarks ──────────────────────────────────────────────────────

/// Paginated recent-history query at varying history table sizes.
fn bench_history_query(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("history_query");
    group.throughput(Throughput::Elements(1));
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(30));

    for history_size in [100usize, 1000, 5000] {
        let store = rt.block_on(store_with_history(history_size));
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
/// History contains 5 task types; the query filters to one (20% selectivity).
fn bench_history_stats(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("history_stats");
    group.throughput(Throughput::Elements(1));
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(30));

    for history_size in [100usize, 1000, 5000] {
        let store = rt.block_on(store_with_history(history_size));
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
                            let _ = store.history_stats("media::thumbnail").await.unwrap();
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
/// History contains 5 task types; the query filters to one (20% selectivity).
fn bench_history_by_type(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("history_by_type");
    group.throughput(Throughput::Elements(1));
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(30));

    for history_size in [100usize, 1000, 5000] {
        let store = rt.block_on(store_with_history(history_size));
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
                            let _ = store
                                .history_by_type("media::thumbnail", 100)
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

criterion_group!(
    benches,
    bench_history_query,
    bench_history_stats,
    bench_history_by_type,
);
criterion_main!(benches);
