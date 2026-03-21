# Migrating from 0.5.x to 0.6.0

0.6.0 replaces the untyped `&TaskContext` in `TypedExecutor` with a
domain-parameterized `DomainTaskContext<'a, D>` wrapper. It also removes the
untyped executor API (`TaskExecutor`, `TaskContext`, `raw_executor`,
`submit_raw`) from the public surface.

### 1. Executor signature change

All `TypedExecutor` implementations must update `execute`, `finalize`, and
`on_cancel` to accept `DomainTaskContext<'a, T::Domain>` instead of
`&'a TaskContext`.

**Before:**

```rust
impl TypedExecutor<Thumbnail> for ThumbnailExec {
    async fn execute<'a>(
        &'a self,
        thumb: Thumbnail,
        ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        // ...
    }
}
```

**After:**

```rust
impl TypedExecutor<Thumbnail> for ThumbnailExec {
    async fn execute<'a>(
        &'a self,
        thumb: Thumbnail,
        ctx: DomainTaskContext<'a, Media>,
    ) -> Result<(), TaskError> {
        // ...
    }
}
```

All accessor methods (`record()`, `token()`, `check_cancelled()`, `progress()`,
`state()`, `domain_state()`, `domain()`, `try_domain()`, IO tracking, and
byte-level progress) are delegated identically — no call-site changes needed.

### 2. Generic executors

For generic `impl<T: TypedTask> TypedExecutor<T>` implementations, use
`T::Domain` as the type parameter:

**Before:**

```rust
impl<T: TypedTask> TypedExecutor<T> for NoopExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: T,
        _ctx: &'a TaskContext,
    ) -> Result<(), TaskError> {
        Ok(())
    }
}
```

**After:**

```rust
impl<T: TypedTask> TypedExecutor<T> for NoopExecutor {
    async fn execute<'a>(
        &'a self,
        _payload: T,
        _ctx: DomainTaskContext<'a, T::Domain>,
    ) -> Result<(), TaskError> {
        Ok(())
    }
}
```

### 3. Child spawning (same domain)

`spawn_child(TaskSubmission)` is no longer exposed on `DomainTaskContext`.
Use `spawn_child_with(task)` which is type-safe — only tasks where
`T::Domain == D` are accepted.

**Before:**

```rust
ctx.spawn_child(
    TaskSubmission::new("upload-part")
        .key(&part.etag)
        .payload_json(&UploadPart { etag: part.etag.clone(), size: part.size })?,
).await?;
```

**After:**

```rust
ctx.spawn_child_with(UploadPart { etag: part.etag.clone(), size: part.size })
    .key(&part.etag)
    .await?;
```

`ChildSpawnBuilder` supports `.key()`, `.priority()`, `.ttl()`, and `.group()`
overrides before `.await`.

### 4. Batch child spawning

`spawn_children(Vec<TaskSubmission>)` (mixed types) is replaced by
`spawn_children_with(tasks)` (single type `T`).

**Before:**

```rust
let subs: Vec<TaskSubmission> = parts.iter()
    .map(|p| TaskSubmission::new("upload-part").key(&p.etag).payload_json(p).unwrap())
    .collect();
ctx.spawn_children(subs).await?;
```

**After:**

```rust
ctx.spawn_children_with(parts).await?;
```

Mixed-type fan-out now requires separate `spawn_child_with` calls per type.
This forgoes SQL batching for that specific pattern.

### 5. Cross-domain children

Use the new `child_of(&ctx)` method instead of manually extracting the parent ID.

**Before:**

```rust
ctx.domain::<Analytics>()
    .submit_with(ScanStartedEvent { .. })
    .parent(ctx.record().id)
    .await?;
```

**After:**

```rust
ctx.domain::<Analytics>()
    .submit_with(ScanStartedEvent { .. })
    .child_of(&ctx)
    .await?;
```

### 6. Removed public API

The following are no longer exported from the crate root:

| Removed | Replacement |
|---|---|
| `TaskExecutor` trait | Use `TypedExecutor<T>` |
| `TaskContext` struct | Use `DomainTaskContext<'a, D>` |
| `Domain::raw_executor(name, exec)` | Use `Domain::task::<T>(exec)` |
| `DomainHandle::submit_raw(sub)` | Use `DomainHandle::submit(task)` or `submit_with(task)` |

### 7. Import changes

**Before:**

```rust
use taskmill::{TaskContext, TaskExecutor, TypedExecutor, /* ... */};
```

**After:**

```rust
use taskmill::{DomainTaskContext, TypedExecutor, /* ... */};
// Optional, if using typed child spawning directly:
use taskmill::ChildSpawnBuilder;
```
