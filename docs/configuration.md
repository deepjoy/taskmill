# Configuration

## Recommended defaults

For most Tauri desktop apps, the defaults work well. Here's what you might want to change:

```rust
use std::sync::Arc;
use std::time::Duration;
use taskmill::{Module, Scheduler, ShutdownMode, StoreConfig, RetentionPolicy};

let scheduler = Scheduler::builder()
    .store_path("tasks.db")
    .module(Module::new("app")
        .executor("my-task", Arc::new(MyExecutor))
        .default_ttl(Duration::from_secs(3600)))
    .max_concurrency(8)                  // match your IO parallelism
    .shutdown_mode(ShutdownMode::Graceful(Duration::from_secs(10)))
    .with_resource_monitoring()
    .store_config(StoreConfig {
        retention_policy: Some(RetentionPolicy::MaxCount(10_000)),
        ..Default::default()
    })
    .build()
    .await?;
```

## SchedulerConfig

Controls scheduling behavior. Set via builder methods or pass directly to `Scheduler::new()`.

| Field | Type | Default | Description | Guidance |
|-------|------|---------|-------------|----------|
| `max_concurrency` | `usize` | 4 | Maximum concurrent running tasks. Adjustable at runtime via `set_max_concurrency()`. | Match your IO parallelism — 4–8 for disk-heavy, higher for network-heavy. |
| `max_retries` | `i32` | 3 | Retry limit before permanent failure. | Increase for flaky networks; decrease for tasks where retrying is wasteful. |
| `preempt_priority` | `Priority` | `REALTIME` (0) | Tasks at or above this priority trigger preemption. | Leave at `REALTIME` unless you need user-initiated tasks to preempt. |
| `poll_interval` | `Duration` | 500ms | Sleep between dispatch cycles. The scheduler also wakes immediately on submit. | Lower = more responsive but slightly more CPU. 250ms is fine for interactive apps. |
| `throughput_sample_size` | `i32` | 20 | Recent completions used for progress extrapolation. | More = smoother estimates but slower to adapt to changes in task behavior. |
| `shutdown_mode` | `ShutdownMode` | `Hard` | `Hard` cancels immediately. `Graceful(Duration)` waits for running tasks. | Always use `Graceful` for desktop apps to avoid data loss. |
| `default_ttl` | `Option<Duration>` | `None` | Global TTL applied to tasks without a per-task or per-type TTL. | Set to catch stale tasks (e.g., `Duration::from_secs(3600)` for 1 hour). |
| `expiry_sweep_interval` | `Option<Duration>` | `Some(30s)` | How often the scheduler sweeps for expired tasks. `None` disables periodic sweeps (dispatch-time checks still apply). | Lower for latency-sensitive expiry; `None` if you only need dispatch-time checks. |

### Builder methods

```rust
use std::sync::Arc;
use std::time::Duration;
use taskmill::{Module, Scheduler, Priority, ShutdownMode};

let scheduler = Scheduler::builder()
    .module(Module::new("app").executor("my-task", Arc::new(MyExecutor)))
    .max_concurrency(8)
    .max_retries(5)
    .preempt_priority(Priority::HIGH)
    .poll_interval(Duration::from_millis(250))
    .shutdown_mode(ShutdownMode::Graceful(Duration::from_secs(30)))
    .build()
    .await?;
```

## StoreConfig

Controls the SQLite connection pool and history retention.

| Field | Type | Default | Description | Guidance |
|-------|------|---------|-------------|----------|
| `max_connections` | `u32` | 16 | SQLite connection pool size. | Increase if you have many concurrent Tauri commands querying task state. |
| `retention_policy` | `Option<RetentionPolicy>` | `None` | Automatic history pruning. | Set this — without it, history grows without bound. `MaxCount(10_000)` is a good start. |
| `prune_interval` | `u64` | 100 | Completions between automatic prune runs. | Lower for apps that complete many tasks quickly; higher for slow-completing tasks. |

```rust
use taskmill::{StoreConfig, RetentionPolicy};

let scheduler = Scheduler::builder()
    .store_config(StoreConfig {
        max_connections: 32,
        retention_policy: Some(RetentionPolicy::MaxCount(10_000)),
        prune_interval: 50,
        ..Default::default()
    })
    .build()
    .await?;
```

## SamplerConfig

Controls the resource monitoring background loop. Only relevant if you call `.with_resource_monitoring()` or provide a custom `ResourceSampler`.

| Field | Type | Default | Description | Guidance |
|-------|------|---------|-------------|----------|
| `interval` | `Duration` | 1s | How often to sample system resources. | 500ms for interactive apps; 2s for background services. |
| `ewma_alpha` | `f64` | 0.3 | Smoothing factor. Higher = more responsive, lower = smoother. | 0.2 for steady workloads, 0.5 for bursty workloads. See [IO & Backpressure](io-and-backpressure.md#ewma-smoothing). |

```rust
use std::time::Duration;
use taskmill::SamplerConfig;

let scheduler = Scheduler::builder()
    .with_resource_monitoring()
    .sampler_config(SamplerConfig {
        interval: Duration::from_millis(500),
        ewma_alpha: 0.5,
    })
    .build()
    .await?;
```

## ShutdownMode

| Variant | Behavior |
|---------|----------|
| `Hard` | Cancel all running tasks immediately when the scheduler stops. |
| `Graceful(Duration)` | Stop dispatching new tasks, wait for running tasks to complete (up to the timeout), then force-cancel any remaining. |

## RetentionPolicy

| Variant | Behavior |
|---------|----------|
| `MaxCount(i64)` | Keep the N most recent history records, prune the rest. |
| `MaxAgeDays(i64)` | Keep records from the last N days, prune older entries. |

## Priority constants

| Constant | Value | Typical use |
|----------|-------|-------------|
| `Priority::REALTIME` | 0 | User-blocking, triggers preemption. |
| `Priority::HIGH` | 64 | User-initiated actions. |
| `Priority::NORMAL` | 128 | App-initiated work (default). |
| `Priority::BACKGROUND` | 192 | Maintenance tasks. |
| `Priority::IDLE` | 255 | Truly optional work. |

Custom: `Priority::new(n)` for any `u8` value.

## Graceful shutdown

When the scheduler stops (the `CancellationToken` passed to `run()` is cancelled):

- **`Hard`** (default) — all running tasks are immediately cancelled.
- **`Graceful(Duration)`** — the scheduler stops dispatching new tasks, waits for running tasks to finish up to the timeout, then cancels any stragglers.

Both modes stop the resource sampler. **For desktop apps, always use `Graceful` to avoid interrupting in-progress uploads or file operations.**

## Task TTL (time-to-live)

Tasks can expire automatically if they haven't started running within a configurable duration. TTL is resolved with a priority chain: **per-task > per-type > global default > none**.

### Per-task TTL

Set directly on a submission:

```rust
use std::time::Duration;
use taskmill::{TaskSubmission, TtlFrom};

let sub = TaskSubmission::new("sync")
    .payload_json(&data)
    .ttl(Duration::from_secs(300))          // expire after 5 minutes
    .ttl_from(TtlFrom::Submission);         // clock starts at submit time (default)
```

`TtlFrom::FirstAttempt` starts the clock when the task is first dispatched — useful when queue wait time shouldn't count against the deadline.

### Per-type TTL

Register a default TTL for all tasks of a given type via the module:

```rust
use std::sync::Arc;
use std::time::Duration;

let scheduler = Scheduler::builder()
    .module(Module::new("media")
        .executor_with_ttl("thumbnail", Arc::new(ThumbExec), Duration::from_secs(600)))
    .build()
    .await?;
```

Or set a module-wide default TTL that applies to all types in the module:

```rust
Module::new("media")
    .executor("thumbnail", Arc::new(ThumbExec))
    .default_ttl(Duration::from_secs(600))
```

Tasks submitted with an explicit `.ttl()` override the module default.

### Global default TTL

Catch-all for tasks without a per-task or per-type TTL:

```rust
use std::time::Duration;

let scheduler = Scheduler::builder()
    .default_ttl(Duration::from_secs(3600))  // 1 hour
    .build()
    .await?;
```

### Expiry sweep

The scheduler catches expired tasks in two ways:

1. **Dispatch-time** — before dispatching a task, the scheduler checks `expires_at`. This has zero extra cost.
2. **Periodic sweep** — every `expiry_sweep_interval` (default 30s), the scheduler batch-expires pending and paused tasks. Disable with `.expiry_sweep_interval(None)`.

### Child TTL inheritance

Children spawned via `ctx.spawn_child()` without an explicit TTL inherit the **remaining** parent TTL. A child can never outlive its parent's deadline. When a parent expires, its pending and paused children are cascade-expired.

### Typed tasks

Implement `ttl()` and `ttl_from()` on your `TypedTask`:

```rust
use std::time::Duration;
use taskmill::{TypedTask, TtlFrom};

impl TypedTask for SyncTask {
    const TASK_TYPE: &'static str = "sync";
    fn ttl(&self) -> Option<Duration> { Some(Duration::from_secs(300)) }
    fn ttl_from(&self) -> TtlFrom { TtlFrom::FirstAttempt }
}
```

## Dependency failure policy

When a task declares dependencies via `.depends_on()`, you can configure what happens if a dependency fails permanently. The default is `Cancel`.

| Variant | Behavior |
|---------|----------|
| `Cancel` (default) | The dependent task is moved to history with `DependencyFailed` status. Other dependents in the chain are also cascade-cancelled. |
| `Fail` | The dependent is moved to history as `DependencyFailed`, but other dependents in the chain are not affected (for manual intervention). |
| `Ignore` | The dependent is unblocked and runs anyway. The executor must handle missing upstream results. |

Set per-submission with `.on_dependency_failure(DependencyFailurePolicy::Ignore)`. There is no global builder option — the default `Cancel` is appropriate for most use cases.

## Application state

Executors often need shared services (HTTP clients, database connections, caches). Register state either globally on the builder or scoped to a specific module.

### Module-scoped state

Module state is visible only to executors within that module:

```rust
let scheduler = Scheduler::builder()
    .module(Module::new("media")
        .executor("thumbnail", Arc::new(ThumbExec))
        .app_state(MediaConfig { cdn_url: "...".into() }))
    .module(Module::new("sync")
        .executor("remote-sync", Arc::new(SyncExec))
        .app_state(SyncConfig { endpoint: "...".into() }))
    .build()
    .await?;

// Inside a media executor — checks module state first, then global:
let cfg = ctx.state::<MediaConfig>().expect("MediaConfig not registered");
```

### Global state

Registered on the builder, visible to all modules as a fallback:

```rust
let scheduler = Scheduler::builder()
    .app_state(SharedDb::new())          // all modules can access this
    .app_state(FeatureFlags { dark_mode: true })
    .module(...)
    .build()
    .await?;
```

State is keyed by `TypeId` — `ctx.state::<T>()` checks module state first, then falls back to global. Libraries that receive a pre-built scheduler can inject global state after construction:

```rust
scheduler.register_state(Arc::new(LibraryState { /* ... */ })).await;
```

## Feature flags

| Feature | Default | Description |
|---------|---------|-------------|
| `sysinfo-monitor` | Enabled | Cross-platform CPU, disk IO, and network monitoring via `sysinfo`. Disable for mobile targets or when using a custom sampler. |

```toml
# Disable platform monitoring
taskmill = { version = "0.4", default-features = false }
```

When disabled, you can still provide a custom `ResourceSampler` via `.resource_sampler()`.

## Module-level concurrency

Each module can have its own concurrency cap, independent of the global limit. Both caps must have headroom for a task to dispatch.

```rust
Module::new("media")
    .executor("thumbnail", Arc::new(ThumbExec))
    .max_concurrency(4)   // at most 4 media tasks running at once
```

Adjust at runtime via the module handle:

```rust
scheduler.module("media").set_max_concurrency(8);
let current = scheduler.module("media").max_concurrency();
```

## Tuning for specific workloads

### Desktop app with file processing

```rust
Scheduler::builder()
    .module(Module::new("files")
        .executor("thumbnail", Arc::new(ThumbExec))
        .max_concurrency(4))     // don't overwhelm the disk
    .with_resource_monitoring()  // auto-defer when disk is busy
    .shutdown_mode(ShutdownMode::Graceful(Duration::from_secs(10)))
    .store_config(StoreConfig {
        retention_policy: Some(RetentionPolicy::MaxCount(10_000)),
        ..Default::default()
    })
```

### Upload/download service

```rust
Scheduler::builder()
    .module(Module::new("uploads")
        .executor("upload", Arc::new(UploadExec))
        .max_concurrency(16))    // network tasks can run in parallel
    .with_resource_monitoring()
    .bandwidth_limit(50_000_000.0)  // 50 MB/s cap
    .group_concurrency("s3-bucket", 4)  // per-endpoint limits
    .shutdown_mode(ShutdownMode::Graceful(Duration::from_secs(30)))
```

### Background indexer

```rust
Scheduler::builder()
    .module(Module::new("indexer")
        .executor("index", Arc::new(IndexExec))
        .max_concurrency(2))     // stay out of the way
    .with_resource_monitoring()
    .sampler_config(SamplerConfig {
        ewma_alpha: 0.2,         // smooth — don't react to spikes
        ..Default::default()
    })
    .shutdown_mode(ShutdownMode::Hard)  // indexing can restart
```

## Builder reference

### `SchedulerBuilder` methods

| Method | Description |
|--------|-------------|
| `store_path(path)` | Path to the SQLite database file. |
| `store(store)` | Use a pre-opened `TaskStore`. |
| `store_config(config)` | Pool size and retention settings. |
| `module(module)` | Register a `Module` (required; at least one must be registered). |
| `max_concurrency(n)` | Set initial global max concurrent tasks. |
| `max_retries(n)` | Set global retry limit. |
| `preempt_priority(p)` | Set preemption threshold. |
| `poll_interval(d)` | Set dispatch cycle interval. |
| `shutdown_mode(mode)` | Set shutdown behavior. |
| `default_ttl(d)` | Global TTL for tasks without a per-task or module-level TTL. |
| `expiry_sweep_interval(opt_d)` | How often to sweep for expired tasks (`None` to disable). |
| `cancel_hook_timeout(d)` | Timeout for `on_cancel` hooks. |
| `pressure_source(source)` | Add a `PressureSource` to the composite. |
| `throttle_policy(policy)` | Set a custom `ThrottlePolicy`. |
| `with_resource_monitoring()` | Enable platform resource monitoring. |
| `resource_sampler(sampler)` | Provide a custom `ResourceSampler`. |
| `sampler_config(config)` | Configure sample interval and smoothing. |
| `bandwidth_limit(bytes_per_sec)` | Set a network bandwidth cap; registers a built-in `NetworkPressure` source. |
| `default_group_concurrency(n)` | Default concurrency limit for grouped tasks (0 = unlimited). |
| `group_concurrency(group, n)` | Per-group concurrency limit override. |
| `app_state(state)` | Register global state visible to all modules. |
| `app_state_arc(arc)` | Register global state from a pre-existing `Arc`. |
| `build()` | Build and return the `Scheduler`. |

### `Module` builder methods

| Method | Description |
|--------|-------------|
| `Module::new(name)` | Create a module with the given name. Task types are prefixed `"{name}::"`. |
| `executor(name, executor)` | Register a `TaskExecutor` by type name. |
| `typed_executor::<T>(executor)` | Register using `T::TASK_TYPE` as the name. |
| `executor_with_ttl(name, executor, ttl)` | Register with a per-type default TTL. |
| `executor_with_retry_policy(name, executor, policy)` | Register with a per-type retry policy. |
| `executor_with_options(name, executor, ttl, policy)` | Register with both TTL and retry policy. |
| `default_priority(p)` | Module-wide priority for all submissions. |
| `default_retry_policy(policy)` | Module-wide retry policy. |
| `default_group(group)` | Module-wide group key. |
| `default_ttl(d)` | Module-wide TTL (overridden by per-task TTL). |
| `default_tag(key, value)` | Tag applied to all submissions through this module's handle. |
| `max_concurrency(n)` | Module-level concurrency cap (independent of global). |
| `app_state(state)` | Register module-scoped state (checked before global state). |
| `app_state_arc(arc)` | Register module-scoped state from a pre-existing `Arc`. |
