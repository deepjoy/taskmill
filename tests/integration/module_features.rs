//! Integration tests: sections P (default layering), Q (module concurrency),
//! and step 7 (namespaced StateMap).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use taskmill::{
    Domain, Priority, Scheduler, SchedulerEvent, TaskContext, TaskError, TaskStore, TaskSubmission,
    TaskTypeConfig, TypedExecutor, TypedTask,
};
use tokio_util::sync::CancellationToken;

use super::common::*;

// ═══════════════════════════════════════════════════════════════════
// P. Default Layering (Step 5)
// ═══════════════════════════════════════════════════════════════════

/// Full 5-layer precedence chain exercised through `submit_with()`:
///
/// Layer 1 (SubmitBuilder override) > Layer 3 (module defaults) >
/// Layer 4 (TypedTask defaults) > Layer 5 (scheduler global defaults).
///
/// Layer 2 (explicit TaskSubmission field) is not relevant for `submit_with()`
/// since the submission is always built from the TypedTask.
#[tokio::test]
async fn submit_typed_five_layer_precedence_chain() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct LayeredTask;

    impl taskmill::TypedTask for LayeredTask {
        type Domain = MediaDomain;
        const TASK_TYPE: &'static str = "layered";
        fn config() -> TaskTypeConfig {
            TaskTypeConfig::new()
                .priority(Priority::HIGH) // layer 4: should be overridden by module (layer 3)
                .group("typed-group") // layer 4: should be overridden by module
                .ttl(std::time::Duration::from_secs(7200)) // layer 4: overridden by module
        }
        fn tags(&self) -> std::collections::HashMap<String, String> {
            [("source".into(), "typed".into())].into()
        }
    }

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .default_ttl(std::time::Duration::from_secs(14400)) // layer 5 (not reached)
        .domain(
            Domain::<MediaDomain>::new()
                .task::<LayeredTask>(NoopExecutor)
                .default_priority(Priority::BACKGROUND) // layer 3: overrides TypedTask HIGH
                .default_group("module-group") // layer 3: overrides typed-group
                .default_ttl(std::time::Duration::from_secs(10800)) // layer 3: 3 h
                .default_tag("tier", "free"),
        )
        .build()
        .await
        .unwrap();

    let media = sched.domain::<MediaDomain>();

    // Layer 1: SubmitBuilder overrides trump everything.
    let outcome = media
        .submit_with(LayeredTask)
        .priority(Priority::REALTIME) // beats module's BACKGROUND
        .ttl(std::time::Duration::from_secs(3600)) // beats module's 3 h
        .await
        .unwrap();

    let task_id = outcome.id().unwrap();
    let task = sched.task(task_id).await.unwrap().unwrap();

    // Layer 1 wins for priority and ttl.
    assert_eq!(task.priority, Priority::REALTIME, "layer 1 priority wins");
    assert_eq!(task.ttl_seconds, Some(3600), "layer 1 ttl wins");

    // Layer 3 (module) wins over layer 4 (TypedTask) for group.
    assert_eq!(
        task.group_key.as_deref(),
        Some("module-group"),
        "layer 3 group wins over TypedTask"
    );

    // Tags: all layers merge correctly.
    assert_eq!(
        task.tags.get("source").map(String::as_str),
        Some("typed"),
        "TypedTask tag preserved"
    );
    assert_eq!(
        task.tags.get("tier").map(String::as_str),
        Some("free"),
        "module tag present"
    );
    assert_eq!(
        task.tags.get("_module").map(String::as_str),
        Some("media"),
        "_module tag injected"
    );

    // task_type is prefixed by the module name.
    assert_eq!(task.task_type, "media::layered");
}

// ═══════════════════════════════════════════════════════════════════
// Q. Module Concurrency (Step 6)
// ═══════════════════════════════════════════════════════════════════

/// Module cap=2, submit 5 tasks — only 2 run concurrently.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn module_cap_limits_concurrency_to_2() {
    let current = Arc::new(AtomicUsize::new(0));
    let max_seen = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .max_concurrency(10) // global cap high — module cap should bind
        .poll_interval(Duration::from_millis(20))
        .domain(
            Domain::<MediaDomain>::new()
                .task::<MediaWorkTask>(ConcurrencyTrackingExecutor {
                    current: current.clone(),
                    max_seen: max_seen.clone(),
                    delay: Duration::from_millis(100),
                })
                .max_concurrency(2),
        )
        .build()
        .await
        .unwrap();

    let media = sched.domain::<MediaDomain>();
    for i in 0..5 {
        media
            .submit_raw(TaskSubmission::new("work").key(format!("t{i}")))
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let mut rx = sched.subscribe();
    let handle = tokio::spawn(async move { sched_clone.run(token_clone).await });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut completed = 0;
    while tokio::time::Instant::now() < deadline && completed < 5 {
        if let Ok(Ok(SchedulerEvent::Completed(..))) =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
        {
            completed += 1;
        }
    }

    token.cancel();
    let _ = handle.await;

    assert_eq!(completed, 5, "all 5 tasks should complete");
    assert!(
        max_seen.load(Ordering::SeqCst) <= 2,
        "module cap 2 should be enforced, got {}",
        max_seen.load(Ordering::SeqCst)
    );
}

/// Module cap=4, group cap=2 — grouped tasks are limited to 2, module cap
/// acts as an independent broader ceiling.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn module_cap_and_group_cap_are_independent() {
    let current = Arc::new(AtomicUsize::new(0));
    let max_seen = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .max_concurrency(10)
        .poll_interval(Duration::from_millis(20))
        .group_concurrency("gpu", 2) // group cap = 2
        .domain(
            Domain::<MediaDomain>::new()
                .task::<MediaWorkTask>(ConcurrencyTrackingExecutor {
                    current: current.clone(),
                    max_seen: max_seen.clone(),
                    delay: Duration::from_millis(100),
                })
                .max_concurrency(4), // module cap = 4
        )
        .build()
        .await
        .unwrap();

    let media = sched.domain::<MediaDomain>();
    // Submit 6 tasks all in the "gpu" group — group cap is the binding constraint.
    for i in 0..6 {
        media
            .submit_raw(
                TaskSubmission::new("work")
                    .key(format!("t{i}"))
                    .group("gpu"),
            )
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let mut rx = sched.subscribe();
    let handle = tokio::spawn(async move { sched_clone.run(token_clone).await });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut completed = 0;
    while tokio::time::Instant::now() < deadline && completed < 6 {
        if let Ok(Ok(SchedulerEvent::Completed(..))) =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
        {
            completed += 1;
        }
    }

    token.cancel();
    let _ = handle.await;

    assert_eq!(completed, 6, "all 6 tasks should complete");
    assert!(
        max_seen.load(Ordering::SeqCst) <= 2,
        "group cap 2 should limit concurrency, got {}",
        max_seen.load(Ordering::SeqCst)
    );
}

/// Ungrouped tasks with module cap=3 — only the module cap is enforced.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ungrouped_task_respects_module_cap() {
    let current = Arc::new(AtomicUsize::new(0));
    let max_seen = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .max_concurrency(10)
        .poll_interval(Duration::from_millis(20))
        .domain(
            Domain::<MediaDomain>::new()
                .task::<MediaWorkTask>(ConcurrencyTrackingExecutor {
                    current: current.clone(),
                    max_seen: max_seen.clone(),
                    delay: Duration::from_millis(100),
                })
                .max_concurrency(3),
        )
        .build()
        .await
        .unwrap();

    let media = sched.domain::<MediaDomain>();
    for i in 0..7 {
        media
            .submit_raw(TaskSubmission::new("work").key(format!("t{i}")))
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let mut rx = sched.subscribe();
    let handle = tokio::spawn(async move { sched_clone.run(token_clone).await });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut completed = 0;
    while tokio::time::Instant::now() < deadline && completed < 7 {
        if let Ok(Ok(SchedulerEvent::Completed(..))) =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
        {
            completed += 1;
        }
    }

    token.cancel();
    let _ = handle.await;

    assert_eq!(completed, 7, "all 7 tasks should complete");
    assert!(
        max_seen.load(Ordering::SeqCst) <= 3,
        "module cap 3 should be enforced, got {}",
        max_seen.load(Ordering::SeqCst)
    );
}

/// Global cap=4, two modules each cap=3 — global cap is the hard ceiling.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn global_cap_is_hard_ceiling_over_module_caps() {
    // Shared counter across both modules' executors to measure total concurrency.
    let total_current = Arc::new(AtomicUsize::new(0));
    let total_max = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .max_concurrency(4) // global ceiling — should bind at 4 even though 3+3=6
        .poll_interval(Duration::from_millis(20))
        .domain(
            Domain::<MediaDomain>::new()
                .task::<MediaWorkTask>(ConcurrencyTrackingExecutor {
                    current: total_current.clone(),
                    max_seen: total_max.clone(),
                    delay: Duration::from_millis(100),
                })
                .max_concurrency(3),
        )
        .domain(
            Domain::<SyncDomain>::new()
                .task::<SyncWorkTask>(ConcurrencyTrackingExecutor {
                    current: total_current.clone(),
                    max_seen: total_max.clone(),
                    delay: Duration::from_millis(100),
                })
                .max_concurrency(3),
        )
        .build()
        .await
        .unwrap();

    let media = sched.domain::<MediaDomain>();
    let sync = sched.domain::<SyncDomain>();
    for i in 0..5 {
        media
            .submit_raw(TaskSubmission::new("work").key(format!("m{i}")))
            .await
            .unwrap();
        sync.submit_raw(TaskSubmission::new("work").key(format!("s{i}")))
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let mut rx = sched.subscribe();
    let handle = tokio::spawn(async move { sched_clone.run(token_clone).await });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut completed = 0;
    while tokio::time::Instant::now() < deadline && completed < 10 {
        if let Ok(Ok(SchedulerEvent::Completed(..))) =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
        {
            completed += 1;
        }
    }

    token.cancel();
    let _ = handle.await;

    assert_eq!(completed, 10, "all 10 tasks should complete");
    assert!(
        total_max.load(Ordering::SeqCst) <= 4,
        "global cap 4 should be the hard ceiling, got {}",
        total_max.load(Ordering::SeqCst)
    );
}

/// `set_max_concurrency` at runtime takes effect on subsequent dispatches.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_max_concurrency_changes_dispatch_behavior() {
    let current = Arc::new(AtomicUsize::new(0));
    let max_seen = Arc::new(AtomicUsize::new(0));

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .max_concurrency(10)
        .poll_interval(Duration::from_millis(20))
        .domain(
            Domain::<MediaDomain>::new()
                .task::<MediaWorkTask>(ConcurrencyTrackingExecutor {
                    current: current.clone(),
                    max_seen: max_seen.clone(),
                    delay: Duration::from_millis(100),
                })
                .max_concurrency(4), // initial cap — will be narrowed at runtime
        )
        .build()
        .await
        .unwrap();

    let media = sched.domain::<MediaDomain>();

    // Narrow the cap to 2 before dispatching anything.
    media.set_max_concurrency(2);
    assert_eq!(
        media.max_concurrency(),
        2,
        "cap should reflect the runtime update"
    );

    for i in 0..6 {
        media
            .submit_raw(TaskSubmission::new("work").key(format!("t{i}")))
            .await
            .unwrap();
    }

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let mut rx = sched.subscribe();
    let handle = tokio::spawn(async move { sched_clone.run(token_clone).await });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut completed = 0;
    while tokio::time::Instant::now() < deadline && completed < 6 {
        if let Ok(Ok(SchedulerEvent::Completed(..))) =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
        {
            completed += 1;
        }
    }

    token.cancel();
    let _ = handle.await;

    assert_eq!(completed, 6, "all 6 tasks should complete");
    assert!(
        max_seen.load(Ordering::SeqCst) <= 2,
        "runtime cap 2 should be enforced, got {}",
        max_seen.load(Ordering::SeqCst)
    );
}

// ── Step 7: Namespaced StateMap ──────────────────────────────────────────────

/// Module A's executor sees its own scoped state but not module B's.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn module_state_is_scoped_to_module() {
    struct ConfigA(#[allow(dead_code)] String);
    struct ConfigB(#[allow(dead_code)] String);

    let saw_a = Arc::new(AtomicBool::new(false));
    let no_b = Arc::new(AtomicBool::new(true)); // true = "never saw B"

    struct CheckerExec {
        saw_a: Arc<AtomicBool>,
        no_b: Arc<AtomicBool>,
    }
    impl<T: TypedTask> TypedExecutor<T> for CheckerExec {
        async fn execute<'a>(&'a self, _payload: T, ctx: &'a TaskContext) -> Result<(), TaskError> {
            self.saw_a
                .store(ctx.state::<ConfigA>().is_some(), Ordering::SeqCst);
            if ctx.state::<ConfigB>().is_some() {
                self.no_b.store(false, Ordering::SeqCst);
            }
            Ok(())
        }
    }

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .poll_interval(Duration::from_millis(20))
        .domain(
            Domain::<DomainA>::new()
                .task::<DomainATask>(CheckerExec {
                    saw_a: Arc::clone(&saw_a),
                    no_b: Arc::clone(&no_b),
                })
                .state(ConfigA("a-config".into())),
        )
        .domain(
            Domain::<DomainB>::new()
                .task::<DomainBTask>(NoopExecutor)
                .state(ConfigB("b-config".into())),
        )
        .build()
        .await
        .unwrap();

    sched
        .domain::<DomainA>()
        .submit_raw(TaskSubmission::new("task").key("t1"))
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let mut rx = sched.subscribe();
    tokio::spawn(async move { sched_clone.run(token_clone).await });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        if let Ok(Ok(SchedulerEvent::Completed(..))) =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
        {
            break;
        }
    }
    token.cancel();

    assert!(
        saw_a.load(Ordering::SeqCst),
        "module A executor should see ConfigA"
    );
    assert!(
        no_b.load(Ordering::SeqCst),
        "module A executor should NOT see ConfigB"
    );
}

/// Global state registered on the builder is accessible from executors in all modules.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn global_state_accessible_from_all_modules() {
    struct SharedConfig(#[allow(dead_code)] String);

    let a_saw = Arc::new(AtomicBool::new(false));
    let b_saw = Arc::new(AtomicBool::new(false));

    struct GlobalChecker(Arc<AtomicBool>);
    impl<T: TypedTask> TypedExecutor<T> for GlobalChecker {
        async fn execute<'a>(&'a self, _payload: T, ctx: &'a TaskContext) -> Result<(), TaskError> {
            self.0
                .store(ctx.state::<SharedConfig>().is_some(), Ordering::SeqCst);
            Ok(())
        }
    }

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .poll_interval(Duration::from_millis(20))
        .app_state(SharedConfig("global".into()))
        .domain(Domain::<DomainA>::new().task::<DomainATask>(GlobalChecker(Arc::clone(&a_saw))))
        .domain(Domain::<DomainB>::new().task::<DomainBTask>(GlobalChecker(Arc::clone(&b_saw))))
        .build()
        .await
        .unwrap();

    sched
        .domain::<DomainA>()
        .submit_raw(TaskSubmission::new("task").key("ta"))
        .await
        .unwrap();
    sched
        .domain::<DomainB>()
        .submit_raw(TaskSubmission::new("task").key("tb"))
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let mut rx = sched.subscribe();
    tokio::spawn(async move { sched_clone.run(token_clone).await });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut completed = 0;
    while tokio::time::Instant::now() < deadline && completed < 2 {
        if let Ok(Ok(SchedulerEvent::Completed(..))) =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
        {
            completed += 1;
        }
    }
    token.cancel();

    assert!(
        a_saw.load(Ordering::SeqCst),
        "module A executor should see global SharedConfig"
    );
    assert!(
        b_saw.load(Ordering::SeqCst),
        "module B executor should see global SharedConfig"
    );
}

/// Module-scoped state shadows global state of the same type for that module's executors.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn module_state_shadows_global_state() {
    struct Config(String);

    let a_value = Arc::new(std::sync::Mutex::new(String::new()));
    let b_value = Arc::new(std::sync::Mutex::new(String::new()));

    struct ValueCapture(Arc<std::sync::Mutex<String>>);
    impl<T: TypedTask> TypedExecutor<T> for ValueCapture {
        async fn execute<'a>(&'a self, _payload: T, ctx: &'a TaskContext) -> Result<(), TaskError> {
            if let Some(cfg) = ctx.state::<Config>() {
                *self.0.lock().unwrap() = cfg.0.clone();
            }
            Ok(())
        }
    }

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .poll_interval(Duration::from_millis(20))
        .app_state(Config("global".into()))
        .domain(
            Domain::<DomainA>::new()
                .task::<DomainATask>(ValueCapture(Arc::clone(&a_value)))
                .state(Config("module-a".into())),
        )
        .domain(Domain::<DomainB>::new().task::<DomainBTask>(ValueCapture(Arc::clone(&b_value))))
        .build()
        .await
        .unwrap();

    sched
        .domain::<DomainA>()
        .submit_raw(TaskSubmission::new("task").key("ta"))
        .await
        .unwrap();
    sched
        .domain::<DomainB>()
        .submit_raw(TaskSubmission::new("task").key("tb"))
        .await
        .unwrap();

    let token = CancellationToken::new();
    let sched_clone = sched.clone();
    let token_clone = token.clone();
    let mut rx = sched.subscribe();
    tokio::spawn(async move { sched_clone.run(token_clone).await });

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut completed = 0;
    while tokio::time::Instant::now() < deadline && completed < 2 {
        if let Ok(Ok(SchedulerEvent::Completed(..))) =
            tokio::time::timeout(Duration::from_millis(100), rx.recv()).await
        {
            completed += 1;
        }
    }
    token.cancel();

    assert_eq!(
        a_value.lock().unwrap().as_str(),
        "module-a",
        "module A executor should see its scoped Config, not global"
    );
    assert_eq!(
        b_value.lock().unwrap().as_str(),
        "global",
        "module B executor (no module state) should fall back to global Config"
    );
}
