use serde::{Deserialize, Serialize};

use crate::priority::Priority;

use super::{BatchSubmission, IoBudget, TaskSubmission, TypedTask};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Thumbnail {
    path: String,
    size: u32,
}

impl TypedTask for Thumbnail {
    const TASK_TYPE: &'static str = "thumbnail";

    fn expected_io(&self) -> IoBudget {
        IoBudget::disk(4096, 1024)
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
    assert_eq!(sub.priority, Priority::NORMAL);
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
        const TASK_TYPE: &'static str = "urgent";

        fn priority(&self) -> Priority {
            Priority::HIGH
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
        const TASK_TYPE: &'static str = "s3-upload";

        fn expected_io(&self) -> IoBudget {
            IoBudget::net(0, self.size)
        }

        fn group_key(&self) -> Option<String> {
            Some(format!("s3://{}", self.bucket))
        }
    }

    let task = S3Upload {
        bucket: "my-bucket".into(),
        size: 10_000_000,
    };
    let sub = TaskSubmission::from_typed(&task);
    assert_eq!(sub.expected_io.net_tx, 10_000_000);
    assert_eq!(sub.expected_io.net_rx, 0);
    assert_eq!(sub.group_key.as_deref(), Some("s3://my-bucket"));
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
        .task(TaskSubmission::new("test").key("b").priority(Priority::REALTIME))
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
