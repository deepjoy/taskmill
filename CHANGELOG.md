# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
