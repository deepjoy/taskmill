//! The [`TypedTask`] trait for strongly-typed task payloads.

use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::domain::{DomainKey, TaskTypeConfig};

/// A strongly-typed task that bundles serialization, domain identity,
/// task type name, and static configuration defaults.
///
/// Each `TypedTask` declares which [`Domain`](crate::Domain) it belongs to
/// and provides static configuration via [`config()`](Self::config).
/// Per-instance overrides (key, label, tags) remain as instance methods.
///
/// Register a [`TypedExecutor<T>`](crate::TypedExecutor) for this task type
/// via [`Domain::task::<T>(executor)`](crate::Domain::task).
///
/// # Example
///
/// ```ignore
/// use std::collections::HashMap;
/// use std::time::Duration;
/// use serde::{Serialize, Deserialize};
/// use taskmill::{TypedTask, TaskTypeConfig, IoBudget, Priority, DomainKey, DuplicateStrategy, RetryPolicy};
///
/// pub struct Media;
/// impl DomainKey for Media { const NAME: &'static str = "media"; }
///
/// #[derive(Serialize, Deserialize)]
/// struct Thumbnail { path: String, size: u32, profile: String }
///
/// impl TypedTask for Thumbnail {
///     type Domain = Media;
///     const TASK_TYPE: &'static str = "thumbnail";
///
///     fn config() -> TaskTypeConfig {
///         TaskTypeConfig::new()
///             .priority(Priority::NORMAL)
///             .expected_io(IoBudget::disk(4096, 1024))
///             .ttl(Duration::from_secs(3600))
///             .retry(RetryPolicy::exponential(3, Duration::from_secs(1), Duration::from_secs(60)))
///             .on_duplicate(DuplicateStrategy::Supersede)
///     }
///
///     fn key(&self) -> Option<String> {
///         Some(format!("thumb:{}:{}", self.path, self.size))
///     }
///
///     fn tags(&self) -> HashMap<String, String> {
///         HashMap::from([("profile".into(), self.profile.clone())])
///     }
/// }
/// ```
pub trait TypedTask: Serialize + DeserializeOwned + Send + 'static {
    /// Domain this task belongs to. Required.
    type Domain: DomainKey;

    /// Unique task type name within the domain (without the domain prefix).
    const TASK_TYPE: &'static str;

    /// Static defaults applied to every submission of this task type.
    ///
    /// Replaces the old `TypedTask` instance methods (`priority`, `expected_io`,
    /// etc.) and `Domain::task_with` overrides.
    /// Domain-level defaults still win over this; per-call `submit_with` overrides
    /// win over everything.
    fn config() -> TaskTypeConfig {
        TaskTypeConfig::default()
    }

    // Instance methods for values that depend on payload content:

    /// Dedup key. Default: SHA-256 of the serialized payload.
    fn key(&self) -> Option<String> {
        None
    }

    /// Human-readable label. Default: derived from key or task type.
    fn label(&self) -> Option<String> {
        None
    }

    /// Metadata tags (payload-derived values). Default: empty.
    fn tags(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}
