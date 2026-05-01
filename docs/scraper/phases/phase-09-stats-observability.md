# Scraper phase 9: stats and observability

## Purpose

Expose generic runtime observability snapshots for scheduler, executor, and
queue state.

## Required reading

- [Phase 0 contract](../phase-0.md)
- [Requirements](../requirements.md)
- [Runtime](../runtime.md)
- [Scheduling overview](../scheduling.md)
- [Rust style guide](../rust-style-guide.md)

## Frozen test rule

The Phase 0 tests are frozen. Stats assertions describe the public observability
contract.

## Frozen tests to preserve

- [stats.rs](../../../scraper/tests/stats.rs)
- [output.rs](../../../scraper/tests/output.rs)
- [scheduling.rs](../../../scraper/tests/scheduling.rs)

## Target tests

- `runtime_stats_expose_per_target_class_and_generator_snapshots`
- `queue_pressure_is_reflected_in_stats`
- remaining stats assertions in scheduling/completion tests

## Scope

- Expose global runtime counters.
- Expose per-target stats.
- Expose per-class stats.
- Expose per-generator stats.
- Report queue pressure and saturation/starvation/throttling counters.

## Out of scope

- Runtime event snapshot emission.
- Redfish adapter metrics.

## Target commands

```sh
cargo test -p nv-redfish-scraper --test stats
cargo test -p nv-redfish-scraper --test output
```

## Done

- Stats target tests pass.
- Stats contain generic runtime facts only.
