# Query APIs

Use these queries to build dashboards, debug stuck tasks, and gather analytics about task performance.

Most queries are available in two places:
- **`DomainHandle<D>`** — scoped to one domain's tasks (preferred). Access via `scheduler.domain::<D>()`.
- **`TaskStore`** — unscoped, across all tasks. Access via `scheduler.store()`.

## Common patterns

**Dashboard** — for most UIs, `scheduler.snapshot()` is all you need. It returns running tasks, queue depth, progress, and pressure in a single call. Use store queries below for custom views.

**Debugging** — use `task_by_id()` to inspect a specific task, or `failed_tasks()` to see recent failures and their error messages.

**Analytics** — use `history_stats()` for per-type aggregates (average duration, failure rate) and `avg_throughput()` to calibrate IO budgets.

## Active task queries

| Method | Returns | Description |
|--------|---------|-------------|
| `running_tasks()` | `Vec<TaskRecord>` | All running tasks, ordered by priority. |
| `running_count()` | `i64` | Count of running tasks. |
| `pending_tasks(limit)` | `Vec<TaskRecord>` | Pending tasks, ordered by priority then age. |
| `pending_count()` | `i64` | Count of pending tasks. |
| `pending_by_type(task_type)` | `Vec<TaskRecord>` | Pending tasks filtered by type. |
| `paused_tasks()` | `Vec<TaskRecord>` | All paused (preempted) tasks. |
| `paused_count()` | `i64` | Count of paused tasks. |
| `task_by_id(id)` | `Option<TaskRecord>` | Look up an active task by row ID. |
| `task_by_key(key)` | `Option<TaskRecord>` | Look up an active task by dedup key. |
| `running_io_totals()` | `(i64, i64)` | Sum of expected disk read and write bytes across running tasks. Useful for comparing against system capacity. |

## Domain-scoped queries (DomainHandle)

These methods are available on `DomainHandle<D>` and are automatically scoped to the domain's task type prefix.

| Method | Returns | Description |
|--------|---------|-------------|
| `handle.snapshot()` | `ModuleSnapshot` | Running tasks, pending/paused counts, progress, byte-level tracking, and pause state for this domain. The primary entry point for per-domain dashboards. |
| `handle.active_tasks()` | `Vec<TaskRecord>` | In-memory running tasks for this domain (no DB call). |
| `handle.estimated_progress()` | `Vec<EstimatedProgress>` | Extrapolated progress for each running task in this domain. |
| `handle.byte_progress()` | `Vec<TaskProgress>` | Live byte-level progress for running tasks in this domain. |
| `handle.dead_letter_tasks(limit, offset)` | `Vec<TaskHistoryRecord>` | Paginated dead-letter (permanently failed) tasks for this domain. |
| `handle.task_ids_by_tags(filters, status)` | `Vec<i64>` | IDs of active tasks in this domain matching the given tag filters and optional status. Fetch full records via `task_by_id()` if needed. |
| `handle.count_by_tag(key, status)` | `Vec<(String, i64)>` | Tag value counts for a given key within this domain. |
| `handle.tag_values(key)` | `Vec<(String, i64)>` | Distinct values for a tag key within this domain. |
| `handle.tag_keys_by_prefix(prefix)` | `Vec<String>` | Discover distinct tag keys matching a prefix (e.g. `"billing."`) within this domain. |
| `handle.task_ids_by_tag_key_prefix(prefix, status)` | `Vec<i64>` | IDs of tasks with any tag key matching the prefix, with optional status filter. Fetch full records via `task_by_id()` if needed. |
| `handle.count_by_tag_key_prefix(prefix, status)` | `i64` | Count tasks with any tag key matching the prefix, with optional status filter. |

## Cross-domain operations (Scheduler)

These methods operate across all domains and are available directly on `Scheduler`.

| Method | Returns | Description |
|--------|---------|-------------|
| `scheduler.domain::<D>()` | `DomainHandle<D>` | Typed handle for a specific registered domain. Panics if `D` is not registered; use `scheduler.try_domain::<D>()` for a fallible variant. |
| `scheduler.active_tasks()` | `Vec<TaskRecord>` | Running tasks from all domains combined. Equivalent to aggregating each domain's `active_tasks()`. |
| `scheduler.task(id)` | `Option<TaskRecord>` | Look up any active task by ID, regardless of which domain owns it. |
| `scheduler.snapshot()` | `SchedulerSnapshot` | Global aggregates: total running, pending, pressure, progress, and recurring schedules. |

See [Multi-Module Applications](multi-module-apps.md#building-a-cross-module-dashboard) for dashboard patterns using these APIs.

## Cancellation

### Single task

`handle.cancel(task_id)` cancels one task. Returns `true` if the task was found and cancelled.

### Bulk cancellation

| Method | Returns | Description |
|--------|---------|-------------|
| `handle.cancel_all()` | `Vec<i64>` | Cancel all tasks belonging to this domain. |
| `handle.cancel_where(predicate)` | `Vec<i64>` | Cancel domain tasks matching a predicate. |
| `handle.cancel_by_tag_key_prefix(prefix)` | `Vec<i64>` | Cancel all tasks with any tag key matching the prefix. |

`cancel_all()` and `cancel_where()` affect tasks in any active status:

- **Pending** tasks are moved to history as `Cancelled`.
- **Running** tasks have their cancellation token triggered and are moved to history as `Cancelled`.
- **Paused** tasks are cancelled immediately (no executor is running).
- **Waiting** tasks (parents waiting for children) are cancelled, and their children are also cancelled (cascade).
- **Children in other domains** are cancelled if they were linked via `.parent()` from a task in this domain.

## Dependency queries

| Method | Returns | Description |
|--------|---------|-------------|
| `task_dependencies(id)` | `Vec<i64>` | IDs of tasks that this task depends on (its prerequisites). |
| `task_dependents(id)` | `Vec<i64>` | IDs of tasks that depend on this task (will be unblocked when it completes). |
| `blocked_tasks()` | `Vec<TaskRecord>` | All tasks currently in `blocked` status, waiting for dependencies. |
| `blocked_count()` | `i64` | Count of blocked tasks. Also available in `SchedulerSnapshot::blocked_count`. |

Dependencies work across domain boundaries. A task in `"process"` can depend on a task in `"ingest"` — the domain boundary does not affect dependency resolution or failure propagation. See [Multi-Module Applications](multi-module-apps.md#cross-module-task-dependencies) for patterns.

> **Task type names in queries** — All store-level queries that accept a `task_type` string expect the *qualified* name including the domain prefix: `"media::thumbnail"`, not `"thumbnail"`. Use `DomainHandle` methods where possible — they apply the prefix automatically.

## History queries

| Method | Returns | Description |
|--------|---------|-------------|
| `history(limit, offset)` | `Vec<TaskHistoryRecord>` | Paginated history, newest first. |
| `history_by_type(task_type, limit)` | `Vec<TaskHistoryRecord>` | History filtered by task type. |
| `history_by_key(key)` | `Vec<TaskHistoryRecord>` | All past runs matching a dedup key. |
| `failed_tasks(limit)` | `Vec<TaskHistoryRecord>` | Recent failures with error messages. |

History records include a `status` field that can be `completed`, `failed`, `cancelled`, `superseded`, `expired`, or `dead_letter`. Filter by status to find expired tasks (e.g., for analytics on TTL effectiveness).

The `history_by_type(task_type)` parameter requires the qualified name including the domain prefix — e.g. `"media::thumbnail"`, not `"thumbnail"`.

## Aggregate queries

| Method | Returns | Description |
|--------|---------|-------------|
| `history_stats(task_type)` | `TypeStats` | Aggregate stats for a task type. |
| `avg_throughput(task_type, limit)` | `(f64, f64)` | Average read/write bytes per second from recent completions. Use this to calibrate IO budgets. |

### TypeStats fields

| Field | Type | Description |
|-------|------|-------------|
| `count` | `i64` | Total completed tasks of this type. |
| `avg_duration_ms` | `f64` | Average wall-clock duration. |
| `avg_read_bytes` | `f64` | Average actual read bytes. |
| `avg_write_bytes` | `f64` | Average actual write bytes. |
| `failure_rate` | `f64` | Fraction of tasks that failed (0.0–1.0). A rate above 0.1 may indicate a systemic issue. |

## Scheduled and recurring task queries

| Method | Returns | Description |
|--------|---------|-------------|
| `next_run_after()` | `Option<DateTime<Utc>>` | Earliest `run_after` timestamp among pending delayed tasks. Useful for knowing when the next scheduled task will fire. |
| `recurring_schedules()` | `Vec<RecurringScheduleInfo>` | All active recurring schedules with their interval, remaining occurrences, and paused state. |

The `SchedulerSnapshot` also includes recurring schedule information:

```rust
let snap = scheduler.snapshot().await?;
// snap.recurring_schedules — Vec<RecurringScheduleInfo> for all active schedules
```

### Managing recurring schedules

Pause, resume, or cancel recurring schedules via the domain handle:

```rust
let handle = scheduler.domain::<App>();

// Pause — stops new occurrences from being enqueued
handle.pause_recurring(task_id).await?;

// Resume — re-enables the schedule from where it left off
handle.resume_recurring(task_id).await?;

// Cancel — permanently removes the recurring schedule
handle.cancel_recurring(task_id).await?;
```

## Unified lookup

Search both active and history tables by dedup key — useful for checking whether a task has been submitted or has already completed. Note that the task type stored in the database is the fully qualified name (e.g. `"media::resize"`, not `"resize"`):

```rust
use taskmill::TaskLookup;

let lookup = scheduler.task_lookup("media::resize", "/photos/img.jpg").await?;
match lookup {
    TaskLookup::Active(record) => {
        println!("Status: {:?}, priority: {}", record.status, record.priority.value());
    }
    TaskLookup::History(record) => {
        println!("Completed at: {:?}, duration: {}ms", record.completed_at, record.duration_ms);
    }
    TaskLookup::NotFound => {
        println!("No task found with this key");
    }
}
```

Or with typed tasks (the domain prefix is applied automatically):

```rust
let lookup = scheduler.lookup_typed(&ResizeTask {
    path: "/photos/img.jpg".into(),
    width: 300,
}).await?;
```

## Pruning

| Method | Returns | Description |
|--------|---------|-------------|
| `prune_history_by_count(keep)` | `u64` | Delete all but the N most recent history records. |
| `prune_history_by_age(days)` | `u64` | Delete history records older than N days. |

## Usage example

```rust
// Domain-scoped snapshot — running tasks, pending count, progress.
let snap = scheduler.domain::<Media>().snapshot().await?;
println!("media: {} running, {} pending", snap.running.len(), snap.pending_count);

// Global dashboard data via the store.
let store = scheduler.store();
let running = store.running_count().await?;
let pending = store.pending_count().await?;
let (read_io, write_io) = store.running_io_totals().await?;

// Per-type analytics — note the qualified type name.
let stats = store.history_stats("media::thumbnail").await?;
println!(
    "media::thumbnail: {} completed, avg {:.0}ms, {:.1}% failure rate",
    stats.count, stats.avg_duration_ms, stats.failure_rate * 100.0,
);

// Paginated history for a UI table
let page = store.history(50, 0).await?;     // first 50
let page2 = store.history(50, 50).await?;   // next 50
```
