//! # Taskmill
//!
//! Adaptive priority work scheduler with IO-aware concurrency and SQLite persistence.
//!
//! Taskmill provides a generic task scheduling system that:
//! - Persists tasks to SQLite so the queue survives restarts
//! - Schedules by priority (0 = highest, 255 = lowest) with named tiers
//! - Deduplicates tasks by key — submitting an already-queued key is a no-op
//! - Tracks expected and actual IO bytes per task for budget-based scheduling
//! - Monitors system CPU and disk throughput to adjust concurrency
//! - Supports composable backpressure from arbitrary external sources
//! - Preempts lower-priority work when high-priority tasks arrive
//! - Retries failed tasks at the same priority level
//! - Records completed/failed task history for queries and IO learning
//! - Emits lifecycle events including progress for UI integration (via broadcast channel)
//! - Supports graceful shutdown with configurable drain timeout
//!
//! # Quick start
//!
//! ```no_run
//! use std::sync::Arc;
//! use taskmill::{
//!     Scheduler, TaskExecutor, TaskContext, TaskError,
//!     TypedTask, Priority,
//! };
//! use serde::{Serialize, Deserialize};
//! use tokio_util::sync::CancellationToken;
//!
//! // 1. Define a task payload.
//! #[derive(Serialize, Deserialize)]
//! struct Thumbnail { path: String, size: u32 }
//!
//! impl TypedTask for Thumbnail {
//!     const TASK_TYPE: &'static str = "thumbnail";
//!     fn expected_read_bytes(&self) -> i64 { 4_096 }
//!     fn expected_write_bytes(&self) -> i64 { 1_024 }
//! }
//!
//! // 2. Implement the executor.
//! struct ThumbnailExecutor;
//!
//! impl TaskExecutor for ThumbnailExecutor {
//!     async fn execute<'a>(
//!         &'a self, ctx: &'a TaskContext,
//!     ) -> Result<(), TaskError> {
//!         let thumb: Thumbnail = ctx.payload()?;
//!         ctx.progress.report(0.5, Some("resizing".into()));
//!         // ... do work, check ctx.token.is_cancelled() ...
//!         ctx.record_read_bytes(4_096);
//!         ctx.record_write_bytes(1_024);
//!         Ok(())
//!     }
//! }
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! // 3. Build and run the scheduler.
//! let scheduler = Scheduler::builder()
//!     .store_path("tasks.db")
//!     .typed_executor::<Thumbnail, _>(Arc::new(ThumbnailExecutor))
//!     .max_concurrency(4)
//!     .with_resource_monitoring()
//!     .build()
//!     .await?;
//!
//! // 4. Submit work.
//! let task = Thumbnail { path: "/photos/a.jpg".into(), size: 256 };
//! scheduler.submit_typed(&task).await?;
//!
//! // 5. Run until cancelled.
//! let token = CancellationToken::new();
//! scheduler.run(token).await;
//! # Ok(())
//! # }
//! ```
//!
//! # Feature flags
//!
//! - **`sysinfo-monitor`** (default): Enables the built-in [`SysinfoSampler`](resource::sysinfo_monitor::SysinfoSampler)
//!   for cross-platform CPU and disk IO monitoring. Disable for mobile targets or
//!   when providing a custom [`ResourceSampler`].

pub mod backpressure;
pub mod priority;
pub mod registry;
pub mod resource;
pub mod scheduler;
pub mod store;
pub mod task;

// Convenience re-exports.
pub use backpressure::{CompositePressure, PressureSource, ThrottlePolicy};
pub use priority::Priority;
pub use registry::{TaskContext, TaskExecutor};
pub use resource::sampler::SamplerConfig;
pub use resource::{ResourceReader, ResourceSampler, ResourceSnapshot};
pub use scheduler::{
    EstimatedProgress, ProgressReporter, Scheduler, SchedulerBuilder, SchedulerConfig,
    SchedulerEvent, SchedulerSnapshot, ShutdownMode,
};
pub use store::{RetentionPolicy, StoreConfig, StoreError, TaskStore};
pub use task::{
    generate_dedup_key, HistoryStatus, ParentResolution, SubmitOutcome, TaskError,
    TaskHistoryRecord, TaskLookup, TaskMetrics, TaskRecord, TaskStatus, TaskSubmission, TypeStats,
    TypedTask,
};

#[cfg(feature = "sysinfo-monitor")]
pub use resource::platform_sampler;
