use serde::{Deserialize, Serialize};

use crate::domain::DomainKey;
use crate::priority::Priority;

use super::{BatchSubmission, IoBudget, TaskSubmission, TypedTask};

// Test-only domain key used by all test TypedTask impls in this module.
struct TestDomain;
impl DomainKey for TestDomain {
    const NAME: &'static str = "test";
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Thumbnail {
    path: String,
    size: u32,
}

impl TypedTask for Thumbnail {
    type Domain = TestDomain;
    const TASK_TYPE: &'static str = "thumbnail";

    fn config() -> crate::domain::TaskTypeConfig {
        crate::domain::TaskTypeConfig::new().expected_io(IoBudget::disk(4096, 1024))
    }
}

#[test]
fn typed_task_to_submission() {
    let task = Thumbnail {
        path: "/photos/a.jpg".into(),
        size: 256,
    };
    let sub = TaskSubmission::from_typed(&task);

    assert_eq!(sub.task_type, "thumbnail");
    assert_eq!(sub.expected_io.disk_read, 4096);
    assert_eq!(sub.expected_io.disk_write, 1024);
    assert!(sub.dedup_key.is_none());

    // Payload round-trips correctly.
    let recovered: Thumbnail = serde_json::from_slice(sub.payload.as_ref().unwrap()).unwrap();
    assert_eq!(recovered, task);
}

#[test]
fn typed_task_custom_priority() {
    #[derive(Serialize, Deserialize)]
    struct Urgent {
        id: u64,
    }

    impl TypedTask for Urgent {
        type Domain = TestDomain;
        const TASK_TYPE: &'static str = "urgent";

        fn config() -> crate::domain::TaskTypeConfig {
            crate::domain::TaskTypeConfig::new().priority(Priority::HIGH)
        }
    }

    let sub = TaskSubmission::from_typed(&Urgent { id: 42 });
    assert_eq!(sub.priority, Priority::HIGH);
    assert_eq!(sub.task_type, "urgent");
}

#[test]
fn typed_task_defaults() {
    #[derive(Serialize, Deserialize)]
    struct Minimal;

    impl TypedTask for Minimal {
        type Domain = TestDomain;
        const TASK_TYPE: &'static str = "minimal";
    }

    let sub = TaskSubmission::from_typed(&Minimal);
    assert_eq!(sub.expected_io, IoBudget::default());
    assert_eq!(sub.priority, Priority::NORMAL);
    assert!(sub.group_key.is_none());
}

#[test]
fn typed_task_with_network_and_group() {
    #[derive(Serialize, Deserialize)]
    struct S3Upload {
        bucket: String,
        size: i64,
    }

    impl TypedTask for S3Upload {
        type Domain = TestDomain;
        const TASK_TYPE: &'static str = "s3-upload";

        fn config() -> crate::domain::TaskTypeConfig {
            // Note: expected_io and group_key were instance methods but are now
            // static config. For payload-dependent values, use key()/tags().
            crate::domain::TaskTypeConfig::new()
                .expected_io(IoBudget::net(0, 10_000_000))
                .group("s3://default-bucket".to_string())
        }
    }

    let task = S3Upload {
        bucket: "my-bucket".into(),
        size: 10_000_000,
    };
    let sub = TaskSubmission::from_typed(&task);
    assert_eq!(sub.expected_io.net_tx, 10_000_000);
    assert_eq!(sub.expected_io.net_rx, 0);
    // Group is now a static config, not payload-dependent.
    assert_eq!(sub.group_key.as_deref(), Some("s3://default-bucket"));
}

#[test]
fn submission_builder_io_and_group() {
    let sub = TaskSubmission::new("upload")
        .expected_io(IoBudget {
            net_rx: 5000,
            net_tx: 10000,
            ..Default::default()
        })
        .group("s3://bucket-a");
    assert_eq!(sub.expected_io.net_rx, 5000);
    assert_eq!(sub.expected_io.net_tx, 10000);
    assert_eq!(sub.group_key.as_deref(), Some("s3://bucket-a"));
}

#[test]
fn typed_task_key_and_label() {
    #[derive(Serialize, Deserialize)]
    struct FileTask {
        path: String,
    }

    impl TypedTask for FileTask {
        type Domain = TestDomain;
        const TASK_TYPE: &'static str = "file-task";

        fn key(&self) -> Option<String> {
            Some(self.path.clone())
        }

        fn label(&self) -> Option<String> {
            Some(format!("Process {}", self.path))
        }
    }

    let task = FileTask {
        path: "/a.txt".into(),
    };
    let sub = TaskSubmission::from_typed(&task);
    assert_eq!(sub.dedup_key.as_deref(), Some("/a.txt"));
    assert_eq!(sub.label, "Process /a.txt");
}

#[test]
fn batch_submission_builder_defaults() {
    let subs = BatchSubmission::new()
        .default_group("g1")
        .default_priority(Priority::HIGH)
        .task(TaskSubmission::new("test").key("a"))
        .task(
            TaskSubmission::new("test")
                .key("b")
                .priority(Priority::REALTIME),
        )
        .task(TaskSubmission::new("test").key("c").group("custom-group"))
        .build();

    assert_eq!(subs.len(), 3);

    // Task without explicit group/priority gets defaults.
    assert_eq!(subs[0].group_key.as_deref(), Some("g1"));
    assert_eq!(subs[0].priority, Priority::HIGH);

    // Task with explicit priority (non-NORMAL) keeps its own.
    assert_eq!(subs[1].group_key.as_deref(), Some("g1"));
    assert_eq!(subs[1].priority, Priority::REALTIME);

    // Task with explicit group keeps its own.
    assert_eq!(subs[2].group_key.as_deref(), Some("custom-group"));
    assert_eq!(subs[2].priority, Priority::HIGH);
}

#[test]
fn batch_submission_builder_no_defaults() {
    let subs = BatchSubmission::new()
        .task(TaskSubmission::new("test").key("a"))
        .build();

    assert_eq!(subs.len(), 1);
    assert!(subs[0].group_key.is_none());
    assert_eq!(subs[0].priority, Priority::NORMAL);
}

#[test]
fn typed_task_with_tags() {
    use std::collections::HashMap;

    #[derive(Serialize, Deserialize)]
    struct TaggedTask {
        profile: String,
    }

    impl TypedTask for TaggedTask {
        type Domain = TestDomain;
        const TASK_TYPE: &'static str = "tagged";

        fn tags(&self) -> HashMap<String, String> {
            HashMap::from([("profile".into(), self.profile.clone())])
        }
    }

    let task = TaggedTask {
        profile: "default".into(),
    };
    let sub = TaskSubmission::from_typed(&task);
    assert_eq!(sub.tags.get("profile").unwrap(), "default");
}

#[test]
fn event_header_includes_tags() {
    use std::collections::HashMap;

    let mut record = super::TaskRecord {
        id: 42,
        task_type: "test".into(),
        key: "abc".into(),
        label: "Test task".into(),
        priority: Priority::NORMAL,
        status: super::TaskStatus::Running,
        payload: None,
        expected_io: IoBudget::default(),
        retry_count: 0,
        last_error: None,
        created_at: chrono::Utc::now(),
        started_at: Some(chrono::Utc::now()),
        parent_id: None,
        fail_fast: true,
        requeue: false,
        requeue_priority: None,
        group_key: None,
        ttl_seconds: None,
        ttl_from: super::TtlFrom::Submission,
        expires_at: None,
        run_after: None,
        recurring_interval_secs: None,
        recurring_max_executions: None,
        recurring_execution_count: 0,
        recurring_paused: false,
        tags: HashMap::new(),
        dependencies: Vec::new(),
        on_dependency_failure: super::submission::DependencyFailurePolicy::Cancel,
        max_retries: None,
        memo: None,
        pause_reasons: super::PauseReasons::NONE,
        pause_duration_ms: 0,
        paused_at_ms: None,
    };
    record.tags.insert("env".into(), "prod".into());
    record.tags.insert("owner".into(), "alice".into());

    let header = record.event_header();
    assert_eq!(header.task_id, 42);
    assert_eq!(header.tags.get("env").unwrap(), "prod");
    assert_eq!(header.tags.get("owner").unwrap(), "alice");
    assert_eq!(header.tags.len(), 2);
}
