//! # Taskmill
//!
//! Adaptive priority work scheduler with IO-aware concurrency and SQLite persistence.
//!
//! Taskmill provides a generic task scheduling system that:
//! - Persists tasks to SQLite so the queue survives restarts
//! - Schedules by priority (0 = highest, 255 = lowest) with [named tiers](Priority)
//! - Deduplicates tasks by key — submitting an already-queued key is a no-op
//! - Tracks expected and actual IO bytes (disk and network) per task for budget-based scheduling
//! - Monitors system CPU, disk, and network throughput to adjust concurrency
//! - Supports [composable backpressure](PressureSource) from arbitrary external sources
//! - Preempts lower-priority work when high-priority tasks arrive
//! - [Retries](TaskError::retryable) failed tasks at the same priority level
//! - Records completed/failed [task history](TaskHistoryRecord) for queries and IO learning
//! - Supports [batch submission](Scheduler::submit_batch) with intra-batch dedup and chunking
//! - Emits [lifecycle events](SchedulerEvent) including progress for UI integration
//! - Reports [byte-level transfer progress](TaskProgress) with EWMA-smoothed throughput and ETA
//! - Supports [graceful shutdown](ShutdownMode) with configurable drain timeout
//!
//! # Concepts
//!
//! ## Task lifecycle
//!
//! A task flows through a linear pipeline:
//!
//! ```text
//! submit → pending → running → completed
//!                  ↘ paused ↗     ↘ failed (retryable → pending)
//!                                  ↘ failed (permanent → history)
//! ```
//!
//! 1. **Submit** — [`Scheduler::submit`] (or [`submit_typed`](Scheduler::submit_typed),
//!    [`submit_batch`](Scheduler::submit_batch),
//!    [`submit_built`](Scheduler::submit_built))
//!    enqueues a [`TaskSubmission`] into the SQLite store.
//! 2. **Pending** — the task waits in a priority queue. The scheduler's run loop
//!    pops the highest-priority pending task on each tick.
//! 3. **Running** — the scheduler calls [`TaskExecutor::execute`] with a
//!    [`TaskContext`] containing the task record, a cancellation token, and a
//!    progress reporter.
//! 4. **Terminal** — on success the task moves to the history table. On failure,
//!    a [`retryable`](TaskError::retryable) error requeues it (up to
//!    [`SchedulerBuilder::max_retries`]); a non-retryable error moves it to
//!    history as failed.
//!
//! ## Deduplication
//!
//! Every task has a dedup key derived from its type name and either an explicit
//! key string or the serialized payload (via SHA-256). Submitting a task whose
//! key already exists returns [`SubmitOutcome::Duplicate`] (or
//! [`Upgraded`](SubmitOutcome::Upgraded) if the new submission has higher
//! priority). This makes it safe to call `submit` idempotently.
//!
//! Within a single [`submit_batch`](Scheduler::submit_batch) call, intra-batch
//! dedup applies a **last-wins** policy: if two tasks share a dedup key, only
//! the last occurrence is submitted and earlier ones receive `Duplicate`.
//!
//! ## Priority & preemption
//!
//! [`Priority`] is a `u8` newtype where **lower values = higher priority**.
//! Named constants ([`REALTIME`](Priority::REALTIME),
//! [`HIGH`](Priority::HIGH), [`NORMAL`](Priority::NORMAL),
//! [`BACKGROUND`](Priority::BACKGROUND), [`IDLE`](Priority::IDLE)) cover
//! common tiers. When a task at or above the
//! [`preempt_priority`](SchedulerBuilder::preempt_priority) threshold is
//! submitted, lower-priority running tasks are cancelled and paused so the
//! urgent work runs immediately.
//!
//! ## IO budgeting
//!
//! Each task declares an [`IoBudget`] covering expected disk and network
//! bytes (via [`TypedTask::expected_io`] or [`TaskSubmission::expected_io`]).
//! The scheduler tracks running IO totals and, when
//! [resource monitoring](SchedulerBuilder::with_resource_monitoring) is enabled,
//! compares them against observed system throughput to avoid over-saturating
//! the disk or network. Executors report actual IO via
//! [`TaskContext::record_read_bytes`] / [`record_write_bytes`](TaskContext::record_write_bytes) /
//! [`record_net_rx_bytes`](TaskContext::record_net_rx_bytes) /
//! [`record_net_tx_bytes`](TaskContext::record_net_tx_bytes),
//! which feeds back into historical throughput averages for future scheduling
//! decisions.
//!
//! To throttle tasks when network bandwidth is saturated, set a bandwidth
//! cap with [`SchedulerBuilder::bandwidth_limit`] — this registers a built-in
//! [`NetworkPressure`] source that maps observed throughput to backpressure.
//!
//! ## Task groups
//!
//! Tasks can be assigned to a named group via [`TaskSubmission::group`] (or
//! [`TypedTask::group_key`]). The scheduler enforces per-group concurrency
//! limits — for example, limiting uploads to any single S3 bucket to 4
//! concurrent tasks. Configure limits at build time with
//! [`SchedulerBuilder::group_concurrency`] and
//! [`SchedulerBuilder::default_group_concurrency`], or adjust at runtime via
//! [`Scheduler::set_group_limit`] and [`Scheduler::set_default_group_concurrency`].
//!
//! ## Child tasks & two-phase execution
//!
//! An executor can spawn child tasks via [`TaskContext::spawn_child`]. When
//! children exist, the parent enters a **waiting** state after its executor
//! returns. Once all children complete, the parent's
//! [`TaskExecutor::finalize`] method is called — useful for assembly work
//! like `CompleteMultipartUpload`. If any child fails and
//! [`fail_fast`](TaskSubmission::fail_fast) is `true` (the default), siblings
//! are cancelled and the parent fails immediately.
//!
//! ## Byte-level progress
//!
//! For long-running transfers (file copies, uploads, downloads), executors can
//! report byte-level progress via [`TaskContext::set_bytes_total`] and
//! [`TaskContext::add_bytes`]. The scheduler maintains per-task atomic counters
//! on the [`IoTracker`](registry::IoTracker) — updates are lock-free and
//! impose no overhead on the executor hot path.
//!
//! When [`SchedulerBuilder::progress_interval`] is set, a background ticker
//! task polls these counters and emits [`TaskProgress`] events on a dedicated
//! broadcast channel (via [`Scheduler::subscribe_progress`]). Each event
//! includes EWMA-smoothed throughput and an estimated time remaining (ETA).
//! The ticker is opt-in — when not configured, there is zero runtime cost.
//!
//! For a one-shot query without the ticker, [`Scheduler::byte_progress`]
//! returns instantaneous snapshots (throughput = 0, no ETA).
//!
//! # Quick start
//!
//! ```no_run
//! use std::sync::Arc;
//! use taskmill::{
//!     Scheduler, TaskExecutor, TaskContext, TaskError,
//!     TypedTask, IoBudget, Priority,
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
//!     fn expected_io(&self) -> IoBudget { IoBudget::disk(4_096, 1_024) }
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
//!         ctx.progress().report(0.5, Some("resizing".into()));
//!         // ... do work, check ctx.token().is_cancelled() ...
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
//! # Common patterns
//!
//! ## Shared application state
//!
//! Register shared services (database pools, HTTP clients, etc.) at build time
//! and retrieve them from any executor via [`TaskContext::state`]:
//!
//! ```ignore
//! struct AppServices { db: DatabasePool, http: reqwest::Client }
//!
//! let scheduler = Scheduler::builder()
//!     .store_path("tasks.db")
//!     .app_state(AppServices { /* ... */ })
//!     .executor("ingest", Arc::new(IngestExecutor))
//!     .build()
//!     .await?;
//!
//! // Inside the executor:
//! async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
//!     let svc = ctx.state::<AppServices>().expect("AppServices not registered");
//!     svc.db.query("...").await?;
//!     Ok(())
//! }
//! ```
//!
//! State can also be injected after construction via
//! [`Scheduler::register_state`] — useful when a library (e.g. shoebox)
//! receives a pre-built scheduler from a parent application.
//!
//! ## Backpressure
//!
//! Implement [`PressureSource`] to feed external signals into the scheduler's
//! throttle decisions. The default [`ThrottlePolicy`] pauses `BACKGROUND`
//! tasks above 50% pressure and `NORMAL` tasks above 75%:
//!
//! ```ignore
//! use std::sync::atomic::{AtomicU32, Ordering};
//! use taskmill::{PressureSource, Scheduler};
//!
//! struct ApiLoad { active: AtomicU32, max: u32 }
//!
//! impl PressureSource for ApiLoad {
//!     fn pressure(&self) -> f32 {
//!         self.active.load(Ordering::Relaxed) as f32 / self.max as f32
//!     }
//!     fn name(&self) -> &str { "api-load" }
//! }
//!
//! let scheduler = Scheduler::builder()
//!     .store_path("tasks.db")
//!     .pressure_source(Box::new(ApiLoad { active: AtomicU32::new(0), max: 100 }))
//!     // .throttle_policy(custom_policy)  // optional override
//!     .build()
//!     .await?;
//! ```
//!
//! ## Events & progress
//!
//! Subscribe to [`SchedulerEvent`]s to drive a UI or collect metrics:
//!
//! ```ignore
//! let mut rx = scheduler.subscribe();
//! tokio::spawn(async move {
//!     while let Ok(event) = rx.recv().await {
//!         match event {
//!             SchedulerEvent::Progress { header, percent, message } => {
//!                 update_progress_bar(header.task_id, percent, message);
//!             }
//!             SchedulerEvent::Completed(header) => {
//!                 mark_done(header.task_id);
//!             }
//!             _ => {}
//!         }
//!     }
//! });
//! ```
//!
//! For byte-level transfer progress with smoothed throughput and ETA,
//! subscribe to the dedicated progress channel:
//!
//! ```ignore
//! let mut progress_rx = scheduler.subscribe_progress();
//! tokio::spawn(async move {
//!     while let Ok(tp) = progress_rx.recv().await {
//!         println!(
//!             "{}: {}/{} bytes ({:.0} B/s, ETA {:?})",
//!             tp.key, tp.bytes_completed, tp.bytes_total.unwrap_or(0),
//!             tp.throughput_bps, tp.eta,
//!         );
//!     }
//! });
//! ```
//!
//! For a single-call dashboard snapshot, use [`Scheduler::snapshot`] which
//! returns a serializable [`SchedulerSnapshot`] with queue depths, running
//! tasks, progress estimates, byte-level progress, and backpressure.
//!
//! ## Group concurrency
//!
//! Limit concurrent tasks within a named group — for example, cap uploads
//! per S3 bucket:
//!
//! ```ignore
//! let scheduler = Scheduler::builder()
//!     .store_path("tasks.db")
//!     .executor("upload-part", Arc::new(UploadPartExecutor))
//!     .default_group_concurrency(4)               // default for all groups
//!     .group_concurrency("s3://hot-bucket", 8)    // override for one group
//!     .build()
//!     .await?;
//!
//! // Tasks declare their group via the submission:
//! let sub = TaskSubmission::new("upload-part")
//!     .group("s3://my-bucket")
//!     .payload_json(&part);
//! scheduler.submit(&sub).await?;
//!
//! // Adjust at runtime:
//! scheduler.set_group_limit("s3://my-bucket", 2);
//! scheduler.remove_group_limit("s3://my-bucket"); // fall back to default
//! ```
//!
//! ## Batch submission
//!
//! Submit many tasks at once with [`Scheduler::submit_batch`] for better
//! throughput (single SQLite transaction instead of N). Use
//! [`BatchSubmission`] to set batch-wide defaults and [`BatchOutcome`] to
//! inspect results:
//!
//! ```ignore
//! use taskmill::{BatchSubmission, TaskSubmission, Priority};
//!
//! let batch = BatchSubmission::new()
//!     .default_group("s3://my-bucket")
//!     .default_priority(Priority::HIGH)
//!     .task(TaskSubmission::new("upload").key("file-1").payload_json(&p1))
//!     .task(TaskSubmission::new("upload").key("file-2").payload_json(&p2));
//!
//! let outcome = scheduler.submit_built(batch).await?;
//! println!("inserted: {:?}, dupes: {}", outcome.inserted(), outcome.duplicated_count());
//! ```
//!
//! Batches with duplicate dedup keys use a **last-wins** policy — only the
//! final occurrence is submitted. Batches larger than 10,000 tasks are
//! automatically chunked into sub-transactions to avoid holding the SQLite
//! write lock for too long.
//!
//! A [`SchedulerEvent::BatchSubmitted`] event is emitted for observability
//! whenever at least one task in the batch was inserted.
//!
//! ## Child tasks
//!
//! Spawn child tasks from an executor to model fan-out work. The parent
//! automatically waits for all children before its [`finalize`](TaskExecutor::finalize)
//! method is called:
//!
//! ```ignore
//! impl TaskExecutor for MultipartUploadExecutor {
//!     async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
//!         let upload: MultipartUpload = ctx.payload()?;
//!         for part in &upload.parts {
//!             ctx.spawn_child(
//!                 TaskSubmission::new("upload-part")
//!                     .key(&part.etag)
//!                     .priority(ctx.record().priority)
//!                     .payload_json(part)
//!                     .expected_io(IoBudget::disk(part.size as i64, 0)),
//!             ).await?;
//!         }
//!         Ok(())
//!     }
//!
//!     async fn finalize<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
//!         // All parts uploaded — complete the multipart upload.
//!         let upload: MultipartUpload = ctx.payload()?;
//!         complete_multipart(&upload).await?;
//!         Ok(())
//!     }
//! }
//! ```
//!
//! # How the dispatch loop works
//!
//! Understanding the run loop helps when tuning [`SchedulerConfig`]:
//!
//! 1. The loop wakes on three conditions: a new task was submitted (via
//!    [`Notify`](tokio::sync::Notify)), the
//!    [`poll_interval`](SchedulerBuilder::poll_interval) elapsed (default
//!    500ms), or the cancellation token fired.
//! 2. Paused tasks are resumed if no active preemptors exist at their
//!    priority level.
//! 3. Pending finalizers (parents whose children all completed) are
//!    dispatched first.
//! 4. The highest-priority pending task is peeked (without claiming it).
//! 5. The dispatch gate checks concurrency limits, group concurrency,
//!    IO budget, and backpressure. If the gate rejects, no slot is consumed.
//! 6. If admitted, the task is atomically claimed (`peek` → `pop_by_id`)
//!    and spawned as a Tokio task.
//! 7. Steps 4–6 repeat until the queue is empty or the gate rejects.
//!
//! # Feature flags
//!
//! - **`sysinfo-monitor`** (default): Enables the built-in [`SysinfoSampler`](resource::sysinfo_monitor::SysinfoSampler)
//!   for cross-platform CPU and disk IO monitoring. Disable for mobile targets or
//!   when providing a custom [`ResourceSampler`]. Without this feature, calling
//!   [`SchedulerBuilder::with_resource_monitoring`] requires a custom sampler
//!   via [`resource_sampler()`](SchedulerBuilder::resource_sampler).

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
pub use resource::network_pressure::NetworkPressure;
pub use resource::sampler::SamplerConfig;
pub use resource::{ResourceReader, ResourceSampler, ResourceSnapshot};
pub use scheduler::{
    EstimatedProgress, GroupLimits, ProgressReporter, Scheduler, SchedulerBuilder, SchedulerConfig,
    SchedulerEvent, SchedulerSnapshot, ShutdownMode, TaskEventHeader, TaskProgress,
};
pub use store::{RetentionPolicy, StoreConfig, StoreError, TaskStore};
pub use task::{
    generate_dedup_key, BatchOutcome, BatchSubmission, HistoryStatus, IoBudget, ParentResolution,
    SubmitOutcome, TaskError, TaskHistoryRecord, TaskLookup, TaskRecord, TaskStatus,
    TaskSubmission, TypeStats, TypedTask,
};

#[cfg(feature = "sysinfo-monitor")]
pub use resource::platform_sampler;
