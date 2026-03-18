//! Benchmarks for retry and backoff strategies.
//!
//! Run with: `cargo bench --bench retry`

use std::sync::Arc;
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use taskmill::{
    BackoffStrategy, Module, RetryPolicy, Scheduler, SchedulerEvent, TaskContext, TaskError,
    TaskExecutor, TaskStore, TaskSubmission,
};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

// ── Executors ───────────────────────────────────────────────────────

struct FailPermanentExecutor;

impl TaskExecutor for FailPermanentExecutor {
    async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        Err(TaskError::permanent("bench: permanent failure"))
    }
}

struct FailRetryableExecutor;

impl TaskExecutor for FailRetryableExecutor {
    async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
        Err(TaskError::retryable("bench: transient failure"))
    }
}

// ── Benchmarks ──────────────────────────────────────────────────────

/// Pure math: compute backoff delay for each strategy at 20 consecutive retry counts.
/// No scheduler involved — isolates the computation cost of `delay_for`.
fn bench_backoff_delay_computation(c: &mut Criterion) {
    let strategies: &[(&str, BackoffStrategy)] = &[
        (
            "constant",
            BackoffStrategy::Constant {
                delay: Duration::from_millis(100),
            },
        ),
        (
            "linear",
            BackoffStrategy::Linear {
                initial: Duration::from_millis(100),
                increment: Duration::from_millis(50),
                max: Duration::from_secs(30),
            },
        ),
        (
            "exponential",
            BackoffStrategy::Exponential {
                initial: Duration::from_millis(100),
                max: Duration::from_secs(60),
                multiplier: 2.0,
            },
        ),
        (
            "exponential_jitter",
            BackoffStrategy::ExponentialJitter {
                initial: Duration::from_millis(100),
                max: Duration::from_secs(60),
                multiplier: 2.0,
            },
        ),
    ];

    let mut group = c.benchmark_group("backoff_delay");
    for (name, strategy) in strategies {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            strategy,
            |b, strategy| {
                b.iter(|| {
                    for retry in 0..20i32 {
                        black_box(strategy.delay_for(black_box(retry)));
                    }
                });
            },
        );
    }
    group.finish();
}

/// E2E: permanent (non-retryable) failure path.
/// 500 tasks each fail immediately with a permanent error and move to history.
fn bench_dispatch_permanent_failure(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("dispatch_permanent_failure_500", |b| {
        b.to_async(&rt).iter(|| async {
            let sched = Scheduler::builder()
                .store(TaskStore::open_memory().await.unwrap())
                .module(Module::new("bench").executor("fail", Arc::new(FailPermanentExecutor)))
                .max_concurrency(8)
                .max_retries(0)
                .poll_interval(Duration::from_millis(10))
                .build()
                .await
                .unwrap();

            for i in 0..500 {
                sched
                    .submit(&TaskSubmission::new("bench::fail").key(format!("pf-{i}")))
                    .await
                    .unwrap();
            }

            let mut rx = sched.subscribe();
            let token = CancellationToken::new();
            let sched_clone = sched.clone();
            let token_clone = token.clone();
            let handle = tokio::spawn(async move { sched_clone.run(token_clone).await });

            let mut terminal = 0;
            while terminal < 500 {
                if let Ok(SchedulerEvent::Failed {
                    will_retry: false, ..
                }) = rx.recv().await
                {
                    terminal += 1;
                }
            }

            token.cancel();
            let _ = handle.await;
        });
    });
}

/// E2E: retryable failure path across all 4 backoff strategies.
/// 100 tasks × (2 retries + 1 initial attempt) = 300 executor calls before dead-lettering.
/// All strategies use zero delay so the bench does not wait on timers.
fn bench_dispatch_retryable_dead_letter(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("retryable_dead_letter");

    let strategies: &[(&str, BackoffStrategy)] = &[
        (
            "constant",
            BackoffStrategy::Constant {
                delay: Duration::ZERO,
            },
        ),
        (
            "linear",
            BackoffStrategy::Linear {
                initial: Duration::ZERO,
                increment: Duration::ZERO,
                max: Duration::ZERO,
            },
        ),
        (
            "exponential",
            BackoffStrategy::Exponential {
                initial: Duration::ZERO,
                max: Duration::ZERO,
                multiplier: 2.0,
            },
        ),
        (
            "exponential_jitter",
            BackoffStrategy::ExponentialJitter {
                initial: Duration::ZERO,
                max: Duration::ZERO,
                multiplier: 2.0,
            },
        ),
    ];

    for (name, strategy) in strategies {
        let policy = RetryPolicy {
            strategy: strategy.clone(),
            max_retries: 2,
        };
        group.bench_function(*name, |b| {
            b.to_async(&rt).iter(|| {
                let policy = policy.clone();
                async move {
                    let sched = Scheduler::builder()
                        .store(TaskStore::open_memory().await.unwrap())
                        .module(Module::new("bench").executor_with_retry_policy(
                            "fail",
                            Arc::new(FailRetryableExecutor),
                            policy,
                        ))
                        .max_concurrency(8)
                        .poll_interval(Duration::from_millis(10))
                        .build()
                        .await
                        .unwrap();

                    for i in 0..100 {
                        sched
                            .submit(&TaskSubmission::new("bench::fail").key(format!("rf-{i}")))
                            .await
                            .unwrap();
                    }

                    let mut rx = sched.subscribe();
                    let token = CancellationToken::new();
                    let sched_clone = sched.clone();
                    let token_clone = token.clone();
                    let handle = tokio::spawn(async move { sched_clone.run(token_clone).await });

                    let mut dead_lettered = 0;
                    while dead_lettered < 100 {
                        if let Ok(SchedulerEvent::DeadLettered { .. }) = rx.recv().await {
                            dead_lettered += 1;
                        }
                    }

                    token.cancel();
                    let _ = handle.await;
                }
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_backoff_delay_computation,
    bench_dispatch_permanent_failure,
    bench_dispatch_retryable_dead_letter,
);
criterion_main!(benches);
