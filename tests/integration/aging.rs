//! Integration tests for priority aging (Phase 1 of fair scheduling).

use std::time::Duration;

use taskmill::{
    AgingConfig, Domain, Priority, Scheduler, SchedulerEvent, TaskStore, TaskSubmission,
};
use tokio_util::sync::CancellationToken;

use super::common::*;

// ── Aged task dispatches before younger ──────────────────────────────

#[tokio::test]
async fn aged_task_dispatches_before_younger() {
    let store = TaskStore::open_memory().await.unwrap();

    // Insert an IDLE task with a very old created_at via raw SQL.
    let old_ms = chrono::Utc::now().timestamp_millis() - 600_000; // 10 min ago
    sqlx::query(
        "INSERT INTO tasks (task_type, key, label, priority, status, created_at) VALUES ('test::test', 'old-idle', 'old-idle', ?, 'pending', ?)",
    )
    .bind(Priority::IDLE.value() as i32)
    .bind(old_ms)
    .execute(store.pool())
    .await
    .unwrap();

    // Submit a fresh NORMAL task.
    store
        .submit(
            &TaskSubmission::new("test::test")
                .key("fresh-normal")
                .priority(Priority::NORMAL),
        )
        .await
        .unwrap();

    let sched = Scheduler::builder()
        .store(store)
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .max_concurrency(1)
        .priority_aging(AgingConfig {
            grace_period: Duration::from_secs(0),
            aging_interval: Duration::from_secs(1),
            max_effective_priority: Priority::HIGH,
            urgent_threshold: None,
        })
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let t = token.clone();
    tokio::spawn(async move { sched.run(t).await });

    // The aged IDLE task should dispatch first (effective priority = HIGH(64) < NORMAL(128)).
    let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();

    if let SchedulerEvent::Dispatched(header) = event {
        assert_eq!(header.label, "old-idle");
        assert_eq!(header.base_priority, Priority::IDLE);
        assert!(
            header.effective_priority.value() < Priority::NORMAL.value(),
            "effective_priority {} should be higher than NORMAL {}",
            header.effective_priority.value(),
            Priority::NORMAL.value(),
        );
    } else {
        panic!("expected Dispatched event, got {:?}", event);
    }

    token.cancel();
}

// ── Builder configures aging ─────────────────────────────────────────

#[tokio::test]
async fn builder_configures_aging() {
    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .priority_aging(AgingConfig::default())
        .build()
        .await
        .unwrap();

    let snap = sched.snapshot().await.unwrap();
    assert!(snap.aging_config.is_some());
}

// ── Snapshot shows aging config ──────────────────────────────────────

#[tokio::test]
async fn snapshot_shows_aging_config() {
    let config = AgingConfig {
        grace_period: Duration::from_secs(120),
        aging_interval: Duration::from_secs(30),
        max_effective_priority: Priority::REALTIME,
        urgent_threshold: None,
    };

    let sched = Scheduler::builder()
        .store(TaskStore::open_memory().await.unwrap())
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .priority_aging(config.clone())
        .build()
        .await
        .unwrap();

    let snap = sched.snapshot().await.unwrap();
    let ac = snap.aging_config.unwrap();
    assert_eq!(ac.grace_period, Duration::from_secs(120));
    assert_eq!(ac.aging_interval, Duration::from_secs(30));
    assert_eq!(ac.max_effective_priority, Priority::REALTIME);
}

// ── Dispatched event has effective priority ──────────────────────────

#[tokio::test]
async fn dispatched_event_has_effective_priority() {
    let store = TaskStore::open_memory().await.unwrap();

    // Insert an old task.
    let old_ms = chrono::Utc::now().timestamp_millis() - 600_000;
    sqlx::query(
        "INSERT INTO tasks (task_type, key, label, priority, status, created_at) VALUES ('test::test', 'aged', 'aged', ?, 'pending', ?)",
    )
    .bind(Priority::IDLE.value() as i32)
    .bind(old_ms)
    .execute(store.pool())
    .await
    .unwrap();

    let sched = Scheduler::builder()
        .store(store)
        .domain(Domain::<TestDomain>::new().task::<TestTask>(NoopExecutor))
        .max_concurrency(1)
        .priority_aging(AgingConfig {
            grace_period: Duration::from_secs(0),
            aging_interval: Duration::from_secs(1),
            max_effective_priority: Priority::HIGH,
            urgent_threshold: None,
        })
        .build()
        .await
        .unwrap();

    let mut rx = sched.subscribe();
    let token = CancellationToken::new();
    let t = token.clone();
    tokio::spawn(async move { sched.run(t).await });

    let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();

    if let SchedulerEvent::Dispatched(header) = event {
        assert_eq!(header.base_priority, Priority::IDLE);
        // effective_priority should be lower numeric value (higher priority) than base.
        assert!(
            header.effective_priority.value() < header.base_priority.value(),
            "expected aging to promote: effective={} base={}",
            header.effective_priority.value(),
            header.base_priority.value(),
        );
    } else {
        panic!("expected Dispatched event, got {:?}", event);
    }

    token.cancel();
}
