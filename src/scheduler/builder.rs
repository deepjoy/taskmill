//! Ergonomic builder for constructing a [`Scheduler`].

use std::any::TypeId;
use std::sync::Arc;

use tokio::time::Duration;

use crate::backpressure::{CompositePressure, ThrottlePolicy};
use crate::priority::Priority;
use crate::registry::TaskExecutor;
use crate::resource::sampler::{SamplerConfig, SmoothedReader};
use crate::resource::{ResourceReader, ResourceSampler};
use crate::store::{StoreConfig, StoreError, TaskStore};
use crate::task::TypedTask;

use super::event::{SchedulerConfig, ShutdownMode};
use super::Scheduler;

/// Ergonomic builder for constructing a [`Scheduler`] with all its dependencies.
///
/// Hides the `Arc<Mutex<...>>` wiring and manages the resource sampler lifecycle.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// use std::sync::Arc;
/// use taskmill::{Scheduler, Priority};
///
/// let scheduler = Scheduler::builder()
///     .store_path("tasks.db")
///     // .executor("scan", Arc::new(my_scan_executor))
///     .max_concurrency(8)
///     .with_resource_monitoring()
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct SchedulerBuilder {
    store_path: Option<String>,
    store_config: StoreConfig,
    store: Option<TaskStore>,
    executors: Vec<(String, Arc<dyn crate::registry::ErasedExecutor>)>,
    config: SchedulerConfig,
    pressure_sources: Vec<Box<dyn crate::backpressure::PressureSource + 'static>>,
    policy: Option<ThrottlePolicy>,
    enable_resource_monitoring: bool,
    custom_sampler: Option<Box<dyn ResourceSampler>>,
    sampler_config: SamplerConfig,
    app_state_entries: Vec<(TypeId, Arc<dyn std::any::Any + Send + Sync>)>,
    bandwidth_limit_bps: Option<f64>,
    default_group_concurrency: usize,
    group_concurrency_overrides: Vec<(String, usize)>,
}

impl SchedulerBuilder {
    /// Create a new builder with default configuration.
    pub fn new() -> Self {
        Self {
            store_path: None,
            store_config: StoreConfig::default(),
            store: None,
            executors: Vec::new(),
            config: SchedulerConfig::default(),
            pressure_sources: Vec::new(),
            policy: None,
            enable_resource_monitoring: false,
            custom_sampler: None,
            sampler_config: SamplerConfig::default(),
            app_state_entries: Vec::new(),
            bandwidth_limit_bps: None,
            default_group_concurrency: 0,
            group_concurrency_overrides: Vec::new(),
        }
    }

    /// Set the SQLite database path. Either this or [`store`](Self::store) must be called.
    pub fn store_path(mut self, path: &str) -> Self {
        self.store_path = Some(path.to_string());
        self
    }

    /// Configure the SQLite connection pool.
    pub fn store_config(mut self, config: StoreConfig) -> Self {
        self.store_config = config;
        self
    }

    /// Use a pre-opened [`TaskStore`] instead of opening one from a path.
    pub fn store(mut self, store: TaskStore) -> Self {
        self.store = Some(store);
        self
    }

    /// Register a task executor for a named type.
    pub fn executor<E: TaskExecutor>(mut self, name: &str, executor: Arc<E>) -> Self {
        self.executors.push((
            name.to_string(),
            executor as Arc<dyn crate::registry::ErasedExecutor>,
        ));
        self
    }

    /// Register an executor using the task type name from a [`TypedTask`].
    ///
    /// Equivalent to `.executor(T::TASK_TYPE, executor)`.
    pub fn typed_executor<T: TypedTask, E: TaskExecutor>(self, executor: Arc<E>) -> Self {
        self.executor(T::TASK_TYPE, executor)
    }

    /// Set maximum concurrent tasks. Default: 4.
    pub fn max_concurrency(mut self, limit: usize) -> Self {
        self.config.max_concurrency = limit;
        self
    }

    /// Set maximum retries before permanent failure. Default: 3.
    pub fn max_retries(mut self, retries: i32) -> Self {
        self.config.max_retries = retries;
        self
    }

    /// Set the priority threshold for preemption. Default: REALTIME.
    pub fn preempt_priority(mut self, priority: Priority) -> Self {
        self.config.preempt_priority = priority;
        self
    }

    /// Set the poll interval. Default: 500ms.
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.config.poll_interval = interval;
        self
    }

    /// Set the shutdown mode. Default: Hard.
    pub fn shutdown_mode(mut self, mode: ShutdownMode) -> Self {
        self.config.shutdown_mode = mode;
        self
    }

    /// Enable byte-level progress reporting at the given interval.
    ///
    /// When set, a background ticker task polls active tasks' byte counters
    /// and emits [`TaskProgress`](super::TaskProgress) events on a dedicated
    /// channel (via [`Scheduler::subscribe_progress`](super::Scheduler::subscribe_progress)).
    ///
    /// Default: `None` (disabled). Typical value: `Duration::from_millis(250)`.
    pub fn progress_interval(mut self, interval: Duration) -> Self {
        self.config.progress_interval = Some(interval);
        self
    }

    /// Set the timeout for [`TaskExecutor::on_cancel`](crate::TaskExecutor::on_cancel) hooks.
    /// Default: 30 seconds.
    pub fn cancel_hook_timeout(mut self, timeout: Duration) -> Self {
        self.config.cancel_hook_timeout = timeout;
        self
    }

    /// Add a backpressure source (used by the default gate).
    pub fn pressure_source(
        mut self,
        source: Box<dyn crate::backpressure::PressureSource + 'static>,
    ) -> Self {
        self.pressure_sources.push(source);
        self
    }

    /// Set a custom throttle policy (used by the default gate). Default: three-tier.
    pub fn throttle_policy(mut self, policy: ThrottlePolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Enable platform resource monitoring (CPU, disk IO) using `sysinfo`.
    ///
    /// This starts a background sampler task that feeds IO data to the
    /// scheduler for budget-based dispatch decisions. The sampler is
    /// automatically stopped when the scheduler shuts down.
    pub fn with_resource_monitoring(mut self) -> Self {
        self.enable_resource_monitoring = true;
        self
    }

    /// Provide a custom [`ResourceSampler`] instead of the default platform one.
    pub fn resource_sampler(mut self, sampler: Box<dyn ResourceSampler>) -> Self {
        self.custom_sampler = Some(sampler);
        self.enable_resource_monitoring = true;
        self
    }

    /// Configure the resource sampler loop.
    pub fn sampler_config(mut self, config: SamplerConfig) -> Self {
        self.sampler_config = config;
        self
    }

    /// Set a network bandwidth cap (combined RX+TX, in bytes per second).
    ///
    /// Registers a built-in [`NetworkPressure`](crate::resource::network_pressure::NetworkPressure)
    /// source that maps observed throughput to backpressure. When throughput
    /// approaches this limit, low-priority tasks are throttled.
    ///
    /// Requires resource monitoring to be enabled (either
    /// [`with_resource_monitoring`](Self::with_resource_monitoring) or a custom
    /// [`resource_sampler`](Self::resource_sampler)).
    pub fn bandwidth_limit(mut self, max_bytes_per_sec: f64) -> Self {
        self.bandwidth_limit_bps = Some(max_bytes_per_sec);
        self.enable_resource_monitoring = true;
        self
    }

    /// Set the default concurrency limit for any grouped task.
    ///
    /// `0` means unlimited (no limit for groups without an explicit override).
    /// Default: 0.
    pub fn default_group_concurrency(mut self, limit: usize) -> Self {
        self.default_group_concurrency = limit;
        self
    }

    /// Set a concurrency limit for a specific task group.
    ///
    /// Tasks with a matching `group_key` will be limited to at most `limit`
    /// concurrent executions.
    pub fn group_concurrency(mut self, group: impl Into<String>, limit: usize) -> Self {
        self.group_concurrency_overrides.push((group.into(), limit));
        self
    }

    /// Register shared application state accessible from every executor via
    /// [`TaskContext::state`](crate::TaskContext::state).
    ///
    /// Multiple types can be registered — each is keyed by its concrete
    /// `TypeId`. Calling this twice with the same `T` overwrites the
    /// previous value.
    ///
    /// The state is stored as `Arc<T>` internally, so it is shared (not
    /// cloned) across all running tasks. This mirrors how Axum, Actix, and
    /// Tauri handle shared application state.
    ///
    /// # Example
    ///
    /// ```ignore
    /// struct AppServices { http: reqwest::Client, db: DatabasePool }
    ///
    /// let services = AppServices { /* ... */ };
    /// Scheduler::builder()
    ///     .app_state(services)
    ///     .build()
    ///     .await?;
    /// ```
    pub fn app_state<T: Send + Sync + 'static>(self, state: T) -> Self {
        self.app_state_arc(Arc::new(state))
    }

    /// Register shared application state from a pre-existing `Arc`.
    ///
    /// Use this instead of [`app_state`](Self::app_state) when you already
    /// have an `Arc<T>` and need to retain a handle for use outside the
    /// scheduler (e.g. to populate `OnceLock` fields after build). Avoids
    /// double-wrapping (`Arc<Arc<T>>`), which would cause
    /// [`TaskContext::state`](crate::TaskContext::state) downcasts to fail.
    ///
    /// Multiple types can be registered — each is keyed by its concrete
    /// `TypeId`.
    pub fn app_state_arc<T: Send + Sync + 'static>(mut self, state: Arc<T>) -> Self {
        self.app_state_entries.push((TypeId::of::<T>(), state));
        self
    }

    /// Build the scheduler. Opens the database and wires all components.
    ///
    /// If resource monitoring is enabled, the sampler background loop is
    /// started and will be stopped automatically when the scheduler shuts
    /// down (via the token passed to [`Scheduler::run`]).
    pub async fn build(self) -> Result<Scheduler, StoreError> {
        // Open or use provided store.
        let store = if let Some(store) = self.store {
            store
        } else if let Some(path) = &self.store_path {
            TaskStore::open_with_config(path, self.store_config).await?
        } else {
            return Err(StoreError::Database(
                "SchedulerBuilder requires either store_path() or store()".into(),
            ));
        };

        // Build registry.
        let mut registry = crate::registry::TaskTypeRegistry::new();
        for (name, executor) in self.executors {
            if registry.get(&name).is_some() {
                panic!("task type '{name}' already registered");
            }
            registry.register_erased(&name, executor);
        }

        // Prepare resource monitoring reader early so NetworkPressure can
        // reference it before the gate is boxed.
        let reader = if self.enable_resource_monitoring {
            Some(SmoothedReader::new())
        } else {
            None
        };

        // Build gate from pressure sources + policy.
        let mut pressure = CompositePressure::new();
        for source in self.pressure_sources {
            pressure.add_source(source);
        }

        // Register NetworkPressure if a bandwidth limit was set.
        if let (Some(bw_limit), Some(ref reader)) = (self.bandwidth_limit_bps, &reader) {
            let net_pressure = crate::resource::network_pressure::NetworkPressure::new(
                Arc::new(reader.clone()) as Arc<dyn ResourceReader>,
                bw_limit,
            );
            pressure.add_source(Box::new(net_pressure));
        }

        let policy = self
            .policy
            .unwrap_or_else(ThrottlePolicy::default_three_tier);
        let gate = Box::new(super::gate::DefaultDispatchGate::new(pressure, policy));

        let app_state = Arc::new(crate::registry::StateMap::from_entries(
            self.app_state_entries,
        ));

        let scheduler =
            Scheduler::with_gate(store, self.config, Arc::new(registry), gate, app_state);

        // Apply group concurrency limits.
        if self.default_group_concurrency > 0 {
            scheduler
                .inner
                .group_limits
                .set_default(self.default_group_concurrency);
        }
        for (group, limit) in self.group_concurrency_overrides {
            scheduler.inner.group_limits.set_limit(group, limit);
        }

        // Set up resource monitoring.
        if let Some(reader) = reader {
            scheduler
                .set_resource_reader(Arc::new(reader.clone()))
                .await;

            #[cfg(feature = "sysinfo-monitor")]
            let sampler: Box<dyn ResourceSampler> = self
                .custom_sampler
                .unwrap_or_else(|| crate::resource::platform_sampler());

            #[cfg(not(feature = "sysinfo-monitor"))]
            let sampler: Box<dyn ResourceSampler> = self
                .custom_sampler
                .expect("resource monitoring enabled but no custom sampler provided and sysinfo-monitor feature is disabled");

            // Spawn sampler loop — it will stop when the scheduler's sampler_token is cancelled.
            let sampler_arc = Arc::new(tokio::sync::Mutex::new(sampler));
            let sampler_config = self.sampler_config;
            let sampler_token = scheduler.inner.sampler_token.clone();
            tokio::spawn(crate::resource::sampler::run_sampler(
                sampler_arc,
                reader,
                sampler_config,
                sampler_token,
            ));
        }

        Ok(scheduler)
    }
}

impl Default for SchedulerBuilder {
    fn default() -> Self {
        Self::new()
    }
}
