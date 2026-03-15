# Guide: Tauri Upload Queue

This guide walks through building a file upload queue in a Tauri desktop app using taskmill. By the end, you'll have an app that queues file uploads, shows progress in the frontend, lets users prioritize urgent uploads, and doesn't saturate the network.

## What we're building

A Tauri app where:
- Users drag files to upload. Each file becomes a task in the queue.
- A progress bar shows upload status for each file.
- Users can right-click to prioritize an upload ("upload this next").
- The scheduler limits concurrent uploads and backs off when bandwidth is saturated.
- If the app crashes, pending uploads resume on restart.

## Setup

Add taskmill to your Tauri app's `Cargo.toml`:

```toml
[dependencies]
taskmill = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio-util = "0.7"
```

## Define the upload task

Use `TypedTask` to define the upload payload with its IO budget and priority:

```rust
use serde::{Serialize, Deserialize};
use taskmill::{TypedTask, IoBudget, Priority};

#[derive(Serialize, Deserialize)]
struct UploadTask {
    file_path: String,
    file_size: u64,
    bucket: String,
}

impl TypedTask for UploadTask {
    const TASK_TYPE: &'static str = "upload";

    fn expected_io(&self) -> IoBudget {
        // Read from disk, write to network
        IoBudget::new(self.file_size as i64, 0, 0, self.file_size as i64)
    }

    fn priority(&self) -> Priority {
        Priority::NORMAL
    }

    fn group_key(&self) -> Option<String> {
        // Limit concurrent uploads per bucket
        Some(self.bucket.clone())
    }

    fn key(&self) -> Option<String> {
        // Dedup by file path — uploading the same file twice is a no-op
        Some(self.file_path.clone())
    }

    fn ttl(&self) -> Option<std::time::Duration> {
        // Expire uploads that haven't started within 30 minutes
        Some(std::time::Duration::from_secs(30 * 60))
    }

    fn label(&self) -> Option<String> {
        // Human-readable name for the UI
        let filename = std::path::Path::new(&self.file_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.file_path.clone());
        Some(filename)
    }
}
```

## Implement the executor

The executor does the actual upload work. It reports progress and checks for preemption between chunks.

```rust
use std::sync::Arc;
use taskmill::{TaskExecutor, TaskContext, TaskError};

struct UploadExecutor;

impl TaskExecutor for UploadExecutor {
    async fn execute<'a>(
        &'a self, ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        let task: UploadTask = ctx.payload()?;

        let file = tokio::fs::read(&task.file_path).await
            .map_err(|e| TaskError::permanent(format!("can't read file: {e}")))?;

        ctx.record_read_bytes(file.len() as i64);

        let chunk_size = 5 * 1024 * 1024; // 5 MB chunks
        let chunks: Vec<_> = file.chunks(chunk_size).collect();

        for (i, chunk) in chunks.iter().enumerate() {
            // Check for preemption before each chunk
            if ctx.token().is_cancelled() {
                return Err(TaskError::retryable("preempted"));
            }

            // Upload the chunk (your upload logic here)
            upload_chunk(&task.bucket, chunk).await
                .map_err(|e| TaskError::retryable(format!("upload failed: {e}")))?;

            ctx.record_net_tx_bytes(chunk.len() as i64);
            ctx.progress().report_fraction(i + 1, chunks.len(), Some("uploading".into()));
        }

        Ok(())
    }
}
```

## Wire up the scheduler

Build the scheduler in your Tauri setup with bandwidth limiting and per-bucket concurrency:

```rust
use std::sync::Arc;
use std::time::Duration;
use tauri::Manager;
use tokio_util::sync::CancellationToken;
use taskmill::{Scheduler, ShutdownMode, StoreConfig, RetentionPolicy};

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_dir = app.path().app_data_dir().unwrap();
            let db_path = app_dir.join("tasks.db");

            let scheduler = tauri::async_runtime::block_on(async {
                Scheduler::builder()
                    .store_path(db_path.to_str().unwrap())
                    .executor("upload", Arc::new(UploadExecutor))
                    .max_concurrency(8)
                    .group_concurrency("my-bucket", 3)  // max 3 uploads to this bucket
                    .bandwidth_limit(50_000_000.0)       // 50 MB/s cap
                    .with_resource_monitoring()
                    .shutdown_mode(ShutdownMode::Graceful(Duration::from_secs(30)))
                    .store_config(StoreConfig {
                        retention_policy: Some(RetentionPolicy::MaxCount(10_000)),
                        ..Default::default()
                    })
                    .build()
                    .await
                    .unwrap()
            });

            // Bridge events to the frontend
            setup_events(app, &scheduler);

            // Start the scheduler loop
            let token = CancellationToken::new();
            let sched = scheduler.clone();
            tauri::async_runtime::spawn(async move {
                sched.run(token).await;
            });

            // Store in Tauri state — Scheduler is Clone, no Arc needed
            app.manage(scheduler);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            submit_upload,
            prioritize_upload,
            cancel_upload,
            scheduler_status,
            pause_all,
            resume_all,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## Expose commands to the frontend

```rust
use taskmill::{Scheduler, SchedulerSnapshot, StoreError, Priority, TaskSubmission};

#[tauri::command]
async fn submit_upload(
    scheduler: tauri::State<'_, Scheduler>,
    file_path: String,
    file_size: u64,
    bucket: String,
) -> Result<String, StoreError> {
    let task = UploadTask { file_path, file_size, bucket };
    let outcome = scheduler.submit_typed(&task).await?;
    Ok(format!("{:?}", outcome))
}

#[tauri::command]
async fn prioritize_upload(
    scheduler: tauri::State<'_, Scheduler>,
    file_path: String,
    file_size: u64,
    bucket: String,
) -> Result<String, StoreError> {
    // Re-submit at HIGH priority — dedup will upgrade the existing task
    let sub = TaskSubmission::new("upload")
        .payload_json(&UploadTask { file_path, file_size, bucket })
        .priority(Priority::HIGH);
    let outcome = scheduler.submit(&sub).await?;
    Ok(format!("{:?}", outcome))
}

#[tauri::command]
async fn cancel_upload(
    scheduler: tauri::State<'_, Scheduler>,
    task_id: i64,
) -> Result<bool, StoreError> {
    scheduler.cancel(task_id).await
}

#[tauri::command]
async fn scheduler_status(
    scheduler: tauri::State<'_, Scheduler>,
) -> Result<SchedulerSnapshot, StoreError> {
    scheduler.snapshot().await
}

#[tauri::command]
async fn pause_all(scheduler: tauri::State<'_, Scheduler>) -> Result<(), StoreError> {
    scheduler.pause_all().await;
    Ok(())
}

#[tauri::command]
async fn resume_all(scheduler: tauri::State<'_, Scheduler>) -> Result<(), StoreError> {
    scheduler.resume_all().await;
    Ok(())
}
```

## Bridge events to the frontend

```rust
use taskmill::SchedulerEvent;

fn setup_events(app: &tauri::App, scheduler: &Scheduler) {
    let mut events = scheduler.subscribe();
    let handle = app.handle().clone();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            handle.emit("taskmill-event", &event).unwrap();
        }
    });
}
```

In your frontend (TypeScript):

```typescript
import { listen } from "@tauri-apps/api/event";

listen("taskmill-event", (event) => {
  const data = event.payload;
  // data has the SchedulerEvent structure — Progress, Completed, Failed, etc.
  // Update your UI based on the event type
});
```

## Handling edge cases

### App backgrounded

Pause all work when the app loses focus to conserve battery and bandwidth:

```rust
// In a Tauri window event handler:
scheduler.pause_all().await;

// When the app regains focus:
scheduler.resume_all().await;
```

### Crash recovery

Handled automatically. When the app restarts, `TaskStore::open()` resets any tasks that were mid-upload back to pending. They'll be re-dispatched and the executor will re-upload from the beginning.

If your upload target supports resumable uploads, you can store the upload session ID in the payload and check for it in the executor before starting a new upload.

### Duplicate uploads

Handled automatically. The `key()` method on `UploadTask` returns the file path, so submitting the same file twice returns `SubmitOutcome::Duplicate`. The UI can show a "already queued" message.

### Stale uploads

The `ttl()` method on `UploadTask` expires queued uploads that haven't started within 30 minutes. Listen for `TaskExpired` events to notify the user:

```rust
SchedulerEvent::TaskExpired { header, age } => {
    notify_user(&format!(
        "{} expired after {:.0}s in the queue",
        header.label, age.as_secs_f64(),
    ));
}
```
