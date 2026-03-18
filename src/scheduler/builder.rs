//! Ergonomic builder for constructing a [`Scheduler`].

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use tokio::time::Duration;

use crate::backpressure::{CompositePressure, ThrottlePolicy};
use crate::domain::DomainKey;
use crate::module::{Module, ModuleEntry, ModuleRegistry};
use crate::priority::Priority;
use crate::resource::sampler::{SamplerConfig, SmoothedReader};
use crate::resource::{ResourceReader, ResourceSampler};
use crate::store::{StoreConfig, StoreError, TaskStore};

use super::event::{SchedulerConfig, ShutdownMode};
use super::Scheduler;

/// Ergonomic builder for constructing a [`Scheduler`] with all its dependencies.
///
/// Hides the `Arc<Mutex<...>>` wiring and manages the resource sampler lifecycle.
///
/// # Example
///
/// ```ignore
/// use taskmill::{Domain, DomainKey, Scheduler};
///
/// struct App;
/// impl DomainKey for App { const NAME: &'static str = "app"; }
///
/// let scheduler = Scheduler::builder()
///     .store_path("tasks.db")
///     .domain(Domain::<App>::new())
///     .max_concurrency(8)
///     .with_resource_monitoring()
///     .build()
///     .await?;
/// ```
pub struct SchedulerBuilder {
    store_path: Option<String>,
    store_config: StoreConfig,
    store: Option<TaskStore>,
    modules: Vec<Module>,
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
            modules: Vec::new(),
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

    /// Register a typed domain. All executor task types within the domain
    /// are automatically prefixed with `"{domain_name}::"` at build time.
    ///
    /// At least one domain must be registered before calling [`build`](Self::build).
    pub fn domain<D: DomainKey>(self, domain: crate::domain::Domain<D>) -> Self {
        self.module(domain.into_module())
    }

    /// Register an untyped module (internal).
    pub(crate) fn module(mut self, module: Module) -> Self {
        self.modules.push(module);
        self
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

    /// Set the default TTL applied to tasks without an explicit TTL.
    ///
    /// `None` (default) means no global TTL.
    pub fn default_ttl(mut self, ttl: Duration) -> Self {
        self.config.default_ttl = Some(ttl);
        self
    }

    /// Set the expiry sweep interval.
    ///
    /// Controls how often the scheduler sweeps for expired tasks. `None`
    /// disables periodic sweeps (dispatch-time checks still apply).
    /// Default: `Some(30s)`.
    pub fn expiry_sweep_interval(mut self, interval: Option<Duration>) -> Self {
        self.config.expiry_sweep_interval = interval;
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
    /// # Errors
    ///
    /// Returns an error if:
    /// - No store was configured (neither `store_path` nor `store`).
    /// - No modules were registered (use `.module()` to register at least one).
    /// - Duplicate module names were registered.
    /// - Two modules register the same prefixed task type.
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

        // Validate: at least one module required.
        if self.modules.is_empty() {
            return Err(StoreError::Database(
                "SchedulerBuilder requires at least one module — use .module() to register one"
                    .into(),
            ));
        }

        // Validate: no duplicate module names.
        let mut seen_names: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for m in &self.modules {
            if !seen_names.insert(m.name()) {
                return Err(StoreError::Database(format!(
                    "duplicate module name '{}'",
                    m.name()
                )));
            }
        }

        // Build registry, prefixing all task types with "{module_name}::".
        let mut registry = crate::registry::TaskTypeRegistry::new();
        let mut module_entries: Vec<ModuleEntry> = Vec::new();
        let mut module_state_map: HashMap<String, crate::registry::StateSnapshot> = HashMap::new();

        for module in self.modules {
            let prefix = module.prefix(); // e.g. "media::"
            let module_name = module.name.clone();

            for exec in &module.executors {
                let prefixed = format!("{}{}", prefix, exec.task_type);
                if registry.get(&prefixed).is_some() {
                    return Err(StoreError::Database(format!(
                        "task type collision: '{}' is registered by multiple modules",
                        prefixed
                    )));
                }
                match (&exec.options.ttl, &exec.options.retry_policy) {
                    (Some(ttl), Some(policy)) => {
                        registry.register_erased_with_ttl(&prefixed, exec.executor.clone(), *ttl);
                        registry.set_retry_policy(&prefixed, policy.clone());
                    }
                    (Some(ttl), None) => {
                        registry.register_erased_with_ttl(&prefixed, exec.executor.clone(), *ttl);
                    }
                    (None, Some(policy)) => {
                        registry.register_erased_with_retry_policy(
                            &prefixed,
                            exec.executor.clone(),
                            policy.clone(),
                        );
                    }
                    (None, None) => {
                        registry.register_erased(&prefixed, exec.executor.clone());
                    }
                }
            }

            // Extract per-module state entries before the push moves `module.name`.
            let app_state_entries = module.app_state_entries;

            module_entries.push(ModuleEntry {
                prefix: prefix.clone(),
                default_priority: module.default_priority,
                default_retry_policy: module.default_retry_policy,
                default_group: module.default_group,
                default_ttl: module.default_ttl,
                default_tags: module.default_tags,
                max_concurrency: module.max_concurrency,
                name: module.name,
            });

            module_state_map.insert(
                module_name,
                crate::registry::StateMap::from_entries(app_state_entries)
                    .snapshot()
                    .await,
            );
        }

        let module_registry = Arc::new(ModuleRegistry::new(module_entries));
        let module_state = Arc::new(module_state_map);

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

        let scheduler = Scheduler::with_gate(
            store,
            self.config,
            Arc::new(registry),
            gate,
            app_state,
            module_registry,
            module_state,
        );

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
