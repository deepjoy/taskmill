# Progress & Events

Taskmill provides real-time progress tracking and lifecycle events so your UI always reflects what's happening — tasks starting, progressing, completing, failing, or being preempted.

## Reporting progress from executors

Executors report progress via `ctx.progress()`. This emits events that your UI can subscribe to for real-time updates.

```rust
impl TypedExecutor<MyTask> for MyExecutor {
    async fn execute(&self, task: MyTask, ctx: &TaskContext) -> Result<(), TaskError> {
        let items = get_work_items(&task);

        for (i, item) in items.iter().enumerate() {
            process(item).await;

            // Percentage-based (0.0 to 1.0)
            ctx.progress().report(
                (i + 1) as f32 / items.len() as f32,
                Some(format!("processed {}/{}", i + 1, items.len())),
            );
        }

        Ok(())
    }
}
```

For count-based progress, use the convenience method:

```rust
ctx.progress().report_fraction(processed, total, Some("importing".into()));
// Automatically computes: processed as f32 / total as f32
```

## Automatic progress extrapolation

Even tasks that don't explicitly report progress show movement in your UI. The scheduler extrapolates progress from historical data:

1. Look up the average duration for this task type from completed tasks in history.
2. Calculate how long the current task has been running.
3. Estimate progress as elapsed time / average duration.
4. If the executor has reported partial progress, blend historical and current rates for a more accurate estimate.
5. Cap at 99% — extrapolation never reaches 100%, so a task only shows "complete" when it actually finishes.

This means your progress bar always moves, even for tasks that don't call `report()`.

## Lifecycle events

All scheduler state changes are broadcast as `SchedulerEvent` variants. Subscribe via `scheduler.subscribe()` for the global stream, `handle.events()` for domain-filtered events, or `handle.task_events::<T>()` for typed per-task-type events:

```rust
// Global event stream (all domains)
let mut events = scheduler.subscribe();
tokio::spawn(async move {
    while let Ok(event) = events.recv().await {
        match &event {
            SchedulerEvent::Progress { header, percent, message } => {
                update_progress_bar(header.task_id, *percent, message.as_deref());
            }
            SchedulerEvent::Completed(header) => {
                mark_done(header.task_id);
            }
            SchedulerEvent::Failed { header, error, will_retry, .. } => {
                if !will_retry {
                    show_error(header.task_id, error);
                }
            }
            _ => {}
        }
    }
});

// Domain-filtered events (only events for one domain)
let media = scheduler.domain::<Media>();
let mut media_events = media.events();
tokio::spawn(async move {
    while let Ok(event) = media_events.recv().await {
        // only media:: events arrive here
    }
});

// Typed per-task-type events (only events for one task type)
let mut thumb_events = media.task_events::<Thumbnail>();
tokio::spawn(async move {
    while let Ok(event) = thumb_events.recv().await {
        match event {
            TaskEvent::Completed { id, record, .. } => {
                println!("thumbnail {id} done");
            }
            TaskEvent::Failed { id, error, .. } => {
                eprintln!("thumbnail {id} failed: {error}");
            }
            _ => {}
        }
    }
});
```

### Event reference

| Event | When it fires |
|-------|---------------|
| `Dispatched(TaskEventHeader)` | Task picked from queue and executor spawned |
| `Completed(TaskEventHeader)` | Task finished successfully |
| `Failed { header, error, will_retry, retry_after }` | Task failed — `will_retry` tells you if it's being requeued, `retry_after` is the backoff delay |
| `Preempted(TaskEventHeader)` | Task paused for higher-priority work |
| `Cancelled(TaskEventHeader)` | Task cancelled via `scheduler.cancel()` |
| `Progress { header, percent, message }` | Progress update from executor |
| `Waiting { task_id, children_count }` | Parent task waiting for children to complete |
| `TaskExpired { header, age }` | Task expired (TTL exceeded) — `age` is the time since the TTL clock started |
| `RecurringSkipped { header, reason }` | A recurring instance was skipped (e.g., pile-up prevention) |
| `RecurringCompleted { header, occurrences }` | A recurring schedule finished all its occurrences |
| `TaskUnblocked { task_id }` | A blocked task's dependencies are all satisfied — it transitions to `pending` |
| `DeadLettered { header, error, retry_count }` | Task exhausted all retries — can be re-submitted via `retry_dead_letter()` |
| `DependencyFailed { task_id, failed_dependency }` | A blocked task was cancelled because a dependency failed permanently |
| `Paused` | Scheduler globally paused via `pause_all()` |
| `Resumed` | Scheduler resumed via `resume_all()` |

Task-specific events share a `TaskEventHeader` with `task_id`, `task_type`, `key`, and `label`. Use `event.header()` to access it generically.

### Which events to listen for

| If you're building... | Listen to |
|-----------------------|-----------|
| A progress bar | `Progress`, `Completed`, `Failed` |
| An activity log | All events |
| Error alerting | `Failed` where `will_retry` is false, `DeadLettered` |
| A "pause/resume" button | `Paused`, `Resumed` |
| Upload status indicators | `Dispatched`, `Progress`, `Completed`, `Failed`, `Preempted` |
| Stale task cleanup UI | `TaskExpired` |
| Recurring schedule monitoring | `RecurringSkipped`, `RecurringCompleted` |
| Dependency chain tracking | `TaskUnblocked`, `DependencyFailed` |

## Querying progress

### All running tasks

```rust
let progress = scheduler.estimated_progress().await;
for p in &progress {
    println!("{} ({}): {:.0}%", p.header.task_type, p.header.key, p.percent * 100.0);
    // p.reported_percent  — last executor-reported value (if any)
    // p.extrapolated_percent — throughput-based estimate (if any)
    // p.percent — best available: reported if present, else extrapolated
}
```

## Dashboard snapshots

For UI dashboards, `scheduler.snapshot()` gathers all scheduler state in a single call — running tasks, queue depth, progress, pressure, and configuration:

```rust
let snap = scheduler.snapshot().await?;
// snap.running          — Vec<TaskRecord> of currently executing tasks
// snap.pending_count    — number of tasks waiting to dispatch
// snap.paused_count     — number of preempted tasks
// snap.progress         — Vec<EstimatedProgress> for every running task
// snap.pressure         — aggregate backpressure (0.0–1.0)
// snap.pressure_breakdown — per-source diagnostics: Vec<(String, f32)>
// snap.max_concurrency  — current concurrency limit
// snap.is_paused        — whether the scheduler is globally paused
```

## Tauri event bridging

All events derive `Serialize`, so they bridge directly to your frontend via Tauri IPC:

```rust
let mut events = scheduler.subscribe();  // or handle.events() for one domain
let handle = app_handle.clone();
tokio::spawn(async move {
    while let Ok(event) = events.recv().await {
        handle.emit("taskmill-event", &event).unwrap();
    }
});
```

Return snapshots from Tauri commands for polling-based UIs:

```rust
#[tauri::command]
async fn scheduler_status(
    scheduler: tauri::State<'_, Scheduler>,
) -> Result<SchedulerSnapshot, StoreError> {
    scheduler.snapshot().await
}
```
