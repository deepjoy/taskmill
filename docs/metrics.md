# Observability Metrics

Taskmill provides built-in observability through two complementary systems:

1. **Always-on internal counters** — cheap `AtomicU64` counters maintained regardless of feature flags, exposed via `Scheduler::metrics_snapshot()`.
2. **`metrics` crate integration** (optional) — when the `metrics` Cargo feature is enabled, the scheduler emits counters, gauges, and histograms via the standard [`metrics`](https://crates.io/crates/metrics) facade. Consumers choose their exporter (Prometheus, StatsD, Datadog, etc.).

## Quick Start

### Without the `metrics` feature (default)

```rust
let snap = scheduler.metrics_snapshot().await;
println!("submitted: {}, completed: {}, failed: {}",
    snap.submitted, snap.completed, snap.failed);
println!("pending: {}, running: {}, pressure: {:.2}",
    snap.pending, snap.running, snap.pressure);
```

### With the `metrics` feature

```toml
[dependencies]
taskmill = { version = "0.6", features = ["metrics"] }
metrics-exporter-prometheus = "0.16"
```

```rust
// Install a Prometheus exporter (or any metrics recorder).
let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
builder.install().expect("failed to install Prometheus recorder");

// Build the scheduler — metrics are automatically emitted.
let scheduler = Scheduler::builder()
    .store_path("tasks.db")
    .domain(Domain::<MyApp>::new().task(MyExecutor))
    .metrics_prefix("myapp")           // → myapp_taskmill_*
    .metrics_label("service", "ingest") // global label on every metric
    .build()
    .await?;
```

## MetricsSnapshot

`Scheduler::metrics_snapshot()` returns a `MetricsSnapshot` struct with:

### Counters (cumulative since scheduler creation)

| Field | Description |
|-------|-------------|
| `submitted` | Total tasks accepted into the queue |
| `dispatched` | Total tasks that entered Running state |
| `completed` | Total successful completions |
| `failed` | Total failures (retryable + permanent) |
| `failed_retryable` | Subset of `failed` that were retryable |
| `retried` | Total retry requeue attempts |
| `dead_lettered` | Tasks that exhausted retries |
| `superseded` | Tasks replaced by newer submissions with the same dedup key |
| `cancelled` | Explicit cancellations |
| `expired` | Tasks that hit TTL before dispatch |
| `preempted` | Tasks preempted by higher-priority work |
| `batches_submitted` | Total `submit_batch()` calls |
| `gate_denials` | Dispatch gate rejections (backpressure, IO budget, concurrency, rate limit) |
| `rate_limit_throttles` | Rate limit token exhaustion events |
| `group_pauses` | Group pause events |
| `group_resumes` | Group resume events |
| `dependency_failures` | Blocked tasks cancelled because a dependency failed |
| `recurring_skipped` | Recurring instances skipped due to pile-up prevention |

### Gauges (point-in-time)

| Field | Description |
|-------|-------------|
| `pending` | Current queue depth |
| `running` | Current running task count |
| `blocked` | Tasks waiting on unmet dependencies |
| `paused` | Tasks in pause state |
| `waiting` | Parent tasks waiting for children |
| `pressure` | Aggregate backpressure (0.0–1.0) |
| `max_concurrency` | Current concurrency cap |
| `groups_paused` | Number of currently paused groups |

## `metrics` Crate Metrics Reference

All metrics use the prefix `taskmill_` (customizable via `SchedulerBuilder::metrics_prefix`).

### Counters

| Metric | Labels | Description |
|--------|--------|-------------|
| `taskmill_tasks_submitted_total` | `type`, `module`, `group` | Total tasks accepted into the queue |
| `taskmill_tasks_dispatched_total` | `type`, `module`, `group` | Total tasks that entered Running state |
| `taskmill_tasks_completed_total` | `type`, `module`, `group` | Total successful completions |
| `taskmill_tasks_failed_total` | `type`, `module`, `group`, `retryable` | Total failures, split by retryability |
| `taskmill_tasks_retried_total` | `type`, `module`, `group` | Total retry requeue attempts |
| `taskmill_tasks_dead_lettered_total` | `type`, `module`, `group` | Tasks that exhausted retries |
| `taskmill_tasks_superseded_total` | `type`, `module`, `group` | Tasks replaced by newer submissions |
| `taskmill_tasks_cancelled_total` | `type`, `module`, `group` | Explicit cancellations |
| `taskmill_tasks_expired_total` | `type`, `module`, `group` | Tasks that hit TTL before dispatch |
| `taskmill_tasks_preempted_total` | `type`, `module` | Tasks preempted by higher-priority work |
| `taskmill_batches_submitted_total` | — | Total batch submission calls |
| `taskmill_gate_denials_total` | `reason` | Dispatch gate rejections by reason |
| `taskmill_rate_limit_throttles_total` | `scope_kind`, `scope` | Rate limit token exhaustion events |
| `taskmill_group_pauses_total` | `group` | Group pause events |
| `taskmill_group_resumes_total` | `group` | Group resume events |
| `taskmill_dependency_failures_total` | — | Blocked tasks cancelled because a dependency failed |
| `taskmill_recurring_skipped_total` | `type`, `module` | Recurring instances skipped |

### Gauges

Updated each dispatch cycle (~500ms default).

| Metric | Labels | Description |
|--------|--------|-------------|
| `taskmill_tasks_pending` | — | Current queue depth |
| `taskmill_tasks_running` | — | Current running task count |
| `taskmill_tasks_blocked` | — | Tasks waiting on unmet dependencies |
| `taskmill_tasks_paused` | — | Tasks in pause state |
| `taskmill_tasks_waiting` | — | Parent tasks waiting for children |
| `taskmill_max_concurrency` | — | Current concurrency cap |
| `taskmill_pressure` | — | Aggregate backpressure (0.0–1.0) |
| `taskmill_pressure_source` | `source` | Per-source pressure level |
| `taskmill_groups_paused_count` | — | Number of currently paused groups |
| `taskmill_rate_limit_tokens_available` | `scope_kind`, `scope` | Current available tokens per rate-limit bucket |
| `taskmill_module_tasks_running` | `module` | Running tasks per registered module |

### Histograms

| Metric | Labels | Description |
|--------|--------|-------------|
| `taskmill_task_duration_seconds` | `type`, `module`, `status` | Wall-clock execution time (completed/failed) |
| `taskmill_task_queue_wait_seconds` | `type`, `module` | Time from submission to dispatch start |
| `taskmill_batch_size` | — | Number of tasks per batch submission call |
| `taskmill_completion_batch_size` | — | Number of completions coalesced per drain cycle |
| `taskmill_failure_batch_size` | — | Number of failures coalesced per drain cycle |

## Recommended Dashboard Layout (Prometheus/Grafana)

### Row 1 — Throughput & Queue Health

| Panel | PromQL | Signal |
|-------|--------|--------|
| Submission rate | `rate(taskmill_tasks_submitted_total[5m])` | How fast work is arriving |
| Throughput | `rate(taskmill_tasks_completed_total[5m])` | How fast work is completing |
| Queue depth | `taskmill_tasks_pending` | Primary health signal |
| Absorption ratio | `rate(taskmill_tasks_dispatched_total[5m]) / rate(taskmill_tasks_submitted_total[5m])` | Values <1.0 = queue growing |

### Row 2 — Failure & Retry Health

| Panel | PromQL | Signal |
|-------|--------|--------|
| Failure rate | `rate(taskmill_tasks_failed_total[5m])` by `retryable` | Transient vs permanent |
| Retry ratio | `rate(taskmill_tasks_retried_total[5m]) / rate(taskmill_tasks_dispatched_total[5m])` | >10% warrants investigation |
| Dead letter rate | `rate(taskmill_tasks_dead_lettered_total[5m])` | Alert on any nonzero |
| Expiry rate | `rate(taskmill_tasks_expired_total[5m])` | Correlate with queue depth |

### Row 3 — Latency Distributions

| Panel | PromQL | Signal |
|-------|--------|--------|
| Execution p50/p95/p99 | `histogram_quantile(0.95, rate(taskmill_task_duration_seconds_bucket[5m]))` | Tail latency |
| Queue wait p50/p95/p99 | `histogram_quantile(0.95, rate(taskmill_task_queue_wait_seconds_bucket[5m]))` | Time in queue |

### Row 4 — Capacity & Bottlenecks

| Panel | PromQL | Signal |
|-------|--------|--------|
| Concurrency utilization | `taskmill_tasks_running / taskmill_max_concurrency` | Sustained 1.0 = at limit |
| Backpressure | `taskmill_pressure` | >0.8 = active throttling |
| Gate denials by reason | `rate(taskmill_gate_denials_total[5m])` by `reason` | Primary bottleneck |

## Suggested Alert Rules

| Alert | Condition | Severity |
|-------|-----------|----------|
| Queue growing | `taskmill_tasks_pending > <threshold>` for 10m | Warning |
| Dead letters | `rate(taskmill_tasks_dead_lettered_total[5m]) > 0` for 5m | Critical |
| High retry ratio | retry ratio > 0.1 for 15m | Warning |
| Sustained backpressure | `taskmill_pressure > 0.9` for 10m | Warning |
| Queue wait SLO breach | p95 queue wait > SLO for 5m | Warning |

## Builder API

```rust
Scheduler::builder()
    // Prefix all metric names: "myapp_taskmill_*"
    .metrics_prefix("myapp")

    // Add global labels to every metric
    .metrics_label("service", "ingest")
    .metrics_label("env", "production")

    // Suppress specific metrics (use unprefixed name)
    .disable_metric("task_duration_seconds")

    .build()
    .await?;
```

## Design Principles

- **Zero-cost when unused** — no overhead when no `metrics` recorder is installed. Internal atomic counters cost a few cache lines.
- **Standard facade** — uses the `metrics` crate so consumers choose their exporter.
- **Source-level instrumentation** — metrics emitted where the event happens, not from a channel subscriber.
- **Bounded label cardinality** — only `type`, `module`, `group`, and `reason` appear as labels. Never `task_id`, `key`, or user-provided `tags`.
- **No allocations on the hot path** — label values are borrowed `&str` or small stack strings.
