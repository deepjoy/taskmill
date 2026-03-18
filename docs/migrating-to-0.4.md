# Migrating from 0.3.x to 0.4.0

0.4.0 reorganizes the entire submission and execution API around **modules** — self-contained bundles of task executors, defaults, and resource policy. This is the dominant breaking change and touches almost every call site. The remaining changes (IO budget consolidation, event headers, retry backoff) are mechanical and covered after the module migration.

> **Database compatibility: existing databases must be wiped before running 0.4.0.**
>
> Task type strings stored in `tasks` and `task_history` use the bare form from 0.3.x (e.g. `"thumbnail"`). In 0.4.0 all task types are prefixed with the module name (e.g. `"media::thumbnail"`). The scheduler cannot route or dispatch records with the old format. Delete the database file before first run — there is no in-place upgrade path.

---

## Modules replace direct executor registration

In 0.3.x, executors were registered directly on `SchedulerBuilder` and tasks were submitted directly on `Scheduler`. In 0.4.0, executors are grouped into a `Module` and tasks are submitted through the module's handle.

### SchedulerBuilder

**Before:**
```rust
let scheduler = Scheduler::builder()
    .store_path("tasks.db")
    .executor("thumbnail", Arc::new(ThumbnailExec))
    .typed_executor::<Transcode>(Arc::new(TranscodeExec))
    .executor_with_ttl("upload", Arc::new(UploadExec), Duration::from_secs(600))
    .max_concurrency(8)
    .build()
    .await?;
```

**After:**
```rust
let scheduler = Scheduler::builder()
    .store_path("tasks.db")
    .module(
        Module::new("media")
            .executor("thumbnail", Arc::new(ThumbnailExec))
            .typed_executor::<Transcode>(Arc::new(TranscodeExec))
            .executor_with_ttl("upload", Arc::new(UploadExec), Duration::from_secs(600))
    )
    .max_concurrency(8)
    .build()
    .await?;
```

At least one `.module()` call is required — `build()` returns an error if no modules are registered. There is no default module. Library authors publishing reusable modules should see [Writing a Reusable Module](library-modules.md) for naming conventions, state isolation, and conflict avoidance.

### Task type namespacing

Every task type is automatically prefixed with `"{module_name}::"` at build time. A task type `"thumbnail"` in a module named `"media"` is stored and referenced as `"media::thumbnail"`.

**What this means for you:**
- `TaskRecord::task_type` now returns `"media::thumbnail"`, not `"thumbnail"`
- Hard-coded task type strings in filters, logging, or history queries must be updated
- `TypedTask::TASK_TYPE` should still use the short form (e.g. `"thumbnail"`) — the prefix is applied by the module, not the executor

### Submitting tasks

**Before:**
```rust
scheduler.submit(TaskSubmission::new("thumbnail").payload_json(&thumb)).await?;
scheduler.submit_typed(&thumb).await?;
```

**After:**
```rust
let media = scheduler.module("media");
media.submit(TaskSubmission::new("thumbnail").payload_json(&thumb)).await?;
media.submit_typed(&thumb).await?;
```

`scheduler.module("media")` returns a `ModuleHandle` — a scoped handle for all operations within that module. It panics if the module is not registered; use `scheduler.try_module("media")` for a fallible variant.

### Cancellation, queries, and control

All per-task and scoped operations have moved from `Scheduler` to `ModuleHandle`.

| 0.3.x (`scheduler.*`)         | 0.4.0 (`handle.*`)              |
|-------------------------------|----------------------------------|
| `cancel(task_id)`             | `handle.cancel(task_id)`         |
| `cancel_group("g")`           | `handle.cancel_all()` or `handle.cancel_where(\|r\| r.group == Some("g".into()))` |
| `tasks_by_tags(filters, s)`   | `handle.tasks_by_tags(filters, s)` |
| `dead_letter_tasks(n, off)`   | `handle.dead_letter_tasks(n, off)` |
| `retry_dead_letter(id)`       | `handle.retry_dead_letter(id)`   |
| `estimated_progress()`        | `handle.estimated_progress()`    |
| `pause_recurring(id)`         | `handle.pause_recurring(id)`     |
| `set_group_limit("g", n)`     | _(unchanged, still on Scheduler)_ |

Methods that operate on all modules (`pause_all`, `resume_all`, `set_max_concurrency`, `subscribe`, `snapshot`, `active_tasks`) remain on `Scheduler`.

---

## Module defaults eliminate per-submission boilerplate

`Module` exposes a set of module-wide defaults that apply to every submission through its handle unless overridden at the call site.

```rust
Module::new("media")
    .default_priority(Priority::NORMAL)
    .default_retry_policy(RetryPolicy {
        strategy: BackoffStrategy::Exponential {
            initial: Duration::from_secs(1),
            max: Duration::from_secs(120),
            multiplier: 2.0,
        },
        max_retries: 5,
    })
    .default_group("media-pipeline")
    .default_ttl(Duration::from_secs(3600))
    .default_tag("team", "media")
    .max_concurrency(4)
```

`max_concurrency` on `Module` is independent of the global limit — both caps must have headroom for a task to dispatch.

### SubmitBuilder and per-call overrides

`ModuleHandle::submit()` and `submit_typed()` return a `SubmitBuilder` rather than a `Future`. Bare `.await` applies all defaults; chain override methods before `.await` to override individual fields for that call only.

```rust
// Zero ceremony — module defaults apply
handle.submit_typed(&thumb).await?;

// Override priority for this call only
handle.submit_typed(&thumb)
    .priority(Priority::HIGH)
    .run_after(Duration::from_secs(30))
    .await?;
```

**Resolution order** (highest → lowest):
1. `SubmitBuilder` per-call override
2. `TaskSubmission` explicit field (for the `submit()` path) or `TypedTask` trait value (for `submit_typed()`)
3. Module default
4. Scheduler global default

---

## Module-scoped application state

In 0.3.x, all app state lived in a single global `StateMap`. In 0.4.0, each module can have its own state, and `TaskContext::state::<T>()` checks module state first before falling back to global state.

**Registering state:**
```rust
// Module-scoped state (only visible to executors in this module)
Module::new("media")
    .app_state(MediaConfig { cdn_url: "...".into() })

// Global state (visible to all modules)
Scheduler::builder()
    .app_state(SharedDb::new(...))
```

**Accessing state in executors is unchanged:**
```rust
let cfg = ctx.state::<MediaConfig>().expect("MediaConfig not registered");
```

The lookup now checks module state first, then global state. No changes are needed if your types are distinct.

---

## TaskContext: module access replaces direct submission

`TaskContext` no longer has `submit()`, `submit_typed()`, or `submit_typed_at()`. Use module handles instead.

**Before:**
```rust
ctx.submit_typed(&NextStep { ... }).await?;
ctx.submit(TaskSubmission::new("notify").payload_json(&n)).await?;
```

**After:**
```rust
// Same-module follow-up
ctx.current_module().submit_typed(&NextStep { ... }).await?;

// Cross-module submission
ctx.module("notifications").submit_typed(&Notify { ... }).await?;

// Fallible cross-module (returns None if not registered)
if let Some(h) = ctx.try_module("analytics") {
    h.submit_typed(&TrackEvent { ... }).await?;
}
```

`current_module()` returns a handle for the module that owns the currently-executing task. It auto-prefixes task types and applies the module's defaults.

### Child task spawning

`spawn_child()` is still available and now module-aware — it auto-prefixes the task type and inherits the parent's remaining TTL and tags.

```rust
// Same-module child (use spawn_child for brevity)
ctx.spawn_child(TaskSubmission::new("postprocess").payload_json(&p)).await?;

// Cross-module child (use .parent() to link the relationship)
ctx.module("storage")
    .submit_typed(&Upload { ... })
    .parent(ctx.task_id())
    .await?;
```

`.parent(id)` on `SubmitBuilder` sets the parent task ID, inherits the parent's remaining TTL, and merges the parent's tags (child-set tags win on conflicts).

---

## `IoBudget` replaces scattered IO fields

The four separate IO byte fields on `TypedTask`, `TaskSubmission`, `TaskRecord`, and `TaskHistoryRecord` have been consolidated into a single `IoBudget` struct. `TaskMetrics` has been removed — use `IoBudget` everywhere instead.

**Before:**
```rust
impl TypedTask for MyTask {
    const TASK_TYPE: &'static str = "my-task";
    fn expected_read_bytes(&self) -> i64 { 4096 }
    fn expected_write_bytes(&self) -> i64 { 1024 }
    fn expected_net_rx_bytes(&self) -> i64 { 0 }
    fn expected_net_tx_bytes(&self) -> i64 { 0 }
}

TaskSubmission::new("upload")
    .expected_io(4096, 1024)
    .expected_net_io(0, 8192)

record.expected_read_bytes
history.actual_read_bytes
```

**After:**
```rust
impl TypedTask for MyTask {
    const TASK_TYPE: &'static str = "my-task";
    fn expected_io(&self) -> IoBudget { IoBudget::disk(4096, 1024) }
}

TaskSubmission::new("upload")
    .expected_io(IoBudget { disk_write: 1024, net_tx: 8192, ..Default::default() })

record.expected_io.disk_read
history.actual_io.map(|io| io.disk_read)
```

`IoBudget` provides two convenience constructors:
- `IoBudget::disk(read, write)` — sets disk fields, zeroes network
- `IoBudget::net(rx, tx)` — sets network fields, zeroes disk

The `TaskContext` recording methods (`record_read_bytes`, `record_write_bytes`, etc.) are unchanged.

---

## `TypedTask` now supports `key()` and `label()`

Two new optional default methods allow typed tasks to declare their own dedup key and UI label. No changes are required for existing implementations.

```rust
impl TypedTask for MyTask {
    const TASK_TYPE: &'static str = "my-task";
    fn key(&self) -> Option<String> { Some(self.file_path.clone()) }
    fn label(&self) -> Option<String> { Some(format!("Process {}", self.file_path)) }
}
```

When `None` (the default), key is derived from payload hash and label from task type.

---

## `SchedulerEvent` uses `TaskEventHeader`

Per-task event variants now carry a `TaskEventHeader` struct instead of repeating `task_id`, `task_type`, `key`, and `label` as individual fields.

**Before:**
```rust
match event {
    SchedulerEvent::Completed { task_id, task_type, key, label } => { ... }
    SchedulerEvent::Failed { task_id, label, error, will_retry, .. } => { ... }
    SchedulerEvent::Progress { task_id, percent, message, .. } => { ... }
}
```

**After:**
```rust
match event {
    SchedulerEvent::Completed(header) => { /* header.task_id, header.label, ... */ }
    SchedulerEvent::Failed { header, error, will_retry, retry_after } => { ... }
    SchedulerEvent::Progress { header, percent, message } => { ... }
}

// Or use the convenience accessor:
if let Some(header) = event.header() { ... }
```

`EstimatedProgress` fields `task_id`, `task_type`, `key`, `label` are also nested under `header: TaskEventHeader`.

### Module-filtered event subscriptions

`ModuleHandle::subscribe()` returns a `ModuleReceiver` that filters the global event stream to events for tasks in that module only, eliminating the need for manual `task_type.starts_with(prefix)` guards.

```rust
let mut rx = scheduler.module("media").subscribe();
while let Ok(event) = rx.recv().await {
    // only media:: events arrive here
}
```

---

## `payload_json()` and `from_typed()` no longer return `Result`

Both methods now always return `Self`, keeping the builder chain unbroken. Serialization errors are deferred and surfaced when calling `.await` on the `SubmitBuilder` as `StoreError::Serialization`.

**Before:**
```rust
let sub = TaskSubmission::new("task")
    .payload_json(&data)?  // breaks the chain
    .priority(Priority::HIGH);

let sub = TaskSubmission::from_typed(&task)?;
```

**After:**
```rust
let sub = TaskSubmission::new("task")
    .payload_json(&data)  // always returns Self
    .priority(Priority::HIGH);

let sub = TaskSubmission::from_typed(&task);
```

Remove any `?` operators on `payload_json()` or `from_typed()` calls.

---

## Adaptive retry with configurable backoff

### `SchedulerEvent::Failed` gains `retry_after` field

The `Failed` event variant now includes an optional `retry_after: Option<Duration>` field. Update any exhaustive pattern matches:

**Before:**
```rust
SchedulerEvent::Failed { header, error, will_retry } => { ... }
```

**After:**
```rust
SchedulerEvent::Failed { header, error, will_retry, retry_after } => { ... }
// retry_after is Some(duration) when backoff is active, None for immediate retry or permanent failure
```

### New `SchedulerEvent::DeadLettered` variant

A new event variant is emitted when a task exhausts its retries:

```rust
SchedulerEvent::DeadLettered { header, error, retry_count } => {
    // task failed with a retryable error but hit its max_retries limit
    handle.retry_dead_letter(header.task_id).await?;
}
```

Add a match arm for this variant if your match is exhaustive.

### `HistoryStatus` gains `DeadLetter` variant

Tasks that exhaust retries now receive `HistoryStatus::DeadLetter` instead of `HistoryStatus::Failed`. Add a match arm in any exhaustive match on `HistoryStatus`.

### `TaskError` gains `retry_after_ms` field

`TaskError` has a new `retry_after_ms: Option<u64>` field. If you construct `TaskError` via struct literals, add `retry_after_ms: None`. The existing constructors (`new`, `retryable`, `permanent`, `cancelled`) are unaffected. Executors can now signal a retry delay:

```rust
Err(TaskError::retryable("rate limited").retry_after(Duration::from_secs(60)))
```

### Per-module retry policy

Retry policies move from `SchedulerBuilder` executor registration to `Module`:

**Before:**
```rust
Scheduler::builder()
    .executor_with_retry_policy("api-call", Arc::new(ApiExecutor), RetryPolicy { ... })
```

**After:**
```rust
Module::new("integrations")
    .executor_with_retry_policy("api-call", Arc::new(ApiExecutor), RetryPolicy { ... })
    // or set a module-wide default:
    .default_retry_policy(RetryPolicy { ... })
```

### Schema migration

Migration `008_retry_backoff.sql` adds a nullable `max_retries INTEGER` column to both `tasks` and `task_history`.
