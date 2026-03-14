//! Task error types for executor failure reporting.

use serde::{Deserialize, Serialize};

/// Reported by the executor on failure.
///
/// The scheduler uses the [`retryable`](Self::retryable) flag to decide
/// whether to requeue the task or move it to history as permanently failed:
///
/// - **Non-retryable** ([`TaskError::new`]): the task moves directly to the
///   history table with status `failed`. Use this for logic errors, invalid
///   payloads, or conditions that won't change on retry.
/// - **Retryable** ([`TaskError::retryable`]): the task is requeued as
///   `pending` with an incremented retry count, keeping the same priority.
///   After [`SchedulerConfig::max_retries`](crate::SchedulerConfig::max_retries)
///   attempts (default 3), the task fails permanently.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskError {
    pub message: String,
    pub retryable: bool,
    /// Whether this error represents a cancellation (not a real failure).
    #[serde(default)]
    pub cancelled: bool,
}

impl TaskError {
    /// Create a **non-retryable** error. The task will fail permanently and
    /// move to the history table.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: false,
            cancelled: false,
        }
    }

    /// Create a **retryable** error. The task will be requeued as pending
    /// and retried up to [`SchedulerConfig::max_retries`](crate::SchedulerConfig::max_retries)
    /// times before failing permanently.
    pub fn retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
            cancelled: false,
        }
    }

    /// Create a **cancellation** error.
    ///
    /// Used by [`TaskContext::check_cancelled`](crate::TaskContext::check_cancelled)
    /// to signal that the task's cancellation token was triggered. The scheduler
    /// treats this differently from a real failure — no retry, and the task is
    /// recorded as `cancelled` in history.
    pub fn cancelled() -> Self {
        Self {
            message: "task cancelled".into(),
            retryable: false,
            cancelled: true,
        }
    }

    /// Returns `true` if this error represents a cancellation.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TaskError {}

impl From<String> for TaskError {
    fn from(message: String) -> Self {
        Self {
            message,
            retryable: false,
            cancelled: false,
        }
    }
}

impl From<&str> for TaskError {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_string(),
            retryable: false,
            cancelled: false,
        }
    }
}

impl From<serde_json::Error> for TaskError {
    fn from(e: serde_json::Error) -> Self {
        Self {
            message: e.to_string(),
            retryable: false,
            cancelled: false,
        }
    }
}

impl From<crate::store::StoreError> for TaskError {
    fn from(e: crate::store::StoreError) -> Self {
        Self {
            message: e.to_string(),
            retryable: false,
            cancelled: false,
        }
    }
}
