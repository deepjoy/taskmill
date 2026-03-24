# Multi-Module Applications

Most production applications need more than one domain. An upload service has ingestion, processing, and notification stages. A media app has transcoding, thumbnail generation, and CDN sync. This guide covers how to assemble multiple domains on a single `Scheduler` and the interactions you need to understand.

## When you need multiple modules

A single domain is fine when all your task types share the same defaults (priority, retry policy, concurrency cap, state). Once you have task types with different operational characteristics — or you're pulling in library crates that bring their own domains — you need multiple domains.

Common reasons:

- **Different concurrency budgets.** Uploads should run 4-wide; thumbnail generation can run 16-wide.
- **Different retry policies.** API calls need exponential backoff; local file operations retry immediately.
- **Scoped state.** Each domain carries its own configuration without polluting a global namespace.
- **Library integration.** Third-party crates register their own domains — you compose them alongside your own.

## App layout: one domain function per feature

Define each domain as a standalone function that returns a `Domain<D>`. This keeps registration clean and makes domains testable in isolation. Each domain is anchored to a zero-sized `DomainKey` struct that gives it a compile-time identity.

```rust
use std::time::Duration;
use taskmill::{Domain, DomainKey, Priority, RetryPolicy};

// Domain identity types — one per feature.
pub struct Ingest;
impl DomainKey for Ingest { const NAME: &'static str = "ingest"; }

pub struct Process;
impl DomainKey for Process { const NAME: &'static str = "process"; }

pub struct Notify;
impl DomainKey for Notify { const NAME: &'static str = "notify"; }

pub fn ingest_domain(config: IngestConfig) -> Domain<Ingest> {
    Domain::<Ingest>::new()
        .task::<FetchTask>(FetchExecutor)
        .task::<ValidateTask>(ValidateExecutor)
        .state(config)
        .max_concurrency(4)
        .default_priority(Priority::NORMAL)
}

pub fn process_domain() -> Domain<Process> {
    Domain::<Process>::new()
        .task::<TranscodeTask>(TranscodeExecutor)
        .task::<ThumbnailTask>(ThumbnailExecutor)
        .max_concurrency(8)
        .default_priority(Priority::BACKGROUND)
}

pub fn notify_domain(config: NotifyConfig) -> Domain<Notify> {
    Domain::<Notify>::new()
        .task::<SendEmailTask>(EmailExecutor)
        .state(config)
        .default_retry(RetryPolicy::exponential(5, Duration::from_secs(5), Duration::from_secs(300)))
}
```

Register all domains at build time:

```rust
let scheduler = Scheduler::builder()
    .store_path("app.db")
    .domain(ingest_domain(ingest_config))
    .domain(process_domain())
    .domain(notify_domain(notify_config))
    .app_state(SharedDb::new())     // global state visible to all domains
    .max_concurrency(16)            // global cap
    .build()
    .await?;
```

`build()` returns an error if two domains share the same name. Use distinct, descriptive names — see [Writing a Reusable Module](library-modules.md#naming-avoid-conflicts-export-a-constant) for naming guidance.

## Global state vs. module state — what goes where

State registered on `SchedulerBuilder::app_state()` is **global** — visible to executors in every domain. State registered on `Domain::state()` is **domain-scoped** — visible only to executors in that domain.

`DomainTaskContext::state::<T>()` checks domain-scoped state first, then falls back to global state.

| What | Where to register | Why |
|------|-------------------|-----|
| Database pool, HTTP client | Global (`builder.app_state()`) | Shared infrastructure used by many domains |
| Domain-specific config (API keys, bucket names) | Domain (`Domain::state()`) | Only relevant to one domain's executors |
| Feature flags, metrics collector | Global | Cross-cutting concerns |

```rust
// In an executor:
let db = ctx.state::<SharedDb>().expect("SharedDb not registered");       // global
let cfg = ctx.state::<IngestConfig>().expect("IngestConfig not registered"); // domain-scoped
```

If two domains register the same type `T` as domain state, each domain's executors see their own instance. The global instance (if any) is shadowed within each domain.

## Sharing the scheduler: Clone and DomainHandle

`Scheduler` is `Clone` (via `Arc`) — pass it freely to async tasks, Tauri commands, or API handlers. Grab typed domain handles at startup for convenient access:

```rust
let scheduler = build_scheduler().await?;

// Grab handles once — they're Clone too.
let ingest: DomainHandle<Ingest> = scheduler.domain::<Ingest>();
let process: DomainHandle<Process> = scheduler.domain::<Process>();

// Use from anywhere.
tokio::spawn(async move {
    ingest.submit(FetchTask { url: "...".into() }).await.unwrap();
});
```

`scheduler.domain::<D>()` panics if the domain isn't registered — use it at well-known call sites where a missing registration is a programming error. For dynamic lookups (e.g., plugin systems), use `scheduler.try_domain::<D>()` which returns `Option<DomainHandle<D>>`.

## Cross-module task dependencies

A task in one domain can depend on a task in another domain. Cross-domain dependencies work identically to same-domain dependencies — the domain boundary does not affect dependency resolution or failure propagation.

### The pattern

Submit a task in domain A, capture its ID, then use that ID as a dependency in domain B:

```rust
let ingest: DomainHandle<Ingest> = scheduler.domain::<Ingest>();
let process: DomainHandle<Process> = scheduler.domain::<Process>();

// Submit in the ingest domain.
let outcome = ingest.submit(FetchTask {
    url: source_url.clone(),
}).await?;

let fetch_id = outcome.id().expect("not a duplicate");

// This task in the process domain won't start until the fetch completes.
process.submit_with(TranscodeTask {
    source: source_url,
})
    .depends_on(fetch_id)
    .await?;
```

### Failure cascade across modules

If the ingest task fails permanently, its dependents in the process domain follow the `DependencyFailurePolicy` — the domain boundary is irrelevant. The default policy (`Cancel`) moves the dependent to history as `DependencyFailed` and cascades to further dependents.

```rust
use taskmill::DependencyFailurePolicy;

// Run the transcode anyway, even if the fetch failed.
process.submit_with(TranscodeTask { source })
    .depends_on(fetch_id)
    .on_dependency_failure(DependencyFailurePolicy::Ignore)
    .await?;
```

### From within an executor

Executors can submit to other domains via `ctx.domain::<D>()`:

```rust
impl TypedExecutor<FetchTask> for FetchExecutor {
    async fn execute(&self, task: FetchTask, ctx: DomainTaskContext<'_, Ingest>) -> Result<(), TaskError> {
        let data = fetch_remote(&task).await?;

        // Submit a follow-up in a different domain.
        ctx.domain::<Process>().submit(TranscodeTask {
            source: data.path,
        }).await.map_err(|e| TaskError::permanent(e.to_string()))?;

        Ok(())
    }
}
```

Use `ctx.try_domain::<Analytics>()` if the target domain is optional (e.g., an analytics plugin that may not be registered).

## Concurrency budgets across modules

### Global cap and per-module cap as AND-gates

Both the global `max_concurrency` and per-domain `max_concurrency` must have headroom for a task to be dispatched. They are **caps, not reservations** — setting a domain's cap to 4 does not guarantee it gets 4 slots.

```
Global max_concurrency: 16
  ├── ingest:  max_concurrency(4)   ← at most 4, but only if global has room
  ├── process: max_concurrency(8)   ← at most 8, but only if global has room
  └── notify:  (no cap)             ← limited only by the global cap
```

A task is dispatched when **all** of these pass:
1. `active_count < global max_concurrency`
2. `domain_running_count < domain max_concurrency` (if set)
3. `group_running_count < group concurrency limit` (if the task has a group)
4. Backpressure / IO budget check passes

### Module starvation: understanding priority competition

A domain with only `BACKGROUND`-priority tasks can be indefinitely deferred when other domains continuously submit `NORMAL` work. This is by design — priority ordering is global across all domains.

If you need guaranteed throughput for a domain:
- **Raise the priority** of its most important tasks to `NORMAL` or `HIGH`.
- **Use task groups** with a dedicated concurrency reservation. A group limit acts as a soft floor: tasks in the group bypass the global priority queue as long as the group has available slots.

### Using group concurrency as a soft floor

```rust
pub struct Sync;
impl DomainKey for Sync { const NAME: &'static str = "sync"; }

// Reserve 2 concurrent slots for background sync, even under load.
let scheduler = Scheduler::builder()
    .domain(
        Domain::<Sync>::new()
            .task::<SyncTask>(SyncExecutor)
            .default_group("sync-reserved")
            .default_priority(Priority::BACKGROUND)
    )
    .group_concurrency("sync-reserved", 2)
    .max_concurrency(16)
    .build()
    .await?;
```

## Coordinating with tags: logical jobs across modules

Tags let you group tasks that belong to the same logical "job" across multiple domains. This is the idiomatic way to cancel, query, or monitor a pipeline that spans domains.

### Tagging tasks at submit time

```rust
let job_id = generate_job_id();

let ingest: DomainHandle<Ingest> = scheduler.domain::<Ingest>();
let process: DomainHandle<Process> = scheduler.domain::<Process>();

ingest.submit_with(FetchTask { url: source.clone() })
    .tag("job_id", &job_id)
    .await?;

process.submit_with(TranscodeTask { source })
    .tag("job_id", &job_id)
    .await?;
```

### Querying job progress across modules

Use typed domain handles to query individual domains and aggregate:

```rust
let ingest: DomainHandle<Ingest> = scheduler.domain::<Ingest>();
let process: DomainHandle<Process> = scheduler.domain::<Process>();

let ingest_snap = ingest.snapshot().await?;
let process_snap = process.snapshot().await?;

let total_pending = ingest_snap.pending_count + process_snap.pending_count;
let total_running = ingest_snap.running.len() + process_snap.running.len();
```

### Bulk-cancelling a job across domains

```rust
let ingest: DomainHandle<Ingest> = scheduler.domain::<Ingest>();
let process: DomainHandle<Process> = scheduler.domain::<Process>();

ingest.cancel_where(|task| {
    task.tags.get("job_id").map(String::as_str) == Some(&job_id)
}).await?;

process.cancel_where(|task| {
    task.tags.get("job_id").map(String::as_str) == Some(&job_id)
}).await?;
```

### Namespace-scoped queries with tag key prefixes

When multiple libraries share a scheduler, each naturally namespaces its tags with a prefix (`billing.customer_id`, `media.pipeline`, etc.). Use the tag key prefix APIs to discover and operate on an entire namespace without knowing every possible key upfront:

```rust
let billing: DomainHandle<Billing> = scheduler.domain::<Billing>();

// Discover all billing.* tag keys in use
let keys = billing.tag_keys_by_prefix("billing.").await?;
// e.g. ["billing.customer_id", "billing.plan", "billing.region"]

// Count how many billing tasks are active
let count = billing.count_by_tag_key_prefix("billing.", None).await?;

// Fetch IDs of matching tasks (optionally filter by status)
let task_ids = billing.task_ids_by_tag_key_prefix("billing.", Some(TaskStatus::Pending)).await?;

// Cancel all tasks in the billing namespace
let cancelled = billing.cancel_by_tag_key_prefix("billing.").await?;
```

LIKE wildcards (`%`, `_`) in the prefix are escaped automatically — only true prefix matching is performed.

## Module-level pause and resume

Each domain can be independently paused and resumed without affecting other domains.

```rust
let ingest: DomainHandle<Ingest> = scheduler.domain::<Ingest>();

// Pause — stops new task dispatch for this domain. Running tasks are
// interrupted (cancellation token triggered) and moved to paused status.
// Pending tasks are also moved to paused.
ingest.pause().await?;

// Resume — clears the domain pause flag and moves paused tasks back to pending.
ingest.resume().await?;

// Check state.
assert!(ingest.is_paused()); // after pause()
```

### Interaction with global pause

The scheduler must be globally **unpaused** AND the domain must be **unpaused** for dispatch to proceed. Domain pause is additive:

| `scheduler.pause_all()` | `handle.pause()` | Dispatch? |
|--------------------------|-------------------|-----------|
| No | No | Yes |
| No | Yes | No |
| Yes | No | No |
| Yes | Yes | No |

`handle.resume()` clears the domain flag but does **not** override a global pause — database tasks stay `paused` until the global scheduler is also resumed.

### Use case

A library domain with a background sync feature that the user can toggle from settings:

```rust
// User toggles sync off in the UI.
scheduler.domain::<Sync>().pause().await?;

// Other domains continue running normally.
scheduler.domain::<Ingest>().submit(task).await?; // still works

// User turns sync back on.
scheduler.domain::<Sync>().resume().await?;
```

## Building a cross-module dashboard

### scheduler.snapshot() vs. per-module snapshot()

`scheduler.snapshot()` returns a `SchedulerSnapshot` with global aggregates — total running, pending, pressure, and progress across all domains.

`handle.snapshot()` returns a `ModuleSnapshot` with per-domain detail — running tasks, pending count, paused count, progress, and byte-level tracking for that domain only.

For a per-domain dashboard, query each domain handle:

```rust
let ingest: DomainHandle<Ingest> = scheduler.domain::<Ingest>();
let process: DomainHandle<Process> = scheduler.domain::<Process>();
let notify: DomainHandle<Notify> = scheduler.domain::<Notify>();

async fn print_snap(name: &str, snap: ModuleSnapshot) {
    println!(
        "{}: {} running, {} pending, {} paused",
        name,
        snap.running.len(),
        snap.pending_count,
        snap.paused_count,
    );
}

print_snap(ingest.name(), ingest.snapshot().await?);
print_snap(process.name(), process.snapshot().await?);
print_snap(notify.name(), notify.snapshot().await?);
```

### scheduler.active_tasks() for a unified running view

`scheduler.active_tasks()` returns all running tasks from all domains in a single `Vec<TaskRecord>`. Equivalent to aggregating each domain's `active_tasks()`, but more convenient for global views.

```rust
let running = scheduler.active_tasks().await;
for task in &running {
    println!("[{}] {} (priority {})", task.task_type, task.label, task.priority.value());
}
```

## Error isolation between modules

Domains share a scheduler but their errors don't interact (beyond explicit dependencies).

### Per-module dead-letter monitoring

Each domain has its own dead-letter view:

```rust
let ingest: DomainHandle<Ingest> = scheduler.domain::<Ingest>();
let failed = ingest.dead_letter_tasks(10, 0).await?;
for task in &failed {
    println!("[ingest] {} failed: {}", task.label, task.last_error.as_deref().unwrap_or("unknown"));
}
```

### Different retry policies per module

Set retry policies at the domain level so each domain handles failures appropriately:

```rust
pub struct Api;
impl DomainKey for Api { const NAME: &'static str = "api"; }

pub struct Files;
impl DomainKey for Files { const NAME: &'static str = "files"; }

// API calls: exponential backoff, 5 retries.
Domain::<Api>::new()
    .default_retry(RetryPolicy::exponential(5, Duration::from_secs(1), Duration::from_secs(120)))

// Local file operations: immediate retry, 3 attempts.
Domain::<Files>::new()
    .default_retry(RetryPolicy::constant(3, Duration::ZERO))
```

### Module-filtered events

Subscribe to events for a single domain without filtering the global stream:

```rust
let ingest: DomainHandle<Ingest> = scheduler.domain::<Ingest>();
let mut rx = ingest.events();
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
use taskmill::{Scheduler, TaskStore, Domain, DomainKey};

#[tokio::test]
async fn test_cross_domain_pipeline() {
    let store = TaskStore::open_memory().await.unwrap();
    let scheduler = Scheduler::builder()
        .store(store)
        .domain(ingest_domain(test_config()))
        .domain(process_domain())
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
