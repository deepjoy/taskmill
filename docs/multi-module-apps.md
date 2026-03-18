# Multi-Module Applications

Most production applications need more than one module. An upload service has ingestion, processing, and notification stages. A media app has transcoding, thumbnail generation, and CDN sync. This guide covers how to assemble multiple modules on a single `Scheduler` and the interactions you need to understand.

## When you need multiple modules

A single module is fine when all your task types share the same defaults (priority, retry policy, concurrency cap, state). Once you have task types with different operational characteristics — or you're pulling in library crates that bring their own modules — you need multiple modules.

Common reasons:

- **Different concurrency budgets.** Uploads should run 4-wide; thumbnail generation can run 16-wide.
- **Different retry policies.** API calls need exponential backoff; local file operations retry immediately.
- **Scoped state.** Each module carries its own configuration without polluting a global namespace.
- **Library integration.** Third-party crates register their own modules — you compose them alongside your own.

## App layout: one module function per feature

Define each module as a standalone function. This keeps registration clean and makes modules testable in isolation.

```rust
use std::sync::Arc;
use std::time::Duration;
use taskmill::{Module, Priority, RetryPolicy, BackoffStrategy};

pub fn ingest_module(config: IngestConfig) -> Module {
    Module::new("ingest")
        .typed_executor::<FetchTask, _>(Arc::new(FetchExecutor))
        .typed_executor::<ValidateTask, _>(Arc::new(ValidateExecutor))
        .app_state(config)
        .max_concurrency(4)
        .default_priority(Priority::NORMAL)
}

pub fn process_module() -> Module {
    Module::new("process")
        .typed_executor::<TranscodeTask, _>(Arc::new(TranscodeExecutor))
        .typed_executor::<ThumbnailTask, _>(Arc::new(ThumbnailExecutor))
        .max_concurrency(8)
        .default_priority(Priority::BACKGROUND)
}

pub fn notify_module(config: NotifyConfig) -> Module {
    Module::new("notify")
        .typed_executor::<SendEmailTask, _>(Arc::new(EmailExecutor))
        .app_state(config)
        .default_retry_policy(RetryPolicy {
            strategy: BackoffStrategy::Exponential {
                initial: Duration::from_secs(5),
                max: Duration::from_secs(300),
                multiplier: 2.0,
            },
            max_retries: 5,
        })
}
```

Register all modules at build time:

```rust
let scheduler = Scheduler::builder()
    .store_path("app.db")
    .module(ingest_module(ingest_config))
    .module(process_module())
    .module(notify_module(notify_config))
    .app_state(SharedDb::new())     // global state visible to all modules
    .max_concurrency(16)            // global cap
    .build()
    .await?;
```

`build()` returns an error if two modules share the same name. Use distinct, descriptive names — see [Writing a Reusable Module](library-modules.md#naming-avoid-conflicts-export-a-constant) for naming guidance.

## Global state vs. module state — what goes where

State registered on `SchedulerBuilder::app_state()` is **global** — visible to executors in every module. State registered on `Module::app_state()` is **module-scoped** — visible only to executors in that module.

`TaskContext::state::<T>()` checks module-scoped state first, then falls back to global state.

| What | Where to register | Why |
|------|-------------------|-----|
| Database pool, HTTP client | Global (`builder.app_state()`) | Shared infrastructure used by many modules |
| Module-specific config (API keys, bucket names) | Module (`Module::app_state()`) | Only relevant to one module's executors |
| Feature flags, metrics collector | Global | Cross-cutting concerns |

```rust
// In an executor:
let db = ctx.state::<SharedDb>().expect("SharedDb not registered");       // global
let cfg = ctx.state::<IngestConfig>().expect("IngestConfig not registered"); // module-scoped
```

If two modules register the same type `T` as module state, each module's executors see their own instance. The global instance (if any) is shadowed within each module.

## Sharing the scheduler: Clone and ModuleHandle

`Scheduler` is `Clone` (via `Arc`) — pass it freely to async tasks, Tauri commands, or API handlers. Grab module handles at startup for convenient access:

```rust
let scheduler = build_scheduler().await?;

// Grab handles once — they're Clone too.
let ingest = scheduler.module("ingest");
let process = scheduler.module("process");

// Use from anywhere.
tokio::spawn(async move {
    ingest.submit_typed(&FetchTask { url: "...".into() }).await.unwrap();
});
```

`scheduler.module("name")` panics if the module isn't registered — use it at well-known call sites where a typo is a programming error. For dynamic lookups (e.g., plugin systems), use `scheduler.try_module("name")` which returns `Option<ModuleHandle>`.

## Cross-module task dependencies

A task in one module can depend on a task in another module. Cross-module dependencies work identically to same-module dependencies — the module boundary does not affect dependency resolution or failure propagation.

### The pattern

Submit a task in module A, capture its ID, then use that ID as a dependency in module B:

```rust
let ingest = scheduler.module("ingest");
let process = scheduler.module("process");

// Submit in the ingest module.
let outcome = ingest.submit_typed(&FetchTask {
    url: source_url.clone(),
}).await?;

let fetch_id = outcome.id().expect("not a duplicate");

// This task in the process module won't start until the fetch completes.
process.submit_typed(&TranscodeTask {
    source: source_url,
})
    .depends_on(fetch_id)
    .await?;
```

### Failure cascade across modules

If the ingest task fails permanently, its dependents in the process module follow the `DependencyFailurePolicy` — the module boundary is irrelevant. The default policy (`Cancel`) moves the dependent to history as `DependencyFailed` and cascades to further dependents.

```rust
use taskmill::DependencyFailurePolicy;

// Run the transcode anyway, even if the fetch failed.
process.submit_typed(&TranscodeTask { source })
    .depends_on(fetch_id)
    .on_dependency_failure(DependencyFailurePolicy::Ignore)
    .await?;
```

### From within an executor

Executors can submit to other modules via `ctx.module("name")`:

```rust
impl TaskExecutor for FetchExecutor {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        let data = fetch_remote(&ctx.payload::<FetchTask>()?).await?;

        // Submit a follow-up in a different module.
        ctx.module("process").submit_typed(&TranscodeTask {
            source: data.path,
        }).await.map_err(|e| TaskError::permanent(e.to_string()))?;

        Ok(())
    }
}
```

Use `ctx.try_module("analytics")` if the target module is optional (e.g., an analytics plugin that may not be registered).

## Concurrency budgets across modules

### Global cap and per-module cap as AND-gates

Both the global `max_concurrency` and per-module `max_concurrency` must have headroom for a task to be dispatched. They are **caps, not reservations** — setting a module's cap to 4 does not guarantee it gets 4 slots.

```
Global max_concurrency: 16
  ├── ingest:  max_concurrency(4)   ← at most 4, but only if global has room
  ├── process: max_concurrency(8)   ← at most 8, but only if global has room
  └── notify:  (no cap)             ← limited only by the global cap
```

A task is dispatched when **all** of these pass:
1. `active_count < global max_concurrency`
2. `module_running_count < module max_concurrency` (if set)
3. `group_running_count < group concurrency limit` (if the task has a group)
4. Backpressure / IO budget check passes

### Module starvation: understanding priority competition

A module with only `BACKGROUND`-priority tasks can be indefinitely deferred when other modules continuously submit `NORMAL` work. This is by design — priority ordering is global across all modules.

If you need guaranteed throughput for a module:
- **Raise the priority** of its most important tasks to `NORMAL` or `HIGH`.
- **Use task groups** with a dedicated concurrency reservation. A group limit acts as a soft floor: tasks in the group bypass the global priority queue as long as the group has available slots.

### Using group concurrency as a soft floor

```rust
// Reserve 2 concurrent slots for background sync, even under load.
let scheduler = Scheduler::builder()
    .module(
        Module::new("sync")
            .typed_executor::<SyncTask, _>(Arc::new(SyncExecutor))
            .default_group("sync-reserved")
            .default_priority(Priority::BACKGROUND)
    )
    .group_concurrency("sync-reserved", 2)
    .max_concurrency(16)
    .build()
    .await?;
```

## Coordinating with tags: logical jobs across modules

Tags let you group tasks that belong to the same logical "job" across multiple modules. This is the idiomatic way to cancel, query, or monitor a pipeline that spans modules.

### Tagging tasks at submit time

```rust
let job_id = generate_job_id();

scheduler.module("ingest")
    .submit_typed(&FetchTask { url: source.clone() })
    .tag("job_id", &job_id)
    .await?;

scheduler.module("process")
    .submit_typed(&TranscodeTask { source })
    .tag("job_id", &job_id)
    .await?;
```

### Querying job progress across modules

Use `scheduler.modules()` to iterate over all modules and aggregate:

```rust
let mut total_pending = 0i64;
let mut total_running = 0;
for handle in scheduler.modules() {
    let snap = handle.snapshot().await?;
    total_pending += snap.pending_count;
    total_running += snap.running.len();
}
```

### Bulk-cancelling a job via scheduler.modules()

```rust
for handle in scheduler.modules() {
    handle.cancel_where(|task| {
        task.tags.get("job_id").map(String::as_str) == Some(&job_id)
    }).await?;
}
```

## Module-level pause and resume

Each module can be independently paused and resumed without affecting other modules.

```rust
let ingest = scheduler.module("ingest");

// Pause — stops new task dispatch for this module. Running tasks are
// interrupted (cancellation token triggered) and moved to paused status.
// Pending tasks are also moved to paused.
ingest.pause().await?;

// Resume — clears the module pause flag and moves paused tasks back to pending.
ingest.resume().await?;

// Check state.
assert!(ingest.is_paused()); // after pause()
```

### Interaction with global pause

The scheduler must be globally **unpaused** AND the module must be **unpaused** for dispatch to proceed. Module pause is additive:

| `scheduler.pause_all()` | `handle.pause()` | Dispatch? |
|--------------------------|-------------------|-----------|
| No | No | Yes |
| No | Yes | No |
| Yes | No | No |
| Yes | Yes | No |

`handle.resume()` clears the module flag but does **not** override a global pause — database tasks stay `paused` until the global scheduler is also resumed.

### Use case

A library module with a background sync feature that the user can toggle from settings:

```rust
// User toggles sync off in the UI.
scheduler.module("sync").pause().await?;

// Other modules continue running normally.
scheduler.module("ingest").submit_typed(&task).await?; // still works

// User turns sync back on.
scheduler.module("sync").resume().await?;
```

## Building a cross-module dashboard

### scheduler.snapshot() vs. per-module snapshot()

`scheduler.snapshot()` returns a `SchedulerSnapshot` with global aggregates — total running, pending, pressure, and progress across all modules.

`handle.snapshot()` returns a `ModuleSnapshot` with per-module detail — running tasks, pending count, paused count, progress, and byte-level tracking for that module only.

For a per-module dashboard, iterate:

```rust
for handle in scheduler.modules() {
    let snap = handle.snapshot().await?;
    println!(
        "{}: {} running, {} pending, {} paused",
        handle.name(),
        snap.running.len(),
        snap.pending_count,
        snap.paused_count,
    );
}
```

### scheduler.active_tasks() for a unified running view

`scheduler.active_tasks()` returns all running tasks from all modules in a single `Vec<TaskRecord>`. Equivalent to aggregating each module's `active_tasks()`, but more convenient for global views.

```rust
let running = scheduler.active_tasks().await?;
for task in &running {
    println!("[{}] {} (priority {})", task.task_type, task.label, task.priority.value());
}
```

## Error isolation between modules

Modules share a scheduler but their errors don't interact (beyond explicit dependencies).

### Per-module dead-letter monitoring

Each module has its own dead-letter view:

```rust
let failed = scheduler.module("ingest").dead_letter_tasks(10, 0).await?;
for task in &failed {
    println!("[ingest] {} failed: {}", task.label, task.last_error.as_deref().unwrap_or("unknown"));
}
```

### Different retry policies per module

Set retry policies at the module level so each module handles failures appropriately:

```rust
// API calls: exponential backoff, 5 retries.
Module::new("api")
    .default_retry_policy(RetryPolicy {
        strategy: BackoffStrategy::Exponential {
            initial: Duration::from_secs(1),
            max: Duration::from_secs(120),
            multiplier: 2.0,
        },
        max_retries: 5,
    })

// Local file operations: immediate retry, 3 attempts.
Module::new("files")
    .default_retry_policy(RetryPolicy {
        strategy: BackoffStrategy::Fixed(Duration::ZERO),
        max_retries: 3,
    })
```

### Module-filtered events

Subscribe to events for a single module without filtering the global stream:

```rust
let mut rx = scheduler.module("ingest").subscribe();
tokio::spawn(async move {
    while let Ok(event) = rx.recv().await {
        // Only ingest:: events arrive here.
        handle_ingest_event(event).await;
    }
});
```

## Testing multi-module setups

Use `TaskStore::open_memory()` for fast, isolated tests that don't touch the filesystem:

```rust
use taskmill::{Scheduler, TaskStore, Module};

#[tokio::test]
async fn test_cross_module_pipeline() {
    let store = TaskStore::open_memory().await.unwrap();
    let scheduler = Scheduler::builder()
        .store(store)
        .module(ingest_module(test_config()))
        .module(process_module())
        .max_concurrency(4)
        .build()
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched = scheduler.clone();
    let run_handle = tokio::spawn(async move { sched.run(token.clone()).await });

    // Submit tasks, assert outcomes...

    token.cancel();
    run_handle.await.unwrap();
}
```

Each test gets its own in-memory database, so tests run in parallel without interference.
