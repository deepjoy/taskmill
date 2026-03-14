# Quick Start

## Installation

Add taskmill to your `Cargo.toml`:

```toml
[dependencies]
taskmill = "0.3"
```

To disable platform resource monitoring (e.g., for mobile targets or custom samplers):

```toml
[dependencies]
taskmill = { version = "0.3", default-features = false }
```

## Implement an executor

Every task type needs code that knows how to do the work. You provide this by implementing the `TaskExecutor` trait. The scheduler calls your executor whenever a task of that type is dispatched.

Your executor receives a `TaskContext` with everything it needs:

- `record()` — the full task record including payload, priority, and retry count
- `token()` — a cancellation token for responding to preemption (see [Priorities & Preemption](priorities-and-preemption.md#handling-preemption-in-executors))
- `progress()` — a reporter for sending progress updates to the UI (see [Progress & Events](progress-and-events.md))
- `state::<T>()` — shared application state you registered at build time

```rust
use std::sync::Arc;
use taskmill::{TaskExecutor, TaskContext, TaskError};

struct ImageResizer;

impl TaskExecutor for ImageResizer {
    async fn execute<'a>(
        &'a self,
        ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        // Deserialize your payload
        let data: Option<serde_json::Value> = ctx.record().deserialize_payload()?;

        // Check for preemption at yield points
        if ctx.token().is_cancelled() {
            return Err(TaskError::retryable("preempted"));
        }

        // Report progress
        ctx.progress().report(0.5, Some("resizing".into()));

        // Do work...

        // Report actual IO
        ctx.record_read_bytes(4096);
        ctx.record_write_bytes(1024);
        Ok(())
    }
}
```

## Build and run the scheduler

The builder wires everything together: it opens the SQLite database, registers your executors, and optionally starts resource monitoring.

```rust
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use taskmill::{Scheduler, Priority, IoBudget, TaskSubmission, ShutdownMode};

#[tokio::main]
async fn main() {
    // Build the scheduler — opens the DB, registers executors, starts monitoring.
    let scheduler = Scheduler::builder()
        .store_path("tasks.db")
        .executor("resize", Arc::new(ImageResizer))
        .max_concurrency(8)
        .shutdown_mode(ShutdownMode::Graceful(Duration::from_secs(10)))
        .with_resource_monitoring()
        .build()
        .await
        .unwrap();

    // Scheduler is Clone — share freely across async tasks and Tauri state.
    let sched = scheduler.clone();

    // Subscribe to lifecycle events for logging or UI updates.
    let mut events = scheduler.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            println!("Event: {:?}", event);
        }
    });

    // Submit a single task with a typed payload.
    let sub = TaskSubmission::new("resize")
        .payload_json(&serde_json::json!({"path": "/photos/image.jpg", "width": 300}))
        .expected_io(IoBudget::disk(4096, 1024));
    scheduler.submit(&sub).await.unwrap();

    // Submit tasks in bulk (single SQLite transaction).
    let paths = vec!["/a.jpg", "/b.jpg", "/c.jpg"];
    let batch: Vec<_> = paths.iter().map(|p| {
        TaskSubmission::new("resize")
            .payload_json(&serde_json::json!({"path": p}))
            .expected_io(IoBudget::disk(4096, 1024))
    }).collect();
    let outcomes = scheduler.submit_batch(&batch).await.unwrap();
    // Each outcome is Inserted, Upgraded, Requeued, or Duplicate.

    // Run the scheduler loop (blocks until the token is cancelled).
    let token = CancellationToken::new();
    scheduler.run(token).await;
}
```

### What just happened?

1. The builder opened `tasks.db` (creating it if needed), ran migrations, and recovered any tasks left running from a previous crash.
2. `submit()` inserted a task into SQLite with a dedup key derived from the payload. If you call `submit()` again with the same payload, it returns `Duplicate` instead of creating a second task.
3. `run()` started the dispatch loop. On each cycle, the scheduler picks the highest-priority pending task, checks whether the system has IO headroom, and if so, spawns your executor in a new tokio task.
4. When the executor finishes, the task moves to history and a `Completed` event is broadcast.

## Typed tasks

For stronger compile-time guarantees, implement the `TypedTask` trait instead of using stringly-typed `TaskSubmission`. This keeps the task type name, priority, and IO budget co-located with the payload struct.

```rust
use serde::{Serialize, Deserialize};
use taskmill::{TypedTask, IoBudget, Priority};

#[derive(Serialize, Deserialize)]
struct ResizeTask {
    path: String,
    width: u32,
}

impl TypedTask for ResizeTask {
    const TASK_TYPE: &'static str = "resize";

    fn expected_io(&self) -> IoBudget { IoBudget::disk(4096, 1024) }
    fn priority(&self) -> Priority { Priority::NORMAL }
}

// Submit:
scheduler.submit_typed(&ResizeTask {
    path: "/photos/img.jpg".into(),
    width: 300,
}).await?;

// In the executor:
let task: ResizeTask = ctx.payload()?;
```

## Child tasks

Some work is naturally hierarchical — a multipart upload needs to upload individual parts, then call `CompleteMultipartUpload`. Taskmill supports this with child tasks and two-phase execution.

Spawn children from within an executor using `ctx.spawn_child()`. The parent automatically enters a `waiting` state until all children complete, then `finalize()` is called on the parent executor.

```rust
impl TaskExecutor for MultipartUploader {
    async fn execute<'a>(
        &'a self, ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        let parts = split_into_parts(&ctx.record().payload);
        for part in parts {
            ctx.spawn_child(
                TaskSubmission::new("upload-part")
                    .payload_json(&part)
                    .expected_io(IoBudget::net(0, part.size))
            ).await?;
        }
        Ok(()) // parent enters 'waiting' state
    }

    async fn finalize<'a>(
        &'a self, ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        // Called after all children complete
        complete_multipart_upload(ctx).await
    }
}
```

By default, if any child fails, its siblings are cancelled and the parent fails immediately (fail-fast). Disable this per-submission with `.fail_fast(false)`.

## Sharing the scheduler

A single `Scheduler` is `Clone` (via `Arc`) and can be shared across your entire application. Multiple state types can coexist — each is keyed by its concrete `TypeId`.

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

## Tauri integration

Taskmill is designed for Tauri. The `Scheduler` drops directly into Tauri state, and all events are serializable for IPC.

```rust
use tauri::Manager;
use taskmill::{Scheduler, SchedulerSnapshot, StoreError};

// Expose scheduler status to the frontend.
#[tauri::command]
async fn scheduler_status(
    scheduler: tauri::State<'_, Scheduler>,
) -> Result<SchedulerSnapshot, StoreError> {
    scheduler.snapshot().await
}

// Bridge events to the frontend.
fn setup_events(app: &tauri::App, scheduler: &Scheduler) {
    let mut events = scheduler.subscribe();
    let handle = app.handle().clone();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            handle.emit("taskmill-event", &event).unwrap();
        }
    });
}
```

For a complete walkthrough, see the [Tauri Upload Queue guide](guides/tauri-upload-queue.md).

## Manual wiring

If you need full control over individual components (custom pressure sources, custom throttle policies, pre-opened stores), you can bypass the builder:

```rust
use std::sync::Arc;
use taskmill::{
    CompositePressure, Scheduler, SchedulerConfig,
    TaskStore, ThrottlePolicy,
};
use taskmill::registry::TaskTypeRegistry;

let store = TaskStore::open("tasks.db").await.unwrap();

let mut registry = TaskTypeRegistry::new();
registry.register("resize", Arc::new(ImageResizer));

let pressure = CompositePressure::new();
let policy = ThrottlePolicy::default_three_tier();

let scheduler = Scheduler::new(
    store,
    SchedulerConfig::default(),
    Arc::new(registry),
    pressure,
    policy,
);
```

## Next steps

Work through the topic guides in order:

1. [Priorities & Preemption](priorities-and-preemption.md) — control which tasks run first
2. [IO & Backpressure](io-and-backpressure.md) — prevent resource saturation
3. [Progress & Events](progress-and-events.md) — show progress and react to state changes
4. [Persistence & Recovery](persistence-and-recovery.md) — understand crash safety and deduplication
5. [Configuration](configuration.md) — tune for your workload
6. [Query APIs](query-apis.md) — build dashboards and debug stuck tasks
7. [Design](design.md) — understand the architecture for advanced use
