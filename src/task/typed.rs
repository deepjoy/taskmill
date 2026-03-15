//! The [`TypedTask`] trait for strongly-typed task payloads.

use std::collections::HashMap;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::priority::Priority;

use super::submission::{DuplicateStrategy, RecurringSchedule};
use super::{IoBudget, TtlFrom};

/// A strongly-typed task that bundles serialization, task type name, and default
/// IO estimates.
///
/// Implementing this trait collapses the fields of [`TaskSubmission`](super::TaskSubmission) into a
/// derive-friendly pattern. Use [`Scheduler::submit_typed`](crate::Scheduler::submit_typed)
/// to submit and [`TaskContext::payload`](crate::TaskContext::payload) on the
/// executor side to deserialize. Each `TypedTask` must have a corresponding
/// [`TaskExecutor`](crate::TaskExecutor) registered under the same
/// [`TASK_TYPE`](Self::TASK_TYPE) name.
///
/// # Example
///
/// ```ignore
/// use std::collections::HashMap;
/// use serde::{Serialize, Deserialize};
/// use taskmill::{TypedTask, IoBudget, Priority};
///
/// #[derive(Serialize, Deserialize)]
/// struct Thumbnail { path: String, size: u32, profile: String }
///
/// impl TypedTask for Thumbnail {
///     const TASK_TYPE: &'static str = "thumbnail";
///     fn expected_io(&self) -> IoBudget { IoBudget::disk(4096, 1024) }
///     fn tags(&self) -> HashMap<String, String> {
///         HashMap::from([("profile".into(), self.profile.clone())])
///     }
/// }
/// ```
pub trait TypedTask: Serialize + DeserializeOwned + Send + 'static {
    /// Unique name used to register and look up the executor.
    const TASK_TYPE: &'static str;

    /// Expected IO budget for this task. Default: zero.
    fn expected_io(&self) -> IoBudget {
        IoBudget::default()
    }

    /// Scheduling priority. Default: [`Priority::NORMAL`].
    fn priority(&self) -> Priority {
        Priority::NORMAL
    }

    /// Optional dedup key. Default: `None` (payload hash used).
    fn key(&self) -> Option<String> {
        None
    }

    /// Optional human-readable label. Default: `None` (derived from key or task type).
    fn label(&self) -> Option<String> {
        None
    }

    /// Optional group key for per-group concurrency limiting. Default: `None`.
    fn group_key(&self) -> Option<String> {
        None
    }

    /// Duplicate-handling strategy. Default: [`DuplicateStrategy::Skip`].
    fn on_duplicate(&self) -> DuplicateStrategy {
        DuplicateStrategy::default()
    }

    /// Optional time-to-live. Default: `None` (no TTL).
    fn ttl(&self) -> Option<Duration> {
        None
    }

    /// When the TTL clock starts. Default: [`TtlFrom::Submission`].
    fn ttl_from(&self) -> TtlFrom {
        TtlFrom::default()
    }

    /// Optional initial delay before first dispatch. Default: `None`.
    fn run_after(&self) -> Option<Duration> {
        None
    }

    /// Optional recurring schedule. Default: `None` (one-shot).
    fn recurring(&self) -> Option<RecurringSchedule> {
        None
    }

    /// Metadata tags for filtering and grouping. Default: empty.
    fn tags(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}
