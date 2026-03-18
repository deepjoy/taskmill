# Taskmill

A persistent, priority-aware task scheduler for Rust with IO-aware concurrency.

Taskmill is a task queue for desktop apps and background services where work needs to survive crashes, respect system resources, and stay visible to users. It persists tasks to SQLite, schedules by priority with preemption, and automatically defers work when disk or network throughput is saturated.

Read more about the [motivation and use cases](docs/why-taskmill.md).

## Quick example

```rust
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use taskmill::{
    Module, Scheduler, IoBudget, TaskSubmission, TaskExecutor,
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
        .module(Module::new("media")
            .executor("thumbnail", Arc::new(ThumbnailGenerator)))
        .max_concurrency(8)
        .with_resource_monitoring()
        .build()
        .await
        .unwrap();

    let media = scheduler.module("media");
    let sub = TaskSubmission::new("thumbnail")
        .payload_json(&serde_json::json!({"path": "/photos/img.jpg"}))
        .expected_io(IoBudget::disk(4096, 1024));
    media.submit(sub).await.unwrap();

    let token = CancellationToken::new();
    scheduler.run(token).await;
}
```

## Key capabilities

- **Survive crashes** — tasks are persisted to SQLite and automatically recovered on restart
- **Stay responsive** — IO-aware scheduling defers work when disk or network throughput is saturated
- **Prioritize what matters** — 256-level priority queue with preemption lets urgent work interrupt background tasks
- **Show progress** — executor-reported and throughput-extrapolated progress for real-time UI updates
- **Expire stale work** — configurable TTL (per-task, per-type, or global) automatically expires tasks that haven't started in time
- **Schedule work** — delay tasks with `run_after`, create recurring schedules with automatic re-enqueueing and pile-up prevention
- **Avoid duplicate work** — key-based deduplication prevents the same task from being queued twice
- **React to system load** — composable backpressure from any signal (disk, network, memory, battery, API limits)
- **Control concurrency** — per-group limits (e.g., per S3 bucket), global limits, runtime-adjustable
- **Build for Tauri** — `Clone`, `Serialize` on all types; events bridge directly to frontends

## Where to start

| If you want to... | Read |
|---|---|
| Understand what taskmill solves | [Why Taskmill](docs/why-taskmill.md) |
| Get running in 5 minutes | [Quick Start](docs/quick-start.md) |
| See it in a real Tauri app | [Guide: Tauri Upload Queue](docs/guides/tauri-upload-queue.md) |
| Look up a term | [Glossary](docs/glossary.md) |

## Documentation

| Guide | What it covers |
|-------|----------------|
| [Quick Start](docs/quick-start.md) | Installation, first executor, builder setup, Tauri integration |
| [Priorities & Preemption](docs/priorities-and-preemption.md) | Priority levels, task groups, preemption, and throttle behavior |
| [IO & Backpressure](docs/io-and-backpressure.md) | IO budgets, resource monitoring, pressure sources, and tuning |
| [Progress & Events](docs/progress-and-events.md) | Progress reporting, lifecycle events, dashboard snapshots |
| [Persistence & Recovery](docs/persistence-and-recovery.md) | Crash recovery, deduplication, history retention |
| [Configuration](docs/configuration.md) | All options, recommended defaults, workload-specific tuning |
| [Query APIs](docs/query-apis.md) | TaskStore queries for dashboards, debugging, and analytics |
| [Design](docs/design.md) | Architecture decisions, extension points, thread safety |

## License

MIT
