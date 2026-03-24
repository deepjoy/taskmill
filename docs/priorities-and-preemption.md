# Priorities & Preemption

## When you need this

Not all work is equally important. A user clicking "upload this file" should not wait behind 500 background thumbnail jobs. A re-index of the entire library should yield to normal operations if the system gets busy.

Taskmill's priority system lets you express these relationships, and preemption enforces them at runtime — pausing lower-priority work so urgent tasks can run immediately.

## Priority levels

Taskmill uses a 256-level priority scale where lower values mean higher priority. Five named constants cover the most common scenarios:

| Constant | Value | When to use |
|----------|-------|-------------|
| `REALTIME` | 0 | User-blocking operations that can't wait. Rare — most apps never need this. |
| `HIGH` | 64 | User-initiated actions: uploading a file they clicked, exporting a document. |
| `NORMAL` | 128 | App-initiated work: generating thumbnails, syncing metadata. |
| `BACKGROUND` | 192 | Maintenance tasks: re-indexing, cleanup, cache warming. |
| `IDLE` | 255 | Truly optional work: analytics, prefetching, speculative processing. |

Custom values between tiers are supported for fine-grained control:

```rust
use taskmill::Priority;

let custom = Priority::new(100); // between HIGH and NORMAL
```

### Choosing priorities for your tasks

A good rule of thumb: **if the user is waiting for it, use `HIGH`. If the app initiated it, use `NORMAL`. If it can wait until later, use `BACKGROUND`.**

Most applications only need `HIGH`, `NORMAL`, and `BACKGROUND`. Reserve `REALTIME` for operations where any delay is unacceptable — it triggers [preemption](#preemption) by default, which pauses all other work.

## Queue ordering

Tasks are dispatched in strict priority order. Within the same priority tier, tasks are dispatched in insertion order (FIFO) — the task submitted first runs first.

## Preemption

When a task at or above `preempt_priority` (default: `REALTIME`) is submitted, the scheduler pauses all lower-priority running work to make room:

1. **Cancel tokens** — the `CancellationToken` of every active task with lower priority is triggered.
2. **Pause in store** — preempted tasks are moved to `paused` status.
3. **Emit events** — a `SchedulerEvent::Preempted` is emitted for each affected task.
4. **Resume later** — paused tasks are only re-dispatched when no active preemptors remain, preventing thrashing between competing priority tiers.

### When to use preemption

Preemption is powerful but rarely needed. The default threshold (`REALTIME`) means only the highest-priority tasks trigger it. **Most apps should leave the default.** If you find yourself frequently preempting, consider whether your task priorities are spread too wide.

Use preemption when:
- A "cancel everything and do this NOW" escape hatch is needed (e.g., emergency sync)
- User-initiated work must always run immediately, even if the system is fully loaded

Don't use preemption when:
- You just want important tasks to run first — priority ordering handles that without preemption
- Tasks are short enough that waiting for a slot is acceptable

### Handling preemption in executors

Executors should check for cancellation at natural yield points — between chunks of work, between loop iterations, etc. This lets the executor clean up gracefully.

```rust
impl TypedExecutor<MyTask> for MyExecutor {
    async fn execute(&self, task: MyTask, ctx: DomainTaskContext<'_, MyTask::Domain>) -> Result<(), TaskError> {
        for chunk in chunks {
            // Check before each unit of work
            if ctx.token().is_cancelled() {
                return Err(TaskError::retryable("preempted"));
            }

            process(chunk).await;
            ctx.record_read_bytes(chunk.len() as i64);
            ctx.progress().report_fraction(i, total, None);
        }

        Ok(())
    }
}
```

Returning a retryable error on preemption is optional — the scheduler handles pausing regardless. But it gives the executor a chance to clean up partial work.

### Configuring the preemption threshold

```rust
let scheduler = Scheduler::builder()
    .preempt_priority(Priority::HIGH)  // now HIGH and REALTIME both trigger preemption
    .build()
    .await?;
```

## Task groups

When multiple tasks share a limited resource (e.g., an API with rate limits, or a specific S3 bucket), you can limit how many run concurrently using task groups.

Assign tasks to a group via `.group()` on `TaskSubmission` or `TaskTypeConfig::group()` in `TypedTask::config()`:

```rust
let sub = TaskSubmission::new("upload")
    .payload_json(&data)
    .group("bucket-prod");  // at most N uploads to this bucket at once
```

Configure limits at build time or adjust at runtime:

```rust
let scheduler = Scheduler::builder()
    .group_concurrency("bucket-prod", 3)       // max 3 concurrent for this group
    .default_group_concurrency(5)              // default for any group not explicitly configured
    .build()
    .await?;

// Adjust at runtime
scheduler.set_group_limit("bucket-prod", 5).await;
scheduler.remove_group_limit("bucket-prod").await;
```

Group limits are checked *in addition to* `max_concurrency` — a task must pass both the global and group gate to be dispatched.

## Domain-level pause and resume

Individual domains can be paused and resumed independently, without affecting other domains. This is useful for features like a user-togglable sync, or temporarily disabling a domain during maintenance.

```rust
let sync = scheduler.domain::<Sync>();

// Pause — stops dispatch, interrupts running tasks, moves everything to paused.
let paused_count = sync.pause().await?;

// Resume — moves paused tasks back to pending, re-enables dispatch.
let resumed_count = sync.resume().await?;
```

**What `pause()` does:**
1. Sets the per-domain `is_paused` flag (prevents new dispatch).
2. Triggers the cancellation token of running tasks and moves them to `paused` in the database.
3. Moves pending tasks to `paused` in the database.

**What `resume()` does:**
1. Clears the per-domain `is_paused` flag.
2. Moves `paused` tasks back to `pending` (unless the global scheduler is also paused).

### Interaction with global pause

The scheduler must be globally unpaused **and** the domain must be unpaused for dispatch to proceed. Domain pause is additive — `handle.resume()` does not override `scheduler.pause_all()`.

| Global paused? | Domain paused? | Dispatch? |
|----------------|----------------|-----------|
| No | No | Yes |
| No | Yes | No |
| Yes | No | No |
| Yes | Yes | No |

### Use case: user-togglable features

```rust
// User disables background sync in settings.
scheduler.domain::<Sync>().pause().await?;

// Other domains continue normally.
scheduler.domain::<Media>().submit(thumb).await?;

// User re-enables sync.
scheduler.domain::<Sync>().resume().await?;
```

See [Multi-Module Applications](multi-module-apps.md#module-level-pause-and-resume) for more patterns.

## Throttle behavior

Throttling is independent of preemption. While preemption *interrupts* running tasks, throttling controls whether pending tasks are *dispatched in the first place* based on current system [pressure](glossary.md#backpressure).

The default three-tier `ThrottlePolicy`:

| Priority tier | Throttled when pressure exceeds |
|---------------|-------------------------------|
| `BACKGROUND` / `IDLE` (192+) | 50% |
| `NORMAL` (128+) | 75% |
| `HIGH` / `REALTIME` | Never |

This means: when the system is moderately busy (>50% pressure), background tasks wait. When it's heavily loaded (>75%), even normal tasks wait. High-priority and realtime tasks always run.

Pressure is an aggregate `0.0..=1.0` value from all registered [pressure sources](io-and-backpressure.md#pressure-sources).

### Custom throttle policies

If the defaults don't fit your workload, define your own thresholds:

```rust
use taskmill::{ThrottlePolicy, Priority};

// Custom: throttle IDLE at 30%, BACKGROUND at 60%, NORMAL at 80%
let policy = ThrottlePolicy::new(vec![
    (Priority::IDLE, 0.3),
    (Priority::BACKGROUND, 0.6),
    (Priority::NORMAL, 0.8),
]);
```

Thresholds are evaluated from lowest priority first. A task is throttled if its priority is at or below the threshold tier and pressure exceeds the limit.

## Retries

When an executor returns a retryable error, the task is automatically requeued at the same priority:

```rust
// In your executor:
return Err(TaskError::retryable("network timeout, will retry"));

// Non-retryable (permanent) failure:
return Err(TaskError::permanent("invalid payload, giving up"));
```

- **Retry limit** — `max_retries` (default 3) controls how many times a task can be retried before it permanently fails.
- **Priority preserved** — retried tasks keep their original priority; they aren't demoted.
- **Dedup key preserved** — the key stays occupied during retries, preventing duplicate submissions while the task is still being worked on.
- **Crash doesn't count** — if the process crashes while a task is running, the crash recovery doesn't increment `retry_count`.
