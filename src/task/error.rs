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
}

impl TaskError {
    /// Create a **non-retryable** error. The task will fail permanently and
    /// move to the history table.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: false,
        }
    }

    /// Create a **retryable** error. The task will be requeued as pending
    /// and retried up to [`SchedulerConfig::max_retries`](crate::SchedulerConfig::max_retries)
    /// times before failing permanently.
    pub fn retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
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
        Self::new(message)
    }
}

impl From<&str> for TaskError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

impl From<serde_json::Error> for TaskError {
    fn from(e: serde_json::Error) -> Self {
        Self::new(e.to_string())
    }
}

impl From<crate::store::StoreError> for TaskError {
    fn from(e: crate::store::StoreError) -> Self {
        Self::new(e.to_string())
    }
}
