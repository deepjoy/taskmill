//! Integration tests: steps 8–11
//! TaskContext module access, cross-module child spawning,
//! Scheduler::modules() convenience, and event module identity.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use taskmill::{
    Domain, Scheduler, SchedulerEvent, TaskContext, TaskError, TaskExecutor, TaskStore,
    TaskSubmission,
};
use tokio_util::sync::CancellationToken;

use super::common::*;

// ── Step 8: TaskContext module access ─────────────────────────────────────

/// Executor in module A that submits a task to module B via `ctx.module("b")`.
struct CrossModuleSubmitter {
    submitted: Arc<AtomicBool>,
}

impl TaskExecutor for CrossModuleSubmitter {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        ctx.module("b")
            .submit(TaskSubmission::new("task").key("cross-module-child"))
            .await
            .map_err(|e| TaskError::new(format!("{e}")))?;
        self.submitted.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn ctx_module_submits_to_other_module_with_prefix_and_defaults() {
    let submitted = Arc::new(AtomicBool::new(false));
    let b_ran = Arc::new(AtomicBool::new(false));
    let submitted_clone = submitted.clone();
    let b_ran_clone = b_ran.clone();

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<DomainA>::new().raw_executor(
            "trigger",
            CrossModuleSubmitter {
                submitted: submitted_clone,
            },
        ))
        .domain(Domain::<DomainB>::new().raw_executor("task", {
            struct B(Arc<AtomicBool>);
            impl TaskExecutor for B {
                async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
                    self.0.store(true, Ordering::SeqCst);
                    Ok(())
                }
            }
            B(b_ran_clone)
        }))
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    sched
        .domain::<DomainA>()
        .submit_raw(TaskSubmission::new("trigger").key("t1"))
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    tokio::spawn(async move { sched_clone.run(token_clone).await });

    tokio::time::sleep(Duration::from_millis(500)).await;
    token.cancel();

    assert!(
        submitted.load(Ordering::SeqCst),
        "module A executor should have run"
    );
    assert!(
        b_ran.load(Ordering::SeqCst),
        "module B task should have been created and run"
    );
}

/// Executor that uses `ctx.current_module()` to submit a follow-up task.
struct SameModuleSubmitter {
    submitted: Arc<AtomicBool>,
}

impl TaskExecutor for SameModuleSubmitter {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        ctx.current_module()
            .submit(TaskSubmission::new("follower").key("same-module-follower"))
            .await
            .map_err(|e| TaskError::new(format!("{e}")))?;
        self.submitted.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn ctx_current_module_applies_owning_module_defaults() {
    let submitted = Arc::new(AtomicBool::new(false));
    let follower_ran = Arc::new(AtomicBool::new(false));
    let submitted_clone = submitted.clone();
    let follower_ran_clone = follower_ran.clone();

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<MediaDomain>::new()
                .raw_executor(
                    "leader",
                    SameModuleSubmitter {
                        submitted: submitted_clone,
                    },
                )
                .raw_executor("follower", {
                    struct Follower(Arc<AtomicBool>);
                    impl TaskExecutor for Follower {
                        async fn execute<'a>(
                            &'a self,
                            _ctx: &'a TaskContext,
                        ) -> Result<(), TaskError> {
                            self.0.store(true, Ordering::SeqCst);
                            Ok(())
                        }
                    }
                    Follower(follower_ran_clone)
                })
                .default_priority(taskmill::Priority::BACKGROUND),
        )
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    sched
        .domain::<MediaDomain>()
        .submit_raw(TaskSubmission::new("leader").key("l1"))
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    tokio::spawn(async move { sched_clone.run(token_clone).await });

    tokio::time::sleep(Duration::from_millis(500)).await;
    token.cancel();

    assert!(
        submitted.load(Ordering::SeqCst),
        "leader executor should have run"
    );
    assert!(
        follower_ran.load(Ordering::SeqCst),
        "follower task submitted via current_module() should run"
    );
}

#[tokio::test]
async fn ctx_try_module_returns_none_for_unknown_module() {
    let result: Arc<std::sync::Mutex<Option<bool>>> = Arc::new(std::sync::Mutex::new(None));
    let result_clone = result.clone();

    struct TryModuleExecutor(Arc<std::sync::Mutex<Option<bool>>>);
    impl TaskExecutor for TryModuleExecutor {
        async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
            let found = ctx.try_module("nonexistent").is_some();
            *self.0.lock().unwrap() = Some(found);
            Ok(())
        }
    }

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().raw_executor("probe", TryModuleExecutor(result_clone)))
        .max_concurrency(2)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    sched
        .domain::<TestDomain>()
        .submit_raw(TaskSubmission::new("probe").key("p1"))
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    tokio::spawn(async move { sched_clone.run(token_clone).await });

    tokio::time::sleep(Duration::from_millis(300)).await;
    token.cancel();

    assert_eq!(
        *result.lock().unwrap(),
        Some(false),
        "try_module('nonexistent') should return None"
    );
}

#[tokio::test]
async fn spawn_child_routes_through_current_module() {
    // Verify spawn_child auto-prefixes the task type with the owning module.
    // The child executor is registered under "child" (unprefixed) in the "test" module.
    let child_ran = Arc::new(AtomicBool::new(false));
    let child_ran_clone = child_ran.clone();

    struct SpawnChildExecutor;
    impl TaskExecutor for SpawnChildExecutor {
        async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
            ctx.spawn_child(TaskSubmission::new("worker").key("spawned-child"))
                .await?;
            Ok(())
        }
    }

    struct WorkerExecutor(Arc<AtomicBool>);
    impl TaskExecutor for WorkerExecutor {
        async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
            self.0.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<TestDomain>::new()
                .raw_executor("spawner", SpawnChildExecutor)
                .raw_executor("worker", WorkerExecutor(child_ran_clone)),
        )
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    sched
        .domain::<TestDomain>()
        .submit_raw(TaskSubmission::new("spawner").key("s1"))
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    tokio::spawn(async move { sched_clone.run(token_clone).await });

    tokio::time::sleep(Duration::from_millis(500)).await;
    token.cancel();

    assert!(
        child_ran.load(Ordering::SeqCst),
        "child spawned via spawn_child should run with auto-prefixed task type"
    );
}

// ── Step 9: Cross-Module Child Spawning ───────────────────────────────────

/// Executor in module "media" that submits a cross-module child to "analytics"
/// using `SubmitBuilder::parent()`.
struct CrossModuleParentExec {
    child_submitted: Arc<AtomicBool>,
}

impl TaskExecutor for CrossModuleParentExec {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        ctx.module("analytics")
            .submit(TaskSubmission::new("work").key("cross-child"))
            .parent(ctx.record().id)
            .await
            .map_err(|e| TaskError::new(format!("{e}")))?;
        self.child_submitted.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Cross-module parent-child: parent in "media", child in "analytics".
/// Parent should enter Waiting, then complete once the analytics child completes.
#[tokio::test]
async fn cross_module_parent_child_lifecycle() {
    let child_submitted = Arc::new(AtomicBool::new(false));
    let analytics_ran = Arc::new(AtomicBool::new(false));
    let child_submitted_clone = child_submitted.clone();
    let analytics_ran_clone = analytics_ran.clone();

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MediaDomain>::new().raw_executor(
            "parent",
            CrossModuleParentExec {
                child_submitted: child_submitted_clone,
            },
        ))
        .domain(Domain::<AnalyticsDomain>::new().raw_executor("work", {
            struct AnalyticsExec(Arc<AtomicBool>);
            impl TaskExecutor for AnalyticsExec {
                async fn execute<'a>(&'a self, _ctx: &'a TaskContext) -> Result<(), TaskError> {
                    self.0.store(true, Ordering::SeqCst);
                    Ok(())
                }
            }
            AnalyticsExec(analytics_ran_clone)
        }))
        .max_concurrency(4)
        .max_retries(0)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    sched
        .domain::<MediaDomain>()
        .submit_raw(TaskSubmission::new("parent").key("media-parent-1"))
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    tokio::spawn(async move { sched_clone.run(token_clone).await });

    // Wait for the media parent to complete (after its analytics child completes).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let parent_completed = wait_for_event(
        &mut rx,
        deadline,
        |evt| matches!(evt, SchedulerEvent::Completed(ref h) if h.task_type == "media::parent"),
    )
    .await;

    token.cancel();

    assert!(
        child_submitted.load(Ordering::SeqCst),
        "media executor should have submitted the analytics child"
    );
    assert!(
        analytics_ran.load(Ordering::SeqCst),
        "analytics::work child should have run"
    );
    assert!(
        parent_completed.is_some(),
        "media::parent should complete once its cross-module child completes"
    );
}

/// Cross-module failure cascade: child in "analytics" fails permanently →
/// parent in "media" is failed (fail_fast = true, the default).
#[tokio::test]
async fn cross_module_failure_cascade() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MediaDomain>::new().raw_executor(
            "parent",
            CrossModuleParentExec {
                child_submitted: Arc::new(AtomicBool::new(false)),
            },
        ))
        .domain(Domain::<AnalyticsDomain>::new().raw_executor("work", AlwaysFailExecutor))
        .max_concurrency(4)
        .max_retries(0)
        .poll_interval(Duration::from_millis(20))
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    sched
        .domain::<MediaDomain>()
        .submit_raw(
            TaskSubmission::new("parent")
                .key("media-parent-cascade")
                .fail_fast(true),
        )
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    tokio::spawn(async move { sched_clone.run(token_clone).await });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let parent_failed = wait_for_event(
        &mut rx,
        deadline,
        |evt| {
            matches!(evt, SchedulerEvent::Failed { ref header, .. } if header.task_type == "media::parent")
        },
    )
    .await;

    token.cancel();

    assert!(
        parent_failed.is_some(),
        "media::parent should be failed when cross-module analytics::work child fails"
    );
}

// ── Step 10: Scheduler::modules() and cross-cutting convenience ──────

/// Domain handles for all registered modules can be obtained individually.
#[tokio::test]
async fn scheduler_domains_returns_registered_modules() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<AlphaDomain>::new().raw_executor("work", NoopExecutor))
        .domain(Domain::<BetaDomain>::new().raw_executor("work", NoopExecutor))
        .domain(Domain::<GammaDomain>::new().raw_executor("work", NoopExecutor))
        .max_concurrency(4)
        .build()
        .await
        .unwrap();

    assert_eq!(sched.domain::<AlphaDomain>().name(), "alpha");
    assert_eq!(sched.domain::<BetaDomain>().name(), "beta");
    assert_eq!(sched.domain::<GammaDomain>().name(), "gamma");
}

/// `scheduler.active_tasks()` returns running tasks from all modules.
#[tokio::test]
async fn scheduler_active_tasks_returns_tasks_from_all_modules() {
    let barrier = Arc::new(tokio::sync::Barrier::new(3));

    let barrier_clone = barrier.clone();
    struct BarrierExecutor(Arc<tokio::sync::Barrier>);
    impl TaskExecutor for BarrierExecutor {
        async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
            self.0.wait().await;
            tokio::select! {
                _ = ctx.token().cancelled() => {},
                _ = tokio::time::sleep(Duration::from_secs(5)) => {},
            }
            Ok(())
        }
    }

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<AlphaDomain>::new().raw_executor("work", BarrierExecutor(barrier.clone())))
        .domain(Domain::<BetaDomain>::new().raw_executor("work", BarrierExecutor(barrier_clone)))
        .max_concurrency(4)
        .poll_interval(Duration::from_millis(10))
        .build()
        .await
        .unwrap();

    sched
        .domain::<AlphaDomain>()
        .submit_raw(TaskSubmission::new("work").key("a1"))
        .await
        .unwrap();
    sched
        .domain::<BetaDomain>()
        .submit_raw(TaskSubmission::new("work").key("b1"))
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    tokio::spawn(async move { sched_clone.run(token_clone).await });

    // Wait until both tasks are running.
    barrier.wait().await;

    let active = sched.active_tasks().await;
    let types: Vec<&str> = active.iter().map(|t| t.task_type.as_str()).collect();

    token.cancel();

    assert!(
        types.contains(&"alpha::work"),
        "alpha::work should be in active tasks; got: {types:?}"
    );
    assert!(
        types.contains(&"beta::work"),
        "beta::work should be in active tasks; got: {types:?}"
    );
}

/// Cross-module cancel-by-tag via individual domain handles cancels matching
/// tasks in all modules and leaves untagged tasks untouched.
/// Tasks stay pending (no run loop) so we verify the return IDs directly.
#[tokio::test]
async fn cross_module_cancel_by_tag_via_domain_handles() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<AlphaDomain>::new().raw_executor("work", NoopExecutor))
        .domain(Domain::<BetaDomain>::new().raw_executor("work", NoopExecutor))
        .max_concurrency(8)
        .build()
        .await
        .unwrap();

    let alpha = sched.domain::<AlphaDomain>();
    let beta = sched.domain::<BetaDomain>();

    // Tagged tasks — targets for cross-module cancel.
    let alpha_tagged = alpha
        .submit_raw(
            TaskSubmission::new("work")
                .key("a-tagged")
                .tag("job_id", "job-1"),
        )
        .await
        .unwrap()
        .id()
        .unwrap();
    let beta_tagged = beta
        .submit_raw(
            TaskSubmission::new("work")
                .key("b-tagged")
                .tag("job_id", "job-1"),
        )
        .await
        .unwrap()
        .id()
        .unwrap();
    // Untagged task — must survive.
    let alpha_untagged = alpha
        .submit_raw(TaskSubmission::new("work").key("a-untagged"))
        .await
        .unwrap()
        .id()
        .unwrap();

    // Cancel "job-1" tasks across all domains (tasks are still pending).
    let mut cancelled_ids: Vec<i64> = Vec::new();
    let ids = alpha
        .cancel_where(|t| t.tags.get("job_id").map(String::as_str) == Some("job-1"))
        .await
        .unwrap();
    cancelled_ids.extend(ids);
    let ids = beta
        .cancel_where(|t| t.tags.get("job_id").map(String::as_str) == Some("job-1"))
        .await
        .unwrap();
    cancelled_ids.extend(ids);

    assert!(
        cancelled_ids.contains(&alpha_tagged),
        "alpha tagged task should have been cancelled; got: {cancelled_ids:?}"
    );
    assert!(
        cancelled_ids.contains(&beta_tagged),
        "beta tagged task should have been cancelled; got: {cancelled_ids:?}"
    );
    assert_eq!(
        cancelled_ids.len(),
        2,
        "exactly 2 tasks should be cancelled"
    );

    // Untagged task must still be in the active store (cancelled tasks move to history).
    assert!(
        sched
            .store()
            .task_by_id(alpha_untagged)
            .await
            .unwrap()
            .is_some(),
        "untagged task should still be in the active store, not moved to history"
    );
}

/// `.parent()` on `SubmitBuilder` inherits remaining parent TTL and tags.
/// No scheduler run needed — just verify the stored child record.
#[tokio::test]
async fn parent_method_inherits_ttl_and_tags() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(
            Domain::<MediaDomain>::new()
                .raw_executor("parent", NoopExecutor)
                .raw_executor("child", NoopExecutor),
        )
        .max_concurrency(2)
        .build()
        .await
        .unwrap();

    let media = sched.domain::<MediaDomain>();

    // Submit parent with a 60-second TTL and a custom tag.
    let parent_outcome = media
        .submit_raw(
            TaskSubmission::new("parent")
                .key("ttl-parent")
                .ttl(Duration::from_secs(60))
                .tag("job", "pipeline-42"),
        )
        .await
        .unwrap();
    let parent_id = parent_outcome.id().unwrap();

    // Submit child with .parent() — no explicit TTL or tags on the child.
    let child_outcome = media
        .submit_raw(TaskSubmission::new("child").key("ttl-child"))
        .parent(parent_id)
        .await
        .unwrap();
    let child_id = child_outcome.id().unwrap();

    let child = sched.store().task_by_id(child_id).await.unwrap().unwrap();

    assert!(
        child.ttl_seconds.is_some(),
        "child should inherit parent TTL"
    );
    assert!(
        child.ttl_seconds.unwrap() > 0,
        "inherited TTL should be positive"
    );
    assert_eq!(
        child.tags.get("job").map(String::as_str),
        Some("pipeline-42"),
        "child should inherit parent tag 'job'"
    );
    // Child's own tags take precedence — a tag set directly on the child
    // should not be overwritten by the parent tag with the same key.
    let child2_outcome = media
        .submit_raw(
            TaskSubmission::new("child")
                .key("ttl-child-2")
                .tag("job", "child-override"),
        )
        .parent(parent_id)
        .await
        .unwrap();
    let child2 = sched
        .store()
        .task_by_id(child2_outcome.id().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        child2.tags.get("job").map(String::as_str),
        Some("child-override"),
        "child's own tag should win over parent tag"
    );
}

// ── Step 11: Event Module Identity ──────────────────────────────────────────

/// Events emitted for a `media::thumbnail` task carry `header.module == "media"`.
#[tokio::test]
async fn event_header_module_field_populated_from_task_type_prefix() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MediaDomain>::new().raw_executor("thumbnail", NoopExecutor))
        .max_concurrency(4)
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();

    sched
        .domain::<MediaDomain>()
        .submit_raw(TaskSubmission::new("thumbnail").key("thumb-1"))
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let tok = token.clone();
    tokio::spawn(async move { sched_clone.run(tok).await });

    // Collect the Completed event and verify the module field.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut found = false;
    while tokio::time::Instant::now() < deadline {
        if let Ok(Ok(SchedulerEvent::Completed(ref h))) =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
        {
            assert_eq!(
                h.module, "media",
                "completed event for media::thumbnail should have module == 'media', got '{}'",
                h.module
            );
            assert_eq!(h.task_type, "media::thumbnail");
            found = true;
            break;
        }
    }
    assert!(found, "timed out waiting for Completed event");

    token.cancel();
}

/// Events received via `DomainHandle::events()` have a `module` field that
/// agrees with the module name — the filter and the field both identify the
/// same module.
#[tokio::test]
async fn module_receiver_events_match_module_field() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<MediaDomain>::new().raw_executor("thumbnail", NoopExecutor))
        .domain(Domain::<SyncDomain>::new().raw_executor("push", NoopExecutor))
        .max_concurrency(8)
        .build()
        .await
        .unwrap();

    let mut media_rx = sched.domain::<MediaDomain>().events();

    // Submit tasks to both modules.
    let media = sched.domain::<MediaDomain>();
    let sync_handle = sched.domain::<SyncDomain>();
    for i in 0..2 {
        media
            .submit_raw(TaskSubmission::new("thumbnail").key(format!("t{i}")))
            .await
            .unwrap();
        sync_handle
            .submit_raw(TaskSubmission::new("push").key(format!("p{i}")))
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let tok = token.clone();
    tokio::spawn(async move { sched_clone.run(tok).await });

    // Collect 2 Completed events from media_rx and assert the module field.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut completions = 0usize;
    while completions < 2 && tokio::time::Instant::now() < deadline {
        if let Ok(Ok(SchedulerEvent::Completed(ref h))) =
            tokio::time::timeout(Duration::from_millis(100), media_rx.recv()).await
        {
            assert_eq!(
                h.module, "media",
                "ModuleReceiver delivered event with wrong module field: '{}'",
                h.module
            );
            completions += 1;
        }
    }
    assert_eq!(completions, 2, "should receive exactly 2 media completions");

    token.cancel();
}
