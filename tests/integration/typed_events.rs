//! Integration tests: Phase 2 typed event streams, cross-domain context,
//! and domain_state.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use taskmill::{
    Domain, DomainHandle, Scheduler, TaskContext, TaskError, TaskEvent, TaskStore, TypedExecutor,
    TypedTask,
};
use tokio_util::sync::CancellationToken;

use super::common::*;

// ── Test task types ──────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug)]
struct Thumbnail {
    path: String,
}

impl TypedTask for Thumbnail {
    type Domain = MediaDomain;
    const TASK_TYPE: &'static str = "thumbnail";
}

struct ThumbnailExec;

impl TypedExecutor<Thumbnail> for ThumbnailExec {
    async fn execute<'a>(
        &'a self,
        _payload: Thumbnail,
        _ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct FailingTask {
    reason: String,
}

impl TypedTask for FailingTask {
    type Domain = MediaDomain;
    const TASK_TYPE: &'static str = "failing";
}

struct AlwaysFailTypedExec;

impl TypedExecutor<FailingTask> for AlwaysFailTypedExec {
    async fn execute<'a>(
        &'a self,
        _payload: FailingTask,
        _ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        Err(TaskError::new("permanent failure"))
    }
}

// ── TypedEventStream tests ──────────────────────────────────────────

#[tokio::test]
async fn task_events_receives_completed_with_record() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MediaDomain>::new().task::<Thumbnail>(ThumbnailExec))
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    let media: DomainHandle<MediaDomain> = sched.domain::<MediaDomain>();
    let mut stream = media.task_events::<Thumbnail>();

    media
        .submit(Thumbnail {
            path: "/img/a.jpg".into(),
        })
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let tok = token.clone();
    tokio::spawn(async move { sched_clone.run(tok).await });

    // Wait for the Completed event.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut found = false;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(100), stream.recv()).await {
            Ok(Ok(TaskEvent::Completed { id, record })) => {
                assert!(id > 0, "task ID should be positive");
                // Verify we can deserialize the payload from the record.
                let payload: Thumbnail =
                    serde_json::from_slice(record.payload.as_deref().unwrap()).unwrap();
                assert_eq!(payload.path, "/img/a.jpg");
                found = true;
                break;
            }
            Ok(Ok(TaskEvent::Dispatched { .. })) => continue,
            _ => continue,
        }
    }

    token.cancel();
    assert!(found, "should receive TaskEvent::Completed with record");
}

#[tokio::test]
async fn task_events_receives_failed_with_record() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MediaDomain>::new().task::<FailingTask>(AlwaysFailTypedExec))
        .max_concurrency(4)
        .max_retries(0)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    let media: DomainHandle<MediaDomain> = sched.domain::<MediaDomain>();
    let mut stream = media.task_events::<FailingTask>();

    media
        .submit(FailingTask {
            reason: "test".into(),
        })
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let tok = token.clone();
    tokio::spawn(async move { sched_clone.run(tok).await });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut found = false;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(100), stream.recv()).await {
            Ok(Ok(TaskEvent::Failed {
                error,
                will_retry,
                record,
                ..
            })) => {
                assert!(!will_retry, "should not retry");
                assert!(error.contains("permanent failure"));
                assert!(record.is_some(), "terminal failure should have a record");
                found = true;
                break;
            }
            Ok(Ok(_)) => continue,
            _ => continue,
        }
    }

    token.cancel();
    assert!(found, "should receive TaskEvent::Failed with record");
}

#[tokio::test]
async fn task_events_filters_to_correct_task_type() {
    // Register two task types, submit both, but only listen for one.
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<MediaDomain>::new()
                .task::<Thumbnail>(ThumbnailExec)
                .task::<FailingTask>(AlwaysFailTypedExec),
        )
        .max_concurrency(4)
        .max_retries(0)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    let media: DomainHandle<MediaDomain> = sched.domain::<MediaDomain>();
    let mut thumb_stream = media.task_events::<Thumbnail>();

    // Submit both task types.
    media
        .submit(Thumbnail {
            path: "/img/b.jpg".into(),
        })
        .await
        .unwrap();
    media
        .submit(FailingTask {
            reason: "noise".into(),
        })
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let tok = token.clone();
    tokio::spawn(async move { sched_clone.run(tok).await });

    // Collect events from the thumbnail stream only.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut thumb_completed = false;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(100), thumb_stream.recv()).await {
            Ok(Ok(TaskEvent::Completed { record, .. })) => {
                let payload: Thumbnail =
                    serde_json::from_slice(record.payload.as_deref().unwrap()).unwrap();
                assert_eq!(payload.path, "/img/b.jpg");
                thumb_completed = true;
                break;
            }
            Ok(Ok(_)) => continue,
            _ => continue,
        }
    }

    token.cancel();
    assert!(
        thumb_completed,
        "thumbnail stream should receive only Thumbnail events"
    );
}

// ── ctx.domain::<D>() tests ─────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct UploadTask {
    file: String,
}

impl TypedTask for UploadTask {
    type Domain = SyncDomain;
    const TASK_TYPE: &'static str = "upload";
}

/// Typed executor that uses `ctx.domain::<SyncDomain>()` to submit a task.
struct CrossDomainTypedExec {
    submitted: Arc<AtomicBool>,
}

impl TypedExecutor<Thumbnail> for CrossDomainTypedExec {
    async fn execute<'a>(
        &'a self,
        thumb: Thumbnail,
        ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        let sync: DomainHandle<SyncDomain> = ctx.domain::<SyncDomain>();
        sync.submit(UploadTask {
            file: thumb.path.clone(),
        })
        .await
        .map_err(|e| TaskError::new(format!("{e}")))?;
        self.submitted.store(true, Ordering::SeqCst);
        Ok(())
    }
}

struct UploadExec {
    ran: Arc<AtomicBool>,
}

impl TypedExecutor<UploadTask> for UploadExec {
    async fn execute<'a>(
        &'a self,
        _payload: UploadTask,
        _ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        self.ran.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn ctx_domain_typed_cross_domain_submission() {
    let submitted = Arc::new(AtomicBool::new(false));
    let upload_ran = Arc::new(AtomicBool::new(false));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<MediaDomain>::new().task::<Thumbnail>(CrossDomainTypedExec {
                submitted: submitted.clone(),
            }),
        )
        .domain(Domain::<SyncDomain>::new().task::<UploadTask>(UploadExec {
            ran: upload_ran.clone(),
        }))
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    let media = sched.domain::<MediaDomain>();
    media
        .submit(Thumbnail {
            path: "/img/c.jpg".into(),
        })
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let tok = token.clone();
    tokio::spawn(async move { sched_clone.run(tok).await });

    tokio::time::sleep(Duration::from_millis(500)).await;
    token.cancel();

    assert!(
        submitted.load(Ordering::SeqCst),
        "media executor should have submitted the sync task"
    );
    assert!(
        upload_ran.load(Ordering::SeqCst),
        "sync::upload should have been created and run"
    );
}

// ── ctx.domain_state tests ──────────────────────────────────────────

struct MediaConfig {
    cdn_url: String,
}

struct DomainStateExec {
    found_url: Arc<std::sync::Mutex<Option<String>>>,
}

impl TypedExecutor<Thumbnail> for DomainStateExec {
    async fn execute<'a>(
        &'a self,
        _payload: Thumbnail,
        ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        let cfg = ctx
            .domain_state::<MediaDomain, MediaConfig>()
            .ok_or_else(|| TaskError::new("missing config"))?;
        *self.found_url.lock().unwrap() = Some(cfg.cdn_url.clone());
        Ok(())
    }
}

#[tokio::test]
async fn ctx_domain_state_retrieves_domain_scoped_state() {
    let found_url: Arc<std::sync::Mutex<Option<String>>> = Arc::new(std::sync::Mutex::new(None));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<MediaDomain>::new()
                .task::<Thumbnail>(DomainStateExec {
                    found_url: found_url.clone(),
                })
                .state(MediaConfig {
                    cdn_url: "https://cdn.example.com".into(),
                }),
        )
        .max_concurrency(2)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    sched
        .domain::<MediaDomain>()
        .submit(Thumbnail {
            path: "/img/d.jpg".into(),
        })
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let tok = token.clone();
    tokio::spawn(async move { sched_clone.run(tok).await });

    tokio::time::sleep(Duration::from_millis(300)).await;
    token.cancel();

    assert_eq!(
        found_url.lock().unwrap().as_deref(),
        Some("https://cdn.example.com"),
        "executor should have retrieved the MediaConfig via domain_state"
    );
}
