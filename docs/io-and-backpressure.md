# IO Tracking & Backpressure

## The problem

Spawning 50 image resizes at once will saturate your disk. The OS I/O queue backs up, file writes stall, the UI freezes, and other applications slow to a crawl. The same happens with network-heavy tasks — bulk uploads can exhaust your bandwidth.

Taskmill prevents this with two complementary mechanisms:

- **IO budget tracking** — each task declares how much IO it expects, and the scheduler checks whether there's headroom before dispatching.
- **Backpressure** — external signals (disk load, network usage, memory, anything you want) tell the scheduler to slow down.

Both are optional. If you don't configure resource monitoring, the scheduler dispatches purely by priority and concurrency limits.

## IO budgets: telling the scheduler what to expect

Every task can declare its expected IO when submitted. This is your best estimate of how many bytes the task will read and write:

```rust
use taskmill::IoBudget;

// Disk-heavy task: reads a 50 KB file, writes a 10 KB thumbnail
let sub = TaskSubmission::new("thumbnail")
    .payload_json(&data)
    .expected_io(IoBudget::disk(50_000, 10_000));

// Network-heavy task: uploads 50 MB
let sub = TaskSubmission::new("upload")
    .payload_json(&upload_payload)
    .expected_io(IoBudget::net(0, 50_000_000));
```

**What if you don't know the IO cost?** Start with a rough estimate. Taskmill stores actual IO from completed tasks, so you can query `avg_throughput()` to refine your estimates over time. If you don't provide an IO budget at all, the task is treated as having zero expected IO and won't be gated.

### Reporting actual IO

Executors report actual IO during execution. This data is stored in history for learning:

```rust
// Disk IO
ctx.record_read_bytes(48_312);
ctx.record_write_bytes(9_876);

// Network IO
ctx.record_net_rx_bytes(response_size);
ctx.record_net_tx_bytes(uploaded_bytes);
```

### How the scheduler uses IO budgets

When resource monitoring is enabled, the scheduler checks IO headroom before dispatching each task:

1. Read the current smoothed disk throughput from the resource monitor.
2. Sum the expected IO across all currently running tasks.
3. Compute a 2-second capacity window: `capacity = throughput * 2.0`.
4. If running IO + the candidate task's IO would exceed 80% of capacity (on either read or write), the task is deferred to the next cycle.

This prevents the scheduler from piling up IO-heavy tasks that would saturate the disk. The threshold is conservative — 80% leaves headroom for other applications and OS operations.

### Learning from history

Use store queries to see how your tasks actually perform and refine future estimates:

```rust
let store = scheduler.store();

// Average throughput for a task type (from recent completions)
let (avg_read_bps, avg_write_bps) = store.avg_throughput("thumbnail", 20).await?;

// Aggregate stats: count, avg duration, avg IO, failure rate
let stats = store.history_stats("thumbnail").await?;
```

## Resource monitoring

Resource monitoring gives the scheduler real-time visibility into system load. **For most applications, just add `.with_resource_monitoring()` to the builder:**

```rust
let scheduler = Scheduler::builder()
    .with_resource_monitoring()  // uses the built-in platform sampler
    .build()
    .await?;
```

This starts a background loop that samples CPU, disk IO, and network throughput every second using the `sysinfo` crate (Linux, macOS, Windows). Readings are smoothed with an [EWMA](glossary.md) (exponentially weighted moving average) to avoid overreacting to momentary spikes — a brief disk flush won't suddenly block all tasks.

### EWMA smoothing

Raw resource samples are smoothed before the scheduler uses them:

```
smoothed = alpha * raw + (1 - alpha) * previous
```

- **Higher alpha** (e.g., 0.5) = more responsive to recent changes. Good for bursty workloads where conditions change quickly.
- **Lower alpha** (e.g., 0.2) = smoother, more stable. Good for steady workloads where you don't want momentary spikes to affect scheduling.
- **Default alpha: 0.3** — a balanced middle ground.

Configure smoothing:

```rust
use std::time::Duration;
use taskmill::SamplerConfig;

let scheduler = Scheduler::builder()
    .with_resource_monitoring()
    .sampler_config(SamplerConfig {
        interval: Duration::from_millis(500),  // sample faster
        ewma_alpha: 0.5,                       // more responsive
    })
    .build()
    .await?;
```

### Advanced: custom samplers

For containers, cgroups, or mobile platforms where `sysinfo` doesn't work, provide your own `ResourceSampler`:

```rust
use taskmill::{ResourceSampler, ResourceSnapshot};

struct CgroupSampler;

impl ResourceSampler for CgroupSampler {
    fn sample(&mut self) -> ResourceSnapshot {
        ResourceSnapshot {
            cpu_usage: read_cgroup_cpu(),              // 0.0–1.0
            io_read_bytes_per_sec: read_blkio_read(),
            io_write_bytes_per_sec: read_blkio_write(),
            net_rx_bytes_per_sec: read_net_rx(),
            net_tx_bytes_per_sec: read_net_tx(),
        }
    }
}

let scheduler = Scheduler::builder()
    .resource_sampler(Box::new(CgroupSampler))
    .build()
    .await?;
```

Disable the built-in sampler in `Cargo.toml`:

```toml
taskmill = { version = "0.3", default-features = false }
```

## Backpressure: external pressure signals

Sometimes you need to slow down for reasons beyond disk IO — an API is rate-limited, memory is tight, the laptop is on battery. Taskmill lets you plug in any number of custom pressure signals.

### Pressure sources

Implement the `PressureSource` trait to expose a `0.0..=1.0` signal:

```rust
use taskmill::PressureSource;

struct MemoryPressure;

impl PressureSource for MemoryPressure {
    fn pressure(&self) -> f32 {
        let used = sys_info::mem_used();
        let total = sys_info::mem_total();
        (used as f32 / total as f32).min(1.0)
    }

    fn name(&self) -> &str { "memory" }
}
```

**Common pressure sources you might build:**

| Signal | What it measures | When to use |
|--------|-----------------|-------------|
| Memory usage | RSS or system memory | Apps that process large files in memory |
| API rate limits | Remaining quota / total quota | Upload services with provider limits |
| Battery level | Charge percentage (inverted) | Mobile or laptop apps |
| Queue depth | Pending tasks / capacity | Prevent unbounded queue growth |

### Composing multiple sources

Multiple sources are aggregated via `CompositePressure`. The aggregate pressure is the **maximum** across all sources — the system is as pressured as its most constrained resource:

```rust
use taskmill::CompositePressure;

let mut pressure = CompositePressure::new();
pressure.add_source(Box::new(MemoryPressure));
pressure.add_source(Box::new(QueueDepthPressure));
// Aggregate = max(memory_pressure, queue_pressure)
```

Or via the builder:

```rust
let scheduler = Scheduler::builder()
    .pressure_source(Box::new(MemoryPressure))
    .pressure_source(Box::new(QueueDepthPressure))
    .build()
    .await?;
```

The aggregate pressure feeds into the [throttle policy](priorities-and-preemption.md#throttle-behavior), which decides which priority tiers to defer.

### How dispatch gating works

The scheduler combines both mechanisms. A task is dispatched only when **both** pass:

1. **Backpressure check** — the throttle policy says this priority tier is allowed at the current pressure level.
2. **IO budget check** — the system has enough IO headroom for this task.

If either check fails, the task stays in the queue and is retried on the next poll cycle.

## Network bandwidth pressure

For upload/download-heavy applications, taskmill includes a built-in `NetworkPressure` source. It maps observed network throughput against a configurable bandwidth cap:

```rust
let scheduler = Scheduler::builder()
    .with_resource_monitoring()
    .bandwidth_limit(100_000_000.0)  // 100 MB/s combined RX+TX cap
    .build()
    .await?;
```

When observed RX+TX throughput approaches the cap, pressure rises toward 1.0 and the throttle policy starts deferring lower-priority tasks. This is especially useful when you want to leave bandwidth for other applications or avoid saturating a network link.

## Diagnostics

The `SchedulerSnapshot` includes pressure readings for debugging — useful for understanding why tasks are being deferred:

```rust
let snap = scheduler.snapshot().await?;
println!("Aggregate pressure: {:.0}%", snap.pressure * 100.0);
for (name, value) in &snap.pressure_breakdown {
    println!("  {}: {:.0}%", name, value * 100.0);
}
```
