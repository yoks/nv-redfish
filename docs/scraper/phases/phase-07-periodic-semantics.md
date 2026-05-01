# Scraper phase 7: periodic semantics

## Purpose

Implement periodic generator semantics without stale job accumulation.

## Required reading

- [Phase 0 contract](../phase-0.md)
- [Requirements](../requirements.md)
- [Runtime](../runtime.md)
- [Scheduling overview](../scheduling.md)
- [Target scheduler](../scheduling-target.md)
- [Rust style guide](../rust-style-guide.md)

## Frozen test rule

The Phase 0 tests are frozen. Periodic behavior must satisfy the frozen overload
and lag assertions.

## Frozen tests to preserve

- [scheduling.rs](../../../scraper/tests/scheduling.rs)
- [stats.rs](../../../scraper/tests/stats.rs)
- [runtime_events.rs](../../../scraper/tests/runtime_events.rs)

## Target tests

- `periodic_generators_do_not_accumulate_stale_jobs`
- `generator_stats_report_lag_missed_intervals_and_actual_interval`
- `overload_is_not_reported_as_periodic_job_queue_depth`

## Scope

- Honor requested generator intervals.
- Create executable periodic work only after selection.
- Avoid one queued job per missed interval.
- Track lag, missed intervals, and actual interval.

## Out of scope

- Runtime event emission for lag.
- Advanced fairness policy.

## Target commands

```sh
cargo test -p nv-redfish-scraper --test scheduling
cargo test -p nv-redfish-scraper --test stats
```

## Done

- Periodic target tests pass.
- Overload is reported through lag/missed intervals, not stale job queue depth.
