//! Type-keyed application state shared across task executors.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

/// Type-keyed map of shared application state.
///
/// Multiple state types can be registered (one value per concrete type).
/// Executors retrieve them via [`TaskContext::state::<T>()`](super::TaskContext::state). This is the
/// same pattern used by Axum `Extensions` and Tauri `State`.
///
/// The map supports post-build insertion via [`Scheduler::register_state`](crate::Scheduler::register_state)
/// so that library consumers (e.g. shoebox inside a Tauri app) can inject
/// state after the scheduler has been constructed by the parent.
#[derive(Default)]
pub(crate) struct StateMap {
    inner: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl StateMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a `StateMap` from pre-collected entries.
    pub(crate) fn from_entries(entries: Vec<(TypeId, Arc<dyn Any + Send + Sync>)>) -> Self {
        Self {
            inner: RwLock::new(entries.into_iter().collect()),
        }
    }

    /// Insert a state value. Overwrites any previous value of the same type.
    pub async fn insert<T: Send + Sync + 'static>(&self, value: Arc<T>) {
        self.inner.write().await.insert(TypeId::of::<T>(), value);
    }

    /// Take a lock-free snapshot for use inside a task context.
    pub(crate) async fn snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            entries: self.inner.read().await.clone(),
        }
    }
}

/// Snapshot of state for passing into a [`TaskContext`](super::TaskContext).
///
/// Created by cloning the inner map under the lock once, then used
/// lock-free for the lifetime of the task execution.
#[derive(Clone, Default)]
pub(crate) struct StateSnapshot {
    entries: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl StateSnapshot {
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.entries
            .get(&TypeId::of::<T>())
            .and_then(|arc| arc.downcast_ref::<T>())
    }
}
