# Scraper phase 6: capacity limits

## Purpose

Enforce runtime-wide and per-target in-flight limits.

## Required reading

- [Phase 0 contract](../phase-0.md)
- [Requirements](../requirements.md)
- [Runtime](../runtime.md)
- [Scheduling overview](../scheduling.md)
- [Root scheduler](../scheduling-root.md)
- [Target scheduler](../scheduling-target.md)
- [Rust style guide](../rust-style-guide.md)

## Frozen test rule

The Phase 0 tests are frozen. Do not relax limit assertions.

## Frozen tests to preserve

- [scheduling.rs](../../../scraper/tests/scheduling.rs)
- [completion.rs](../../../scraper/tests/completion.rs)
- [stats.rs](../../../scraper/tests/stats.rs)

## Target tests

- `target_and_global_in_flight_limits_are_respected`
- in-flight counter assertions in completion and stats tests

## Scope

- Enforce global maximum in-flight work.
- Enforce per-target maximum in-flight work.
- Reserve capacity before dispatch.
- Release capacity exactly once after completion.
- Track saturation state for later stats/events.

## Out of scope

- Full cost budget policy.
- Fairness between targets/classes beyond admission safety.

## Target commands

```sh
cargo test -p nv-redfish-scraper --test scheduling
cargo test -p nv-redfish-scraper --test completion
cargo test -p nv-redfish-scraper --test stats
```

## Done

- Capacity target tests pass.
- Runtime-wide and per-target counters cannot drift.
