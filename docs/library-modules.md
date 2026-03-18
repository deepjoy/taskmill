# Writing a Reusable Module

This guide covers how to package a taskmill module as a standalone Rust crate that other applications can pull in as a dependency. A library module owns its executors, typed tasks, and scoped state — the host application registers it with `Scheduler::builder().module(...)` and everything works without further wiring.

## Design goals for library modules

A good library module is:

- **Self-contained.** It brings its own executors, task types, defaults, and state. The host should not need to know internal details.
- **Conflict-free.** It uses a unique module name that avoids collisions with the host's modules or other libraries.
- **Decoupled from host state.** Executors only access module-scoped state. They never reach into the host's global state map for types the host did not explicitly promise.
- **Testable in isolation.** The module can be exercised against an in-memory store without a full application scaffold.

## Naming: avoid conflicts, export a constant

Module names are global within a scheduler. If two `.module()` calls use the same name, `build()` returns an error. Avoid generic names like `"upload"` or `"sync"` — prefix with your crate or organization name.

Export the name as a constant so callers can reference it without string duplication:

```rust
/// Module name for the acme-cdn crate.
pub const MODULE_NAME: &str = "acme-cdn";
```

Task types are automatically prefixed at build time. A task type `"upload"` in the `"acme-cdn"` module is stored in the database as `"acme-cdn::upload"`. Callers never need to construct the prefixed form themselves — the `ModuleHandle` does it.

## What to export from your crate

A minimal library module exports:

1. **A module constructor** that returns a `Module`.
2. **A module name constant** for the host to look up the handle.
3. **Typed task structs** so the host can submit tasks.
4. **A configuration struct** if the module needs runtime settings.

```rust
use std::sync::Arc;
use std::time::Duration;
use serde::{Serialize, Deserialize};
use taskmill::{
    Module, TypedTask, IoBudget, Priority, RetryPolicy, BackoffStrategy,
};

// ── Public API ──────────────────────────────────────────────────

pub const MODULE_NAME: &str = "acme-cdn";

/// Configuration for the acme-cdn module.
pub struct AcmeCdnConfig {
    pub endpoint: String,
    pub api_key: String,
    pub max_upload_concurrency: usize,
}

/// Upload a file to the CDN.
#[derive(Serialize, Deserialize)]
pub struct UploadTask {
    pub file_id: String,
    pub path: String,
    pub size: u64,
}

impl TypedTask for UploadTask {
    const TASK_TYPE: &'static str = "upload";

    fn expected_io(&self) -> IoBudget {
        IoBudget::net(0, self.size as i64)
    }

    fn priority(&self) -> Priority { Priority::NORMAL }

    fn key(&self) -> Option<String> {
        Some(self.file_id.clone())
    }

    fn label(&self) -> Option<String> {
        Some(format!("Upload {}", self.file_id))
    }
}

/// Purge a file from the CDN edge cache.
#[derive(Serialize, Deserialize)]
pub struct PurgeTask {
    pub file_id: String,
}

impl TypedTask for PurgeTask {
    const TASK_TYPE: &'static str = "purge";
    fn key(&self) -> Option<String> { Some(self.file_id.clone()) }
}

/// Build the acme-cdn module. Call this from the host's scheduler builder.
pub fn acme_cdn_module(config: AcmeCdnConfig) -> Module {
    Module::new(MODULE_NAME)
        .typed_executor::<UploadTask, _>(Arc::new(UploadExecutor))
        .typed_executor::<PurgeTask, _>(Arc::new(PurgeExecutor))
        .max_concurrency(config.max_upload_concurrency)
        .default_retry_policy(RetryPolicy {
            strategy: BackoffStrategy::Exponential {
                initial: Duration::from_secs(2),
                max: Duration::from_secs(120),
                multiplier: 2.0,
            },
            max_retries: 5,
        })
        .default_tag("provider", "acme-cdn")
        .app_state(config)
}

// ── Internal (not exported) ─────────────────────────────────────

struct UploadExecutor;
struct PurgeExecutor;
// ... impl TaskExecutor for each ...
```

The host wires it in one line:

```rust
let scheduler = Scheduler::builder()
    .store_path("app.db")
    .module(acme_cdn_module(AcmeCdnConfig {
        endpoint: "https://cdn.acme.io".into(),
        api_key: std::env::var("ACME_KEY").unwrap(),
        max_upload_concurrency: 4,
    }))
    .build()
    .await?;

let cdn = scheduler.module(acme_cdn::MODULE_NAME);
cdn.submit_typed(&UploadTask {
    file_id: "abc123".into(),
    path: "/data/photo.jpg".into(),
    size: 2_000_000,
}).await?;
```

## Module state vs. required global state

### The anti-pattern: reaching into host state

Executors can access any type via `ctx.state::<T>()`. It is tempting to grab a database pool or HTTP client that the host registered globally:

```rust
// BAD: invisible coupling to the host's global state
impl TaskExecutor for UploadExecutor {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        let db = ctx.state::<AppDb>().expect("host must register AppDb");
        // ...
    }
}
```

This compiles, but it means the library silently requires the host to register `AppDb` as global state. Nothing in the type system enforces it. If the host forgets, the executor panics at runtime.

### The pattern: inject dependencies via module constructor

Pass everything the module needs through its constructor and register it as module-scoped state:

```rust
pub fn acme_cdn_module(config: AcmeCdnConfig) -> Module {
    Module::new(MODULE_NAME)
        .typed_executor::<UploadTask, _>(Arc::new(UploadExecutor))
        .app_state(config) // module-scoped — only visible to this module's executors
        // ...
}
```

Inside the executor:

```rust
impl TaskExecutor for UploadExecutor {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        let config = ctx.state::<AcmeCdnConfig>()
            .expect("AcmeCdnConfig is registered by acme_cdn_module()");
        // safe — this module always registers its own config
    }
}
```

`ctx.state::<T>()` checks module-scoped state first, then falls back to global state. Because the module constructor registers `AcmeCdnConfig` via `Module::app_state()`, the executor always finds it regardless of what the host does with global state.

## Exposing a typed handle wrapper

For a polished library API, wrap `ModuleHandle` in a domain-specific struct so callers don't need to know internal task type names:

```rust
/// Typed handle for submitting CDN tasks.
pub struct CdnHandle {
    inner: taskmill::ModuleHandle,
}

impl CdnHandle {
    /// Get the CDN handle from a scheduler.
    ///
    /// Panics if the acme-cdn module was not registered.
    pub fn from_scheduler(scheduler: &taskmill::Scheduler) -> Self {
        Self {
            inner: scheduler.module(MODULE_NAME),
        }
    }

    /// Get the CDN handle, returning `None` if the module is not registered.
    pub fn try_from_scheduler(scheduler: &taskmill::Scheduler) -> Option<Self> {
        scheduler.try_module(MODULE_NAME).map(|h| Self { inner: h })
    }

    /// Upload a file to the CDN.
    pub fn upload(&self, task: &UploadTask) -> taskmill::SubmitBuilder {
        self.inner.submit_typed(task)
    }

    /// Purge a file from the CDN edge cache.
    pub fn purge(&self, task: &PurgeTask) -> taskmill::SubmitBuilder {
        self.inner.submit_typed(task)
    }

    /// Subscribe to CDN module events only.
    pub fn subscribe(&self) -> taskmill::ModuleReceiver<taskmill::SchedulerEvent> {
        self.inner.subscribe()
    }
}
```

The host uses domain methods instead of generic `submit_typed`:

```rust
let cdn = CdnHandle::from_scheduler(&scheduler);
cdn.upload(&UploadTask { file_id: "abc".into(), path: "/f.jpg".into(), size: 1024 }).await?;
```

## Late-binding state injection

Sometimes a library module needs state that is only available after the scheduler is built — for example, a shared HTTP client pool created during application startup.

Use `scheduler.register_state()` to inject global state after `build()` but before `run()`:

```rust
let scheduler = Scheduler::builder()
    .store_path("app.db")
    .module(acme_cdn_module(config))
    .build()
    .await?;

// State created after build — perhaps from another init step.
let http_pool = Arc::new(HttpPool::new());
scheduler.register_state(http_pool).await;

// Now start the scheduler.
let token = CancellationToken::new();
scheduler.run(token).await;
```

The executor accesses it through the normal `ctx.state::<HttpPool>()` path. Since `register_state` writes to global state, all modules can see it.

**Race condition warning:** `register_state()` is safe to call before `scheduler.run()`. After `run()` starts dispatching tasks, there are no ordering guarantees with in-flight executors. An executor that runs before the state is registered will see `None` from `ctx.state::<T>()`. Always register late-binding state before calling `run()`.

## Handling scheduler.try_module() for optional integration

If your library provides optional integration with another module, use `try_module()` to avoid panicking when it is not registered:

```rust
impl TaskExecutor for UploadExecutor {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        // Core upload logic...
        do_upload(ctx).await?;

        // Optional: notify analytics module if present
        if let Some(analytics) = ctx.try_module("analytics") {
            analytics.submit_typed(&TrackEvent {
                action: "cdn_upload".into(),
                // ...
            }).await.ok(); // best-effort — don't fail the upload
        }

        Ok(())
    }
}
```

This pattern lets your library cooperate with modules from other crates without requiring them. Document which optional module names your library looks for so the host knows what to wire up.

## Testing your module in isolation

`TaskStore::open_memory()` creates an in-memory SQLite database. Combine it with `Scheduler::builder().store()` to test your module without touching the filesystem:

```rust
#[tokio::test]
async fn upload_task_completes() {
    let store = TaskStore::open_memory().await.unwrap();

    let scheduler = Scheduler::builder()
        .store(store)
        .module(acme_cdn_module(AcmeCdnConfig {
            endpoint: "http://localhost:9999".into(),
            api_key: "test-key".into(),
            max_upload_concurrency: 2,
        }))
        .build()
        .await
        .unwrap();

    let cdn = scheduler.module(MODULE_NAME);
    let mut events = cdn.subscribe();

    cdn.submit_typed(&UploadTask {
        file_id: "test-1".into(),
        path: "/tmp/test.jpg".into(),
        size: 1024,
    }).await.unwrap();

    let token = CancellationToken::new();
    let sched = scheduler.clone();
    let cancel = token.clone();
    tokio::spawn(async move { sched.run(cancel).await });

    // Wait for completion event.
    let event = tokio::time::timeout(
        Duration::from_secs(5),
        events.recv(),
    ).await.unwrap().unwrap();

    token.cancel();
}
```

Key points for testing:

- `TaskStore::open_memory()` gives you a fresh database per test. No cleanup needed.
- Register only your module — no need for the host's modules or global state.
- Spawn `scheduler.run()` in a background task and cancel the token when done.
- Use `cdn.subscribe()` (module-filtered) to receive only your module's events.

## Checklist before publishing

- [ ] **Unique module name.** Use an organization or crate prefix (e.g. `"acme-cdn"`, not `"cdn"`). Export it as `pub const MODULE_NAME`.
- [ ] **No global state access.** Executors only use `ctx.state::<T>()` for types registered via `Module::app_state()` in your own module constructor. Document any late-binding state requirements.
- [ ] **Constructor takes config.** All knobs (endpoints, keys, concurrency limits) are parameters of the module constructor function, not hardcoded.
- [ ] **TypedTask for every task type.** Use `typed_executor` over string-based `executor` registration. Export the typed task structs so callers can use `submit_typed`.
- [ ] **Serde derives on task structs.** Both `Serialize` and `Deserialize` — the host needs `Serialize` for submission, your executor needs `Deserialize` for `ctx.payload::<T>()`.
- [ ] **TASK_TYPE uses short form.** `const TASK_TYPE: &'static str = "upload"`, not `"acme-cdn::upload"`. The module prefixes it automatically.
- [ ] **key() returns a stable dedup key.** If your task has a natural identity (file ID, URL), return it from `TypedTask::key()` so duplicate submissions are handled correctly.
- [ ] **Default retry policy set.** Use `Module::default_retry_policy()` with sensible backoff for your use case.
- [ ] **Module concurrency capped.** Set `Module::max_concurrency()` to a reasonable default. The host can adjust at runtime via `ModuleHandle::set_max_concurrency()`.
- [ ] **Tests use open_memory().** No filesystem side effects. Tests should pass in CI without special setup.
- [ ] **Typed handle wrapper (optional).** If your module has more than two task types, consider wrapping `ModuleHandle` in a domain-specific API struct.
- [ ] **Document optional integrations.** If your executors use `ctx.try_module("other")`, list those optional module names in your crate docs.
