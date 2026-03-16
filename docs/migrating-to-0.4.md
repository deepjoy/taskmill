# Migrating from 0.3.x to 0.4.0

This guide covers the breaking API changes in taskmill 0.4.0. All changes are API-level — database columns are unchanged, so existing data is fully compatible.

## `IoBudget` replaces scattered IO fields

The four separate IO byte fields on `TypedTask`, `TaskSubmission`, `TaskRecord`, and `TaskHistoryRecord` have been consolidated into a single `IoBudget` struct. `TaskMetrics` has been removed — use `IoBudget` everywhere instead.

**Before:**
```rust
// TypedTask: 4 separate methods
impl TypedTask for MyTask {
    const TASK_TYPE: &'static str = "my-task";
    fn expected_read_bytes(&self) -> i64 { 4096 }
    fn expected_write_bytes(&self) -> i64 { 1024 }
    fn expected_net_rx_bytes(&self) -> i64 { 0 }
    fn expected_net_tx_bytes(&self) -> i64 { 0 }
}

// TaskSubmission: two builder methods
TaskSubmission::new("upload")
    .expected_io(4096, 1024)
    .expected_net_io(0, 8192)

// Accessing fields on TaskRecord / TaskHistoryRecord
record.expected_read_bytes
history.actual_read_bytes
```

**After:**
```rust
// TypedTask: single method returning IoBudget
impl TypedTask for MyTask {
    const TASK_TYPE: &'static str = "my-task";
    fn expected_io(&self) -> IoBudget { IoBudget::disk(4096, 1024) }
}

// TaskSubmission: single builder method
TaskSubmission::new("upload")
    .expected_io(IoBudget { disk_write: 1024, net_tx: 8192, ..Default::default() })

// Accessing fields on TaskRecord / TaskHistoryRecord
record.expected_io.disk_read
history.actual_io.map(|io| io.disk_read)
```

`IoBudget` provides two convenience constructors:
- `IoBudget::disk(read, write)` — sets disk fields, zeroes network
- `IoBudget::net(rx, tx)` — sets network fields, zeroes disk

The `TaskContext` recording methods (`record_read_bytes`, `record_write_bytes`, etc.) are unchanged.

## `TypedTask` now supports `key()` and `label()`

Two new optional default methods allow typed tasks to declare their own dedup key and UI label:

```rust
impl TypedTask for MyTask {
    const TASK_TYPE: &'static str = "my-task";
    fn key(&self) -> Option<String> { Some(self.file_path.clone()) }
    fn label(&self) -> Option<String> { Some(format!("Process {}", self.file_path)) }
}
```

When `None` (the default), behavior is unchanged — key is derived from payload hash, label from task type. Existing `TypedTask` impls require no changes.

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

## `payload_json()` and `from_typed()` no longer return `Result`

Both methods now always return `Self`, keeping the builder chain unbroken. Serialization errors are deferred and surfaced when calling `scheduler.submit()` / `store.submit()` as `StoreError::Serialization`.

**Before:**
```rust
let sub = TaskSubmission::new("task")
    .key("k")
    .payload_json(&data)?  // breaks the chain
    .priority(Priority::HIGH);

let sub = TaskSubmission::from_typed(&task)?;
```

**After:**
```rust
let sub = TaskSubmission::new("task")
    .key("k")
    .payload_json(&data)  // always returns Self
    .priority(Priority::HIGH);

let sub = TaskSubmission::from_typed(&task);
```

Remove any `?` operators on `payload_json()` or `from_typed()` calls. Errors are still caught before the task is persisted — they just surface at submit time instead.

## Adaptive retry with configurable backoff

### `SchedulerEvent::Failed` gains `retry_after` field

The `Failed` event variant now includes an optional `retry_after: Option<Duration>` field indicating when the next retry will happen. Update any exhaustive pattern matches:

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
    // Task failed with a retryable error but hit its max_retries limit.
    // Use scheduler.retry_dead_letter(header.task_id) to re-submit.
}
```

Add a match arm for this variant if your match is exhaustive.

### `HistoryStatus` gains `DeadLetter` variant

Tasks that exhaust their retries now receive `HistoryStatus::DeadLetter` instead of `HistoryStatus::Failed`. This distinguishes "might succeed if retried" from "permanently broken." Add a match arm for `DeadLetter` in any exhaustive match on `HistoryStatus`.

### `TaskError` gains `retry_after_ms` field

`TaskError` has a new `retry_after_ms: Option<u64>` field. If you construct `TaskError` via struct literals, add `retry_after_ms: None`. The existing constructors (`new`, `retryable`, `permanent`, `cancelled`) are unaffected.

Executors can now signal a retry delay:

```rust
Err(TaskError::retryable("rate limited").retry_after(Duration::from_secs(60)))
```

### New builder methods (non-breaking)

`SchedulerBuilder` gains two new methods for per-type retry policies:

```rust
// Register with a retry policy (backoff strategy + max_retries)
.executor_with_retry_policy("api-call", Arc::new(ApiExecutor), RetryPolicy {
    strategy: BackoffStrategy::Exponential {
        initial: Duration::from_secs(1),
        max: Duration::from_secs(300),
        multiplier: 2.0,
    },
    max_retries: 5,
})

// Register with both TTL and retry policy
.executor_with_options("upload", Arc::new(UploadExecutor),
    Some(Duration::from_secs(600)),     // TTL
    Some(RetryPolicy::default()),       // retry policy
)
```

### New dead-letter query and resubmit APIs (non-breaking)

```rust
// Query tasks that exhausted retries
let dead = scheduler.dead_letter_tasks(10, 0).await?;

// Re-submit a dead-lettered task (resets retry count)
scheduler.retry_dead_letter(task_history_id).await?;
```

### Schema migration

Migration `008_retry_backoff.sql` adds a nullable `max_retries INTEGER` column to both `tasks` and `task_history`. Existing tasks read back `max_retries = None` and fall back to the global `SchedulerConfig::max_retries`.
