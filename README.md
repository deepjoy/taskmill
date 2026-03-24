# Taskmill

A persistent, priority-aware task scheduler for Rust with IO-aware concurrency.

Taskmill is a task queue for desktop apps and background services where work needs to survive crashes, respect system resources, and stay visible to users. It persists tasks to SQLite, schedules by priority with preemption, and automatically defers work when disk or network throughput is saturated.

Read more about the [motivation and use cases](docs/why-taskmill.md).

## Quick example

```rust
use tokio_util::sync::CancellationToken;
use serde::{Serialize, Deserialize};
use taskmill::{
    Domain, DomainKey, DomainHandle, Scheduler, TypedExecutor,
    TypedTask, TaskTypeConfig, TaskContext, TaskError, IoBudget,
};

// 1. Define a domain and a typed task.
struct Media;
impl DomainKey for Media { const NAME: &'static str = "media"; }

#[derive(Serialize, Deserialize)]
struct Thumbnail { path: String, size: u32 }

impl TypedTask for Thumbnail {
    type Domain = Media;
    const TASK_TYPE: &'static str = "thumbnail";

    fn config() -> TaskTypeConfig {
        TaskTypeConfig::new().expected_io(IoBudget::disk(4096, 1024))
    }

    fn key(&self) -> Option<String> { Some(self.path.clone()) }
}

// 2. Implement a typed executor — no manual deserialization.
struct ThumbnailGenerator;

impl TypedExecutor<Thumbnail> for ThumbnailGenerator {
    async fn execute(&self, thumb: Thumbnail, ctx: &TaskContext) -> Result<(), TaskError> {
        ctx.progress().report(0.5, Some("resizing".into()));
        ctx.record_read_bytes(4096);
        ctx.record_write_bytes(1024);
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    // 3. Build the scheduler with a typed domain.
    let scheduler = Scheduler::builder()
        .store_path("tasks.db")
        .domain(Domain::<Media>::new()
            .task::<Thumbnail>(ThumbnailGenerator))
        .max_concurrency(8)
        .with_resource_monitoring()
        .build()
        .await
        .unwrap();

    // 4. Submit via a typed domain handle — compile-time domain enforcement.
    let media: DomainHandle<Media> = scheduler.domain::<Media>();
    media.submit(Thumbnail { path: "/photos/img.jpg".into(), size: 256 }).await.unwrap();

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
- **Rate-limit dispatch** — token-bucket rate limits per task type and/or group cap start rate independently of concurrency
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
| [Quick Start](docs/quick-start.md) | Installation, domains, typed executors, builder setup, Tauri integration |
| [Multi-Module Applications](docs/multi-module-apps.md) | Composing multiple domains, cross-domain patterns, concurrency budgets |
| [Writing a Reusable Module](docs/library-modules.md) | Publishing a domain as a library crate |
| [Priorities & Preemption](docs/priorities-and-preemption.md) | Priority levels, task groups, preemption, and throttle behavior |
| [IO & Backpressure](docs/io-and-backpressure.md) | IO budgets, resource monitoring, pressure sources, and tuning |
| [Progress & Events](docs/progress-and-events.md) | Progress reporting, lifecycle events, typed event streams |
| [Persistence & Recovery](docs/persistence-and-recovery.md) | Crash recovery, deduplication, history retention |
| [Configuration](docs/configuration.md) | All options, recommended defaults, workload-specific tuning |
| [Query APIs](docs/query-apis.md) | TaskStore queries for dashboards, debugging, and analytics |
| [Design](docs/design.md) | Architecture decisions, extension points, thread safety |

## License

MIT
