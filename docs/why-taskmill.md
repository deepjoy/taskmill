# Why Taskmill

## The problem

Desktop apps and background services often need to do work in the background — generating thumbnails, uploading files, syncing data, scanning directories. The naive approach is to spawn async tasks and push results through channels. This works until it doesn't:

- **Crashes lose work.** If the process dies, everything in-flight disappears. Users have to re-trigger uploads, re-scan libraries, re-generate thumbnails.
- **The system gets overwhelmed.** Spawning 200 image resizes at once saturates the disk. The UI freezes, other apps slow down, and the laptop fan spins up.
- **Users can't see what's happening.** There's no built-in way to show progress, pause work, or let urgent tasks jump the queue.
- **You end up building a scheduler.** Priority queues, retry logic, deduplication, persistence, backpressure — each one is a small problem, but together they're a significant engineering effort.

Taskmill is that engineering effort, packaged as a library.

## What taskmill gives you

- **Work survives crashes.** Tasks are persisted to SQLite. If the process dies, pending and in-progress work is automatically recovered on restart — no user action needed.
- **The system stays responsive.** The scheduler monitors disk and network throughput and automatically defers work when resources are saturated. Your app doesn't freeze the system.
- **Urgent work runs first.** A 256-level priority queue with preemption lets important tasks interrupt background work. User-initiated uploads run before bulk indexing.
- **Progress comes for free.** Executors can report progress explicitly, but even without that, the scheduler extrapolates from historical data so your UI always shows movement.
- **No duplicate work.** Built-in deduplication means you can safely call "upload this file" multiple times — only one task is created.
- **Built for Tauri.** Every public type is `Clone` and `Serialize`. Events bridge directly to your frontend. The `Scheduler` drops into `tauri::State` without wrapping.

## When to use taskmill

**Good fit:**
- Desktop apps with file processing (photo editors, media managers, backup tools)
- Upload and download managers
- Background indexing and search (scanning directories, building metadata)
- Any system where tasks have measurable IO costs and you don't want to saturate the disk or network

**Not a good fit:**
- Distributed job queues across multiple machines — taskmill is single-process, single-node
- Sub-millisecond scheduling — SQLite adds a few milliseconds of overhead per dispatch
- CPU-only compute farms — IO-aware scheduling doesn't help if your bottleneck is CPU
- Web request handlers — use your web framework's middleware and connection pooling instead

## Comparison with alternatives

| Approach | Persistence | IO awareness | Priority | Desktop focus |
|----------|------------|-------------|----------|--------------|
| `tokio::spawn` + channels | No | No | No | No |
| `apalis` / `background_jobs` | Yes (Redis/Postgres) | No | Basic | No (server-oriented) |
| Roll your own | Whatever you build | Whatever you build | Whatever you build | Whatever you build |
| **Taskmill** | Yes (SQLite) | Yes | 256-level with preemption | Yes (Tauri-native) |

Taskmill is ~3,500 lines of tested Rust. It handles the edge cases (crash recovery, dedup races, preemption anti-thrash, amortized pruning) so you don't have to.
