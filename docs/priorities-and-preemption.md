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

Tasks are dispatched in priority order. Within the same priority tier, tasks are dispatched in insertion order (FIFO) — the task submitted first runs first.

When [priority aging](#priority-aging) is enabled, the scheduler uses *effective priority* (base priority adjusted for wait time) instead of stored priority. When [group weights](#weighted-fair-scheduling) are configured, the scheduler uses a multi-pass dispatch loop that allocates slots proportionally to group weights before falling back to global priority order.

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

### Rate limiting

While concurrency limits control how many tasks run *simultaneously*, rate limits control how many tasks *start per unit of time*. This is essential when fast tasks (completing in milliseconds) produce bursts that overwhelm external APIs.

Rate limits use a token-bucket algorithm and can be scoped by task type, group, or both:

```rust
Scheduler::builder()
    .rate_limit("media::upload", RateLimit::per_second(100))
    .group_rate_limit("s3://prod", RateLimit::per_second(50).with_burst(75))
    .build()
    .await?;
```

When a task is rate-limited, the scheduler sets its `run_after` to the next token availability. This prevents head-of-line blocking — other task types dispatch normally while the rate-limited type waits.

Rate limits are checked *after* all other gate checks (backpressure, IO budget, concurrency), so tokens are never wasted on tasks that would be rejected for other reasons.

See [Configuration — Rate limiting](configuration.md#rate-limiting) for full details and runtime adjustment APIs.

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

## Priority aging

When high-priority tasks arrive continuously, low-priority work can be starved indefinitely. Priority aging prevents this by gradually promoting tasks that have been waiting too long.

### How it works

At dispatch time, the scheduler computes an *effective priority* for each pending task:

```text
age = now - created_at - pause_duration
promotions = max(0, (age - grace_period) / aging_interval)
effective = max(base_priority - promotions, max_effective_priority)
```

Lower numeric value = higher priority. Aging *decreases* the numeric value (promotes the task). The stored priority is never mutated — effective priority is a pure dispatch-time computation.

### Configuring aging

```rust
use taskmill::{AgingConfig, Priority, Scheduler};
use std::time::Duration;

let scheduler = Scheduler::builder()
    .priority_aging(AgingConfig {
        grace_period: Duration::from_secs(300),    // 5 min before aging starts
        aging_interval: Duration::from_secs(60),   // promote 1 level per minute
        max_effective_priority: Priority::HIGH,     // can't age above HIGH
        urgent_threshold: None,                     // see fair scheduling
    })
    // ...
    .build()
    .await?;
```

| Field | Default | Description |
|-------|---------|-------------|
| `grace_period` | 5 minutes | How long a task must wait before aging begins. |
| `aging_interval` | 60 seconds | Time between each one-step priority promotion. |
| `max_effective_priority` | `HIGH` (64) | Priority ceiling — tasks cannot age above this. Use `REALTIME` to allow aging to the absolute highest level. |
| `urgent_threshold` | `None` | When effective priority reaches this level, the task may bypass group weight allocation (see [weighted fair scheduling](#weighted-fair-scheduling)). Must be `>=` `max_effective_priority` numerically. |

### Aging interactions

- **Paused tasks** — the aging clock is frozen while a task is paused. Accumulated pause time is excluded from the age calculation.
- **Retry** — when a task is requeued for retry, the aging clock continues from the original `created_at`. The task has been waiting even longer.
- **Recurring tasks** — each new recurring instance gets a fresh `created_at`, so the aging clock starts at zero.
- **Child tasks** — children inherit the *higher* of the parent's current effective priority and the child's own configured priority. This promotes children of aged parents without demoting children whose task type is inherently higher-priority.
- **Supersede / resubmit** — the new task gets a fresh `created_at`, so the aging clock resets.
- **Crash recovery** — if the scheduler crashes while tasks are paused, the accumulated pause time is correctly accounted for on recovery (aging clock runs slightly fast for the crash window, which is acceptable for anti-starvation).

### Observability

Effective priority is visible in events and snapshots:

- `TaskEventHeader` includes both `base_priority` and `effective_priority`. Compare them to detect aging: `effective_priority < base_priority` means the task has been promoted.
- `SchedulerSnapshot` includes `aging_config` when aging is enabled.

### Opt-in, zero cost when off

Without `priority_aging()`, dispatch queries use the original `ORDER BY priority ASC, id ASC` — fully index-ordered with zero overhead.

## Weighted fair scheduling

Group concurrency limits are *caps* (max N slots for group X), not *floors*. Without weighted allocation, a group with a large pending queue can fill all available global slots (up to its cap), starving other groups with legitimate work.

Weighted fair scheduling allocates dispatch slots proportionally to group weights, ensuring each group gets a fair share of capacity.

### How it works

When group weights are configured, the scheduler uses a three-pass dispatch loop:

1. **Fair pass** — each group (including ungrouped tasks as a virtual group) receives slots proportional to its weight. Minimum slot guarantees (`min_slots`) are honored first.
2. **Greedy pass** — any slots left unfilled by the fair pass (under-demand groups, rate-limited tasks) are filled by global priority order. This makes the scheduler work-conserving.
3. **Urgent pass** — tasks aged past `urgent_threshold` (if configured) bypass group weights but still respect `max_concurrency`. This is a safety valve for severely starved tasks.

### Configuring group weights

```rust
use taskmill::Scheduler;

let scheduler = Scheduler::builder()
    .group_weight("api-calls", 3)      // 3x the weight of default
    .group_weight("background", 1)     // 1x (default weight)
    .default_group_weight(1)           // weight for groups without an override
    .group_minimum_slots("critical", 2) // always at least 2 slots for "critical"
    .group_concurrency("api-calls", 8) // cap at 8 concurrent (still enforced)
    .max_concurrency(16)
    // ...
    .build()
    .await?;
```

Weights are relative — `(A:3, B:1)` gives A 75% and B 25% of capacity. Ungrouped tasks participate as a virtual group with the default weight, ensuring they compete fairly rather than only receiving leftovers.

### Runtime adjustment

```rust
// Update weight at runtime
scheduler.set_group_weight("api-calls", 5);

// Remove override, falling back to default weight
scheduler.remove_group_weight("api-calls");

// Reset all weights
scheduler.reset_group_weights();

// Set minimum guaranteed slots
scheduler.set_group_minimum_slots("critical", 3);
```

### Interactions with other features

- **Group concurrency caps** — caps are applied during allocation and also enforced by the dispatch gate as a safety net. The allocation respects caps; excess is redistributed to other groups.
- **Group pause** — paused groups are excluded from the allocation. Their capacity is released to other groups. On resume, they re-enter the allocation on the next dispatch cycle.
- **Rate limits** — rate limit checks still run during fair dispatch. A rate-limited task leaves its group's slot unfilled, which the greedy pass fills from other groups.
- **Preemption** — preemption operates independently of the allocation. A preempting task may temporarily exceed its group's allocation; the allocation rebalances on the next cycle.
- **Priority aging** — aging and fair scheduling compose. An aged task in a low-weight group dispatches in priority order within its group's allocation. Tasks aged past `urgent_threshold` bypass group weights entirely.

### Observability

`SchedulerSnapshot` includes `group_allocations` — a `Vec<GroupAllocationInfo>` with per-group detail:

```rust
let snap = scheduler.snapshot().await?;
for alloc in &snap.group_allocations {
    println!(
        "{}: weight={}, slots={}, running={}, pending={}, min={:?}, cap={:?}",
        alloc.group, alloc.weight, alloc.allocated_slots,
        alloc.running, alloc.pending, alloc.min_slots, alloc.cap,
    );
}
```

A `GroupWeightChanged` event is emitted when `set_group_weight()` is called.
