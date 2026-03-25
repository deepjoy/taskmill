# Quick Start

## Installation

Add taskmill to your `Cargo.toml`:

```toml
[dependencies]
taskmill = "0.6"
```

To disable platform resource monitoring (e.g., for mobile targets or custom samplers):

```toml
[dependencies]
taskmill = { version = "0.6", default-features = false }
```

## Core concepts

In 0.5, executors live inside **domains** — typed, self-contained bundles that own a set of task types together with their defaults and resource policy. Each domain has a compile-time identity via the `DomainKey` trait. You register domains with the builder; at runtime you interact through a `DomainHandle<D>`.

```
pub struct Media;                      <- zero-sized type
impl DomainKey for Media {             <- compile-time identity
    const NAME: &'static str = "media";
}

Domain::<Media>::new()                 <- define executors, defaults, and state
  .task::<ResizeTask>(ImageResizer)
  .default_priority(...)
  .state(...)

Scheduler::builder()                   <- compose domains, set global policy
  .domain(Domain::<Media>::new()...)
  .max_concurrency(8)

scheduler.domain::<Media>()            <- get a typed handle at runtime
  .submit(task).await?                 <- submit, cancel, query — all scoped to this domain
```

## Implement an executor

Every task type needs code that knows how to do the work. Implement the `TypedExecutor<T>` trait. The scheduler deserializes the payload for you and passes it directly to your executor.

Your executor receives the deserialized payload `T` and a `DomainTaskContext` with everything it needs:

- `record()` — the full task record including priority and retry count
- `token()` — a cancellation token for responding to preemption (see [Priorities & Preemption](priorities-and-preemption.md#handling-preemption-in-executors))
- `progress()` — a reporter for sending progress updates to the UI (see [Progress & Events](progress-and-events.md))
- `state::<T>()` — shared application state registered at build time

```rust
use taskmill::{TypedExecutor, TypedTask, DomainTaskContext, TaskError, TaskTypeConfig, IoBudget, DomainKey};
use serde::{Serialize, Deserialize};

// Define the domain identity.
pub struct Media;
impl DomainKey for Media {
    const NAME: &'static str = "media";
}

// Define the typed task payload.
#[derive(Serialize, Deserialize)]
struct ResizeTask {
    path: String,
    width: u32,
}

impl TypedTask for ResizeTask {
    type Domain = Media;
    const TASK_TYPE: &'static str = "resize";

    fn config() -> TaskTypeConfig {
        TaskTypeConfig::new()
            .expected_io(IoBudget::disk(4096, 1024))
    }
}

// Implement the typed executor.
struct ImageResizer;

impl TypedExecutor<ResizeTask> for ImageResizer {
    async fn execute(
        &self,
        task: ResizeTask,
        ctx: DomainTaskContext<'_, Media>,
    ) -> Result<(), TaskError> {
        // Check for preemption at yield points
        if ctx.token().is_cancelled() {
            return Err(TaskError::retryable("preempted"));
        }

        // Report progress
        ctx.progress().report(0.5, Some("resizing".into()));

        // Do work with task.path, task.width...

        // Report actual IO
        ctx.record_read_bytes(4096);
        ctx.record_write_bytes(1024);
        Ok(())
    }
}
```

## Define a domain

Group your executors into a `Domain<D>`. This is the unit of composition — define it once and register it anywhere. The domain name is derived from `D::NAME`, so there is no stringly-typed constructor.

```rust
use std::time::Duration;
use taskmill::{Domain, DomainKey, Priority};

pub struct Media;
impl DomainKey for Media {
    const NAME: &'static str = "media";
}

pub fn media_domain() -> Domain<Media> {
    Domain::<Media>::new()
        .task::<ResizeTask>(ImageResizer)
        .default_priority(Priority::NORMAL)
        .default_ttl(Duration::from_secs(3600))
}
```

Domain-wide defaults apply to every submission through the domain's handle unless overridden per-call or per-task-type via `TypedTask::config()`.

## Build and run the scheduler

```rust
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use taskmill::{Domain, DomainKey, DomainHandle, Scheduler, ShutdownMode};

pub struct Media;
impl DomainKey for Media {
    const NAME: &'static str = "media";
}

#[tokio::main]
async fn main() {
    // Build the scheduler — opens the DB, registers domains, starts monitoring.
    let scheduler = Scheduler::builder()
        .store_path("tasks.db")
        .domain(media_domain())
        .max_concurrency(8)
        .shutdown_mode(ShutdownMode::Graceful(Duration::from_secs(10)))
        .with_resource_monitoring()
        .build()
        .await
        .unwrap();

    // Get a typed handle for the media domain.
    let media: DomainHandle<Media> = scheduler.domain::<Media>();

    // Scheduler is Clone — share freely across async tasks and Tauri state.
    let sched = scheduler.clone();

    // Subscribe to lifecycle events for logging or UI updates.
    let mut events = scheduler.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            println!("Event: {:?}", event);
        }
    });

    // Submit a single task through the domain handle.
    // The handle auto-prefixes the task type ("resize" -> "media::resize").
    media.submit(ResizeTask {
        path: "/photos/image.jpg".into(),
        width: 300,
    }).await.unwrap();

    // Submit tasks in bulk (single SQLite transaction).
    let paths = vec!["/a.jpg", "/b.jpg", "/c.jpg"];
    let tasks: Vec<_> = paths.iter().map(|p| ResizeTask {
        path: p.to_string(),
        width: 300,
    }).collect();
    media.submit_batch(tasks).await.unwrap();

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

For compile-time guarantees, implement `TypedTask` on your payload struct. This keeps the task type name, domain identity, and static configuration co-located with the payload.

`TypedTask` requires an associated `type Domain: DomainKey` that ties the task to a specific domain. Static defaults (priority, IO budget, TTL, retry policy, dedup strategy) are returned from `fn config() -> TaskTypeConfig`. Per-instance values like dedup keys and tags remain as instance methods.

```rust
use std::time::Duration;
use serde::{Serialize, Deserialize};
use taskmill::{TypedTask, TaskTypeConfig, IoBudget, Priority, DomainKey, RetryPolicy};

pub struct Media;
impl DomainKey for Media {
    const NAME: &'static str = "media";
}

#[derive(Serialize, Deserialize)]
struct ResizeTask {
    path: String,
    width: u32,
}

impl TypedTask for ResizeTask {
    type Domain = Media;
    const TASK_TYPE: &'static str = "resize";

    fn config() -> TaskTypeConfig {
        TaskTypeConfig::new()
            .expected_io(IoBudget::disk(4096, 1024))
            .priority(Priority::NORMAL)
            .ttl(Duration::from_secs(600))
            .retry(RetryPolicy::exponential(3, Duration::from_secs(1), Duration::from_secs(60)))
    }

    // Optional: custom dedup key derived from the payload.
    fn key(&self) -> Option<String> {
        Some(format!("resize:{}:{}", self.path, self.width))
    }
}

// Register using the typed form — task type comes from ResizeTask::TASK_TYPE,
// domain from ResizeTask::Domain. No Arc::new() needed.
Domain::<Media>::new().task::<ResizeTask>(ImageResizer);

// Submit — domain handle applies defaults and prefixes the type.
media.submit(ResizeTask {
    path: "/photos/img.jpg".into(),
    width: 300,
}).await?;
```

`submit_with()` returns a `DomainSubmitBuilder` — chain overrides before awaiting:

```rust
media.submit_with(task)
    .priority(Priority::HIGH)
    .run_after(Duration::from_secs(30))
    .await?;
```

## Child tasks

Some work is naturally hierarchical — a multipart upload needs to upload individual parts, then call `CompleteMultipartUpload`. Taskmill supports this with child tasks and two-phase execution.

Spawn children from within an executor using `ctx.spawn_child_with()`. Children are automatically domain-aware: the task type is prefixed and domain defaults are applied. The parent enters a `waiting` state until all children complete, then `finalize()` is called on the parent executor.

```rust
impl TypedExecutor<MultipartUpload> for MultipartUploader {
    async fn execute<'a>(
        &'a self, upload: MultipartUpload, ctx: DomainTaskContext<'a, Media>,
    ) -> Result<(), TaskError> {
        let parts = split_into_parts(&upload);
        for part in parts {
            ctx.spawn_child_with(UploadPart {
                etag: part.etag.clone(),
                size: part.size,
            }).await?;
        }
        Ok(()) // parent enters 'waiting' state
    }

    async fn finalize<'a>(
        &'a self, upload: MultipartUpload, _memo: (), ctx: DomainTaskContext<'a, Media>,
    ) -> Result<(), TaskError> {
        // Called after all children complete
        complete_multipart_upload(&upload).await
    }
}
```

For cross-domain children, use `ctx.domain::<D>()` to get a handle and `.child_of(&ctx)` on the submit builder:

```rust
ctx.domain::<Storage>()
    .submit_with(Upload { /* ... */ })
    .child_of(&ctx)
    .await?;
```

By default, if any child fails, its siblings are cancelled and the parent fails immediately (fail-fast). Disable this per-submission with `.fail_fast(false)`:

```rust
// Typed builder
media.submit_with(ScanTask { .. })
    .fail_fast(false)
    .await?;

// Untyped
scheduler.submit(
    TaskSubmission::new("scan")
        .fail_fast(false)
).await?;
```

## Sibling tasks

When a child task needs to spawn peer tasks under the same parent (flat hierarchy), use `ctx.spawn_sibling_with()` instead of manually extracting and threading the parent ID. The new task's `parent_id` is set to the current task's `parent_id`, making it a peer under the same orchestrator.

```rust
impl TypedExecutor<ScanL1DirTask> for DirScanner {
    async fn execute<'a>(
        &'a self, task: ScanL1DirTask, ctx: DomainTaskContext<'a, Scanner>,
    ) -> Result<(), TaskError> {
        for subdir in list_subdirs(&task.prefix).await? {
            ctx.spawn_sibling_with(ScanL1DirTask {
                bucket: task.bucket.clone(),
                prefix: subdir,
            })
            .key(&format!("{}:{}", task.bucket, subdir))
            .await?;
        }
        Ok(())
    }
}
```

For high-fan-out patterns, use `spawn_siblings_with()` which routes through a single-transaction batch path:

```rust
let siblings: Vec<ScanL1DirTask> = subdirs.into_iter()
    .map(|d| ScanL1DirTask { bucket: bucket.clone(), prefix: d })
    .collect();
ctx.spawn_siblings_with(siblings).await?;
```

If the current task has no parent (i.e. it's a root task), both methods return `StoreError::InvalidState` instead of silently creating a root task.

For cross-domain siblings, use `.sibling_of(&ctx)` on the submit builder:

```rust
ctx.domain::<Analytics>()
    .submit_with(ScanStartedEvent { .. })
    .sibling_of(&ctx)?
    .priority(Priority::HIGH)
    .await?;
```

| Method | `parent_id` on new task |
|---|---|
| `submit_with(task)` | `None` (root) |
| `submit_with(task).parent(id)` | Explicit ID |
| `ctx.spawn_child_with(task)` | Current task's ID |
| `ctx.spawn_sibling_with(task)` | Current task's `parent_id` |

## Sharing the scheduler

A single `Scheduler` is `Clone` (via `Arc`) and can be shared across your entire application. Domains can carry their own scoped state, so library domains don't need to share a namespace with the host app's state.

```rust
use taskmill::{Domain, DomainKey, DomainHandle, Scheduler};

pub struct Media;
impl DomainKey for Media { const NAME: &'static str = "media"; }

pub struct Sync;
impl DomainKey for Sync { const NAME: &'static str = "sync"; }

// Each domain brings its own state.
let scheduler = Scheduler::builder()
    .store_path("app.db")
    .domain(
        Domain::<Media>::new()
            .task::<ResizeTask>(ImageResizer)
            .state(MediaConfig { cdn_url: "...".into() })
    )
    .domain(
        Domain::<Sync>::new()
            .task::<RemoteSyncTask>(SyncExecutor)
            .state(SyncConfig { endpoint: "...".into() })
    )
    // Global state shared across all domains.
    .app_state(SharedDb::new())
    .max_concurrency(8)
    .build()
    .await
    .unwrap();

// Each domain's state is visible to its own executors first,
// then falls back to global state.
let media_state = ctx.state::<MediaConfig>(); // only in media domain executors
let db = ctx.state::<SharedDb>();             // anywhere

// The host manages the run loop.
let token = CancellationToken::new();
scheduler.run(token).await;
```

Libraries that receive a pre-built scheduler can still inject global state after construction (call before `scheduler.run()`):

```rust
scheduler.register_state(Arc::new(LibraryState { /* ... */ })).await;
```

For applications with more than two domains, or when integrating third-party library domains, see [Multi-Module Applications](multi-module-apps.md) for guidance on cross-domain dependencies, concurrency budgets, and coordinated cancellation.

## Delayed and recurring tasks

### Delayed tasks

Schedule a task to run after a specific delay or at a specific point in time.

```rust
use std::time::Duration;
use chrono::Utc;

// Run after a delay
media.submit_with(CleanupTask {
    path: "/tmp/stale".into(),
}).run_after(Duration::from_secs(3600)).await?;

// Run at a specific time (via raw submission for run_at support)
media.submit_raw(
    TaskSubmission::new("report")
        .payload_json(&serde_json::json!({"date": "2025-01-15"}))
        .run_at(Utc::now() + chrono::Duration::hours(6))
).await?;
```

If the `run_after` time is in the past (e.g., because the app was offline), the task runs immediately on the next dispatch cycle.

### Recurring tasks

A recurring task automatically re-submits itself on a schedule after each completion.

```rust
use taskmill::{TaskSubmission, RecurringSchedule};

media.submit_raw(
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

Recurring schedules can be paused, resumed, or cancelled at runtime through the domain handle:

```rust
let media: DomainHandle<Media> = scheduler.domain::<Media>();

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
let media: DomainHandle<Media> = scheduler.domain::<Media>();

let upload = media.submit(UploadFileTask {
    path: "/data/file.bin".into(),
}).await?;

// Only runs after upload succeeds
media.submit_with(DeleteOldVersionTask {
    path: "/data/file.bin".into(),
}).depends_on(upload.id().unwrap()).await?;
```

### Fan-in (multiple dependencies)

```rust
let a = media.submit(FetchTask { source: "a".into() }).await?;
let b = media.submit(FetchTask { source: "b".into() }).await?;

// Only runs after both A and B complete
media.submit_with(MergeTask { sources: vec!["a".into(), "b".into()] })
    .depends_on_all([a.id().unwrap(), b.id().unwrap()])
    .await?;
```

### Cross-domain dependencies

Dependencies work across domain boundaries. A task in one domain can depend on a task in another domain — the domain boundary does not affect dependency resolution or failure propagation.

```rust
pub struct Ingest;
impl DomainKey for Ingest { const NAME: &'static str = "ingest"; }

pub struct Process;
impl DomainKey for Process { const NAME: &'static str = "process"; }

let ingest: DomainHandle<Ingest> = scheduler.domain::<Ingest>();
let process: DomainHandle<Process> = scheduler.domain::<Process>();

// Submit in the ingest domain, capture the ID.
let outcome = ingest.submit(FetchTask { url: source.clone() }).await?;
let fetch_id = outcome.id().expect("not a duplicate");

// This task in the process domain won't start until the fetch completes.
process.submit_with(TranscodeTask { source })
    .depends_on(fetch_id)
    .await?;
```

See [Multi-Module Applications](multi-module-apps.md#cross-module-task-dependencies) for more patterns including failure cascades and in-executor cross-domain submission.

### Failure handling

By default, if a dependency fails permanently the dependent is cancelled and recorded as `DependencyFailed` in history. Change this per-submission:

```rust
use taskmill::DependencyFailurePolicy;

media.submit_with(CleanupTask { path: "/tmp/stale".into() })
    .depends_on(upload_id)
    .on_dependency_failure(DependencyFailurePolicy::Ignore) // run anyway
    .await?;
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
use taskmill::{Scheduler, DomainHandle, DomainKey, SchedulerSnapshot, StoreError};

pub struct Media;
impl DomainKey for Media { const NAME: &'static str = "media"; }

// Expose scheduler status to the frontend.
#[tauri::command]
async fn scheduler_status(
    scheduler: tauri::State<'_, Scheduler>,
) -> Result<SchedulerSnapshot, StoreError> {
    scheduler.snapshot().await
}

// Submit tasks from frontend commands.
#[tauri::command]
async fn resize_image(
    scheduler: tauri::State<'_, Scheduler>,
    path: String,
    width: u32,
) -> Result<(), StoreError> {
    let media: DomainHandle<Media> = scheduler.domain::<Media>();
    media.submit(ResizeTask { path, width }).await?;
    Ok(())
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
7. [Metrics & Observability](metrics.md) — internal counters, `metrics` crate integration, Prometheus dashboards
8. [Multi-Module Applications](multi-module-apps.md) — assemble multiple domains, cross-domain dependencies, tags, and dashboards
9. [Writing a Reusable Module](library-modules.md) — publish a domain as a library crate
10. [Design](design.md) — understand the architecture for advanced use
