# Migrating from 0.6.x to 0.7.0

0.7.0 adds group-level pause/resume, token-bucket rate limiting, priority
aging, weighted fair scheduling, and typed executor memos. It also consolidates
the migration files and normalizes all timestamps to epoch milliseconds,
**requiring a fresh database**.

---

### 1. Database recreation required

Migrations were consolidated from 9 chronological files into 4 object-oriented
files, and all timestamp columns were converted from `TEXT` to epoch millisecond
`INTEGER`. Existing SQLite databases **cannot be upgraded in-place** — delete
the database file and let the scheduler recreate it on startup.

This is acceptable for a pre-1.0 release: the `tasks` table is a transient work
queue that drains, and `task_history` is diagnostic.

---

### 2. Executor return type: typed Memo

`execute()` now returns `Result<Memo, TaskError>` instead of `Result<(), TaskError>`,
and `finalize()` receives the memo between the payload and context arguments.
The `Memo` type parameter defaults to `()`, so existing executors only need a
minimal update.

**Before:**

```rust
impl TypedExecutor<Thumbnail> for ThumbnailExec {
    async fn execute<'a>(
        &'a self,
        thumb: Thumbnail,
        ctx: DomainTaskContext<'a, Media>,
    ) -> Result<(), TaskError> {
        // ...
        Ok(())
    }

    async fn finalize<'a>(
        &'a self,
        thumb: Thumbnail,
        ctx: DomainTaskContext<'a, Media>,
    ) -> Result<(), TaskError> {
        Ok(())
    }
}
```

**After (no memo):**

```rust
impl TypedExecutor<Thumbnail> for ThumbnailExec {
    async fn execute<'a>(
        &'a self,
        thumb: Thumbnail,
        ctx: DomainTaskContext<'a, Media>,
    ) -> Result<(), TaskError> {
        // ...
        Ok(())
    }

    async fn finalize<'a>(
        &'a self,
        thumb: Thumbnail,
        _memo: (),                          // ← new parameter
        ctx: DomainTaskContext<'a, Media>,
    ) -> Result<(), TaskError> {
        Ok(())
    }
}
```

**After (with memo):**

```rust
#[derive(Serialize, Deserialize)]
struct ScanMemo { scan_start_ns: i64, batch_id: u64 }

impl TypedExecutor<Thumbnail, ScanMemo> for ThumbnailExec {
    async fn execute<'a>(
        &'a self,
        thumb: Thumbnail,
        ctx: DomainTaskContext<'a, Media>,
    ) -> Result<ScanMemo, TaskError> {       // ← returns memo
        Ok(ScanMemo { scan_start_ns: 42, batch_id: 1 })
    }

    async fn finalize<'a>(
        &'a self,
        thumb: Thumbnail,
        memo: ScanMemo,                      // ← receives memo
        ctx: DomainTaskContext<'a, Media>,
    ) -> Result<(), TaskError> {
        println!("batch {}", memo.batch_id);
        Ok(())
    }
}
```

Register memo-bearing executors with `task_memo` instead of `task`:

```rust
// No memo (unchanged):
domain.task::<Thumbnail>(ThumbnailExec)

// With memo:
domain.task_memo(ThumbnailExec)   // Memo type inferred from the impl
```

---

### 3. `SubmitOutcome::Inserted` is now a struct variant

**Before:**

```rust
match outcome {
    SubmitOutcome::Inserted(id) => { /* ... */ }
    // ...
}
```

**After:**

```rust
match outcome {
    SubmitOutcome::Inserted { id, group_paused } => {
        if group_paused {
            println!("task {id} submitted to a paused group");
        }
    }
    // ...
}
```

---

### 4. Tag query methods renamed and return `Vec<i64>`

Four tag-based query methods were renamed and now return task IDs instead of
full `TaskRecord`s:

| Before | After |
|---|---|
| `tasks_by_tags()` → `Vec<TaskRecord>` | `task_ids_by_tags()` → `Vec<i64>` |
| `tasks_by_tags_with_prefix()` → `Vec<TaskRecord>` | `task_ids_by_tags_with_prefix()` → `Vec<i64>` |
| `tasks_by_tag_key_prefix()` → `Vec<TaskRecord>` | `task_ids_by_tag_key_prefix()` → `Vec<i64>` |
| `tasks_by_tag_key_prefix_with_prefix()` → `Vec<TaskRecord>` | `task_ids_by_tag_key_prefix_with_prefix()` → `Vec<i64>` |

Fetch full records with `task_by_id()` if needed.

---

### 5. New feature: group pause / resume

Pause and resume all tasks in a group at runtime. Tasks submitted to a paused
group are inserted as `Paused` and dispatched when the group is resumed.

```rust
scheduler.pause_group("s3-uploads").await?;
scheduler.resume_group("s3-uploads").await?;

// Auto-resume after a deadline (~5 s latency):
scheduler.pause_group_until("s3-uploads", deadline).await?;

// Query state:
let paused: bool = scheduler.is_group_paused("s3-uploads");
let all: Vec<String> = scheduler.paused_groups();
```

Available on `Scheduler`, `DomainHandle`, and `ModuleHandle`.

New events: `SchedulerEvent::GroupPaused { .. }` and `GroupResumed { .. }`.

---

### 6. New feature: token-bucket rate limiting

Apply per-task-type or per-group rate limits. Rate-limited tasks are deferred
(via `run_after`) until the next token is available, avoiding head-of-line
blocking.

```rust
use taskmill::RateLimit;

let sched = Scheduler::builder()
    .rate_limit("media::upload", RateLimit::per_second(5))
    .group_rate_limit("s3-bucket", RateLimit::per_second(10).with_burst(20))
    .build()
    .await?;

// Reconfigure at runtime:
scheduler.set_rate_limit("media::upload", RateLimit::per_second(20));
scheduler.remove_rate_limit("media::upload");

scheduler.set_group_rate_limit("s3-bucket", RateLimit::per_minute(100));
scheduler.remove_group_rate_limit("s3-bucket");
```

Available on `Scheduler`, `DomainHandle`, and `ModuleHandle`.

---

### 7. New feature: priority aging

Prevent task starvation by automatically promoting the effective priority of
long-waiting tasks. The stored priority is never mutated — effective priority is
a pure dispatch-time computation.

```rust
use taskmill::AgingConfig;

Scheduler::builder()
    .priority_aging(AgingConfig {
        grace_period: Duration::from_secs(300),   // 5 min before aging starts
        aging_interval: Duration::from_secs(60),  // one promotion per minute
        max_effective_priority: Priority::HIGH,    // ceiling
        urgent_threshold: None,                    // optional: bypass weights
    })
    .build()
    .await?;
```

Pause duration is excluded from the aging clock.

---

### 8. New feature: weighted fair scheduling

Allocate dispatch capacity proportionally across groups. The scheduler is
work-conserving — unused slots are redistributed by global priority order.

```rust
Scheduler::builder()
    .group_weight("api-calls", 3)         // 75% of capacity
    .group_weight("background", 1)        // 25% of capacity
    .default_group_weight(1)
    .group_minimum_slots("alerts", 2)     // guaranteed floor
    .build()
    .await?;

// Runtime adjustment:
scheduler.set_group_weight("api-calls", 5);
scheduler.remove_group_weight("api-calls");
scheduler.set_group_minimum_slots("alerts", 4);
```

When combined with `urgent_threshold` in `AgingConfig`, severely aged tasks
bypass group allocation as a safety valve.

---

### 9. New feature: `fail_fast()` on submit builders

Control whether a parent task fails immediately when any child fails (default),
or waits for all children to complete first.

```rust
// Wait for all children before resolving the parent:
scheduler.submit(
    &TaskSubmission::new("pipeline::assemble")
        .fail_fast(false)
).await?;

// Typed domain API:
media.submit_with(AssembleTask { .. })
    .fail_fast(false)
    .await?;
```

---

### 10. New feature: tag key prefix queries

Discover tag keys and find tasks by tag key namespace, with proper SQL wildcard
escaping.

```rust
// Discover tag keys by prefix:
let keys = store.tag_keys_by_prefix("billing.").await?;
// → ["billing.customer_id", "billing.plan"]

// Count tasks with matching tag keys:
let count = store.count_by_tag_key_prefix("billing.", None).await?;

// Get task IDs:
let ids = store.task_ids_by_tag_key_prefix("billing.", Some(TaskStatus::Pending)).await?;

// Domain-scoped:
let keys = billing_handle.tag_keys_by_prefix("billing.").await?;
let cancelled = billing_handle.cancel_by_tag_key_prefix("billing.").await?;
```

---

### 11. Import changes

```rust
// New types you may need:
use taskmill::{
    AgingConfig,      // priority aging configuration
    PauseReasons,     // bitmask for pause sources
    RateLimit,        // token-bucket rate limit config
};
```
