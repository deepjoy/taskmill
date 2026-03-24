# Guide: Background Processing Service

This guide walks through using taskmill in a background service (daemon, CLI tool, or server process) that processes files. Unlike the [Tauri guide](tauri-upload-queue.md), there's no UI — the focus is on signal handling, container-friendly resource monitoring, and server-oriented tuning.

## What we're building

A service that:
- Watches a directory for new images
- Queues processing tasks (thumbnail generation, EXIF extraction)
- Prioritizes by file type (RAW files get `BACKGROUND`, JPEGs get `NORMAL`)
- Monitors disk IO to avoid saturating the system
- Shuts down gracefully on SIGTERM

## Define the task

```rust
use serde::{Serialize, Deserialize};
use taskmill::{TypedTask, TaskTypeConfig, IoBudget, Priority, DomainKey};

// Define the domain identity
pub struct Images;
impl DomainKey for Images { const NAME: &'static str = "images"; }

#[derive(Serialize, Deserialize)]
struct ProcessImageTask {
    path: String,
    file_size: u64,
    is_raw: bool,
}

impl TypedTask for ProcessImageTask {
    type Domain = Images;
    const TASK_TYPE: &'static str = "process-image";

    fn config() -> TaskTypeConfig {
        TaskTypeConfig::new()
            .priority(Priority::NORMAL)
            .expected_io(IoBudget::disk(50_000, 10_000))
    }

    fn key(&self) -> Option<String> {
        Some(self.path.clone())
    }
}
```

## Implement the executor

```rust
use taskmill::{TypedExecutor, DomainTaskContext, TaskError};

struct ImageProcessor;

impl TypedExecutor<ProcessImageTask> for ImageProcessor {
    async fn execute(&self, task: ProcessImageTask, ctx: DomainTaskContext<'_, Images>) -> Result<(), TaskError> {
        // Read the source image
        let data = tokio::fs::read(&task.path).await
            .map_err(|e| TaskError::permanent(format!("can't read: {e}")))?;
        ctx.record_read_bytes(data.len() as i64);

        if ctx.token().is_cancelled() {
            return Err(TaskError::retryable("preempted"));
        }

        // Generate thumbnail
        let thumb = generate_thumbnail(&data)
            .map_err(|e| TaskError::retryable(format!("processing failed: {e}")))?;

        let thumb_path = thumbnail_path(&task.path);
        tokio::fs::write(&thumb_path, &thumb).await
            .map_err(|e| TaskError::retryable(format!("can't write: {e}")))?;
        ctx.record_write_bytes(thumb.len() as i64);

        ctx.progress().report(1.0, Some("done".into()));
        Ok(())
    }
}
```

## Define the domain

```rust
use taskmill::Domain;

pub fn images_domain() -> Domain<Images> {
    Domain::<Images>::new()
        .task::<ProcessImageTask>(ImageProcessor)
        .max_concurrency(4)
}
```

## Set up the service

```rust
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use taskmill::{Scheduler, ShutdownMode, StoreConfig, RetentionPolicy, SamplerConfig};

#[tokio::main]
async fn main() {
    tracing_subscriber::init();

    let scheduler = Scheduler::builder()
        .store_path("/var/lib/myservice/tasks.db")
        .domain(images_domain())
        .max_concurrency(4)
        .max_retries(5)
        .with_resource_monitoring()
        .sampler_config(SamplerConfig {
            ewma_alpha: 0.2,  // smooth — don't overreact to spikes
            interval: Duration::from_secs(2),
        })
        .shutdown_mode(ShutdownMode::Graceful(Duration::from_secs(30)))
        .store_config(StoreConfig {
            retention_policy: Some(RetentionPolicy::MaxAgeDays(30)),
            ..Default::default()
        })
        .build()
        .await
        .unwrap();

    // Log events
    let mut events = scheduler.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            tracing::info!(?event, "scheduler event");
        }
    });

    // Watch for new files and submit tasks
    let images = scheduler.domain::<Images>();
    tokio::spawn(async move {
        watch_directory("/data/incoming", images).await;
    });

    // Shut down gracefully on SIGTERM
    let token = CancellationToken::new();
    let shutdown_token = token.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        tracing::info!("shutting down...");
        shutdown_token.cancel();
    });

    scheduler.run(token).await;
    tracing::info!("shutdown complete");
}
```

The watcher submits tasks through the `DomainHandle`:

```rust
async fn watch_directory(path: &str, handle: DomainHandle<Images>) {
    // ... watch for new files ...
    let task = ProcessImageTask { path: file_path, file_size, is_raw };
    handle.submit(task).await.unwrap();
}
```

## Custom resource sampler for containers

If your service runs in a container, the built-in `sysinfo` sampler may not reflect cgroup limits. Provide a custom sampler:

```rust
use taskmill::{ResourceSampler, ResourceSnapshot};

struct CgroupSampler;

impl ResourceSampler for CgroupSampler {
    fn sample(&mut self) -> ResourceSnapshot {
        ResourceSnapshot {
            cpu_usage: read_cgroup_cpu(),
            io_read_bytes_per_sec: read_blkio_read(),
            io_write_bytes_per_sec: read_blkio_write(),
            net_rx_bytes_per_sec: 0.0,
            net_tx_bytes_per_sec: 0.0,
        }
    }
}

// In the builder:
Scheduler::builder()
    .resource_sampler(Box::new(CgroupSampler))
    // ...
```

Disable the default sampler in `Cargo.toml`:

```toml
taskmill = { version = "0.6", default-features = false }
```

## Key differences from desktop

| Concern | Desktop (Tauri) | Background service |
|---------|----------------|-------------------|
| Event bridging | `app_handle.emit()` to frontend | `tracing::info!()` to logs |
| Shutdown | App close event | SIGTERM / SIGINT |
| Resource monitoring | Built-in `sysinfo` | Custom sampler for cgroups |
| Concurrency | Low (4–8) to keep UI responsive | Higher (8–16) for throughput |
| Smoothing | Default (0.3) — responsive | Lower (0.2) — stable |
| Retention | `MaxCount` — keep N recent | `MaxAgeDays` — time-based cleanup |
