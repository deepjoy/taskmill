use std::time::Duration;

use chrono::Utc;

use crate::priority::Priority;
use crate::task::{HistoryStatus, IoBudget, PauseReasons, TaskStatus, TaskSubmission};

use super::super::TaskStore;
use super::FailBackoff;

async fn test_store() -> TaskStore {
    TaskStore::open_memory().await.unwrap()
}

fn make_submission(key: &str, priority: Priority) -> TaskSubmission {
    TaskSubmission::new("test")
        .key(key)
        .priority(priority)
        .payload_raw(b"hello".to_vec())
        .expected_io(IoBudget::disk(1000, 500))
}

#[tokio::test]
async fn priority_ordering() {
    let store = test_store().await;

    let bg = make_submission("bg", Priority::BACKGROUND);
    let rt = make_submission("rt", Priority::REALTIME);
    let normal = make_submission("normal", Priority::NORMAL);

    let bg_key = bg.effective_key();
    let rt_key = rt.effective_key();
    let normal_key = normal.effective_key();

    store.submit(&bg).await.unwrap();
    store.submit(&rt).await.unwrap();
    store.submit(&normal).await.unwrap();

    let first = store.pop_next().await.unwrap().unwrap();
    assert_eq!(first.key, rt_key);

    let second = store.pop_next().await.unwrap().unwrap();
    assert_eq!(second.key, normal_key);

    let third = store.pop_next().await.unwrap().unwrap();
    assert_eq!(third.key, bg_key);
}

#[tokio::test]
async fn complete_moves_to_history() {
    let store = test_store().await;
    let sub = make_submission("done", Priority::NORMAL);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();

    store
        .complete(task.id, &IoBudget::disk(2000, 1000))
        .await
        .unwrap();

    assert!(store.task_by_key(&key).await.unwrap().is_none());

    let hist = store.history_by_key(&key).await.unwrap();
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].status, HistoryStatus::Completed);
    assert_eq!(hist[0].actual_io.unwrap().disk_read, 2000);
}

#[tokio::test]
async fn fail_retryable_requeues() {
    let store = test_store().await;
    let sub = make_submission("retry-me", Priority::HIGH);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();

    store
        .fail(
            task.id,
            "transient error",
            true,
            3,
            &IoBudget::default(),
            &FailBackoff::default(),
        )
        .await
        .unwrap();

    let requeued = store.task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(requeued.status, TaskStatus::Pending);
    assert_eq!(requeued.retry_count, 1);
    assert_eq!(requeued.last_error.as_deref(), Some("transient error"));
}

#[tokio::test]
async fn fail_exhausted_retries_moves_to_history() {
    let store = test_store().await;
    let sub = make_submission("permanent", Priority::NORMAL);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();

    store
        .fail(
            task.id,
            "err1",
            true,
            1,
            &IoBudget::default(),
            &FailBackoff::default(),
        )
        .await
        .unwrap();
    let task = store.pop_next().await.unwrap().unwrap();
    assert_eq!(task.retry_count, 1);
    store
        .fail(
            task.id,
            "err2",
            true,
            1,
            &IoBudget::disk(100, 50),
            &FailBackoff::default(),
        )
        .await
        .unwrap();

    assert!(store.task_by_key(&key).await.unwrap().is_none());
    // Exhausted retries now produce dead_letter status (phase 5).
    let hist = store.dead_letter_tasks(10, 0).await.unwrap();
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].status, HistoryStatus::DeadLetter);
}

#[tokio::test]
async fn pause_and_resume() {
    let store = test_store().await;
    store
        .submit(&make_submission("pausable", Priority::NORMAL))
        .await
        .unwrap();
    let task = store.pop_next().await.unwrap().unwrap();

    store
        .pause(task.id, PauseReasons::PREEMPTION)
        .await
        .unwrap();
    let paused = store.paused_tasks().await.unwrap();
    assert_eq!(paused.len(), 1);
    assert_eq!(paused[0].status, TaskStatus::Paused);
    assert_eq!(paused[0].pause_reasons, PauseReasons::PREEMPTION);

    store.resume(task.id).await.unwrap();
    let pending = store.pending_tasks(10).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].status, TaskStatus::Pending);
    assert_eq!(pending[0].pause_reasons, PauseReasons::NONE);
}

#[tokio::test]
async fn pause_reasons_accumulate_across_sources() {
    let store = test_store().await;
    let sub = TaskSubmission::new("mod1.work").key("multi-pause");
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();

    // Preemption pause.
    store
        .pause(task.id, PauseReasons::PREEMPTION)
        .await
        .unwrap();
    let t = store.task_by_id(task.id).await.unwrap().unwrap();
    assert_eq!(t.pause_reasons, PauseReasons::PREEMPTION);

    // Module pause ORs in the MODULE bit.
    store.pause_pending_by_type_prefix("mod1.").await.unwrap();
    let t = store.task_by_id(task.id).await.unwrap().unwrap();
    assert!(t.pause_reasons.contains(PauseReasons::PREEMPTION));
    assert!(t.pause_reasons.contains(PauseReasons::MODULE));

    // Clearing preemption leaves MODULE bit.
    store.resume_preempted(task.id).await.unwrap();
    let t = store.task_by_id(task.id).await.unwrap().unwrap();
    assert_eq!(t.status, TaskStatus::Paused);
    assert!(!t.pause_reasons.contains(PauseReasons::PREEMPTION));
    assert!(t.pause_reasons.contains(PauseReasons::MODULE));

    // Clearing module fully resumes.
    store.resume_paused_by_type_prefix("mod1.").await.unwrap();
    let t = store.task_by_id(task.id).await.unwrap().unwrap();
    assert_eq!(t.status, TaskStatus::Pending);
    assert_eq!(t.pause_reasons, PauseReasons::NONE);
}

#[tokio::test]
async fn preemption_paused_tasks_filters_by_bit() {
    let store = test_store().await;

    // Task A: preemption-paused.
    let sub_a = TaskSubmission::new("test").key("a");
    store.submit(&sub_a).await.unwrap();
    let a = store.pop_next().await.unwrap().unwrap();
    store.pause(a.id, PauseReasons::PREEMPTION).await.unwrap();

    // Task B: module-paused only.
    let sub_b = TaskSubmission::new("test").key("b");
    store.submit(&sub_b).await.unwrap();
    store.pause_pending_by_type_prefix("test").await.unwrap();

    let preempted = store.preemption_paused_tasks().await.unwrap();
    // A has PREEMPTION bit (and MODULE bit added by prefix pause).
    // B has only MODULE bit.
    assert_eq!(preempted.len(), 1);
    assert_eq!(preempted[0].id, a.id);
}

#[tokio::test]
async fn clear_pause_bit_resumes_sole_reason_tasks() {
    let store = test_store().await;

    // Task with only GLOBAL reason.
    let sub_a = TaskSubmission::new("test").key("global-only");
    store.submit(&sub_a).await.unwrap();
    let a = store.pop_next().await.unwrap().unwrap();
    store.pause(a.id, PauseReasons::GLOBAL).await.unwrap();

    // Task with GLOBAL + PREEMPTION reasons.
    let sub_b = TaskSubmission::new("test").key("multi");
    store.submit(&sub_b).await.unwrap();
    let b = store.pop_next().await.unwrap().unwrap();
    store.pause(b.id, PauseReasons::PREEMPTION).await.unwrap();
    store.pause(b.id, PauseReasons::GLOBAL).await.unwrap();

    let fully_resumed = store.clear_pause_bit(PauseReasons::GLOBAL).await.unwrap();
    assert_eq!(fully_resumed, 1); // Only task A fully resumed.

    let ta = store.task_by_id(a.id).await.unwrap().unwrap();
    assert_eq!(ta.status, TaskStatus::Pending);
    assert_eq!(ta.pause_reasons, PauseReasons::NONE);

    let tb = store.task_by_id(b.id).await.unwrap().unwrap();
    assert_eq!(tb.status, TaskStatus::Paused);
    assert!(tb.pause_reasons.contains(PauseReasons::PREEMPTION));
    assert!(!tb.pause_reasons.contains(PauseReasons::GLOBAL));
}

#[tokio::test]
async fn module_pause_resume_with_bitmask() {
    let store = test_store().await;

    let sub = TaskSubmission::new("mod1.upload").key("m1");
    store.submit(&sub).await.unwrap();

    // Module pause sets MODULE bit.
    let paused_count = store.pause_pending_by_type_prefix("mod1.").await.unwrap();
    assert_eq!(paused_count, 1);
    let t = store
        .task_by_key(&sub.effective_key())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(t.status, TaskStatus::Paused);
    assert_eq!(t.pause_reasons, PauseReasons::MODULE);

    // Module resume clears MODULE bit → fully resumes.
    let resumed_count = store.resume_paused_by_type_prefix("mod1.").await.unwrap();
    assert_eq!(resumed_count, 1);
    let t = store
        .task_by_key(&sub.effective_key())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(t.status, TaskStatus::Pending);
    assert_eq!(t.pause_reasons, PauseReasons::NONE);
}

#[tokio::test]
async fn running_io_totals() {
    let store = test_store().await;

    let sub = TaskSubmission::new("test")
        .key("io-1")
        .priority(Priority::NORMAL)
        .payload_raw(b"hello".to_vec())
        .expected_io(IoBudget::disk(5000, 2000));
    store.submit(&sub).await.unwrap();

    let sub2 = TaskSubmission::new("test")
        .key("io-2")
        .priority(Priority::NORMAL)
        .payload_raw(b"hello".to_vec())
        .expected_io(IoBudget::disk(3000, 1000));
    store.submit(&sub2).await.unwrap();

    store.pop_next().await.unwrap();
    store.pop_next().await.unwrap();

    let (read, write) = store.running_io_totals().await.unwrap();
    assert_eq!(read, 8000);
    assert_eq!(write, 3000);
}

#[tokio::test]
async fn key_freed_after_completion() {
    let store = test_store().await;
    let sub = make_submission("reuse", Priority::NORMAL);
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();
    store.complete(task.id, &IoBudget::default()).await.unwrap();

    let outcome = store.submit(&sub).await.unwrap();
    assert!(outcome.is_inserted());
}

#[tokio::test]
async fn requeue_running_task() {
    let store = test_store().await;
    let sub = make_submission("rq", Priority::NORMAL);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();
    assert_eq!(task.status, TaskStatus::Running);

    store.requeue(task.id).await.unwrap();
    let t = store.task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(t.status, TaskStatus::Pending);
    assert!(t.started_at.is_none());
}

#[tokio::test]
async fn peek_next_does_not_modify_status() {
    let store = test_store().await;
    let sub = make_submission("peek-me", Priority::NORMAL);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();

    let peeked = store.peek_next().await.unwrap().unwrap();
    assert_eq!(peeked.key, key);
    assert_eq!(peeked.status, TaskStatus::Pending);

    let t = store.task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(t.status, TaskStatus::Pending);
    assert!(t.started_at.is_none());

    let peeked2 = store.peek_next().await.unwrap().unwrap();
    assert_eq!(peeked2.id, peeked.id);
}

#[tokio::test]
async fn peek_next_empty_queue() {
    let store = test_store().await;
    assert!(store.peek_next().await.unwrap().is_none());
}

#[tokio::test]
async fn pop_by_id_claims_pending_task() {
    let store = test_store().await;
    let sub = make_submission("claim-me", Priority::NORMAL);
    let key = sub.effective_key();
    let id = store.submit(&sub).await.unwrap().id().unwrap();

    let task = store.pop_by_id(id).await.unwrap().unwrap();
    assert_eq!(task.key, key);
    assert_eq!(task.status, TaskStatus::Running);
    assert!(task.started_at.is_some());
}

#[tokio::test]
async fn pop_by_id_returns_none_if_already_running() {
    let store = test_store().await;
    let sub = make_submission("already-taken", Priority::NORMAL);
    store.submit(&sub).await.unwrap();

    let task = store.pop_next().await.unwrap().unwrap();

    assert!(store.pop_by_id(task.id).await.unwrap().is_none());
}

#[tokio::test]
async fn pop_by_id_returns_none_for_nonexistent() {
    let store = test_store().await;
    assert!(store.pop_by_id(9999).await.unwrap().is_none());
}

#[tokio::test]
async fn peek_then_pop_by_id_workflow() {
    let store = test_store().await;
    let sub = make_submission("peek-pop", Priority::NORMAL);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();

    let peeked = store.peek_next().await.unwrap().unwrap();
    let claimed = store.pop_by_id(peeked.id).await.unwrap().unwrap();
    assert_eq!(claimed.key, key);
    assert_eq!(claimed.status, TaskStatus::Running);

    assert!(store.peek_next().await.unwrap().is_none());
}

// ── Tag lifecycle tests ───────────────────────────────────────────

#[tokio::test]
async fn tags_copied_to_history_on_complete() {
    let store = test_store().await;
    let sub = TaskSubmission::new("test")
        .key("hist-tags-complete")
        .tag("env", "staging")
        .tag("owner", "alice");

    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();
    store.complete(task.id, &IoBudget::default()).await.unwrap();

    let mut hist = store.history_by_key(&sub.effective_key()).await.unwrap();
    assert_eq!(hist.len(), 1);
    store.populate_history_tags(&mut hist).await.unwrap();
    assert_eq!(hist[0].tags.get("env").unwrap(), "staging");
    assert_eq!(hist[0].tags.get("owner").unwrap(), "alice");
}

#[tokio::test]
async fn tags_copied_to_history_on_fail() {
    let store = test_store().await;
    let sub = TaskSubmission::new("test")
        .key("hist-tags-fail")
        .tag("region", "us-west");

    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();
    store
        .fail(
            task.id,
            "boom",
            false,
            0,
            &IoBudget::default(),
            &FailBackoff::default(),
        )
        .await
        .unwrap();

    let mut hist = store.failed_tasks(10).await.unwrap();
    assert_eq!(hist.len(), 1);
    store.populate_history_tags(&mut hist).await.unwrap();
    assert_eq!(hist[0].tags.get("region").unwrap(), "us-west");
}

#[tokio::test]
async fn tags_copied_to_history_on_cancel() {
    let store = test_store().await;
    let sub = TaskSubmission::new("test")
        .key("hist-tags-cancel")
        .tag("priority_class", "low");

    let id = store.submit(&sub).await.unwrap().id().unwrap();
    store.cancel_to_history(id).await.unwrap();

    let mut hist = store.history_by_key(&sub.effective_key()).await.unwrap();
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].status, HistoryStatus::Cancelled);
    store.populate_history_tags(&mut hist).await.unwrap();
    assert_eq!(hist[0].tags.get("priority_class").unwrap(), "low");
}

#[tokio::test]
async fn tags_copied_to_history_on_expire() {
    use std::time::Duration;

    let store = test_store().await;
    let sub = TaskSubmission::new("test")
        .key("hist-tags-expire")
        .tag("source", "cron")
        .ttl(Duration::from_secs(0)); // Expire immediately.

    store.submit(&sub).await.unwrap();

    // Small delay so expires_at is in the past.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let expired = store.expire_tasks().await.unwrap();
    assert!(!expired.is_empty());

    let mut hist = store.history_by_key(&sub.effective_key()).await.unwrap();
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].status, HistoryStatus::Expired);
    store.populate_history_tags(&mut hist).await.unwrap();
    assert_eq!(hist[0].tags.get("source").unwrap(), "cron");
}

#[tokio::test]
async fn tags_preserved_on_recurring_requeue() {
    use std::time::Duration;

    let store = test_store().await;
    let sub = TaskSubmission::new("test")
        .key("recurring-tags")
        .tag("schedule", "hourly")
        .recurring(Duration::from_secs(3600));

    store.submit(&sub).await.unwrap();
    let mut task = store.pop_next().await.unwrap().unwrap();
    store
        .populate_tags(std::slice::from_mut(&mut task))
        .await
        .unwrap();
    assert_eq!(task.tags.get("schedule").unwrap(), "hourly");

    store
        .complete_with_record(&task, &IoBudget::default())
        .await
        .unwrap();

    // The next recurring instance should have the same tags.
    let key = sub.effective_key();
    let next = store.task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(next.tags.get("schedule").unwrap(), "hourly");
}

#[tokio::test]
async fn tags_in_pop_next() {
    let store = test_store().await;
    let sub = TaskSubmission::new("test")
        .key("pop-tags")
        .tag("color", "blue");

    store.submit(&sub).await.unwrap();
    let mut task = store.pop_next().await.unwrap().unwrap();
    store
        .populate_tags(std::slice::from_mut(&mut task))
        .await
        .unwrap();
    assert_eq!(task.tags.get("color").unwrap(), "blue");
}

// ── max_retries persistence (Phase 3) ────────────────────────────

#[tokio::test]
async fn max_retries_round_trips_through_insert_and_select() {
    let store = test_store().await;
    let sub = TaskSubmission::new("test")
        .key("mr-roundtrip")
        .max_retries(5);

    let id = store.submit(&sub).await.unwrap().id().unwrap();
    let task = store.task_by_id(id).await.unwrap().unwrap();
    assert_eq!(task.max_retries, Some(5));
}

#[tokio::test]
async fn max_retries_none_when_not_set() {
    let store = test_store().await;
    let sub = TaskSubmission::new("test").key("mr-none");

    let id = store.submit(&sub).await.unwrap().id().unwrap();
    let task = store.task_by_id(id).await.unwrap().unwrap();
    assert_eq!(task.max_retries, None);
}

#[tokio::test]
async fn max_retries_preserved_in_history_on_complete() {
    let store = test_store().await;
    let sub = TaskSubmission::new("test")
        .key("mr-hist-complete")
        .max_retries(7);

    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();
    assert_eq!(task.max_retries, Some(7));

    store.complete(task.id, &IoBudget::default()).await.unwrap();

    let key = sub.effective_key();
    let history = store.history_by_key(&key).await.unwrap();
    assert!(!history.is_empty());
    assert_eq!(history[0].max_retries, Some(7));
}

#[tokio::test]
async fn max_retries_preserved_in_history_on_fail() {
    let store = test_store().await;
    let sub = TaskSubmission::new("test")
        .key("mr-hist-fail")
        .max_retries(3);

    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();

    // Permanent failure (non-retryable).
    store
        .fail(
            task.id,
            "boom",
            false,
            0,
            &IoBudget::default(),
            &FailBackoff::default(),
        )
        .await
        .unwrap();

    let key = sub.effective_key();
    let history = store.history_by_key(&key).await.unwrap();
    assert!(!history.is_empty());
    assert_eq!(history[0].max_retries, Some(3));
    assert_eq!(history[0].status, HistoryStatus::Failed);
}

#[tokio::test]
async fn max_retries_null_reads_back_as_none() {
    let store = test_store().await;
    // Submit without max_retries (NULL in DB).
    let sub = TaskSubmission::new("test").key("mr-null");
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();
    assert_eq!(task.max_retries, None);

    // Complete it and verify history also has None.
    store.complete(task.id, &IoBudget::default()).await.unwrap();
    let key = sub.effective_key();
    let history = store.history_by_key(&key).await.unwrap();
    assert_eq!(history[0].max_retries, None);
}

// ── Phase 4: Retry with backoff ─────────────────────────────────

#[tokio::test]
async fn backoff_constant_sets_run_after() {
    use crate::task::BackoffStrategy;
    use std::time::Duration;

    let store = test_store().await;
    let sub = make_submission("const-backoff", Priority::NORMAL);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();

    let strategy = BackoffStrategy::Constant {
        delay: Duration::from_secs(60),
    };
    store
        .fail(
            task.id,
            "transient",
            true,
            3,
            &IoBudget::default(),
            &FailBackoff {
                strategy: Some(&strategy),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let requeued = store.task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(requeued.status, TaskStatus::Pending);
    assert_eq!(requeued.retry_count, 1);
    // run_after should be set roughly 60s in the future.
    let run_after = requeued.run_after.expect("run_after should be set");
    let diff = run_after - chrono::Utc::now();
    assert!(
        diff.num_seconds() >= 55 && diff.num_seconds() <= 65,
        "expected run_after ~60s in the future, got {}s",
        diff.num_seconds()
    );
}

#[tokio::test]
async fn backoff_exponential_increases_across_retries() {
    use crate::task::BackoffStrategy;
    use std::time::Duration;

    let store = test_store().await;
    let sub = make_submission("exp-backoff", Priority::NORMAL);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();

    let strategy = BackoffStrategy::Exponential {
        initial: Duration::from_secs(10),
        max: Duration::from_secs(3600),
        multiplier: 2.0,
    };

    // First failure (retry_count=0): delay = 10s
    let task = store.pop_next().await.unwrap().unwrap();
    assert_eq!(task.retry_count, 0);
    store
        .fail(
            task.id,
            "err",
            true,
            5,
            &IoBudget::default(),
            &FailBackoff {
                strategy: Some(&strategy),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requeued = store.task_by_key(&key).await.unwrap().unwrap();
    let run_after_1 = requeued.run_after.expect("run_after should be set");
    let diff_1 = (run_after_1 - chrono::Utc::now()).num_seconds();
    assert!(
        (7..=13).contains(&diff_1),
        "retry 0: expected ~10s delay, got {diff_1}s"
    );

    // Manually clear run_after so we can pop the task for the next retry.
    sqlx::query("UPDATE tasks SET run_after = NULL WHERE key = ?")
        .bind(&key)
        .execute(store.pool())
        .await
        .unwrap();

    // Second failure (retry_count=1): delay = 20s
    let task = store.pop_next().await.unwrap().unwrap();
    assert_eq!(task.retry_count, 1);
    store
        .fail(
            task.id,
            "err",
            true,
            5,
            &IoBudget::default(),
            &FailBackoff {
                strategy: Some(&strategy),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let requeued = store.task_by_key(&key).await.unwrap().unwrap();
    let run_after_2 = requeued.run_after.expect("run_after should be set");
    let diff_2 = (run_after_2 - chrono::Utc::now()).num_seconds();
    assert!(
        (17..=23).contains(&diff_2),
        "retry 1: expected ~20s delay, got {diff_2}s"
    );
}

#[tokio::test]
async fn executor_retry_after_overrides_strategy() {
    use crate::task::BackoffStrategy;
    use std::time::Duration;

    let store = test_store().await;
    let sub = make_submission("override-backoff", Priority::NORMAL);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();

    // Strategy says 10s, but executor override says 120s.
    let strategy = BackoffStrategy::Constant {
        delay: Duration::from_secs(10),
    };
    store
        .fail(
            task.id,
            "rate limited",
            true,
            3,
            &IoBudget::default(),
            &FailBackoff {
                strategy: Some(&strategy),
                executor_retry_after_ms: Some(120_000),
            },
        )
        .await
        .unwrap();

    let requeued = store.task_by_key(&key).await.unwrap().unwrap();
    let run_after = requeued.run_after.expect("run_after should be set");
    let diff = (run_after - chrono::Utc::now()).num_seconds();
    // Should be ~120s, not ~10s.
    assert!(
        (115..=125).contains(&diff),
        "expected ~120s delay from executor override, got {diff}s"
    );
}

#[tokio::test]
async fn no_backoff_requeues_immediately() {
    let store = test_store().await;
    let sub = make_submission("no-backoff", Priority::NORMAL);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();

    // No strategy, no executor override → immediate retry.
    store
        .fail(
            task.id,
            "err",
            true,
            3,
            &IoBudget::default(),
            &FailBackoff::default(),
        )
        .await
        .unwrap();

    let requeued = store.task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(requeued.status, TaskStatus::Pending);
    assert_eq!(requeued.retry_count, 1);
    // run_after should remain None (immediate dispatch).
    assert!(
        requeued.run_after.is_none(),
        "run_after should be None for immediate retry"
    );
}

#[tokio::test]
async fn permanent_error_skips_retry_moves_to_history() {
    use crate::task::BackoffStrategy;
    use std::time::Duration;

    let store = test_store().await;
    let sub = make_submission("permanent-err", Priority::NORMAL);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();

    // Even with a backoff strategy, non-retryable errors go straight to history.
    let strategy = BackoffStrategy::Constant {
        delay: Duration::from_secs(60),
    };
    store
        .fail(
            task.id,
            "fatal error",
            false,
            3,
            &IoBudget::default(),
            &FailBackoff {
                strategy: Some(&strategy),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    // Should be gone from the active queue.
    assert!(store.task_by_key(&key).await.unwrap().is_none());

    // Should be in history as failed.
    let hist = store.failed_tasks(10).await.unwrap();
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].status, HistoryStatus::Failed);
    assert_eq!(hist[0].last_error.as_deref(), Some("fatal error"));
}

// ── Phase 5: Dead-letter state ──────────────────────────────────

#[tokio::test]
async fn exhausted_retries_produce_dead_letter_status() {
    let store = test_store().await;
    let sub = make_submission("dl-exhausted", Priority::NORMAL);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();

    // First failure: retry_count=0, max_retries=1 → requeue.
    let task = store.pop_next().await.unwrap().unwrap();
    assert_eq!(task.retry_count, 0);
    store
        .fail(
            task.id,
            "transient",
            true,
            1,
            &IoBudget::default(),
            &FailBackoff::default(),
        )
        .await
        .unwrap();

    // Second failure: retry_count=1, max_retries=1 → exhausted → dead_letter.
    let task = store.pop_next().await.unwrap().unwrap();
    assert_eq!(task.retry_count, 1);
    store
        .fail(
            task.id,
            "still transient",
            true,
            1,
            &IoBudget::disk(100, 50),
            &FailBackoff::default(),
        )
        .await
        .unwrap();

    // Should be gone from active queue.
    assert!(store.task_by_key(&key).await.unwrap().is_none());

    // Should be in history as dead_letter (not failed).
    let hist = store.history_by_key(&key).await.unwrap();
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].status, HistoryStatus::DeadLetter);
    assert_eq!(hist[0].last_error.as_deref(), Some("still transient"));
    assert_eq!(hist[0].retry_count, 2); // retry_count incremented
}

#[tokio::test]
async fn non_retryable_error_still_produces_failed_status() {
    let store = test_store().await;
    let sub = make_submission("dl-permanent", Priority::NORMAL);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();

    // Non-retryable error with remaining retries → should be "failed", not "dead_letter".
    store
        .fail(
            task.id,
            "permanent error",
            false,
            3,
            &IoBudget::default(),
            &FailBackoff::default(),
        )
        .await
        .unwrap();

    assert!(store.task_by_key(&key).await.unwrap().is_none());

    let hist = store.history_by_key(&key).await.unwrap();
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].status, HistoryStatus::Failed);

    // Should NOT appear in dead_letter_tasks query.
    let dl = store.dead_letter_tasks(10, 0).await.unwrap();
    assert!(dl.is_empty());
}

#[tokio::test]
async fn dead_letter_tasks_query_returns_only_dead_lettered() {
    let store = test_store().await;

    // Create a dead-lettered task (retryable, exhausted).
    let sub_dl = make_submission("dl-query-dl", Priority::NORMAL);
    store.submit(&sub_dl).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();
    store
        .fail(
            task.id,
            "transient",
            true,
            0, // max_retries=0 → immediately exhausted
            &IoBudget::default(),
            &FailBackoff::default(),
        )
        .await
        .unwrap();

    // Create a failed task (non-retryable).
    let sub_fail = make_submission("dl-query-fail", Priority::NORMAL);
    store.submit(&sub_fail).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();
    store
        .fail(
            task.id,
            "permanent",
            false,
            3,
            &IoBudget::default(),
            &FailBackoff::default(),
        )
        .await
        .unwrap();

    // Create a completed task.
    let sub_ok = make_submission("dl-query-ok", Priority::NORMAL);
    store.submit(&sub_ok).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();
    store.complete(task.id, &IoBudget::default()).await.unwrap();

    // dead_letter_tasks should return only the dead-lettered one.
    let dl = store.dead_letter_tasks(10, 0).await.unwrap();
    assert_eq!(dl.len(), 1);
    assert_eq!(dl[0].status, HistoryStatus::DeadLetter);
    assert_eq!(dl[0].task_type, "test");

    // failed_tasks should return only the permanently failed one.
    let failed = store.failed_tasks(10).await.unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].status, HistoryStatus::Failed);
}

// ── Group pause state management (Step 3a) ──────────────────────

#[tokio::test]
async fn pause_group_state_is_idempotent() {
    let store = test_store().await;
    assert!(store.pause_group_state("g1", None).await.unwrap());
    assert!(!store.pause_group_state("g1", None).await.unwrap()); // no-op
}

#[tokio::test]
async fn resume_group_state_is_idempotent() {
    let store = test_store().await;
    store.pause_group_state("g1", None).await.unwrap();
    assert!(store.resume_group_state("g1").await.unwrap());
    assert!(!store.resume_group_state("g1").await.unwrap()); // no-op
}

#[tokio::test]
async fn paused_groups_lists_all() {
    let store = test_store().await;
    store.pause_group_state("g2", None).await.unwrap();
    store.pause_group_state("g1", None).await.unwrap();

    let groups = store.paused_groups().await.unwrap();
    assert_eq!(groups.len(), 2);
    // Ordered by paused_at.
    assert_eq!(groups[0].0, "g2");
    assert_eq!(groups[1].0, "g1");
}

#[tokio::test]
async fn is_group_paused_reflects_state() {
    let store = test_store().await;
    assert!(!store.is_group_paused("g1").await.unwrap());
    store.pause_group_state("g1", None).await.unwrap();
    assert!(store.is_group_paused("g1").await.unwrap());
    store.resume_group_state("g1").await.unwrap();
    assert!(!store.is_group_paused("g1").await.unwrap());
}

#[tokio::test]
async fn groups_due_for_resume_finds_elapsed() {
    let store = test_store().await;
    let past = Utc::now() - chrono::Duration::seconds(10);
    store
        .pause_group_state("g1", Some(past.timestamp_millis()))
        .await
        .unwrap();
    // Group with future resume_at should not appear.
    let future = Utc::now() + chrono::Duration::seconds(3600);
    store
        .pause_group_state("g2", Some(future.timestamp_millis()))
        .await
        .unwrap();
    // Group with no resume_at should not appear.
    store.pause_group_state("g3", None).await.unwrap();

    let due = store.groups_due_for_resume().await.unwrap();
    assert_eq!(due, vec!["g1"]);
}

// ── Bulk pause / resume by group (Step 3b) ──────────────────────

#[tokio::test]
async fn pause_tasks_in_group_sets_status_and_bits() {
    let store = test_store().await;
    let sub1 = TaskSubmission::new("test").key("t1").group("g1");
    let sub2 = TaskSubmission::new("test").key("t2").group("g2");
    let key1 = sub1.effective_key();
    let key2 = sub2.effective_key();
    store.submit(&sub1).await.unwrap();
    store.submit(&sub2).await.unwrap();

    let count = store.pause_tasks_in_group("g1").await.unwrap();
    assert_eq!(count, 1);

    let t1 = store.task_by_key(&key1).await.unwrap().unwrap();
    assert_eq!(t1.status, TaskStatus::Paused);
    assert!(t1.pause_reasons.contains(PauseReasons::GROUP));

    let t2 = store.task_by_key(&key2).await.unwrap().unwrap();
    assert_eq!(t2.status, TaskStatus::Pending);
    assert!(t2.pause_reasons.is_empty());
}

#[tokio::test]
async fn pause_tasks_in_group_adds_bit_to_already_paused() {
    let store = test_store().await;
    let sub = TaskSubmission::new("test").key("t1").group("g1");
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();

    // Preemption pause first.
    store
        .pause(task.id, PauseReasons::PREEMPTION)
        .await
        .unwrap();
    let t = store.task_by_id(task.id).await.unwrap().unwrap();
    assert_eq!(t.pause_reasons, PauseReasons::PREEMPTION);

    // Group pause should OR in the GROUP bit.
    store.pause_tasks_in_group("g1").await.unwrap();
    let t = store.task_by_id(task.id).await.unwrap().unwrap();
    assert_eq!(t.status, TaskStatus::Paused);
    assert!(t.pause_reasons.contains(PauseReasons::PREEMPTION));
    assert!(t.pause_reasons.contains(PauseReasons::GROUP));
}

#[tokio::test]
async fn resume_paused_by_group_fully_resumes_sole_reason() {
    let store = test_store().await;
    let sub = TaskSubmission::new("test").key("t1").group("g1");
    store.submit(&sub).await.unwrap();

    store.pause_tasks_in_group("g1").await.unwrap();
    let fully_resumed = store.resume_paused_by_group("g1").await.unwrap();
    assert_eq!(fully_resumed, 1);

    let t = store.task_by_id(1).await.unwrap().unwrap();
    assert_eq!(t.status, TaskStatus::Pending);
    assert_eq!(t.pause_reasons, PauseReasons::NONE);
}

#[tokio::test]
async fn resume_paused_by_group_clears_bit_but_stays_paused_with_other_reasons() {
    let store = test_store().await;
    let sub = TaskSubmission::new("test").key("t1").group("g1");
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();

    // Preemption pause + group pause.
    store
        .pause(task.id, PauseReasons::PREEMPTION)
        .await
        .unwrap();
    store.pause_tasks_in_group("g1").await.unwrap();

    let fully_resumed = store.resume_paused_by_group("g1").await.unwrap();
    assert_eq!(fully_resumed, 0); // Still held by PREEMPTION.

    let t = store.task_by_id(task.id).await.unwrap().unwrap();
    assert_eq!(t.status, TaskStatus::Paused);
    assert!(t.pause_reasons.contains(PauseReasons::PREEMPTION));
    assert!(!t.pause_reasons.contains(PauseReasons::GROUP));
}

// ── TTL behavior with group pause (Step 3c) ─────────────────────

#[tokio::test]
async fn expire_tasks_expires_group_paused() {
    let store = test_store().await;
    store
        .submit(
            &TaskSubmission::new("test")
                .key("t1")
                .group("g1")
                .ttl(Duration::from_millis(1)),
        )
        .await
        .unwrap();

    store.pause_group_state("g1", None).await.unwrap();
    store.pause_tasks_in_group("g1").await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Sweep SHOULD expire the group-paused task — TTL is a hard deadline.
    let expired = store.expire_tasks().await.unwrap();
    assert_eq!(expired.len(), 1);
}

// ── Query helpers (Step 3d) ─────────────────────────────────────

#[tokio::test]
async fn pending_and_paused_count_for_group() {
    let store = test_store().await;
    let sub1 = TaskSubmission::new("test").key("t1").group("g1");
    let sub2 = TaskSubmission::new("test").key("t2").group("g1");
    let sub3 = TaskSubmission::new("test").key("t3").group("g2");
    store.submit(&sub1).await.unwrap();
    store.submit(&sub2).await.unwrap();
    store.submit(&sub3).await.unwrap();

    assert_eq!(store.pending_count_for_group("g1").await.unwrap(), 2);
    assert_eq!(store.paused_count_for_group("g1").await.unwrap(), 0);

    store.pause_tasks_in_group("g1").await.unwrap();

    assert_eq!(store.pending_count_for_group("g1").await.unwrap(), 0);
    assert_eq!(store.paused_count_for_group("g1").await.unwrap(), 2);
    // g2 unaffected.
    assert_eq!(store.pending_count_for_group("g2").await.unwrap(), 1);
    assert_eq!(store.paused_count_for_group("g2").await.unwrap(), 0);
}
