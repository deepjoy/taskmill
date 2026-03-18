# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
