//! Performance benchmarks for the taskmill scheduler.
//!
//! Run with: `cargo bench -p taskmill`

use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use taskmill::{
    Priority, Scheduler, SchedulerEvent, TaskContext, TaskError, TaskExecutor, TaskStore,
    TaskSubmission,
};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

// ── Test Executors ──────────────────────────────────────────────────

struct NoopExecutor;

impl TaskExecutor for NoopExecutor {
    async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        Ok(())
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

async fn build_scheduler(max_concurrency: usize) -> Scheduler {
    Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .executor("test", Arc::new(NoopExecutor))
        .max_concurrency(max_concurrency)
        .poll_interval(std::time::Duration::from_millis(10))
        .build()
        .await
        .unwrap()
}

// ── Benchmarks ──────────────────────────────────────────────────────

fn bench_submit(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("submit_1000_tasks", |b| {
        b.to_async(&rt).iter(|| async {
            let sched = build_scheduler(4).await;
            for i in 0..1000 {
                sched
                    .submit(&TaskSubmission::new("test").key(format!("s-{i}")))
                    .await
                    .unwrap();
            }
        });
    });
}

fn bench_submit_dedup_hit(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("submit_dedup_hit_1000", |b| {
        b.to_async(&rt).iter(|| async {
            let sched = build_scheduler(4).await;
            // First submit creates the task.
            sched
                .submit(&TaskSubmission::new("test").key("same-key"))
                .await
                .unwrap();
            // Subsequent submits hit the dedup path.
            for _ in 0..999 {
                sched
                    .submit(&TaskSubmission::new("test").key("same-key"))
                    .await
                    .unwrap();
            }
        });
    });
}

fn bench_dispatch_and_complete(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("dispatch_and_complete_1000", |b| {
        b.to_async(&rt).iter(|| async {
            let sched = build_scheduler(8).await;

            for i in 0..1000 {
                sched
                    .submit(&TaskSubmission::new("test").key(format!("d-{i}")))
                    .await
                    .unwrap();
            }

            let mut rx = sched.subscribe();
            let token = CancellationToken::new();
            let sched_clone = sched.clone();
            let token_clone = token.clone();
            let handle = tokio::spawn(async move {
                sched_clone.run(token_clone).await;
            });

            let mut completed = 0;
            while completed < 1000 {
                if let Ok(SchedulerEvent::Completed { .. }) = rx.recv().await {
                    completed += 1;
                }
            }

            token.cancel();
            let _ = handle.await;
        });
    });
}

fn bench_peek_next_varying_depth(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("peek_next");

    for size in [100, 1000, 5000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async move {
                let store = TaskStore::open_memory().await.unwrap();
                for i in 0..size {
                    store
                        .submit(&TaskSubmission::new("test").key(format!("pk-{i}")))
                        .await
                        .unwrap();
                }
                // Bench just the peek_next call.
                for _ in 0..100 {
                    let _ = store.peek_next().await.unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_concurrency_scaling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrency_scaling");

    for concurrency in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            &concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| async move {
                    let sched = build_scheduler(concurrency).await;

                    for i in 0..500 {
                        sched
                            .submit(&TaskSubmission::new("test").key(format!("cs-{i}")))
                            .await
                            .unwrap();
                    }

                    let mut rx = sched.subscribe();
                    let token = CancellationToken::new();
                    let sched_clone = sched.clone();
                    let token_clone = token.clone();
                    let handle = tokio::spawn(async move {
                        sched_clone.run(token_clone).await;
                    });

                    let mut completed = 0;
                    while completed < 500 {
                        if let Ok(SchedulerEvent::Completed { .. }) = rx.recv().await {
                            completed += 1;
                        }
                    }

                    token.cancel();
                    let _ = handle.await;
                });
            },
        );
    }

    group.finish();
}

fn bench_batch_submit(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("batch_submit_1000", |b| {
        b.to_async(&rt).iter(|| async {
            let sched = build_scheduler(4).await;
            let submissions: Vec<_> = (0..1000)
                .map(|i| TaskSubmission::new("test").key(format!("b-{i}")))
                .collect();
            sched.submit_batch(&submissions).await.unwrap();
        });
    });
}

fn bench_mixed_priority_dispatch(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("mixed_priority_dispatch_500", |b| {
        b.to_async(&rt).iter(|| async {
            let sched = build_scheduler(4).await;

            let priorities = [
                Priority::IDLE,
                Priority::BACKGROUND,
                Priority::NORMAL,
                Priority::HIGH,
                Priority::REALTIME,
            ];

            for i in 0..500 {
                let priority = priorities[i % priorities.len()];
                sched
                    .submit(
                        &TaskSubmission::new("test")
                            .key(format!("mp-{i}"))
                            .priority(priority),
                    )
                    .await
                    .unwrap();
            }

            let mut rx = sched.subscribe();
            let token = CancellationToken::new();
            let sched_clone = sched.clone();
            let token_clone = token.clone();
            let handle = tokio::spawn(async move {
                sched_clone.run(token_clone).await;
            });

            let mut completed = 0;
            while completed < 500 {
                if let Ok(SchedulerEvent::Completed { .. }) = rx.recv().await {
                    completed += 1;
                }
            }

            token.cancel();
            let _ = handle.await;
        });
    });
}

criterion_group!(
    benches,
    bench_submit,
    bench_submit_dedup_hit,
    bench_dispatch_and_complete,
    bench_peek_next_varying_depth,
    bench_concurrency_scaling,
    bench_batch_submit,
    bench_mixed_priority_dispatch,
);
criterion_main!(benches);
