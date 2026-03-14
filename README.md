# Taskmill

Adaptive priority work scheduler with IO-aware concurrency and SQLite persistence.

Taskmill is an async task queue for Rust applications that persists work to SQLite,
schedules by priority with IO-budget awareness, and supports preemption, retries, and
composable backpressure. Designed for desktop apps (Tauri, etc.) and background services
where tasks have measurable IO costs and the system needs to avoid saturating disk
throughput.

## Quick example

```rust
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use taskmill::{
    Scheduler, Priority, IoBudget, TaskSubmission, TaskExecutor,
    TaskContext, TaskError,
};

struct ThumbnailGenerator;

impl TaskExecutor for ThumbnailGenerator {
    async fn execute<'a>(
        &'a self, ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        ctx.progress().report(0.5, Some("resizing".into()));
        ctx.record_read_bytes(4096);
        ctx.record_write_bytes(1024);
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let scheduler = Scheduler::builder()
        .store_path("tasks.db")
        .executor("thumbnail", Arc::new(ThumbnailGenerator))
        .max_concurrency(8)
        .with_resource_monitoring()
        .build()
        .await
        .unwrap();

    let sub = TaskSubmission::new("thumbnail")
        .payload_json(&serde_json::json!({"path": "/photos/img.jpg"}))
        .expected_io(IoBudget::disk(4096, 1024));
    scheduler.submit(&sub).await.unwrap();

    let token = CancellationToken::new();
    scheduler.run(token).await;
}
```

## Shared scheduler (library embedding)

A single `Scheduler` can be shared across an application and any libraries it embeds.
Multiple state types can coexist — each is keyed by its concrete `TypeId`, and new state
can be injected after the scheduler is built via `register_state`.

```rust
use std::sync::Arc;
use taskmill::Scheduler;

// The host app builds the scheduler and registers its own executors.
let scheduler = Scheduler::builder()
    .store_path("app.db")
    .executor("thumbnail", Arc::new(ThumbnailGenerator))
    .app_state(MyAppServices { /* ... */ })
    .max_concurrency(4)
    .build()
    .await
    .unwrap();

// A library can inject its own state after build.
scheduler.register_state(Arc::new(LibraryState { /* ... */ })).await;

// Both the host and the library submit tasks to the same queue.
// The host manages the run loop.
let token = CancellationToken::new();
scheduler.run(token).await;
```

## Features

- **SQLite persistence** — tasks survive restarts; crash recovery requeues interrupted work
- **256-level priority queue** — with preemption of lower-priority tasks
- **IO-aware scheduling** — defers work when disk or network throughput is saturated
- **Key-based deduplication** — SHA-256 keys prevent duplicate submissions
- **Composable backpressure** — plug in external pressure signals with custom throttle policies
- **Cross-platform resource monitoring** — CPU, disk IO, and network throughput via `sysinfo` (Linux, macOS, Windows)
- **Network bandwidth pressure** — built-in `NetworkPressure` source throttles tasks when bandwidth is saturated
- **Retries** — automatic requeue of retryable failures with configurable limits
- **Progress reporting** — executor-reported and throughput-extrapolated progress
- **Lifecycle events** — broadcast events for UI integration (Tauri, etc.)
- **Typed payloads** — serialize/deserialize structured task data
- **Batch submission** — bulk enqueue in a single SQLite transaction
- **Graceful shutdown** — configurable drain timeout before force-cancellation
- **Task group concurrency** — limit concurrent tasks per named group (e.g., per S3 bucket)
- **Global pause/resume** — pause all work when the app is backgrounded
- **Type-keyed application state** — register multiple state types, inject pre- or post-build
- **Clone-friendly** — `Scheduler` is `Clone` via `Arc` for easy sharing
- **Serde on all public types** — ready for Tauri IPC

For a detailed breakdown of every feature, see [docs/features.md](docs/features.md).

## Documentation

| Guide | Description |
|-------|-------------|
| [Quick Start](docs/quick-start.md) | Installation, first executor, builder setup, and running the scheduler |
| [Features](docs/features.md) | Complete feature list with descriptions |
| [Priorities & Preemption](docs/priorities-and-preemption.md) | Priority levels, preemption mechanics, and throttle behavior |
| [IO Tracking & Backpressure](docs/io-and-backpressure.md) | IO budgets, resource monitoring, pressure sources, and throttle policies |
| [Persistence & Recovery](docs/persistence-and-recovery.md) | SQLite schema, crash recovery, deduplication, and history retention |
| [Progress Reporting](docs/progress-reporting.md) | Executor progress, extrapolation, dashboard snapshots, and lifecycle events |
| [Configuration](docs/configuration.md) | All configuration options for scheduler, store, sampler, and feature flags |
| [Query APIs](docs/query-apis.md) | Full `TaskStore` query reference for dashboards and debugging |

## License

MIT
