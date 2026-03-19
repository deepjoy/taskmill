# Migrating from 0.4.x to 0.5.0

0.5.0 replaces the stringly-typed `Module` / `ModuleHandle` API with a **domain-centric API** that enforces module identity and task ownership at compile time. Every `Module` becomes a `Domain<D>` keyed on a Rust type; every `ModuleHandle` becomes a `DomainHandle<D>`; and executors receive a typed, deserialized payload rather than a raw `TaskContext`.

The changes are mechanical — there are no new database migrations and no changes to stored task type strings.

---

## 1. Declare a `DomainKey`

Every module needs a zero-sized marker type that implements `DomainKey`. This is the compile-time identity for the domain.

**Before (0.4):**
```rust
// Module name lived only in a string literal
Module::new("media")
```

**After (0.5):**
```rust
pub struct Media;
impl DomainKey for Media {
    const NAME: &'static str = "media";
}
```

`NAME` must match the string you previously passed to `Module::new(...)`. The scheduler still uses this string to prefix task types in the database (`"media::thumbnail"`), so existing records are unaffected.

---

## 2. Replace `Module` with `Domain<D>`

`Module` is no longer part of the public API. Replace every `Module::new(...)` call with `Domain::<D>::new()`.

**Before:**
```rust
let module = Module::new("media")
    .executor("thumbnail", Arc::new(ThumbnailExec::new(cdn)))
    .typed_executor::<Transcode>(Arc::new(TranscodeExec::new()))
    .executor_with_ttl("upload", Arc::new(UploadExec), Duration::from_secs(600))
    .default_priority(Priority::NORMAL)
    .default_retry_policy(RetryPolicy { ... })
    .default_group("pipeline")
    .default_ttl(Duration::from_secs(3600))
    .default_tag("team", "media")
    .max_concurrency(4)
    .app_state(MediaConfig { cdn_url: "...".into() });
```

**After:**
```rust
let domain = Domain::<Media>::new()
    .task::<Thumbnail>(ThumbnailExec::new(cdn))   // no Arc needed
    .task::<Transcode>(TranscodeExec::new())
    .task_with::<Upload>(UploadExec, TaskTypeOptions { ttl: Some(Duration::from_secs(600)), ..Default::default() })
    .default_priority(Priority::NORMAL)
    .default_retry(RetryPolicy { ... })           // renamed: default_retry_policy → default_retry
    .default_group("pipeline")
    .default_ttl(Duration::from_secs(3600))
    .default_tag("team", "media")
    .max_concurrency(4)
    .state(MediaConfig { cdn_url: "...".into() });  // renamed: app_state → state
```

**Method renames on the domain builder:**

| 0.4 (`Module`)                                | 0.5 (`Domain<D>`)                         |
|-----------------------------------------------|-------------------------------------------|
| `.executor("name", Arc::new(e))`              | `.task::<T>(e)` (see §3)                  |
| `.typed_executor::<T>(Arc::new(e))`           | `.task::<T>(e)`                           |
| `.executor_with_ttl("name", Arc::new(e), d)`  | `.task_with::<T>(e, TaskTypeOptions { ttl: Some(d), .. })` |
| `.default_retry_policy(p)`                    | `.default_retry(p)`                       |
| `.app_state(v)`                               | `.state(v)` (or `.state_arc(arc)`)        |

### Register with the scheduler

`SchedulerBuilder::module()` is now private. Use `SchedulerBuilder::domain()`:

**Before:**
```rust
Scheduler::builder().module(module)
```

**After:**
```rust
Scheduler::builder().domain(domain)
```

---

## 3. Implement `TypedTask` with `type Domain`

`TypedTask` gains a required associated type `Domain` that binds each task to its domain. Static defaults (priority, IO, TTL, retry, etc.) move from per-method overrides into a single `config()` method that returns `TaskTypeConfig`.

**Before:**
```rust
impl TypedTask for Thumbnail {
    const TASK_TYPE: &'static str = "thumbnail";

    fn priority(&self) -> Priority { Priority::NORMAL }
    fn expected_io(&self) -> IoBudget { IoBudget::disk(4096, 1024) }
    fn ttl(&self) -> Option<Duration> { Some(Duration::from_secs(3600)) }
    fn key(&self) -> Option<String> { Some(format!("thumb:{}:{}", self.path, self.size)) }
}
```

**After:**
```rust
impl TypedTask for Thumbnail {
    type Domain = Media;                     // ← required
    const TASK_TYPE: &'static str = "thumbnail";

    fn config() -> TaskTypeConfig {          // ← static, not &self
        TaskTypeConfig::new()
            .priority(Priority::NORMAL)
            .expected_io(IoBudget::disk(4096, 1024))
            .ttl(Duration::from_secs(3600))
            .retry(RetryPolicy::exponential(3, Duration::from_secs(1), Duration::from_secs(60)))
            .on_duplicate(DuplicateStrategy::Supersede)
    }

    fn key(&self) -> Option<String> {        // instance methods unchanged
        Some(format!("thumb:{}:{}", self.path, self.size))
    }
}
```

`key()`, `label()`, and `tags()` remain as `&self` instance methods because their values depend on the payload. Everything else moves to `config()`.

The compiler enforces that you only register `T` with the domain whose `DomainKey` matches `T::Domain`:

```rust
Domain::<Media>::new()
    .task::<Thumbnail>(...)   // ok — Thumbnail::Domain = Media
    .task::<Upload>(...)      // ok — Upload::Domain = Media
    // .task::<SendEmail>(...) // compile error — SendEmail::Domain ≠ Media
```

---

## 4. Implement `TypedExecutor<T>` instead of `TaskExecutor`

Executors no longer need to deserialize the payload themselves. Implement `TypedExecutor<T>` and receive the typed payload directly.

**Before:**
```rust
impl TaskExecutor for ThumbnailExec {
    async fn execute<'a>(&'a self, ctx: &'a TaskContext) -> Result<(), TaskError> {
        let thumb: Thumbnail = ctx.payload()?;  // manual deserialization
        process(&thumb, ctx).await
    }
}
```

**After:**
```rust
impl TypedExecutor<Thumbnail> for ThumbnailExec {
    async fn execute(&self, thumb: Thumbnail, ctx: &TaskContext) -> Result<(), TaskError> {
        process(&thumb, ctx).await
    }
}
```

The `finalize` and `on_cancel` hooks follow the same pattern:

```rust
impl TypedExecutor<Thumbnail> for ThumbnailExec {
    async fn finalize(&self, thumb: Thumbnail, ctx: &TaskContext) -> Result<(), TaskError> {
        // called after all children settle
        Ok(())
    }

    async fn on_cancel(&self, thumb: Thumbnail, ctx: &TaskContext) -> Result<(), TaskError> {
        // cleanup on preemption/cancellation
        Ok(())
    }
}
```

The `TaskExecutor` trait remains available as an escape hatch via `Domain::raw_executor("name", exec)`, but prefer `TypedExecutor<T>` for all new code.

---

## 5. Replace `ModuleHandle` with `DomainHandle<D>`

`scheduler.module("media")` is now private. Use `scheduler.domain::<Media>()`.

**Before:**
```rust
let media = scheduler.module("media");   // ModuleHandle
```

**After:**
```rust
let media = scheduler.domain::<Media>(); // DomainHandle<Media>
```

### Submission

| 0.4 (`ModuleHandle`)                           | 0.5 (`DomainHandle<D>`)                   |
|------------------------------------------------|-------------------------------------------|
| `handle.submit_typed(&task).await?`            | `domain.submit(task).await?`              |
| `handle.submit_typed(&task).priority(p).await?`| `domain.submit_with(task).priority(p).await?` |
| `handle.submit(sub).await?`                    | `domain.submit_raw(sub).await?`           |

Note that `submit` takes the task **by value** in 0.5, not by reference. Use `.clone()` if you need the value afterward.

```rust
// 0.4
media.submit_typed(&thumb).priority(Priority::HIGH).await?;

// 0.5
media.submit_with(thumb).priority(Priority::HIGH).await?;
// or zero-ceremony:
media.submit(thumb).await?;
```

All other handle methods (`cancel`, `pause`, `resume`, `snapshot`, `active_tasks`, `dead_letter_tasks`, `retry_dead_letter`, `cancel_all`, `cancel_where`, `tasks_by_tags`, `set_max_concurrency`, `pause_recurring`, `resume_recurring`, `cancel_recurring`) are unchanged on `DomainHandle<D>`.

---

## 6. Update cross-domain access in executors

`ctx.current_module()` and `ctx.module("name")` are now internal (`pub(crate)`). Use `ctx.domain::<D>()` for cross-domain submission from within an executor.

**Before:**
```rust
// same-module follow-up
ctx.current_module().submit_typed(&NextStep { ... }).await?;

// cross-module
ctx.module("notifications").submit_typed(&Notify { ... }).await?;
if let Some(h) = ctx.try_module("analytics") {
    h.submit_typed(&Track { ... }).await?;
}
```

**After:**
```rust
// same-domain follow-up
ctx.domain::<Media>().submit(NextStep { ... }).await?;

// cross-domain
ctx.domain::<Notifications>().submit(Notify { ... }).await?;
if let Some(h) = ctx.try_domain::<Analytics>() {
    h.submit(Track { ... }).await?;
}
```

`spawn_child` is unchanged — it still auto-prefixes the task type and inherits TTL/tags from the parent:

```rust
ctx.spawn_child(TaskSubmission::new("postprocess").payload_json(&p)).await?;
```

---

## 7. Update event subscriptions

**Before:**
```rust
let mut rx = scheduler.module("media").subscribe();   // ModuleReceiver<SchedulerEvent>
while let Ok(event) = rx.recv().await {
    match event {
        SchedulerEvent::Completed(header) if header.task_type.ends_with("thumbnail") => { ... }
        _ => {}
    }
}
```

**After:**
```rust
// All events for the domain:
let mut rx = scheduler.domain::<Media>().events();   // ModuleReceiver<SchedulerEvent>

// Or per-type typed stream (preferred):
let mut stream = media.task_events::<Thumbnail>();   // TypedEventStream<Thumbnail>
while let Ok(event) = stream.recv().await {
    match event {
        TaskEvent::Completed { id, record } => {
            let thumb: Thumbnail = serde_json::from_slice(
                record.payload.as_deref().unwrap()
            ).unwrap();
            println!("done: {}", thumb.path);
        }
        TaskEvent::Failed { id, error, will_retry, .. } => { ... }
        TaskEvent::DeadLettered { id, record, .. } => { ... }
        TaskEvent::Progress { id, percent, message } => { ... }
        _ => {}
    }
}
```

`TypedEventStream` filters by both domain and task type, so no manual `task_type` string matching is needed. Terminal variants include an `Arc<TaskHistoryRecord>` for zero-cost access to the history entry.

---

## Summary checklist

- [ ] For each `Module::new("name")`: declare a `struct Name; impl DomainKey for Name { const NAME = "name"; }`.
- [ ] Replace `Module::new(...)` with `Domain::<Name>::new()`.
- [ ] Replace `.executor(...)` / `.typed_executor::<T>(Arc::new(e))` with `.task::<T>(e)`.
- [ ] Add `type Domain = Name;` to every `TypedTask` impl.
- [ ] Replace per-method `TypedTask` defaults with `fn config() -> TaskTypeConfig { ... }`.
- [ ] Replace `impl TaskExecutor for E { async fn execute(&self, ctx) }` with `impl TypedExecutor<T> for E { async fn execute(&self, payload: T, ctx) }`.
- [ ] Replace `SchedulerBuilder::module(m)` with `SchedulerBuilder::domain(d)`.
- [ ] Replace `scheduler.module("name")` / `scheduler.try_module("name")` with `scheduler.domain::<Name>()` / `scheduler.try_domain::<Name>()`.
- [ ] Replace `handle.submit_typed(&task)` with `domain.submit(task)` (note: by value).
- [ ] Replace `handle.submit_typed(&task).priority(p)` with `domain.submit_with(task).priority(p)`.
- [ ] Replace `handle.subscribe()` with `domain.events()` or `domain.task_events::<T>()`.
- [ ] Replace `ctx.current_module()` and `ctx.module("name")` with `ctx.domain::<Name>()`.
- [ ] Rename `.default_retry_policy(p)` → `.default_retry(p)` on the domain builder.
- [ ] Rename `.app_state(v)` → `.state(v)` on the domain builder.
