//! Integration tests for the taskmill scheduler.
//!
//! Tests are split into submodules by feature area:
//! - `scheduler_core`: sections A–L (priority, retry, preemption, backpressure,
//!   concurrency, run loop, child tasks, crash recovery, batch, IO metrics,
//!   diagnostics, delayed/recurring tasks)
//! - `dependencies`: section M (task dependency graph)
//! - `retry_policy`: Phase 6 (adaptive retry, backoff, per-type policies)
//! - `modules`: sections N (module registration, ModuleHandle)
//! - `module_features`: sections P–Q + step 7 (default layering, module
//!   concurrency, namespaced StateMap)
//! - `cross_module`: steps 8–11 (TaskContext module access, cross-module child
//!   spawning, Scheduler::modules(), event module identity)

#[path = "integration/common.rs"]
mod common;
#[path = "integration/cross_module.rs"]
mod cross_module;
#[path = "integration/dependencies.rs"]
mod dependencies;
#[path = "integration/module_features.rs"]
mod module_features;
#[path = "integration/modules.rs"]
mod modules;
#[path = "integration/retry_policy.rs"]
mod retry_policy;
#[path = "integration/scheduler_core.rs"]
mod scheduler_core;
#[path = "integration/typed_events.rs"]
mod typed_events;
