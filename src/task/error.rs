//! Task error types for executor failure reporting.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Reported by the executor on failure.
///
/// The scheduler uses the [`retryable`](Self::retryable) flag to decide
/// whether to requeue the task or move it to history as permanently failed:
///
/// - **Non-retryable** ([`TaskError::new`] / [`TaskError::permanent`]): the
///   task moves directly to the history table with status `failed`. Use this
///   for logic errors, invalid payloads, or conditions that won't change on retry.
/// - **Retryable** ([`TaskError::retryable`]): the task is requeued as
///   `pending` with an incremented retry count, keeping the same priority.
///   After [`SchedulerConfig::max_retries`](crate::SchedulerConfig::max_retries)
///   attempts (default 3), the task fails permanently.
///
/// Retryable errors can optionally include a [`retry_after`](Self::retry_after)
/// delay to override the backoff strategy's computed delay for a single retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskError {
    pub message: String,
    pub retryable: bool,
    /// Whether this error represents a cancellation (not a real failure).
    #[serde(default)]
    pub cancelled: bool,
    /// Executor-requested retry delay in milliseconds. When set, overrides the
    /// backoff strategy's computed delay for this single retry. Useful for
    /// respecting `Retry-After` headers from upstream services.
    #[serde(default)]
    pub retry_after_ms: Option<u64>,
}

impl TaskError {
    /// Create a **non-retryable** error. The task will fail permanently and
    /// move to the history table.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: false,
            cancelled: false,
            retry_after_ms: None,
        }
    }

    /// Create a **permanent** (non-retryable) error. The task will fail
    /// immediately and move to history, skipping any remaining retries.
    ///
    /// Alias for [`TaskError::new`] — use whichever reads better at the
    /// call site.
    pub fn permanent(message: impl Into<String>) -> Self {
        Self::new(message)
    }

    /// Create a **retryable** error. The task will be requeued as pending
    /// and retried up to [`SchedulerConfig::max_retries`](crate::SchedulerConfig::max_retries)
    /// times before failing permanently.
    pub fn retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
            cancelled: false,
            retry_after_ms: None,
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
            retry_after_ms: None,
        }
    }

    /// Returns `true` if this error represents a cancellation.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// Request that the scheduler wait at least `delay` before retrying
    /// this task. Overrides the type's backoff strategy for this single
    /// retry attempt.
    pub fn retry_after(mut self, delay: Duration) -> Self {
        self.retry_after_ms = Some(delay.as_millis() as u64);
        self
    }

    /// Returns the executor-requested retry delay, if any.
    pub fn retry_delay(&self) -> Option<Duration> {
        self.retry_after_ms.map(Duration::from_millis)
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
            retry_after_ms: None,
        }
    }
}

impl From<&str> for TaskError {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_string(),
            retryable: false,
            cancelled: false,
            retry_after_ms: None,
        }
    }
}

impl From<serde_json::Error> for TaskError {
    fn from(e: serde_json::Error) -> Self {
        Self {
            message: e.to_string(),
            retryable: false,
            cancelled: false,
            retry_after_ms: None,
        }
    }
}

impl From<crate::store::StoreError> for TaskError {
    fn from(e: crate::store::StoreError) -> Self {
        Self {
            message: e.to_string(),
            retryable: false,
            cancelled: false,
            retry_after_ms: None,
        }
    }
}
