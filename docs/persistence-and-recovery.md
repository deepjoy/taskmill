# Persistence & Recovery

Taskmill persists all task state to SQLite. Work survives process restarts, crashes, and power loss — no manual recovery needed.

> **0.4.0 note: databases from 0.3.x are not compatible.** Task type strings stored by 0.3.x use bare names (e.g. `"thumbnail"`); 0.4.0 stores qualified names (e.g. `"media::thumbnail"`). Delete the database file before first run after upgrading — there is no in-place migration path.

## What survives a crash

When your app starts up, taskmill automatically recovers:

- **All queued tasks** are still in the queue at their original priority.
- **Running tasks** are reset to pending and re-dispatched. The crash doesn't count as a retry.
- **Dedup keys stay occupied** — no duplicate submissions sneak in during recovery.
- **Retry counts are preserved** — a task that had retried twice before the crash still has two retries used.

The guarantee is **at-least-once execution**: a task might run partially, crash, and re-run from the beginning. Design your executors to be idempotent (or to check for partial work) so re-execution is safe.

## Scheduled task recovery

Delayed and recurring tasks are fully persistent and survive restarts:

- **Recurring schedules survive restarts** — the schedule metadata (interval, remaining occurrences, paused state) is persisted in SQLite. After a restart, recurring tasks resume their schedules automatically.
- **Missed delayed tasks run immediately** — if a task's `run_after` timestamp is in the past when the scheduler starts (e.g., the app was offline), the task is dispatched on the first cycle rather than being silently dropped.
- **Running recurring instances are reset to pending** — this is the same behavior as all running tasks during crash recovery. The crash does not count as a retry, and the recurring schedule continues normally after the re-run completes.

## Dependency recovery

Task dependency edges (stored in the `task_deps` table) are fully persisted and survive restarts.

- **Blocked tasks stay blocked.** Their edges are in `task_deps` and resolution happens normally when their dependencies complete.
- **Running dependencies are reset to pending.** This is the standard crash recovery behavior for all running tasks. Once the reset dependency re-executes and completes, its dependents are unblocked as usual.
- **Stale edge cleanup on startup.** During recovery, the scheduler deletes any edges in `task_deps` that point to tasks no longer in the active queue (e.g., if a cancellation was interrupted mid-operation). Any blocked tasks left with zero remaining edges are then transitioned to `pending`.

No manual intervention is needed — dependency chains resume correctly after any restart or crash.

## Deduplication

A common problem: your app submits "upload photo.jpg" twice because the user clicked a button while a sync was already running. Without dedup, you'd upload the same file twice.

Taskmill prevents this automatically. Every task gets a unique key derived from its type and payload. If you submit a task with a key that's already in the queue, the duplicate is silently ignored.

### How keys work

The key is `SHA-256(task_type + ":" + payload)`. Two tasks with the same type and payload always get the same key.

You can also provide explicit keys when the default isn't right — for example, when two payloads represent the same logical work (different timestamps but same file path):

```rust
let sub = TaskSubmission::new("upload")
    .payload_json(&data)
    .key("/photos/img.jpg");  // dedup on file path, not full payload
```

### Key lifecycle

A key is "occupied" while the task is active — pending, running, paused, waiting, or retrying. When the task completes or permanently fails (moves to history), the key is freed and the same work can be submitted again.

### Submission outcomes

```rust
use taskmill::SubmitOutcome;

let outcome = scheduler.module("app").submit(submission).await?;
match outcome {
    SubmitOutcome::Inserted(id) => println!("new task: {id}"),
    SubmitOutcome::Duplicate => println!("already queued"),
    SubmitOutcome::Upgraded(id) => println!("priority upgraded: {id}"),
    SubmitOutcome::Requeued(id) => println!("requeued from history: {id}"),
}
```

`submit_batch()` applies the same dedup within a single transaction:

```rust
use taskmill::BatchSubmission;

let batch = BatchSubmission::new()
    .task(sub1).task(sub2).task(sub3);
let outcome = scheduler.module("app").submit_batch(batch).await?;
// outcome.duplicated_count() — how many were Duplicate
```

### Looking up tasks by dedup key

Check whether a task has been submitted (or has already completed). In 0.4 the task type in the database is the qualified name:

```rust
use taskmill::TaskLookup;

let lookup = scheduler.task_lookup("app::resize", "/photos/img.jpg").await?;
match lookup {
    TaskLookup::Active(record) => println!("still running: {:?}", record.status),
    TaskLookup::History(record) => println!("completed: {:?}", record.completed_at),
    TaskLookup::NotFound => println!("never submitted"),
}
```

## History and retention

Completed and failed tasks are moved to a history table. Without pruning, this table grows without bound. Configure automatic retention to keep it manageable.

### Recommended settings

For most desktop apps, keeping the last 10,000 records is a good default:

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

Pruning is amortized — it runs every N task completions (default 100, configurable via `StoreConfig::prune_interval`). This keeps per-task overhead low. Pruning errors are logged but don't affect the completed task.

### Manual pruning

```rust
let store = scheduler.store();
let deleted = store.prune_history_by_count(5_000).await?;
let deleted = store.prune_history_by_age(30).await?;
```

## Task TTL and expiry

Tasks with a TTL that haven't started running before their deadline are automatically expired. Expired tasks are moved to the history table with `HistoryStatus::Expired` — they are never silently deleted.

### What happens on expiry

1. The task is removed from the active `tasks` table.
2. A history record is created with `status = 'expired'`.
3. A `TaskExpired` event is emitted with the task header and age.
4. If the expired task has pending or paused children, they are cascade-expired too.

### TTL and crash recovery

TTL deadlines (`expires_at`) are persisted in SQLite, so they survive crashes. After a restart, the scheduler's first expiry sweep picks up any tasks that expired while the process was down.

### TTL and deduplication

When a task expires, its dedup key is freed (since it moves to history). The same work can be submitted again after expiry.

## Testing with in-memory stores

For tests, use an in-memory database that doesn't touch the filesystem:

```rust
let store = TaskStore::open_memory().await?;
```

## SQLite details

You normally don't need to know the schema, but it's documented here for debugging and advanced use.

### `tasks` — active queue

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER PRIMARY KEY | Insertion-order ID |
| `task_type` | TEXT NOT NULL | Executor lookup name |
| `key` | TEXT NOT NULL UNIQUE | SHA-256 dedup key |
| `label` | TEXT NOT NULL | Human-readable display name |
| `priority` | INTEGER NOT NULL | 0–255 (lower = higher priority) |
| `status` | TEXT DEFAULT 'pending' | `pending`, `running`, `paused`, or `waiting` |
| `payload` | BLOB | Opaque task data (max 1 MiB) |
| `expected_read_bytes` | INTEGER | Estimated disk read IO |
| `expected_write_bytes` | INTEGER | Estimated disk write IO |
| `expected_net_rx_bytes` | INTEGER | Estimated network RX |
| `expected_net_tx_bytes` | INTEGER | Estimated network TX |
| `parent_id` | INTEGER | Parent task ID (NULL for top-level) |
| `fail_fast` | INTEGER DEFAULT 1 | Whether child failure cancels siblings |
| `retry_count` | INTEGER DEFAULT 0 | Retries so far |
| `last_error` | TEXT | Most recent error message |
| `created_at` | TEXT | ISO 8601 timestamp |
| `started_at` | TEXT | Set when dispatched, cleared on pause |
| `ttl_seconds` | INTEGER | TTL duration in seconds (NULL = no TTL) |
| `ttl_from` | TEXT DEFAULT 'submission' | When TTL clock starts: `submission` or `first_attempt` |
| `expires_at` | TEXT | ISO 8601 deadline (NULL = no expiry) |

**Indexes:**
- `idx_tasks_pending(status, priority ASC, id ASC) WHERE status = 'pending'` — fast priority-ordered dispatch.
- `idx_tasks_expires(expires_at ASC) WHERE expires_at IS NOT NULL AND status IN ('pending', 'paused')` — efficient expiry sweep.

### `task_history` — completed and failed tasks

All columns from `tasks`, plus:

| Column | Type | Description |
|--------|------|-------------|
| `actual_read_bytes` | INTEGER | Reported by executor |
| `actual_write_bytes` | INTEGER | Reported by executor |
| `actual_net_rx_bytes` | INTEGER | Reported by executor |
| `actual_net_tx_bytes` | INTEGER | Reported by executor |
| `completed_at` | TEXT | ISO 8601 timestamp |
| `duration_ms` | INTEGER | Wall-clock duration |
| `status` | TEXT | `completed`, `failed`, `cancelled`, `superseded`, or `expired` |
| `ttl_seconds` | INTEGER | TTL duration in seconds (NULL = no TTL) |
| `ttl_from` | TEXT DEFAULT 'submission' | When TTL clock started |
| `expires_at` | TEXT | ISO 8601 deadline (NULL = no expiry) |

**Index:** `idx_history_type_completed(task_type, completed_at DESC) WHERE status = 'completed'` — for per-type history queries and throughput calculations.

### WAL mode

The database uses SQLite WAL (Write-Ahead Logging) for concurrent reads with serialized writes. Multiple Tauri commands can query task status simultaneously while the scheduler is dispatching work.

### Connection pooling

The default pool size is 16 connections. Increase via `StoreConfig::max_connections` if you have many concurrent readers:

```rust
let scheduler = Scheduler::builder()
    .store_config(StoreConfig {
        max_connections: 32,
        ..Default::default()
    })
    .build()
    .await?;
```
