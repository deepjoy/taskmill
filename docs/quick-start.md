# Quick Start

## Installation

Add taskmill to your `Cargo.toml`:

```toml
[dependencies]
taskmill = "0.4"
```

To disable platform resource monitoring (e.g., for mobile targets or custom samplers):

```toml
[dependencies]
taskmill = { version = "0.4", default-features = false }
```

## Core concepts

In 0.4, executors live inside **modules** — self-contained bundles that own a set of task types together with their defaults and resource policy. You register modules with the builder; at runtime you interact through a `ModuleHandle`.

```
Module::new("name")       ← define executors, defaults, and state
  .executor(...)
  .default_priority(...)
  .app_state(...)

Scheduler::builder()      ← compose modules, set global policy
  .module(my_module())
  .max_concurrency(8)

scheduler.module("name")  ← get a scoped handle at runtime
  .submit_typed(...)      ← submit, cancel, query — all scoped to this module
  .await?
```

## Implement an executor

Every task type needs code that knows how to do the work. Implement the `TaskExecutor` trait. The scheduler calls your executor whenever a task of that type is dispatched.

Your executor receives a `TaskContext` with everything it needs:

- `record()` — the full task record including payload, priority, and retry count
- `token()` — a cancellation token for responding to preemption (see [Priorities & Preemption](priorities-and-preemption.md#handling-preemption-in-executors))
- `progress()` — a reporter for sending progress updates to the UI (see [Progress & Events](progress-and-events.md))
- `state::<T>()` — shared application state registered at build time

```rust
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

## Define a module

Group your executors into a `Module`. This is the unit of composition — define it once and register it anywhere.

```rust
use std::sync::Arc;
use std::time::Duration;
use taskmill::{Module, Priority};

pub fn media_module() -> Module {
    Module::new("media")
        .executor("resize", Arc::new(ImageResizer))
        .default_priority(Priority::NORMAL)
        .default_ttl(Duration::from_secs(3600))
}
```

Module-wide defaults apply to every submission through the module's handle unless overridden per-call. This eliminates repetition across submissions.

## Build and run the scheduler

```rust
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use taskmill::{Module, Scheduler, IoBudget, TaskSubmission, ShutdownMode};

#[tokio::main]
async fn main() {
    // Build the scheduler — opens the DB, registers modules, starts monitoring.
    let scheduler = Scheduler::builder()
        .store_path("tasks.db")
        .module(media_module())
        .max_concurrency(8)
        .shutdown_mode(ShutdownMode::Graceful(Duration::from_secs(10)))
        .with_resource_monitoring()
        .build()
        .await
        .unwrap();

    // Get a scoped handle for the media module.
    let media = scheduler.module("media");

    // Scheduler is Clone — share freely across async tasks and Tauri state.
    let sched = scheduler.clone();

    // Subscribe to lifecycle events for logging or UI updates.
    let mut events = scheduler.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            println!("Event: {:?}", event);
        }
    });

    // Submit a single task through the module handle.
    // The handle auto-prefixes the task type ("resize" → "media::resize").
    let sub = TaskSubmission::new("resize")
        .payload_json(&serde_json::json!({"path": "/photos/image.jpg", "width": 300}))
        .expected_io(IoBudget::disk(4096, 1024));
    media.submit(sub).await.unwrap();

    // Submit tasks in bulk (single SQLite transaction).
    let paths = vec!["/a.jpg", "/b.jpg", "/c.jpg"];
    let subs: Vec<_> = paths.iter().map(|p| {
        TaskSubmission::new("resize")
            .payload_json(&serde_json::json!({"path": p}))
            .expected_io(IoBudget::disk(4096, 1024))
    }).collect();
    for sub in subs {
        media.submit(sub).await.unwrap();
    }

    // Run the scheduler loop (blocks until the token is cancelled).
    let token = CancellationToken::new();
    scheduler.run(token).await;
}
```

### What just happened?

1. The builder opened `tasks.db`, ran migrations, and recovered any tasks left running from a previous crash.
2. `media.submit()` prefixed the task type to `"media::resize"` and inserted it into SQLite with a dedup key. Submitting the same payload twice returns `Duplicate`.
3. `run()` started the dispatch loop. On each cycle the scheduler picks the highest-priority pending task, checks IO headroom, and spawns your executor in a new tokio task.
4. When the executor finishes, the task moves to history and a `Completed` event is broadcast.

## Typed tasks

For stronger compile-time guarantees, implement `TypedTask` instead of using stringly-typed `TaskSubmission`. This keeps the task type name, priority, and IO budget co-located with the payload struct.

```rust
use std::time::Duration;
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

    // Optional: expire if not started within 10 minutes.
    // fn ttl(&self) -> Option<Duration> { Some(Duration::from_secs(600)) }
}

// Register using the typed form — task type comes from ResizeTask::TASK_TYPE.
Module::new("media").typed_executor::<ResizeTask, _>(Arc::new(ImageResizer));

// Submit — module handle applies defaults and prefixes the type.
media.submit_typed(&ResizeTask {
    path: "/photos/img.jpg".into(),
    width: 300,
}).await?;

// In the executor:
let task: ResizeTask = ctx.payload()?;
```

`submit_typed()` returns a `SubmitBuilder` — bare `.await` applies all defaults, or chain overrides before awaiting:

```rust
media.submit_typed(&task)
    .priority(Priority::HIGH)
    .run_after(Duration::from_secs(30))
    .await?;
```

## Child tasks

Some work is naturally hierarchical — a multipart upload needs to upload individual parts, then call `CompleteMultipartUpload`. Taskmill supports this with child tasks and two-phase execution.

Spawn children from within an executor using `ctx.spawn_child()`. Children are automatically module-aware: the task type is prefixed and module defaults are applied. The parent enters a `waiting` state until all children complete, then `finalize()` is called on the parent executor.

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

For cross-module children, use `.parent()` on `SubmitBuilder`:

```rust
ctx.module("storage")
    .submit_typed(&Upload { ... })
    .parent(ctx.record().id)
    .await?;
```

By default, if any child fails, its siblings are cancelled and the parent fails immediately (fail-fast). Disable this per-submission with `.fail_fast(false)`.

## Sharing the scheduler

A single `Scheduler` is `Clone` (via `Arc`) and can be shared across your entire application. Modules can carry their own scoped state, so library modules don't need to share a namespace with the host app's state.

```rust
use std::sync::Arc;
use taskmill::{Module, Scheduler};

// Each module brings its own state.
let scheduler = Scheduler::builder()
    .store_path("app.db")
    .module(
        Module::new("media")
            .typed_executor::<ResizeTask, _>(Arc::new(ImageResizer))
            .app_state(MediaConfig { cdn_url: "...".into() })
    )
    .module(
        Module::new("sync")
            .executor("remote-sync", Arc::new(SyncExecutor))
            .app_state(SyncConfig { endpoint: "...".into() })
    )
    // Global state shared across all modules.
    .app_state(SharedDb::new())
    .max_concurrency(8)
    .build()
    .await
    .unwrap();

// Each module's state is visible to its own executors first,
// then falls back to global state.
let media_state = ctx.state::<MediaConfig>(); // only in media module executors
let db = ctx.state::<SharedDb>();             // anywhere

// The host manages the run loop.
let token = CancellationToken::new();
scheduler.run(token).await;
```

Libraries that receive a pre-built scheduler can still inject global state after construction:

```rust
scheduler.register_state(Arc::new(LibraryState { /* ... */ })).await;
```

## Delayed and recurring tasks

### Delayed tasks

Schedule a task to run after a specific delay or at a specific point in time.

```rust
use std::time::Duration;
use chrono::Utc;

// Run after a delay
media.submit(
    TaskSubmission::new("cleanup")
        .payload_json(&serde_json::json!({"path": "/tmp/stale"}))
        .run_after(Duration::from_secs(3600))
).await?;

// Run at a specific time
media.submit(
    TaskSubmission::new("report")
        .payload_json(&serde_json::json!({"date": "2025-01-15"}))
        .run_at(Utc::now() + chrono::Duration::hours(6))
).await?;
```

If the `run_after` time is in the past (e.g., because the app was offline), the task runs immediately on the next dispatch cycle.

### Recurring tasks

A recurring task automatically re-submits itself on a schedule after each completion.

```rust
use taskmill::RecurringSchedule;

media.submit(
    TaskSubmission::new("sync")
        .payload_json(&serde_json::json!({"source": "remote"}))
        .recurring(RecurringSchedule::new(Duration::from_secs(300))) // every 5 minutes
).await?;
```

`RecurringSchedule` supports additional options:

```rust
let schedule = RecurringSchedule::new(Duration::from_secs(60))
    .max_occurrences(100)          // stop after 100 runs
    .initial_delay(Duration::from_secs(10)); // wait 10s before the first run
```

Pile-up prevention is built in: if a recurring instance hasn't been dispatched yet when the next occurrence is due, the new instance is skipped.

### Managing recurring schedules

Recurring schedules can be paused, resumed, or cancelled at runtime through the module handle:

```rust
let media = scheduler.module("media");

// Pause — stops new occurrences from being enqueued
media.pause_recurring(task_id).await?;

// Resume — re-enables the schedule
media.resume_recurring(task_id).await?;

// Cancel — permanently stops the schedule and removes it
media.cancel_recurring(task_id).await?;
```

## Task dependencies

Tasks can declare dependencies on other tasks. A dependent task stays in `blocked` status until all its dependencies complete successfully.

### Simple chain

```rust
let upload = media.submit(
    TaskSubmission::new("upload-file")
        .payload_json(&upload_plan)
).await?;

// Only runs after upload succeeds
media.submit(
    TaskSubmission::new("delete-old-version")
        .depends_on(upload.id().unwrap())
        .payload_json(&delete_plan)
).await?;
```

### Fan-in (multiple dependencies)

```rust
let a = media.submit(TaskSubmission::new("fetch-a").payload_json(&a_data)).await?;
let b = media.submit(TaskSubmission::new("fetch-b").payload_json(&b_data)).await?;

// Only runs after both A and B complete
media.submit(
    TaskSubmission::new("merge")
        .depends_on_all([a.id().unwrap(), b.id().unwrap()])
        .payload_json(&merge_plan)
).await?;
```

### Failure handling

By default, if a dependency fails permanently the dependent is cancelled and recorded as `DependencyFailed` in history. Change this per-submission:

```rust
use taskmill::DependencyFailurePolicy;

media.submit(
    TaskSubmission::new("cleanup")
        .depends_on(upload_id)
        .on_dependency_failure(DependencyFailurePolicy::Ignore) // run anyway
        .payload_json(&cleanup_plan)
).await?;
```

| Policy | Behavior |
|--------|----------|
| `Cancel` (default) | Dependent is moved to history as `DependencyFailed`. |
| `Fail` | Same as `Cancel`, but doesn't cascade to other dependents in the chain. |
| `Ignore` | Dependent is unblocked and runs anyway — your executor must handle missing upstream results. |

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

## Next steps

Work through the topic guides in order:

1. [Priorities & Preemption](priorities-and-preemption.md) — control which tasks run first
2. [IO & Backpressure](io-and-backpressure.md) — prevent resource saturation
3. [Progress & Events](progress-and-events.md) — show progress and react to state changes
4. [Persistence & Recovery](persistence-and-recovery.md) — understand crash safety and deduplication
5. [Configuration](configuration.md) — tune for your workload
6. [Query APIs](query-apis.md) — build dashboards and debug stuck tasks
7. [Design](design.md) — understand the architecture for advanced use
