use crate::priority::Priority;
use crate::task::{HistoryStatus, IoBudget, TaskLookup, TaskStatus, TaskSubmission};

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
async fn task_by_id_lookup() {
    let store = test_store().await;
    let sub = make_submission("by-id", Priority::NORMAL);
    let id = store.submit(&sub).await.unwrap().id().unwrap();

    let task = store.task_by_id(id).await.unwrap().unwrap();
    assert_eq!(task.id, id);
    assert_eq!(task.key, sub.effective_key());

    assert!(store.task_by_id(9999).await.unwrap().is_none());
}

#[tokio::test]
async fn history_by_id_lookup() {
    let store = test_store().await;
    let sub = make_submission("hist-id", Priority::NORMAL);
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();

    store
        .complete(task.id, &IoBudget::disk(100, 50))
        .await
        .unwrap();

    let hist = store.history_by_key(&sub.effective_key()).await.unwrap();
    assert_eq!(hist.len(), 1);
    let hist_id = hist[0].id;

    let record = store.history_by_id(hist_id).await.unwrap().unwrap();
    assert_eq!(record.key, sub.effective_key());
    assert_eq!(record.actual_io.unwrap().disk_read, 100);

    assert!(store.history_by_id(9999).await.unwrap().is_none());
}

#[tokio::test]
async fn history_stats_computation() {
    let store = test_store().await;

    for i in 0..3 {
        let sub = make_submission(&format!("stat-{i}"), Priority::NORMAL);
        store.submit(&sub).await.unwrap();
        let task = store.pop_next().await.unwrap().unwrap();
        store
            .complete(task.id, &IoBudget::disk(1000, 500))
            .await
            .unwrap();
    }

    let stats = store.history_stats("test").await.unwrap();
    assert_eq!(stats.count, 3);
    assert!(stats.failure_rate == 0.0);
}

#[tokio::test]
async fn open_with_custom_config() {
    let store = TaskStore::open_memory().await.unwrap();
    let count = store.pending_count().await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn delete_task() {
    let store = test_store().await;
    let sub = make_submission("del-me", Priority::NORMAL);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();

    let task = store.task_by_key(&key).await.unwrap().unwrap();
    assert!(store.delete(task.id).await.unwrap());
    assert!(store.task_by_key(&key).await.unwrap().is_none());

    assert!(!store.delete(task.id).await.unwrap());
}

#[tokio::test]
async fn task_lookup_active() {
    let store = test_store().await;
    let sub = make_submission("lookup-active", Priority::NORMAL);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();

    let result = store.task_lookup(&key).await.unwrap();
    assert!(matches!(result, TaskLookup::Active(ref r) if r.status == TaskStatus::Pending));

    store.pop_next().await.unwrap();
    let result = store.task_lookup(&key).await.unwrap();
    assert!(matches!(result, TaskLookup::Active(ref r) if r.status == TaskStatus::Running));
}

#[tokio::test]
async fn task_lookup_history() {
    let store = test_store().await;
    let sub = make_submission("lookup-hist", Priority::NORMAL);
    let key = sub.effective_key();
    store.submit(&sub).await.unwrap();
    let task = store.pop_next().await.unwrap().unwrap();
    store.complete(task.id, &IoBudget::default()).await.unwrap();

    let result = store.task_lookup(&key).await.unwrap();
    assert!(matches!(result, TaskLookup::History(ref r) if r.status == HistoryStatus::Completed));
}

#[tokio::test]
async fn task_lookup_not_found() {
    let store = test_store().await;
    let key = crate::task::generate_dedup_key("nope", Some(b"nope"));
    let result = store.task_lookup(&key).await.unwrap();
    assert!(matches!(result, TaskLookup::NotFound));
}

#[tokio::test]
async fn prune_by_count() {
    let store = test_store().await;

    for i in 0..5 {
        let sub = make_submission(&format!("prune-{i}"), Priority::NORMAL);
        store.submit(&sub).await.unwrap();
        let task = store.pop_next().await.unwrap().unwrap();
        store.complete(task.id, &IoBudget::default()).await.unwrap();
    }

    let hist = store.history(100, 0).await.unwrap();
    assert_eq!(hist.len(), 5);

    let deleted = store.prune_history_by_count(3).await.unwrap();
    assert_eq!(deleted, 2);

    let hist = store.history(100, 0).await.unwrap();
    assert_eq!(hist.len(), 3);
}

// ── Tag query tests ───────────────────────────────────────────────

#[tokio::test]
async fn tasks_by_tags_single_filter() {
    let store = test_store().await;

    store
        .submit(&TaskSubmission::new("test").key("tbt-1").tag("env", "prod"))
        .await
        .unwrap();
    store
        .submit(
            &TaskSubmission::new("test")
                .key("tbt-2")
                .tag("env", "staging"),
        )
        .await
        .unwrap();
    store
        .submit(&TaskSubmission::new("test").key("tbt-3").tag("env", "prod"))
        .await
        .unwrap();

    let results = store.tasks_by_tags(&[("env", "prod")], None).await.unwrap();
    assert_eq!(results.len(), 2);

    let results = store
        .tasks_by_tags(&[("env", "staging")], None)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn tasks_by_tags_multiple_filters_and() {
    let store = test_store().await;

    store
        .submit(
            &TaskSubmission::new("test")
                .key("multi-1")
                .tag("env", "prod")
                .tag("region", "us"),
        )
        .await
        .unwrap();
    store
        .submit(
            &TaskSubmission::new("test")
                .key("multi-2")
                .tag("env", "prod")
                .tag("region", "eu"),
        )
        .await
        .unwrap();
    store
        .submit(
            &TaskSubmission::new("test")
                .key("multi-3")
                .tag("env", "staging")
                .tag("region", "us"),
        )
        .await
        .unwrap();

    // AND semantics: only task matching both filters.
    let results = store
        .tasks_by_tags(&[("env", "prod"), ("region", "us")], None)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);

    // With status filter.
    let results = store
        .tasks_by_tags(&[("env", "prod")], Some(TaskStatus::Pending))
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn count_by_tag_groups() {
    let store = test_store().await;

    for i in 0..3 {
        store
            .submit(
                &TaskSubmission::new("test")
                    .key(format!("free-{i}"))
                    .tag("tier", "free"),
            )
            .await
            .unwrap();
    }
    for i in 0..2 {
        store
            .submit(
                &TaskSubmission::new("test")
                    .key(format!("pro-{i}"))
                    .tag("tier", "pro"),
            )
            .await
            .unwrap();
    }

    let groups = store.count_by_tag("tier", None).await.unwrap();
    assert_eq!(groups.len(), 2);
    // Sorted by count descending.
    assert_eq!(groups[0].0, "free");
    assert_eq!(groups[0].1, 3);
    assert_eq!(groups[1].0, "pro");
    assert_eq!(groups[1].1, 2);
}

#[tokio::test]
async fn tag_values_distinct() {
    let store = test_store().await;

    store
        .submit(&TaskSubmission::new("test").key("tv-1").tag("color", "red"))
        .await
        .unwrap();
    store
        .submit(&TaskSubmission::new("test").key("tv-2").tag("color", "red"))
        .await
        .unwrap();
    store
        .submit(&TaskSubmission::new("test").key("tv-3").tag("color", "blue"))
        .await
        .unwrap();

    let values = store.tag_values("color").await.unwrap();
    assert_eq!(values.len(), 2);
    // Sorted by count descending.
    assert_eq!(values[0], ("red".to_string(), 2));
    assert_eq!(values[1], ("blue".to_string(), 1));

    // Non-existent key returns empty.
    let empty = store.tag_values("nonexistent").await.unwrap();
    assert!(empty.is_empty());
}

// ── Tag key prefix query tests ──────────────────────────────────

#[tokio::test]
async fn tag_keys_by_prefix_discovers_keys() {
    let store = test_store().await;

    store
        .submit(
            &TaskSubmission::new("test")
                .key("tkp-1")
                .tag("billing.customer_id", "cust_1")
                .tag("billing.plan", "enterprise"),
        )
        .await
        .unwrap();
    store
        .submit(
            &TaskSubmission::new("test")
                .key("tkp-2")
                .tag("media.pipeline", "transcode"),
        )
        .await
        .unwrap();

    let keys = store.tag_keys_by_prefix("billing.").await.unwrap();
    assert_eq!(keys, vec!["billing.customer_id", "billing.plan"]);

    let keys = store.tag_keys_by_prefix("media.").await.unwrap();
    assert_eq!(keys, vec!["media.pipeline"]);

    // Non-matching prefix returns empty.
    let keys = store.tag_keys_by_prefix("nonexistent.").await.unwrap();
    assert!(keys.is_empty());
}

#[tokio::test]
async fn tasks_by_tag_key_prefix_finds_tasks() {
    let store = test_store().await;

    store
        .submit(
            &TaskSubmission::new("test")
                .key("tp-1")
                .tag("billing.plan", "free"),
        )
        .await
        .unwrap();
    store
        .submit(
            &TaskSubmission::new("test")
                .key("tp-2")
                .tag("billing.customer_id", "cust_1")
                .tag("media.codec", "h265"),
        )
        .await
        .unwrap();
    store
        .submit(
            &TaskSubmission::new("test")
                .key("tp-3")
                .tag("media.codec", "h264"),
        )
        .await
        .unwrap();

    let tasks = store.tasks_by_tag_key_prefix("billing.", None).await.unwrap();
    assert_eq!(tasks.len(), 2); // tp-1 and tp-2

    let tasks = store.tasks_by_tag_key_prefix("media.", None).await.unwrap();
    assert_eq!(tasks.len(), 2); // tp-2 and tp-3
}

#[tokio::test]
async fn tasks_by_tag_key_prefix_with_status_filter() {
    let store = test_store().await;

    store
        .submit(
            &TaskSubmission::new("test")
                .key("ts-1")
                .tag("billing.plan", "free"),
        )
        .await
        .unwrap();

    let tasks = store
        .tasks_by_tag_key_prefix("billing.", Some(TaskStatus::Pending))
        .await
        .unwrap();
    assert_eq!(tasks.len(), 1);

    let tasks = store
        .tasks_by_tag_key_prefix("billing.", Some(TaskStatus::Running))
        .await
        .unwrap();
    assert_eq!(tasks.len(), 0);
}

#[tokio::test]
async fn count_by_tag_key_prefix_counts_distinct_tasks() {
    let store = test_store().await;

    // Task with two billing.* tags should be counted once.
    store
        .submit(
            &TaskSubmission::new("test")
                .key("cp-1")
                .tag("billing.plan", "free")
                .tag("billing.customer_id", "cust_1"),
        )
        .await
        .unwrap();
    store
        .submit(
            &TaskSubmission::new("test")
                .key("cp-2")
                .tag("billing.plan", "pro"),
        )
        .await
        .unwrap();
    store
        .submit(
            &TaskSubmission::new("test")
                .key("cp-3")
                .tag("media.codec", "h265"),
        )
        .await
        .unwrap();

    let count = store.count_by_tag_key_prefix("billing.", None).await.unwrap();
    assert_eq!(count, 2); // cp-1 counted once despite two matching keys

    let count = store.count_by_tag_key_prefix("media.", None).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn tag_keys_by_prefix_empty_prefix_returns_all() {
    let store = test_store().await;

    store
        .submit(
            &TaskSubmission::new("test")
                .key("ep-1")
                .tag("billing.plan", "free")
                .tag("media.codec", "h265"),
        )
        .await
        .unwrap();

    let keys = store.tag_keys_by_prefix("").await.unwrap();
    assert_eq!(keys.len(), 2);
}

#[tokio::test]
async fn tag_keys_by_prefix_escapes_like_wildcards() {
    let store = test_store().await;

    store
        .submit(
            &TaskSubmission::new("test")
                .key("esc-1")
                .tag("a%b.key", "val")
                .tag("a_b.key", "val")
                .tag("axb.key", "val"),
        )
        .await
        .unwrap();

    // "a%b." should only match the literal "a%b.key", not "axb.key"
    let keys = store.tag_keys_by_prefix("a%b.").await.unwrap();
    assert_eq!(keys, vec!["a%b.key"]);

    // "a_b." should only match the literal "a_b.key", not "axb.key"
    let keys = store.tag_keys_by_prefix("a_b.").await.unwrap();
    assert_eq!(keys, vec!["a_b.key"]);
}

// ── Domain-scoped (_with_prefix) tests ──────────────────────────

#[tokio::test]
async fn tag_keys_by_prefix_with_prefix_scopes_to_task_type() {
    let store = test_store().await;

    // Two different task_type prefixes, same tag key prefix.
    store
        .submit(
            &TaskSubmission::new("billing::invoice")
                .key("wp-1")
                .tag("region.zone", "us-east"),
        )
        .await
        .unwrap();
    store
        .submit(
            &TaskSubmission::new("media::transcode")
                .key("wp-2")
                .tag("region.cluster", "gpu-1"),
        )
        .await
        .unwrap();

    // Scoped to "billing::" — only sees billing task's tags.
    let keys = store
        .tag_keys_by_prefix_with_prefix("billing::", "region.")
        .await
        .unwrap();
    assert_eq!(keys, vec!["region.zone"]);

    // Scoped to "media::" — only sees media task's tags.
    let keys = store
        .tag_keys_by_prefix_with_prefix("media::", "region.")
        .await
        .unwrap();
    assert_eq!(keys, vec!["region.cluster"]);
}

#[tokio::test]
async fn tasks_by_tag_key_prefix_with_prefix_scopes_to_task_type() {
    let store = test_store().await;

    store
        .submit(
            &TaskSubmission::new("billing::charge")
                .key("wpt-1")
                .tag("billing.plan", "pro"),
        )
        .await
        .unwrap();
    store
        .submit(
            &TaskSubmission::new("media::encode")
                .key("wpt-2")
                .tag("billing.plan", "free"),
        )
        .await
        .unwrap();

    // Both tasks have billing.* tags, but scoped query returns only the billing module's task.
    let tasks = store
        .tasks_by_tag_key_prefix_with_prefix("billing::", "billing.", None)
        .await
        .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task_type, "billing::charge");
}

#[tokio::test]
async fn count_by_tag_key_prefix_with_prefix_scopes_to_task_type() {
    let store = test_store().await;

    store
        .submit(
            &TaskSubmission::new("billing::charge")
                .key("wpc-1")
                .tag("billing.plan", "pro"),
        )
        .await
        .unwrap();
    store
        .submit(
            &TaskSubmission::new("media::encode")
                .key("wpc-2")
                .tag("billing.plan", "free"),
        )
        .await
        .unwrap();

    let count = store
        .count_by_tag_key_prefix_with_prefix("billing::", "billing.", None)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // Global count sees both.
    let count = store.count_by_tag_key_prefix("billing.", None).await.unwrap();
    assert_eq!(count, 2);
}
