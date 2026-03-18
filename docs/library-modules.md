# Writing a Reusable Module

This guide covers how to package a taskmill domain as a standalone Rust crate that other applications can pull in as a dependency. A library domain owns its executors, typed tasks, and scoped state — the host application registers it with `Scheduler::builder().domain(...)` and everything works without further wiring.

## Design goals for library modules

A good library module is:

- **Self-contained.** It brings its own executors, task types, defaults, and state. The host should not need to know internal details.
- **Conflict-free.** It uses a unique `DomainKey` type whose `NAME` avoids collisions with the host's domains or other libraries.
- **Decoupled from host state.** Executors only access domain-scoped state. They never reach into the host's global state map for types the host did not explicitly promise.
- **Testable in isolation.** The domain can be exercised against an in-memory store without a full application scaffold.

## Naming: the DomainKey type IS the identity

Domain names are global within a scheduler. If two `.domain()` calls use the same `NAME`, `build()` returns an error. Avoid generic names like `"upload"` or `"sync"` — prefix with your crate or organization name.

Export a zero-sized `DomainKey` struct as the public identity of your domain. Callers reference it as a type parameter instead of duplicating a name string:

```rust
/// Domain identity for the acme-cdn crate.
pub struct AcmeCdn;

impl DomainKey for AcmeCdn {
    const NAME: &'static str = "acme-cdn";
}
```

Task types are automatically prefixed at build time. A task type `"upload"` in the `"acme-cdn"` domain is stored in the database as `"acme-cdn::upload"`. Callers never need to construct the prefixed form themselves — the `DomainHandle` does it.

## What to export from your crate

A minimal library module exports:

1. **A `DomainKey` struct** — the type-level identity of the domain.
2. **A domain constructor** that returns a `Domain<D>`.
3. **Typed task structs** so the host can submit tasks.
4. **A configuration struct** if the domain needs runtime settings.

```rust
use std::time::Duration;
use serde::{Serialize, Deserialize};
use taskmill::{
    Domain, DomainKey, TypedTask, TypedExecutor, TaskTypeConfig,
    TaskContext, TaskError, IoBudget, Priority, RetryPolicy,
};

// ── Public API ──────────────────────────────────────────────────

pub struct AcmeCdn;
impl DomainKey for AcmeCdn {
    const NAME: &'static str = "acme-cdn";
}

/// Configuration for the acme-cdn domain.
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
    type Domain = AcmeCdn;
    const TASK_TYPE: &'static str = "upload";

    fn config() -> TaskTypeConfig {
        TaskTypeConfig::new()
            .expected_io(IoBudget::net(0, 1_000_000))
            .priority(Priority::NORMAL)
    }

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
    type Domain = AcmeCdn;
    const TASK_TYPE: &'static str = "purge";
    fn key(&self) -> Option<String> { Some(self.file_id.clone()) }
}

/// Build the acme-cdn domain. Call this from the host's scheduler builder.
pub fn acme_cdn_domain(config: AcmeCdnConfig) -> Domain<AcmeCdn> {
    Domain::<AcmeCdn>::new()
        .task::<UploadTask>(UploadExecutor)
        .task::<PurgeTask>(PurgeExecutor)
        .max_concurrency(config.max_upload_concurrency)
        .default_retry(RetryPolicy::exponential(
            5,
            Duration::from_secs(2),
            Duration::from_secs(120),
        ))
        .default_tag("provider", "acme-cdn")
        .state(config)
}

// ── Internal (not exported) ─────────────────────────────────────

struct UploadExecutor;
struct PurgeExecutor;
// ... impl TypedExecutor<UploadTask> for UploadExecutor { ... }
// ... impl TypedExecutor<PurgeTask> for PurgeExecutor { ... }
```

The host wires it in one line:

```rust
use acme_cdn::{AcmeCdn, AcmeCdnConfig, UploadTask};
use taskmill::{DomainHandle, Scheduler};

let scheduler = Scheduler::builder()
    .store_path("app.db")
    .domain(acme_cdn_domain(AcmeCdnConfig {
        endpoint: "https://cdn.acme.io".into(),
        api_key: std::env::var("ACME_KEY").unwrap(),
        max_upload_concurrency: 4,
    }))
    .build()
    .await?;

let cdn: DomainHandle<AcmeCdn> = scheduler.domain::<AcmeCdn>();
cdn.submit(UploadTask {
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
impl TypedExecutor<UploadTask> for UploadExecutor {
    async fn execute(&self, task: UploadTask, ctx: &TaskContext) -> Result<(), TaskError> {
        let db = ctx.state::<AppDb>().expect("host must register AppDb");
        // ...
    }
}
```

This compiles, but it means the library silently requires the host to register `AppDb` as global state. Nothing in the type system enforces it. If the host forgets, the executor panics at runtime.

### The pattern: inject dependencies via domain constructor

Pass everything the domain needs through its constructor and register it as domain-scoped state:

```rust
pub fn acme_cdn_domain(config: AcmeCdnConfig) -> Domain<AcmeCdn> {
    Domain::<AcmeCdn>::new()
        .task::<UploadTask>(UploadExecutor)
        .state(config) // domain-scoped — only visible to this domain's executors
        // ...
}
```

Inside the executor:

```rust
impl TypedExecutor<UploadTask> for UploadExecutor {
    async fn execute(&self, task: UploadTask, ctx: &TaskContext) -> Result<(), TaskError> {
        let config = ctx.state::<AcmeCdnConfig>()
            .expect("AcmeCdnConfig is registered by acme_cdn_domain()");
        // safe — this domain always registers its own config
    }
}
```

`ctx.state::<T>()` checks domain-scoped state first, then falls back to global state. Because the domain constructor registers `AcmeCdnConfig` via `Domain::state()`, the executor always finds it regardless of what the host does with global state.

## Exposing a typed handle wrapper

With the domain-centric API, `DomainHandle<D>` is already a typed handle scoped to your domain. Callers obtain it with `scheduler.domain::<AcmeCdn>()` — no manual wrapper struct needed.

Compare the old approach (a hand-written `CdnHandle` wrapping an untyped `ModuleHandle`) with the new one:

```rust
// Before (old API): manual wrapper required
let cdn = CdnHandle::from_scheduler(&scheduler);
cdn.upload(&task).await?;

// After (new API): DomainHandle<AcmeCdn> IS the typed handle
let cdn: DomainHandle<AcmeCdn> = scheduler.domain::<AcmeCdn>();
cdn.submit(UploadTask { /* ... */ }).await?;
cdn.submit(PurgeTask { /* ... */ }).await?;
```

`DomainHandle<AcmeCdn>` enforces at compile time that you can only submit task types whose `type Domain = AcmeCdn`. Attempting to submit a task from another domain is a type error.

If you still want convenience methods (e.g. `cdn.upload(file_id, path, size)` that constructs the struct internally), you can add them as extension methods on `DomainHandle<AcmeCdn>`:

```rust
/// Extension methods for CDN operations.
pub trait CdnHandleExt {
    fn upload(&self, file_id: &str, path: &str, size: u64)
        -> impl std::future::Future<Output = Result<SubmitOutcome, StoreError>> + Send;
}

impl CdnHandleExt for DomainHandle<AcmeCdn> {
    async fn upload(&self, file_id: &str, path: &str, size: u64)
        -> Result<SubmitOutcome, StoreError>
    {
        self.submit(UploadTask {
            file_id: file_id.into(),
            path: path.into(),
            size,
        }).await
    }
}
```

Domain-filtered events are also built in — `cdn.events()` returns only events for the `acme-cdn` domain:

```rust
let mut events = cdn.events();
tokio::spawn(async move {
    while let Ok(event) = events.recv().await {
        // Only acme-cdn events appear here.
    }
});
```

## Late-binding state injection

Sometimes a library domain needs state that is only available after the scheduler is built — for example, a shared HTTP client pool created during application startup.

Use `scheduler.register_state()` to inject global state after `build()` but before `run()`:

```rust
let scheduler = Scheduler::builder()
    .store_path("app.db")
    .domain(acme_cdn_domain(config))
    .build()
    .await?;

// State created after build — perhaps from another init step.
let http_pool = Arc::new(HttpPool::new());
scheduler.register_state(http_pool).await;

// Now start the scheduler.
let token = CancellationToken::new();
scheduler.run(token).await;
```

The executor accesses it through the normal `ctx.state::<HttpPool>()` path. Since `register_state` writes to global state, all domains can see it.

**Race condition warning:** `register_state()` is safe to call before `scheduler.run()`. After `run()` starts dispatching tasks, there are no ordering guarantees with in-flight executors. An executor that runs before the state is registered will see `None` from `ctx.state::<T>()`. Always register late-binding state before calling `run()`.

## Handling optional cross-domain integration

If your library provides optional integration with another domain, use `ctx.try_domain::<D>()` to avoid panicking when it is not registered:

```rust
use analytics::{Analytics, TrackEvent};

impl TypedExecutor<UploadTask> for UploadExecutor {
    async fn execute(&self, task: UploadTask, ctx: &TaskContext) -> Result<(), TaskError> {
        // Core upload logic...
        do_upload(ctx).await?;

        // Optional: notify analytics domain if present
        if let Some(analytics) = ctx.try_domain::<Analytics>() {
            analytics.submit(TrackEvent {
                action: "cdn_upload".into(),
                // ...
            }).await.ok(); // best-effort — don't fail the upload
        }

        Ok(())
    }
}
```

Because `try_domain` takes a type parameter, the dependency on the `Analytics` domain key is visible in your `Cargo.toml` (you import the struct from the `analytics` crate). This is strictly better than the old string-based `try_module("analytics")` approach — if the analytics crate renames its domain, your code fails at compile time rather than silently doing nothing at runtime.

Document which optional domain types your library looks for so the host knows what to wire up.

## Testing your module in isolation

`TaskStore::open_memory()` creates an in-memory SQLite database. Combine it with `Scheduler::builder().store()` to test your domain without touching the filesystem:

```rust
#[tokio::test]
async fn upload_task_completes() {
    let store = TaskStore::open_memory().await.unwrap();

    let scheduler = Scheduler::builder()
        .store(store)
        .domain(acme_cdn_domain(AcmeCdnConfig {
            endpoint: "http://localhost:9999".into(),
            api_key: "test-key".into(),
            max_upload_concurrency: 2,
        }))
        .build()
        .await
        .unwrap();

    let cdn: DomainHandle<AcmeCdn> = scheduler.domain::<AcmeCdn>();
    let mut events = cdn.events();

    cdn.submit(UploadTask {
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
- Register only your domain — no need for the host's domains or global state.
- Spawn `scheduler.run()` in a background task and cancel the token when done.
- Use `cdn.events()` (domain-filtered) to receive only your domain's events.

## Checklist before publishing

- [ ] **Unique domain name.** Use an organization or crate prefix (e.g. `"acme-cdn"`, not `"cdn"`). Export the `DomainKey` struct as the public identity.
- [ ] **No global state access.** Executors only use `ctx.state::<T>()` for types registered via `Domain::state()` in your own domain constructor. Document any late-binding state requirements.
- [ ] **Constructor takes config.** All knobs (endpoints, keys, concurrency limits) are parameters of the domain constructor function, not hardcoded.
- [ ] **TypedTask for every task type.** Use `Domain::task::<T>(executor)` for registration. Each task struct declares `type Domain = YourDomainKey`. Export the typed task structs so callers can use `handle.submit(task)`.
- [ ] **Serde derives on task structs.** Both `Serialize` and `Deserialize` — the host needs `Serialize` for submission, your executor needs `Deserialize` for the typed payload.
- [ ] **TASK_TYPE uses short form.** `const TASK_TYPE: &'static str = "upload"`, not `"acme-cdn::upload"`. The domain prefixes it automatically.
- [ ] **Static defaults in `config()`.** Use `TypedTask::config()` with `TaskTypeConfig` for priority, IO budget, TTL, retry policy, and group key. Reserve instance methods (`key()`, `label()`, `tags()`) for payload-dependent values.
- [ ] **key() returns a stable dedup key.** If your task has a natural identity (file ID, URL), return it from `TypedTask::key()` so duplicate submissions are handled correctly.
- [ ] **Default retry policy set.** Use `Domain::default_retry()` with a `RetryPolicy::exponential(...)` for sensible backoff across the domain.
- [ ] **Domain concurrency capped.** Set `Domain::max_concurrency()` to a reasonable default. The host can adjust at runtime via `DomainHandle::set_max_concurrency()`.
- [ ] **Tests use open_memory().** No filesystem side effects. Tests should pass in CI without special setup.
- [ ] **Document optional integrations.** If your executors use `ctx.try_domain::<OtherDomain>()`, list those optional domain crate dependencies in your docs.
