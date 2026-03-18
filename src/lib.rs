//! # Taskmill
//!
//! Adaptive priority work scheduler with IO-aware concurrency and SQLite persistence.
//!
//! Taskmill provides a generic task scheduling system that:
//! - Persists tasks to SQLite so the queue survives restarts
//! - Schedules by priority (0 = highest, 255 = lowest) with [named tiers](Priority)
//! - Deduplicates tasks by key with configurable [`DuplicateStrategy`] (skip, supersede, or reject)
//! - Tracks expected and actual IO bytes (disk and network) per task for budget-based scheduling
//! - Monitors system CPU, disk, and network throughput to adjust concurrency
//! - Supports [composable backpressure](PressureSource) from arbitrary external sources
//! - Preempts lower-priority work when high-priority tasks arrive
//! - [Retries](TaskError::retryable) failed tasks with configurable [backoff](BackoffStrategy)
//!   ([`Constant`](BackoffStrategy::Constant), [`Linear`](BackoffStrategy::Linear),
//!   [`Exponential`](BackoffStrategy::Exponential), [`ExponentialJitter`](BackoffStrategy::ExponentialJitter))
//!   and per-type [retry policies](RetryPolicy)
//! - Records completed/failed [task history](TaskHistoryRecord) for queries and IO learning
//! - Supports [batch submission](Scheduler::submit_batch) with intra-batch dedup and chunking
//! - Emits [lifecycle events](SchedulerEvent) including progress for UI integration
//! - Reports [byte-level transfer progress](TaskProgress) with EWMA-smoothed throughput and ETA
//! - Supports [task cancellation](Scheduler::cancel) with history recording and cleanup hooks
//! - Supports [task superseding](DuplicateStrategy::Supersede) for atomic cancel-and-replace
//! - Supports [task TTL](TtlFrom) with automatic expiry, per-type defaults, and child inheritance
//! - Supports [graceful shutdown](ShutdownMode) with configurable drain timeout
//!
//! # Concepts
//!
//! ## Task lifecycle
//!
//! A task flows through a linear pipeline:
//!
//! ```text
//! submit → blocked ─(deps met)─→ pending ──────────────→ running → completed
//!                                   ↑    ↓     ↘ paused ↗     ↘ failed (retryable → pending, with backoff delay)
//!                         (run_after elapsed)                  ↘ failed (permanent → history)
//!                                   │                          ↘ dead_letter (retries exhausted → history)
//!                           pending (gated)                    ↘ cancelled (via cancel() or supersede)
//!                               cancelled                      ↘ expired (TTL, cascade to children)
//!                               superseded
//!                               expired (TTL)
//!    blocked ─(dep failed)─→ dep_failed (history)
//! ```
//!
//! 1. **Submit** — [`ModuleHandle::submit`] (or [`submit_typed`](ModuleHandle::submit_typed),
//!    [`submit_batch`](ModuleHandle::submit_batch))
//!    enqueues a [`TaskSubmission`] into the SQLite store. The module handle
//!    auto-prefixes the task type with the module name and applies module defaults.
//! 2. **Pending** — the task waits in a priority queue. The scheduler's run loop
//!    pops the highest-priority pending task on each tick.
//! 3. **Running** — the scheduler calls [`TaskExecutor::execute`] with a
//!    [`TaskContext`] containing the task record, a cancellation token, and a
//!    progress reporter.
//! 4. **Terminal** — on success the task moves to the history table. On failure,
//!    a [`retryable`](TaskError::retryable) error requeues it (up to
//!    [`SchedulerBuilder::max_retries`] or per-type [`RetryPolicy::max_retries`])
//!    with a configurable [`BackoffStrategy`] delay; a non-retryable
//!    ([`permanent`](TaskError::permanent)) error moves it to history as failed.
//!    Tasks that exhaust all retries enter [`dead_letter`](HistoryStatus::DeadLetter)
//!    state — queryable via [`ModuleHandle::dead_letter_tasks`] and manually
//!    re-submittable via [`ModuleHandle::retry_dead_letter`].
//!
//! ## Deduplication & duplicate strategies
//!
//! Every task has a dedup key derived from its type name and either an explicit
//! key string or the serialized payload (via SHA-256). What happens when a
//! submission's key matches an existing task depends on the
//! [`DuplicateStrategy`]:
//!
//! - **`Skip`** (default) — attempt a priority upgrade or requeue, otherwise
//!   return [`SubmitOutcome::Duplicate`]. Safe for idempotent `submit` calls.
//! - **`Supersede`** — cancel the existing task (recording it in history as
//!   [`HistoryStatus::Superseded`]) and replace it with the new submission.
//!   For running tasks the cancellation token is fired, the
//!   [`on_cancel`](TaskExecutor::on_cancel) hook runs, and children are
//!   cascade-cancelled. Returns [`SubmitOutcome::Superseded`].
//! - **`Reject`** — return [`SubmitOutcome::Rejected`] without modifying the
//!   existing task.
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
//! ## Task TTL & automatic expiry
//!
//! Tasks can be given a time-to-live (TTL) so they expire automatically if they
//! haven't started running within the allowed window. TTL is resolved with a
//! priority chain: **per-task** > **per-type** > **global default** > none.
//!
//! The TTL clock can start at submission time ([`TtlFrom::Submission`], the
//! default) or when the task is first dispatched ([`TtlFrom::FirstAttempt`]).
//! Expired tasks are moved to history with [`HistoryStatus::Expired`] and a
//! [`SchedulerEvent::TaskExpired`] event is emitted.
//!
//! The scheduler catches expired tasks in two places:
//! - **Dispatch time** — when a task is about to be dispatched, its `expires_at`
//!   is checked first.
//! - **Periodic sweep** — a background sweep (default every 30s, configurable
//!   via [`SchedulerBuilder::expiry_sweep_interval`]) batch-expires pending and
//!   paused tasks whose deadline has passed.
//!
//! When a parent task expires, its pending and paused children are
//! cascade-expired.
//!
//! Child tasks without an explicit TTL inherit the **remaining** parent TTL
//! (with `TtlFrom::Submission`), so a child can never outlive its parent's
//! deadline.
//!
//! ## Task metadata tags
//!
//! Tasks can carry schema-free key-value metadata tags for filtering, grouping,
//! and display — without deserializing the task payload. Tags are immutable
//! after submission and are persisted, indexed, and queryable.
//!
//! Set tags per-task via [`TaskSubmission::tag`], per-type via
//! [`TypedTask::tags`], or as batch defaults via
//! [`BatchSubmission::default_tag`]. Tag keys and values are validated at submit
//! time against [`MAX_TAG_KEY_LEN`], [`MAX_TAG_VALUE_LEN`], and
//! [`MAX_TAGS_PER_TASK`].
//!
//! Child tasks inherit parent tags by default (child tags take precedence).
//! Tags are copied to history on all terminal transitions and are included in
//! [`TaskEventHeader`] for event subscribers.
//!
//! Query by tags via the module handle with [`ModuleHandle::tasks_by_tags`]
//! (AND semantics), [`ModuleHandle::count_by_tag`] (grouped counts), or
//! [`ModuleHandle::tag_values`] (distinct values).
//!
//! ## Delayed & scheduled tasks
//!
//! A task can declare **when** it becomes eligible for dispatch:
//!
//! - **Immediate** (default) — dispatched as soon as a slot is free.
//! - **Delayed** (one-shot) — [`TaskSubmission::run_after`] or
//!   [`TaskSubmission::run_at`] sets a `run_after` timestamp. The task enters
//!   `pending` immediately but is invisible to the dispatch loop until its
//!   timestamp passes.
//! - **Recurring** — [`TaskSubmission::recurring`] or
//!   [`TaskSubmission::recurring_schedule`] configures automatic re-enqueueing.
//!   After each execution, the scheduler creates a new pending instance with
//!   `run_after` set to `now + interval`.
//!
//! Delayed tasks interact naturally with other features:
//! - **Priority**: `run_after` tasks respect normal priority ordering among
//!   eligible tasks.
//! - **TTL**: A delayed task's TTL clock ticks from submission (or first
//!   attempt). If the TTL expires before `run_after`, the task expires.
//! - **Dedup**: Recurring tasks reuse the same dedup key. Pile-up prevention
//!   skips creating a new instance if the previous one is still pending.
//! - **Parent/Child**: Recurring tasks cannot be children (enforced at submit).
//!
//! Recurring schedules can be paused, resumed, or cancelled via
//! [`ModuleHandle::pause_recurring`], [`ModuleHandle::resume_recurring`], and
//! [`ModuleHandle::cancel_recurring`]. Pausing stops new instances from being
//! created without affecting any currently running instance.
//!
//! The scheduler optimizes idle wakeups: when the next scheduled task is far
//! in the future, the run loop sleeps until `min(poll_interval, next_run_after)`
//! instead of waking every poll interval.
//!
//! ## Task dependencies
//!
//! Tasks can declare **dependencies on other tasks** so they only become
//! eligible for dispatch after their prerequisites complete. Dependencies
//! are peer-to-peer relationships — distinct from the parent-child hierarchy
//! used for fan-out/finalize patterns. Parent-child means "I spawned you and
//! I finalize after you." A dependency means "I cannot start until you finish."
//! The two compose orthogonally: a child task can depend on an unrelated peer,
//! and a parent can depend on another peer.
//!
//! ### Blocked status
//!
//! A task with unresolved dependencies enters the [`TaskStatus::Blocked`]
//! state. Blocked tasks are invisible to the dispatch loop — the existing
//! `WHERE status = 'pending'` filter excludes them automatically. Resolution
//! is event-driven: when a dependency completes, the scheduler checks whether
//! the dependent's remaining edges have all been satisfied. If so, the task
//! transitions to `pending` and becomes eligible for dispatch. If a dependency
//! was already completed at submission time, its edge is skipped entirely; if
//! all dependencies are already complete, the task starts as `pending`
//! immediately.
//!
//! ### Failure policy
//!
//! [`DependencyFailurePolicy`] controls what happens when a dependency fails
//! permanently (after exhausting retries):
//!
//! - **`Cancel`** (default) — the dependent is moved to history with
//!   [`HistoryStatus::DependencyFailed`] and its own dependents are
//!   cascade-failed.
//! - **`Fail`** — same terminal status, but does not cascade to other
//!   dependents in the same chain (useful for manual intervention).
//! - **`Ignore`** — the failed edge is removed and, if no other edges
//!   remain, the dependent is unblocked. Use with caution — the dependent
//!   must tolerate missing upstream results.
//!
//! ### Circular dependency detection
//!
//! At submission time the scheduler walks the dependency graph upward from
//! each declared dependency using iterative BFS (bounded stack depth). If
//! the new task's ID is encountered during the walk, submission fails with
//! a [`StoreError::CyclicDependency`] error. This catches both direct
//! cycles (A depends on B, B depends on A) and transitive cycles
//! (A → B → C → A).
//!
//! ### Interaction with other features
//!
//! - **Dedup**: a blocked task still occupies its dedup key. Duplicate
//!   submissions follow normal [`DuplicateStrategy`] rules.
//! - **TTL**: a blocked task's TTL clock ticks normally. If the TTL expires
//!   while blocked, the task moves to history as [`HistoryStatus::Expired`]
//!   and its edges are cleaned up.
//! - **Recurring**: recurring tasks can declare dependencies. Each generated
//!   instance starts as `blocked` independently — useful for "run B every
//!   hour, but only after A's latest run completes."
//! - **Delayed**: `run_after` and dependencies compose. A task with both
//!   starts as `blocked`, transitions to `pending` when deps are met, but
//!   is still gated by `run_after` in the dispatch query.
//! - **Groups**: blocked tasks are not dispatched, so they do not count
//!   against group concurrency limits.
//!
//! ## Cancellation
//!
//! Tasks can be cancelled via the [`ModuleHandle`] — individually with
//! [`ModuleHandle::cancel`], all at once with [`ModuleHandle::cancel_all`],
//! or by predicate with [`ModuleHandle::cancel_where`]. Cancelled tasks are
//! recorded in the history table as [`HistoryStatus::Cancelled`] rather than
//! silently deleted.
//!
//! For running tasks, cancellation fires the
//! [`on_cancel`](TaskExecutor::on_cancel) hook (with a configurable
//! [`cancel_hook_timeout`](SchedulerBuilder::cancel_hook_timeout)) so
//! executors can clean up external resources — for example, aborting an S3
//! multipart upload. Executors can check for cancellation cooperatively via
//! [`TaskContext::check_cancelled`].
//!
//! Cancelling a parent task cascade-cancels all its children.
//!
//! ## Byte-level progress
//!
//! For long-running transfers (file copies, uploads, downloads), executors can
//! report byte-level progress via [`TaskContext::set_bytes_total`] and
//! [`TaskContext::add_bytes`]. The scheduler maintains per-task atomic counters
//! on the `IoTracker` — updates are lock-free and
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
//!     Module, Scheduler, TaskExecutor, TaskContext, TaskError,
//!     TypedTask, IoBudget, Priority, TaskSubmission,
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
//!     .module(Module::new("media").typed_executor::<Thumbnail, _>(Arc::new(ThumbnailExecutor)))
//!     .max_concurrency(4)
//!     .with_resource_monitoring()
//!     .build()
//!     .await?;
//!
//! // 4. Submit work (task type is prefixed with the module name).
//! let task = Thumbnail { path: "/photos/a.jpg".into(), size: 256 };
//! scheduler.submit(&TaskSubmission::new("media::thumbnail").payload_json(&task)).await?;
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
//! and retrieve them from any executor via [`TaskContext::state`]. State can be
//! module-scoped (checked first) or global (fallback):
//!
//! ```ignore
//! struct AppServices { db: DatabasePool, http: reqwest::Client }
//! struct IngestConfig { bucket: String }
//!
//! let scheduler = Scheduler::builder()
//!     .store_path("tasks.db")
//!     .app_state(AppServices { /* ... */ })            // global — all modules
//!     .module(Module::new("ingest")
//!         .executor("ingest", Arc::new(IngestExecutor))
//!         .app_state(IngestConfig { bucket: "...".into() }))  // module-scoped
//!     .build()
//!     .await?;
//!
//! // Inside an ingest executor — module state checked first, then global:
//! async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
//!     let cfg = ctx.state::<IngestConfig>().expect("IngestConfig not registered");
//!     let svc = ctx.state::<AppServices>().expect("AppServices not registered");
//!     svc.db.query("...").await?;
//!     Ok(())
//! }
//! ```
//!
//! State can also be injected globally after construction via
//! [`Scheduler::register_state`] — useful when a library
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
//!     .module(Module::new("uploads")
//!         .executor("upload-part", Arc::new(UploadPartExecutor)))
//!     .default_group_concurrency(4)               // default for all groups
//!     .group_concurrency("s3://hot-bucket", 8)    // override for one group
//!     .build()
//!     .await?;
//!
//! // Tasks declare their group via the submission (or TypedTask::group_key):
//! let sub = TaskSubmission::new("upload-part")
//!     .group("s3://my-bucket")
//!     .payload_json(&part);
//! scheduler.module("uploads").submit(sub).await?;
//!
//! // Adjust at runtime:
//! scheduler.set_group_limit("s3://my-bucket", 2);
//! scheduler.remove_group_limit("s3://my-bucket"); // fall back to default
//! ```
//!
//! ## Batch submission
//!
//! Submit many tasks at once for better throughput (single SQLite transaction
//! instead of N). Use [`BatchSubmission`] to set batch-wide defaults and
//! [`BatchOutcome`] to inspect results. Submit via the module handle:
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
//! let outcome = scheduler.module("uploads").submit_batch(batch).await?;
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
//! method is called. `spawn_child` is module-aware: the task type is
//! auto-prefixed with the owning module's namespace.
//!
//! ```ignore
//! impl TaskExecutor for MultipartUploadExecutor {
//!     async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
//!         let upload: MultipartUpload = ctx.payload()?;
//!         for part in &upload.parts {
//!             // "upload-part" is prefixed with the owning module name automatically.
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
//! For cross-module children, use [`SubmitBuilder::parent`] via `ctx.module()`:
//!
//! ```ignore
//! ctx.module("storage")
//!     .submit_typed(&Upload { ... })
//!     .parent(ctx.record().id)
//!     .await?;
//! ```
//!
//! ## Cancellation & cleanup hooks
//!
//! Cancel tasks individually or in bulk. Implement
//! [`on_cancel`](TaskExecutor::on_cancel) to clean up external resources:
//!
//! ```ignore
//! impl TaskExecutor for UploadExecutor {
//!     async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
//!         // Cooperatively check for cancellation in long loops.
//!         for chunk in chunks {
//!             ctx.check_cancelled()?;
//!             upload_chunk(chunk).await?;
//!         }
//!         Ok(())
//!     }
//!
//!     async fn on_cancel<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
//!         // Abort the in-progress multipart upload.
//!         let upload: Upload = ctx.payload()?;
//!         abort_multipart(&upload.upload_id).await?;
//!         Ok(())
//!     }
//! }
//!
//! // Cancel by ID through the module handle (scoped to that module):
//! scheduler.module("uploads").cancel(task_id).await?;
//!
//! // Bulk cancel all tasks in a module:
//! scheduler.module("uploads").cancel_all().await?;
//!
//! // Cancel by predicate:
//! scheduler.module("uploads")
//!     .cancel_where(|t| t.priority == Priority::BACKGROUND).await?;
//! ```
//!
//! ## Task TTL
//!
//! Set a TTL on individual submissions, on a task type, or as a global default:
//!
//! ```ignore
//! use std::time::Duration;
//! use taskmill::{Scheduler, TaskSubmission, TtlFrom};
//!
//! // Per-task TTL — expire if not started within 5 minutes.
//! let sub = TaskSubmission::new("sync")
//!     .payload_json(&data)
//!     .ttl(Duration::from_secs(300))
//!     .ttl_from(TtlFrom::Submission);
//! scheduler.submit(&sub).await?;
//!
//! // Per-type default — every "thumbnail" task gets a 10-minute TTL
//! // unless overridden per-task.
//! let scheduler = Scheduler::builder()
//!     .store_path("tasks.db")
//!     .module(Module::new("media")
//!         .executor_with_ttl("thumbnail", Arc::new(ThumbExec), Duration::from_secs(600)))
//!     .build()
//!     .await?;
//!
//! // Global default — catch-all for any task without a per-task or per-type TTL.
//! let scheduler = Scheduler::builder()
//!     .store_path("tasks.db")
//!     .default_ttl(Duration::from_secs(3600)) // 1 hour
//!     .build()
//!     .await?;
//! ```
//!
//! For typed tasks, implement [`TypedTask::ttl`] and [`TypedTask::ttl_from`]:
//!
//! ```ignore
//! impl TypedTask for SyncTask {
//!     const TASK_TYPE: &'static str = "sync";
//!     fn ttl(&self) -> Option<Duration> { Some(Duration::from_secs(300)) }
//!     fn ttl_from(&self) -> TtlFrom { TtlFrom::FirstAttempt }
//! }
//! ```
//!
//! ## Scheduled tasks
//!
//! Delay a task or create recurring schedules:
//!
//! ```ignore
//! use std::time::Duration;
//! use taskmill::{TaskSubmission, RecurringSchedule};
//!
//! let handle = scheduler.module("app");
//!
//! // One-shot delay — dispatch after 30 seconds.
//! handle.submit(TaskSubmission::new("cleanup")
//!     .key("stale-uploads")
//!     .run_after(Duration::from_secs(30))).await?;
//!
//! // Recurring — abort stale uploads every 6 hours.
//! handle.submit(TaskSubmission::new("cleanup")
//!     .key("stale-uploads")
//!     .recurring(Duration::from_secs(6 * 3600))).await?;
//!
//! // Full schedule control — initial delay, max executions.
//! handle.submit(TaskSubmission::new("sync")
//!     .key("daily-sync")
//!     .recurring_schedule(RecurringSchedule {
//!         interval: Duration::from_secs(86400),
//!         initial_delay: Some(Duration::from_secs(60)),
//!         max_executions: Some(30),
//!     })).await?;
//!
//! // Pause/resume/cancel recurring schedules via the module handle.
//! handle.pause_recurring(task_id).await?;
//! handle.resume_recurring(task_id).await?;
//! handle.cancel_recurring(task_id).await?;
//! ```
//!
//! ## Task chains
//!
//! Use [`TaskSubmission::depends_on`] to build dependency chains between
//! independent tasks. Unlike parent-child relationships (which model
//! fan-out from a single executor), chains connect separately submitted
//! tasks into ordered workflows.
//!
//! ### Sequential chain
//!
//! Upload a file, verify its checksum, then delete the local copy:
//!
//! ```ignore
//! let handle = scheduler.module("pipeline");
//!
//! let upload = handle.submit(
//!     TaskSubmission::new("upload").key("file-a").payload_json(&upload_plan)
//! ).await?;
//!
//! let verify = handle.submit(
//!     TaskSubmission::new("verify")
//!         .key("file-a-verify")
//!         .depends_on(upload.id().unwrap())
//!         .payload_json(&verify_plan)
//! ).await?;
//!
//! handle.submit(
//!     TaskSubmission::new("delete-local")
//!         .key("file-a-delete")
//!         .depends_on(verify.id().unwrap())
//!         .payload_json(&delete_plan)
//! ).await?;
//! ```
//!
//! ### Fan-in
//!
//! Multiple uploads converging on a single finalize step:
//!
//! ```ignore
//! let handle = scheduler.module("pipeline");
//! let mut upload_ids = Vec::new();
//! for part in &parts {
//!     let outcome = handle.submit(
//!         TaskSubmission::new("upload-part")
//!             .key(&part.key)
//!             .payload_json(part)
//!     ).await?;
//!     upload_ids.push(outcome.id().unwrap());
//! }
//!
//! handle.submit(
//!     TaskSubmission::new("finalize")
//!         .key("finalize-upload")
//!         .depends_on_all(upload_ids)
//!         .payload_json(&finalize_plan)
//! ).await?;
//! ```
//!
//! ### Diamond dependency
//!
//! Task A fans out to B and C, which both converge on D:
//!
//! ```ignore
//! let handle = scheduler.module("pipeline");
//!
//! let a = handle.submit(
//!     TaskSubmission::new("extract").key("a").payload_json(&extract)
//! ).await?;
//! let a_id = a.id().unwrap();
//!
//! let b = handle.submit(
//!     TaskSubmission::new("transform-x")
//!         .key("b").depends_on(a_id).payload_json(&tx)
//! ).await?;
//!
//! let c = handle.submit(
//!     TaskSubmission::new("transform-y")
//!         .key("c").depends_on(a_id).payload_json(&ty)
//! ).await?;
//!
//! handle.submit(
//!     TaskSubmission::new("load")
//!         .key("d")
//!         .depends_on_all([b.id().unwrap(), c.id().unwrap()])
//!         .payload_json(&load)
//! ).await?;
//! ```
//!
//! ## Task superseding
//!
//! Use [`DuplicateStrategy::Supersede`] for "latest-value-wins" scenarios
//! like continuous file sync, where re-submitting an already-queued task
//! should atomically cancel the old one and replace it:
//!
//! ```ignore
//! use taskmill::{TaskSubmission, DuplicateStrategy};
//!
//! let sub = TaskSubmission::new("sync-file")
//!     .key("path/to/file.txt")
//!     .payload_json(&new_content)
//!     .on_duplicate(DuplicateStrategy::Supersede);
//!
//! let outcome = scheduler.module("sync").submit(sub).await?;
//! // outcome is Superseded { new_task_id, replaced_task_id } if a duplicate existed,
//! // or Inserted(id) if this was the first submission.
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
//! 2. Expired tasks are swept (if the sweep interval has elapsed).
//! 3. Paused tasks are resumed if no active preemptors exist at their
//!    priority level.
//! 4. Pending finalizers (parents whose children all completed) are
//!    dispatched first.
//! 5. The highest-priority pending task is peeked (without claiming it).
//!    If it has expired, it is moved to history and the next candidate is
//!    tried.
//! 6. The dispatch gate checks concurrency limits, group concurrency,
//!    IO budget, and backpressure. If the gate rejects, no slot is consumed.
//! 7. If admitted, the task is atomically claimed (`peek` → `pop_by_id`)
//!    and spawned as a Tokio task.
//! 8. Steps 5–7 repeat until the queue is empty or the gate rejects.
//!
//! # Feature flags
//!
//! - **`sysinfo-monitor`** (default): Enables the built-in [`SysinfoSampler`](resource::sysinfo_monitor::SysinfoSampler)
//!   for cross-platform CPU and disk IO monitoring. Disable for mobile targets or
//!   when providing a custom [`ResourceSampler`]. Without this feature, calling
//!   [`SchedulerBuilder::with_resource_monitoring`] requires a custom sampler
//!   via [`resource_sampler()`](SchedulerBuilder::resource_sampler).

pub mod backpressure;
pub mod module;
pub mod priority;
pub mod registry;
pub mod resource;
pub mod scheduler;
pub mod store;
pub mod task;

// Convenience re-exports.
pub use backpressure::{CompositePressure, PressureSource, ThrottlePolicy};
pub use module::{Module, ModuleHandle, ModuleReceiver, ModuleRegistry, ModuleSnapshot};
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
    generate_dedup_key, BackoffStrategy, BatchOutcome, BatchSubmission, DependencyFailurePolicy,
    DuplicateStrategy, HistoryStatus, IoBudget, ModuleSubmitDefaults, ParentResolution,
    RecurringSchedule, RecurringScheduleInfo, RetryPolicy, SubmitBuilder, SubmitOutcome, TaskError,
    TaskHistoryRecord, TaskLookup, TaskRecord, TaskStatus, TaskSubmission, TtlFrom, TypeStats,
    TypedTask, MAX_TAGS_PER_TASK, MAX_TAG_KEY_LEN, MAX_TAG_VALUE_LEN,
};

#[cfg(feature = "sysinfo-monitor")]
pub use resource::platform_sampler;
