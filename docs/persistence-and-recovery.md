# Persistence & Recovery

Taskmill persists all task state to SQLite, ensuring work survives process restarts, crashes, and power loss.

## SQLite schema

Two tables manage the task lifecycle:

### `tasks` — active queue

Holds pending, running, and paused tasks.

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER PRIMARY KEY | Insertion-order ID |
| `task_type` | TEXT NOT NULL | Executor lookup name |
| `key` | TEXT NOT NULL UNIQUE | SHA-256 dedup key |
| `label` | TEXT NOT NULL | Human-readable display name (original dedup key or task type) |
| `priority` | INTEGER NOT NULL | 0–255 (lower = higher priority) |
| `status` | TEXT DEFAULT 'pending' | `pending`, `running`, `paused`, or `waiting` |
| `payload` | BLOB | Opaque task data (max 1 MiB) |
| `expected_read_bytes` | INTEGER | Estimated disk read IO (part of `IoBudget`) |
| `expected_write_bytes` | INTEGER | Estimated disk write IO (part of `IoBudget`) |
| `expected_net_rx_bytes` | INTEGER | Estimated network RX (part of `IoBudget`) |
| `expected_net_tx_bytes` | INTEGER | Estimated network TX (part of `IoBudget`) |
| `parent_id` | INTEGER | Parent task ID for child tasks (NULL for top-level) |
| `fail_fast` | INTEGER DEFAULT 1 | Whether child failure cancels siblings and fails parent |
| `retry_count` | INTEGER DEFAULT 0 | Number of retries so far |
| `last_error` | TEXT | Most recent error message |
| `created_at` | TEXT | ISO 8601 timestamp |
| `started_at` | TEXT | Set when dispatched, cleared on pause |

**Index:** `idx_tasks_pending(status, priority ASC, id ASC) WHERE status = 'pending'` — partial index for efficient priority-ordered pop.

### `task_history` — completed and failed tasks

| Column | Type | Description |
|--------|------|-------------|
| *(all columns from `tasks`, including `label`, `parent_id`, `fail_fast`)* | | |
| `actual_read_bytes` | INTEGER | Reported by executor (part of `IoBudget`) |
| `actual_write_bytes` | INTEGER | Reported by executor (part of `IoBudget`) |
| `actual_net_rx_bytes` | INTEGER | Reported by executor (part of `IoBudget`) |
| `actual_net_tx_bytes` | INTEGER | Reported by executor (part of `IoBudget`) |
| `completed_at` | TEXT | ISO 8601 timestamp |
| `duration_ms` | INTEGER | Wall-clock duration |
| `status` | TEXT | `completed` or `failed` |

**Index:** `idx_history_type_completed(task_type, completed_at DESC) WHERE status = 'completed'` — for per-type history queries and throughput calculations.

## Crash recovery

On startup, `TaskStore::open()` runs a recovery query:

```sql
UPDATE tasks SET status = 'pending', started_at = NULL WHERE status = 'running'
```

This resets any tasks that were mid-execution when the process died. The behavior:

- Tasks return to the priority queue at their original priority
- `retry_count` is preserved (crash doesn't count as a retry)
- Dedup keys remain occupied (no duplicate submissions during recovery)
- Tasks are re-dispatched in priority order on the next scheduler cycle

## Deduplication

### How keys are generated

Every task gets a SHA-256 key: `SHA-256(task_type + ":" + (explicit_key OR payload))`.

- **Implicit key** — if no key is provided, the payload bytes are used. Tasks with the same type and payload get the same key.
- **Explicit key** — use the `.key()` builder method to control deduplication yourself. Useful when two payloads represent the same logical work (e.g., different timestamps but same file path). The explicit key is also stored as the display `label`.
- **Type scoping** — the task type is always part of the hash, so `("resize", payload)` and `("compress", payload)` never collide.

### Lifecycle

A key is "occupied" while the task is in the `tasks` table (pending, running, paused, waiting, or retrying). When the task moves to `task_history` (completed or failed), the key is freed and can be resubmitted.

### Submission behavior

```rust
use taskmill::SubmitOutcome;

let outcome = scheduler.submit(&submission).await?;
match outcome {
    SubmitOutcome::Inserted(id) => println!("new task: {id}"),
    SubmitOutcome::Duplicate => println!("already queued"),
    SubmitOutcome::Upgraded(id) => println!("priority upgraded: {id}"),
    SubmitOutcome::Requeued(id) => println!("requeued from history: {id}"),
}
```

`submit_batch()` applies the same dedup within a single transaction:

```rust
let outcomes = scheduler.submit_batch(&[sub1, sub2, sub3]).await?;
// outcomes: Vec<SubmitOutcome>  — sub2 might be Duplicate
```

### Looking up tasks by dedup key

```rust
use taskmill::TaskLookup;

let lookup = scheduler.task_lookup("resize", "/photos/img.jpg").await?;
match lookup {
    TaskLookup::Active(record) => println!("still running: {:?}", record.status),
    TaskLookup::History(record) => println!("completed: {:?}", record.completed_at),
    TaskLookup::NotFound => println!("never submitted"),
}
```

## History retention

Without pruning, `task_history` grows without bound. Configure automatic retention:

### By count

Keep the N most recent records:

```rust
use taskmill::{StoreConfig, RetentionPolicy};

let scheduler = Scheduler::builder()
    .store_config(StoreConfig {
        retention_policy: Some(RetentionPolicy::MaxCount(10_000)),
        ..Default::default()
    })
    .build()
    .await?;
```

### By age

Keep records from the last N days:

```rust
let scheduler = Scheduler::builder()
    .store_config(StoreConfig {
        retention_policy: Some(RetentionPolicy::MaxAgeDays(90)),
        ..Default::default()
    })
    .build()
    .await?;
```

### Pruning frequency

Pruning is amortized — it runs every N task completions (default 100, configurable via `StoreConfig::prune_interval`). Pruning errors are logged but don't affect the completed task.

### Manual pruning

```rust
let store = scheduler.store();
let deleted = store.prune_history_by_count(5_000).await?;
let deleted = store.prune_history_by_age(30).await?;
```

## WAL mode

The database uses SQLite WAL (Write-Ahead Logging) for concurrent reads with serialized writes. This means multiple readers can query task status while the scheduler is dispatching work.

## Connection pooling

The default pool size is 16 connections. Configure via `StoreConfig::max_connections`:

```rust
let scheduler = Scheduler::builder()
    .store_config(StoreConfig {
        max_connections: 32,
        ..Default::default()
    })
    .build()
    .await?;
```

## In-memory store for testing

For tests, use an in-memory database that doesn't touch the filesystem:

```rust
let store = TaskStore::open_memory().await?;
```
