use crate::priority::Priority;
use crate::task::{IoBudget, SubmitOutcome, TaskSubmission, MAX_PAYLOAD_BYTES};

use super::super::TaskStore;

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
async fn submit_and_pop() {
    use crate::task::TaskStatus;
    let store = test_store().await;
    let sub = make_submission("job-1", Priority::NORMAL);
    let expected_key = sub.effective_key();

    let outcome = store.submit(&sub).await.unwrap();
    assert!(outcome.is_inserted());

    let task = store.pop_next(None).await.unwrap().unwrap();
    assert_eq!(task.key, expected_key);
    assert_eq!(task.status, TaskStatus::Running);
    assert!(task.started_at.is_some());
}

#[tokio::test]
async fn dedup_prevents_duplicate_key() {
    let store = test_store().await;
    let sub = make_submission("dup-key", Priority::NORMAL);

    let first = store.submit(&sub).await.unwrap();
    assert!(first.is_inserted());

    let second = store.submit(&sub).await.unwrap();
    assert_eq!(second, SubmitOutcome::Duplicate);
}

#[tokio::test]
async fn dedup_upgrades_priority() {
    let store = test_store().await;

    let sub_normal = make_submission("upgrade-me", Priority::NORMAL);
    let first = store.submit(&sub_normal).await.unwrap();
    assert!(first.is_inserted());

    let sub_high = make_submission("upgrade-me", Priority::HIGH);
    let second = store.submit(&sub_high).await.unwrap();
    assert!(matches!(second, SubmitOutcome::Upgraded(_)));

    let key = sub_normal.effective_key();
    let task = store.task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(task.priority, Priority::HIGH);

    let sub_bg = make_submission("upgrade-me", Priority::BACKGROUND);
    let third = store.submit(&sub_bg).await.unwrap();
    assert_eq!(third, SubmitOutcome::Duplicate);

    let task = store.task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(task.priority, Priority::HIGH);
}

#[tokio::test]
async fn dedup_requeues_when_running() {
    let store = test_store().await;

    let sub = make_submission("running-task", Priority::NORMAL);
    store.submit(&sub).await.unwrap();
    let task = store.pop_next(None).await.unwrap().unwrap();

    let sub_high = make_submission("running-task", Priority::HIGH);
    let outcome = store.submit(&sub_high).await.unwrap();
    assert!(matches!(outcome, SubmitOutcome::Requeued(_)));

    let key = sub.effective_key();
    let running = store.task_by_key(&key).await.unwrap().unwrap();
    assert!(running.requeue);
    assert_eq!(running.requeue_priority, Some(Priority::HIGH));

    store.complete(task.id, &IoBudget::default()).await.unwrap();

    let requeued = store.task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(requeued.status, crate::task::TaskStatus::Pending);
    assert_eq!(requeued.priority, Priority::HIGH);
    assert!(!requeued.requeue);
    assert_eq!(requeued.requeue_priority, None);

    let popped = store.pop_next(None).await.unwrap().unwrap();
    assert_eq!(popped.id, task.id);
}

#[tokio::test]
async fn dedup_requeue_already_requeued_same_priority() {
    let store = test_store().await;

    let sub = make_submission("rq-dup", Priority::NORMAL);
    store.submit(&sub).await.unwrap();
    store.pop_next(None).await.unwrap();

    let sub_high = make_submission("rq-dup", Priority::HIGH);
    let outcome = store.submit(&sub_high).await.unwrap();
    assert!(matches!(outcome, SubmitOutcome::Requeued(_)));

    let outcome2 = store.submit(&sub_high).await.unwrap();
    assert_eq!(outcome2, SubmitOutcome::Duplicate);
}

#[tokio::test]
async fn dedup_requeue_upgrades_priority() {
    let store = test_store().await;

    let sub = make_submission("rq-upgrade", Priority::BACKGROUND);
    store.submit(&sub).await.unwrap();
    store.pop_next(None).await.unwrap();

    let sub_normal = make_submission("rq-upgrade", Priority::NORMAL);
    let outcome = store.submit(&sub_normal).await.unwrap();
    assert!(matches!(outcome, SubmitOutcome::Requeued(_)));

    let sub_high = make_submission("rq-upgrade", Priority::HIGH);
    let outcome2 = store.submit(&sub_high).await.unwrap();
    assert!(matches!(outcome2, SubmitOutcome::Requeued(_)));

    let key = sub.effective_key();
    let task = store.task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(task.requeue_priority, Some(Priority::HIGH));
}

#[tokio::test]
async fn permanent_failure_drops_requeue() {
    let store = test_store().await;

    let sub = make_submission("fail-rq", Priority::NORMAL);
    store.submit(&sub).await.unwrap();
    let task = store.pop_next(None).await.unwrap().unwrap();

    let sub_high = make_submission("fail-rq", Priority::HIGH);
    store.submit(&sub_high).await.unwrap();

    store
        .fail(
            task.id,
            "boom",
            false,
            0,
            &IoBudget::default(),
            &Default::default(),
        )
        .await
        .unwrap();

    let outcome = store.submit(&sub).await.unwrap();
    assert!(outcome.is_inserted());
}

#[tokio::test]
async fn dedup_allows_same_key_different_types() {
    let store = test_store().await;

    let sub_a = TaskSubmission::new("type_a").key("shared-key");
    let sub_b = TaskSubmission::new("type_b").key("shared-key");

    let first = store.submit(&sub_a).await.unwrap();
    assert!(first.is_inserted());

    let second = store.submit(&sub_b).await.unwrap();
    assert!(second.is_inserted());
}

#[tokio::test]
async fn dedup_by_payload_when_no_key() {
    let store = test_store().await;

    let sub = TaskSubmission::new("ingest").payload_raw(b"same-data".to_vec());

    let first = store.submit(&sub).await.unwrap();
    assert!(first.is_inserted());

    let second = store.submit(&sub).await.unwrap();
    assert_eq!(second, SubmitOutcome::Duplicate);

    let sub2 = TaskSubmission::new("ingest").payload_raw(b"different-data".to_vec());
    let third = store.submit(&sub2).await.unwrap();
    assert!(third.is_inserted());
}

#[tokio::test]
async fn payload_size_limit() {
    use crate::store::StoreError;
    let store = test_store().await;
    let mut sub = make_submission("big", Priority::NORMAL);
    sub.payload = Some(vec![0u8; MAX_PAYLOAD_BYTES + 1]);

    let err = store.submit(&sub).await.unwrap_err();
    assert!(matches!(err, StoreError::PayloadTooLarge));
}

#[tokio::test]
async fn submit_batch_inserts_all() {
    let store = test_store().await;
    let subs: Vec<_> = (0..5)
        .map(|i| make_submission(&format!("batch-{i}"), Priority::NORMAL))
        .collect();

    let results = store.submit_batch(&subs).await.unwrap();
    assert_eq!(results.len(), 5);
    assert!(results.iter().all(|r| r.is_inserted()));

    let count = store.pending_count().await.unwrap();
    assert_eq!(count, 5);
}

#[tokio::test]
async fn submit_batch_dedup() {
    let store = test_store().await;
    let sub = make_submission("dup", Priority::NORMAL);

    // Intra-batch dedup: last-wins, so the first is Duplicate and the
    // second (last occurrence) is Inserted.
    let results = store
        .submit_batch(&[sub.clone(), sub.clone()])
        .await
        .unwrap();
    assert_eq!(results[0], SubmitOutcome::Duplicate);
    assert!(results[1].is_inserted());

    // Re-submitting the same key hits the DB-level dedup.
    let results = store.submit_batch(&[sub]).await.unwrap();
    assert_eq!(results[0], SubmitOutcome::Duplicate);
}

#[tokio::test]
async fn submit_batch_empty() {
    let store = test_store().await;
    let results = store.submit_batch(&[]).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn submit_batch_intra_dedup_last_wins() {
    let store = test_store().await;

    // Two tasks with the same dedup key but different priorities.
    // Last-wins: the second task (HIGH) should be inserted, first skipped.
    let sub_normal = make_submission("same-key", Priority::NORMAL);
    let sub_high = make_submission("same-key", Priority::HIGH);

    let results = store
        .submit_batch(&[sub_normal.clone(), sub_high.clone()])
        .await
        .unwrap();
    assert_eq!(results[0], SubmitOutcome::Duplicate);
    assert!(results[1].is_inserted());

    // Verify the stored task has the second task's priority.
    let key = sub_normal.effective_key();
    let task = store.task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(task.priority, Priority::HIGH);
}

#[tokio::test]
async fn submit_batch_large_chunking() {
    use super::BATCH_CHUNK_SIZE;

    let store = test_store().await;
    let count = BATCH_CHUNK_SIZE + 100;
    let subs: Vec<_> = (0..count)
        .map(|i| make_submission(&format!("chunk-{i}"), Priority::NORMAL))
        .collect();

    let results = store.submit_batch(&subs).await.unwrap();
    assert_eq!(results.len(), count);
    assert!(results.iter().all(|r| r.is_inserted()));

    let pending = store.pending_count().await.unwrap();
    assert_eq!(pending, count as i64);
}

#[tokio::test]
async fn submit_batch_rejects_oversized_payload() {
    use crate::store::StoreError;
    let store = test_store().await;
    let sub = make_submission("ok", Priority::NORMAL);
    let big = TaskSubmission::new("test")
        .key("big")
        .payload_raw(vec![0u8; MAX_PAYLOAD_BYTES + 1]);

    let err = store.submit_batch(&[sub.clone(), big]).await.unwrap_err();
    assert!(matches!(err, StoreError::PayloadTooLarge));

    let count = store.pending_count().await.unwrap();
    assert_eq!(count, 0);
}

// ── Tag tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn submit_with_tags() {
    let store = test_store().await;
    let sub = TaskSubmission::new("test")
        .key("tagged-1")
        .tag("profile", "default")
        .tag("source", "upload");

    let outcome = store.submit(&sub).await.unwrap();
    let id = outcome.id().unwrap();

    let task = store.task_by_id(id).await.unwrap().unwrap();
    assert_eq!(task.tags.len(), 2);
    assert_eq!(task.tags.get("profile").unwrap(), "default");
    assert_eq!(task.tags.get("source").unwrap(), "upload");
}

#[tokio::test]
async fn submit_batch_with_tags() {
    let store = test_store().await;
    let subs: Vec<_> = (0..3)
        .map(|i| {
            TaskSubmission::new("test")
                .key(format!("batch-tag-{i}"))
                .tag("batch", "true")
                .tag("index", i.to_string())
        })
        .collect();

    let results = store.submit_batch(&subs).await.unwrap();
    assert!(results.iter().all(|r| r.is_inserted()));

    for (i, result) in results.iter().enumerate() {
        let task = store
            .task_by_id(result.id().unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(task.tags.get("batch").unwrap(), "true");
        assert_eq!(task.tags.get("index").unwrap(), &i.to_string());
    }
}

#[tokio::test]
async fn submit_with_default_tags() {
    use crate::task::BatchSubmission;

    let store = test_store().await;
    let subs = BatchSubmission::new()
        .default_tag("env", "prod")
        .default_tag("region", "us-east")
        .task(TaskSubmission::new("test").key("dt-1"))
        .task(
            TaskSubmission::new("test")
                .key("dt-2")
                .tag("region", "eu-west"),
        )
        .build();

    let results = store.submit_batch(&subs).await.unwrap();

    // First task gets both defaults.
    let t1 = store
        .task_by_id(results[0].id().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(t1.tags.get("env").unwrap(), "prod");
    assert_eq!(t1.tags.get("region").unwrap(), "us-east");

    // Second task overrides "region" but inherits "env".
    let t2 = store
        .task_by_id(results[1].id().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(t2.tags.get("env").unwrap(), "prod");
    assert_eq!(t2.tags.get("region").unwrap(), "eu-west");
}

#[tokio::test]
async fn tags_validation_key_too_long() {
    use crate::store::StoreError;
    use crate::task::MAX_TAG_KEY_LEN;

    let store = test_store().await;
    let long_key = "x".repeat(MAX_TAG_KEY_LEN + 1);
    let sub = TaskSubmission::new("test")
        .key("bad-key")
        .tag(long_key, "value");

    let err = store.submit(&sub).await.unwrap_err();
    assert!(matches!(err, StoreError::InvalidTag(_)));
}

#[tokio::test]
async fn tags_validation_value_too_long() {
    use crate::store::StoreError;
    use crate::task::MAX_TAG_VALUE_LEN;

    let store = test_store().await;
    let long_val = "x".repeat(MAX_TAG_VALUE_LEN + 1);
    let sub = TaskSubmission::new("test")
        .key("bad-val")
        .tag("key", long_val);

    let err = store.submit(&sub).await.unwrap_err();
    assert!(matches!(err, StoreError::InvalidTag(_)));
}

#[tokio::test]
async fn tags_validation_too_many() {
    use crate::store::StoreError;
    use crate::task::MAX_TAGS_PER_TASK;

    let store = test_store().await;
    let mut sub = TaskSubmission::new("test").key("too-many");
    for i in 0..=MAX_TAGS_PER_TASK {
        sub = sub.tag(format!("key-{i}"), "value");
    }

    let err = store.submit(&sub).await.unwrap_err();
    assert!(matches!(err, StoreError::InvalidTag(_)));
}

#[tokio::test]
async fn tags_preserved_on_supersede() {
    use crate::task::DuplicateStrategy;

    let store = test_store().await;
    let sub1 = TaskSubmission::new("test")
        .key("supersede-tags")
        .tag("version", "1")
        .on_duplicate(DuplicateStrategy::Supersede);
    let id1 = store.submit(&sub1).await.unwrap().id().unwrap();

    let t1 = store.task_by_id(id1).await.unwrap().unwrap();
    assert_eq!(t1.tags.get("version").unwrap(), "1");

    // Supersede with new tags.
    let sub2 = TaskSubmission::new("test")
        .key("supersede-tags")
        .tag("version", "2")
        .tag("extra", "yes")
        .on_duplicate(DuplicateStrategy::Supersede);
    let outcome = store.submit(&sub2).await.unwrap();
    let id2 = outcome.id().unwrap();

    let t2 = store.task_by_id(id2).await.unwrap().unwrap();
    assert_eq!(t2.tags.get("version").unwrap(), "2");
    assert_eq!(t2.tags.get("extra").unwrap(), "yes");
}

#[tokio::test]
async fn tags_dedup_no_change() {
    let store = test_store().await;
    let sub = TaskSubmission::new("test")
        .key("dedup-tags")
        .tag("env", "prod");

    store.submit(&sub).await.unwrap();
    let outcome = store.submit(&sub).await.unwrap();
    assert_eq!(outcome, SubmitOutcome::Duplicate);

    // Tags should still be intact on the original task.
    let key = sub.effective_key();
    let task = store.task_by_key(&key).await.unwrap().unwrap();
    assert_eq!(task.tags.get("env").unwrap(), "prod");
}
