# Glossary

Quick reference for terms used throughout the taskmill documentation.

| Term | Definition |
|------|------------|
| **Backpressure** | Slowing down new work when the system is already busy. Taskmill uses [pressure sources](io-and-backpressure.md#backpressure-external-pressure-signals) to detect load and [throttle policies](priorities-and-preemption.md#throttle-behavior) to decide which tasks to defer. |
| **Delayed task** | A task with a `run_after` timestamp that defers dispatch until that time arrives. If the timestamp is in the past (e.g., after a restart), the task runs immediately. See [Quick Start](quick-start.md#delayed-tasks). |
| **Deduplication (dedup)** | Preventing the same task from being queued twice. Taskmill generates a SHA-256 key from the task type and payload; a second submission with the same key is silently ignored. See [Persistence & Recovery](persistence-and-recovery.md#deduplication). |
| **Dispatch** | Moving a task from "waiting in line" (pending) to "actively running." The scheduler dispatches tasks in priority order, subject to concurrency limits and backpressure. |
| **EWMA** | Exponentially Weighted Moving Average — a smoothing technique that gives recent measurements more weight than old ones. Taskmill uses EWMA to smooth resource readings so the scheduler doesn't overreact to momentary spikes. See [IO & Backpressure](io-and-backpressure.md#ewma-smoothing). |
| **Executor** | Your code that performs the actual work for a task type. Implements the `TaskExecutor` trait. See [Quick Start](quick-start.md#implement-an-executor). |
| **IO budget** | An estimate of how many bytes a task will read and write (disk and/or network), submitted alongside the task. The scheduler uses IO budgets to avoid overwhelming the disk. See [IO & Backpressure](io-and-backpressure.md#io-budgets-telling-the-scheduler-what-to-expect). |
| **Pile-up prevention** | The mechanism that skips a recurring task instance when the previous instance hasn't been dispatched yet, preventing unbounded queue growth under sustained load. See [Quick Start](quick-start.md#recurring-tasks). |
| **Preemption** | Pausing lower-priority work so higher-priority work can run immediately. Preempted tasks resume automatically once the urgent work finishes. See [Priorities & Preemption](priorities-and-preemption.md#preemption). |
| **Pressure source** | Anything that signals the system is busy — disk IO, network throughput, memory usage, API rate limits, battery level. Returns a value from 0.0 (idle) to 1.0 (saturated). See [IO & Backpressure](io-and-backpressure.md#pressure-sources). |
| **Task group** | A named set of tasks that share a concurrency limit. For example, you might limit uploads to a specific S3 bucket to 3 at a time. See [Priorities & Preemption](priorities-and-preemption.md#task-groups). |
| **Throttle policy** | Rules that map system pressure to dispatch decisions. The default policy defers background tasks when pressure exceeds 50% and normal tasks when it exceeds 75%, but never blocks high-priority work. See [Priorities & Preemption](priorities-and-preemption.md#throttle-behavior). |
| **TTL (time-to-live)** | A duration after which a task automatically expires if it hasn't started running. Configurable per-task, per-type, or as a global default. See [Configuration](configuration.md#task-ttl-time-to-live). |
| **TtlFrom** | Controls when the TTL clock starts: `Submission` (at submit time, the default) or `FirstAttempt` (when the task is first dispatched). See [Configuration](configuration.md#task-ttl-time-to-live). |
| **Recurring schedule** | The configuration that controls how a recurring task re-submits itself: interval (or delay between runs), maximum number of occurrences, and initial delay. Managed via `RecurringSchedule`. See [Quick Start](quick-start.md#recurring-tasks). |
| **Recurring task** | A task that automatically re-submits itself after each completion according to a `RecurringSchedule`. Recurring schedules survive restarts and support pause, resume, and cancel operations. See [Quick Start](quick-start.md#recurring-tasks). |
| **Typed task** | A struct that implements the `TypedTask` trait, giving you compile-time type safety for task payloads, priorities, and IO budgets instead of stringly-typed submissions. See [Quick Start](quick-start.md#typed-tasks). |
