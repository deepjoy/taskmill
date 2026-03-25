# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.1](https://github.com/deepjoy/taskmill/compare/v0.7.0...v0.7.1) - 2026-03-25

### Added

- implement sibling task spawning with automatic parent-ID inheritance ([#89](https://github.com/deepjoy/taskmill/pull/89))

## [0.7.0](https://github.com/deepjoy/taskmill/compare/v0.6.0...v0.7.0) - 2026-03-24

### Added

- implement observability metrics export ([#20](https://github.com/deepjoy/taskmill/pull/20)) ([#88](https://github.com/deepjoy/taskmill/pull/88))
- [**breaking**] implement priority aging and weighted fair scheduling ([#84](https://github.com/deepjoy/taskmill/pull/84))
- [**breaking**] implement token-bucket rate limiting per task type and group ([#82](https://github.com/deepjoy/taskmill/pull/82))
- [**breaking**] implement group-level pause and resume ([#81](https://github.com/deepjoy/taskmill/pull/81))
- add Throughput to all criterion benchmarks so critcmp shows ops/sec ([#78](https://github.com/deepjoy/taskmill/pull/78))
- [**breaking**] normalize timestamps from TEXT to epoch millisecond INTEGER ([#75](https://github.com/deepjoy/taskmill/pull/75))
- expose `fail_fast()` on `SubmitBuilder` and `DomainSubmitBuilder` ([#73](https://github.com/deepjoy/taskmill/pull/73))
- add tag key prefix queries for namespace-scoped discovery ([#71](https://github.com/deepjoy/taskmill/pull/71))
- [**breaking**] support passing state from execute() to finalize() via typed Memo ([#69](https://github.com/deepjoy/taskmill/pull/69))

### Fixed

- use distinct critcmp group names so PR benchmark diff shows two columns ([#77](https://github.com/deepjoy/taskmill/pull/77))
- use distinct critcmp group names for PR benchmark comparison ([#74](https://github.com/deepjoy/taskmill/pull/74))
- use distinct critcmp group names for PR benchmark comparison ([#72](https://github.com/deepjoy/taskmill/pull/72))

### Other

- add migration guide for 0.7.0 ([#85](https://github.com/deepjoy/taskmill/pull/85))
- consolidate paused_groups migration into standard include_str! pattern ([#83](https://github.com/deepjoy/taskmill/pull/83))
- [**breaking**] improve benchmark throughput across submit, dispatch, retry, and failure paths ([#80](https://github.com/deepjoy/taskmill/pull/80))
- skip transaction for retry requeue and tune slow benchmarks ([#79](https://github.com/deepjoy/taskmill/pull/79))
- [**breaking**] consolidate migrations from 9 chronological files into 4 object-oriented files ([#76](https://github.com/deepjoy/taskmill/pull/76))

## [0.6.0](https://github.com/deepjoy/taskmill/compare/v0.5.3...v0.6.0) - 2026-03-21

### Added

- [**breaking**] replace `&TaskContext` with `DomainTaskContext<D>` for type-safe child spawning ([#68](https://github.com/deepjoy/taskmill/pull/68))

### Other

- migrate all tests/benches from TaskExecutor to TypedExecutor ([#66](https://github.com/deepjoy/taskmill/pull/66))

## [0.5.3](https://github.com/deepjoy/taskmill/compare/v0.5.2...v0.5.3) - 2026-03-19

### Other

- batch dispatch and completion SQL to reduce round-trips (~56% faster) ([#61](https://github.com/deepjoy/taskmill/pull/61))

## [0.5.2](https://github.com/deepjoy/taskmill/compare/v0.5.1...v0.5.2) - 2026-03-19

### Other

- reduce SQL round-trips and CPU overhead in scheduler hot paths ([#60](https://github.com/deepjoy/taskmill/pull/60))
- coalesce task completions into batched transactions ([#59](https://github.com/deepjoy/taskmill/pull/59))
- reduce SQL round-trips in scheduler hot paths ([#57](https://github.com/deepjoy/taskmill/pull/57))

## [0.5.1](https://github.com/deepjoy/taskmill/compare/v0.5.0...v0.5.1) - 2026-03-19

### Fixed

- *(bench)* eliminate per-sample scheduler setup cost in history benchmarks ([#55](https://github.com/deepjoy/taskmill/pull/55))
- *(bench)* remove premature cancellation token call in history benchmark setup ([#54](https://github.com/deepjoy/taskmill/pull/54))
- *(ci)* bootstrap _benchmarks branch on first push to main ([#53](https://github.com/deepjoy/taskmill/pull/53))
- *(ci)* restore stderr capture for benchmark output on main ([#51](https://github.com/deepjoy/taskmill/pull/51))
- *(ci)* exclude lib target from cargo bench to fix benchmark CI ([#49](https://github.com/deepjoy/taskmill/pull/49))

### Other

- decompose internal god objects into focused, single-responsibility modules ([#56](https://github.com/deepjoy/taskmill/pull/56))
- eliminate stringly-typed history status and DRY violations ([#52](https://github.com/deepjoy/taskmill/pull/52))

## [0.5.0](https://github.com/deepjoy/taskmill/compare/v0.4.0...v0.5.0) - 2026-03-18

### Added

- [**breaking**] domain-centric API with typed event streams ([#48](https://github.com/deepjoy/taskmill/pull/48))

### Fixed

- *(ci)* capture benchmark output from stderr in workflow ([#47](https://github.com/deepjoy/taskmill/pull/47))

### Other

- add multi-module guides, benchmarks, and CI benchmark workflow ([#45](https://github.com/deepjoy/taskmill/pull/45))

## [0.4.0](https://github.com/deepjoy/taskmill/compare/v0.3.1...v0.4.0) - 2026-03-18

### Added

- implement multi-module API ([#44](https://github.com/deepjoy/taskmill/pull/44))
- adaptive retry with configurable backoff strategies ([#42](https://github.com/deepjoy/taskmill/pull/42))
- add task metadata tags ([#40](https://github.com/deepjoy/taskmill/pull/40))
- task dependencies with blocked status, cycle detection, and failure cascading ([#39](https://github.com/deepjoy/taskmill/pull/39))
- delayed and recurring task scheduling ([#38](https://github.com/deepjoy/taskmill/pull/38))
- task TTL with automatic expiry, sweep, and child inheritance ([#33](https://github.com/deepjoy/taskmill/pull/33))
- task superseding with atomic cancel-and-replace ([#32](https://github.com/deepjoy/taskmill/pull/32))
- task cancellation with abort hooks ([#31](https://github.com/deepjoy/taskmill/pull/31))
- bulk task submission with BatchOutcome, BatchSubmission builder, intra-batch dedup, and chunking ([#30](https://github.com/deepjoy/taskmill/pull/30))
- add byte-level progress reporting with EWMA throughput tracking ([#29](https://github.com/deepjoy/taskmill/pull/29))

### Other

- split large store modules into focused sub-modules ([#43](https://github.com/deepjoy/taskmill/pull/43))
- rewrite documentation to be user-facing ([#28](https://github.com/deepjoy/taskmill/pull/28))
- [**breaking**] consolidate IO fields into IoBudget and introduce TaskEventHeader ([#26](https://github.com/deepjoy/taskmill/pull/26))

## [0.3.1](https://github.com/deepjoy/taskmill/compare/v0.3.0...v0.3.1) - 2026-03-14

### Fixed

- scheduler performance and correctness improvements ([#24](https://github.com/deepjoy/taskmill/pull/24))
- *(taskmill)* atomic parent resolution and weak scheduler reference in TaskContext ([#22](https://github.com/deepjoy/taskmill/pull/22))

### Other

- *(taskmill)* split large modules into focused submodules and optimize completion hot path ([#25](https://github.com/deepjoy/taskmill/pull/25))
- *(taskmill)* add integration tests and criterion benchmarks ([#21](https://github.com/deepjoy/taskmill/pull/21))

## [0.3.0](https://github.com/deepjoy/taskmill/compare/v0.2.0...v0.3.0) - 2026-03-14

### Added

- add network IO pressure and per-group concurrency limiting ([#7](https://github.com/deepjoy/taskmill/pull/7))

### Other

- [**breaking**] builder pattern for TaskSubmission and accessor methods for TaskContext ([#6](https://github.com/deepjoy/taskmill/pull/6))
- [**breaking**] simplify executor API with incremental IO tracking and expanded docs ([#4](https://github.com/deepjoy/taskmill/pull/4))

## [0.2.0](https://github.com/deepjoy/taskmill/compare/v0.1.1...v0.2.0) - 2026-03-14

### Added

- *(taskmill)* add hierarchical child tasks with two-phase execution ([#2](https://github.com/deepjoy/taskmill/pull/2))
- initial release (migrated from deepjoy/shoebox) ([#1](https://github.com/deepjoy/taskmill/pull/1))

### Other

- Initial commit

## [0.1.1](https://github.com/deepjoy/shoebox/compare/taskmill-v0.1.0...taskmill-v0.1.1) - 2026-03-10

### Added

- add pagination, filtering, query optimization, and trigger-based staleness for duplicatesFunctional improvements ([#53](https://github.com/deepjoy/shoebox/pull/53))

### Fixed

- *(taskmill)* flush WAL and close database connection on shutdown ([#57](https://github.com/deepjoy/shoebox/pull/57))

## [0.1.0](https://github.com/deepjoy/shoebox/releases/tag/taskmill-v0.1.0) - 2026-03-05

### Added

- *(taskmill)* type-keyed state map with post-build injection ([#46](https://github.com/deepjoy/shoebox/pull/46))
- *(taskmill)* requeue duplicate submissions when task is running ([#45](https://github.com/deepjoy/shoebox/pull/45))
- *(taskmill)* add adaptive priority task scheduler with IO-aware concurrency ([#38](https://github.com/deepjoy/shoebox/pull/38))

### Fixed

- *(taskmill)* resolve SQLite BUSY errors with proper transaction handling ([#40](https://github.com/deepjoy/shoebox/pull/40))

### Other

- *(taskmill)* separate priority from task payload, upgrade on dedup ([#44](https://github.com/deepjoy/shoebox/pull/44))
