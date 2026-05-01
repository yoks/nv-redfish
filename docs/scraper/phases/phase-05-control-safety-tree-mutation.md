# Scraper phase 5: control safety and tree mutation

## Purpose

Make control operations safe when interleaved with scheduling and output
draining.

## Required reading

- [Phase 0 contract](../phase-0.md)
- [Requirements](../requirements.md)
- [Runtime](../runtime.md)
- [Scheduling overview](../scheduling.md)
- [Root scheduler](../scheduling-root.md)
- [Target scheduler](../scheduling-target.md)
- [Rust style guide](../rust-style-guide.md)

## Frozen test rule

The Phase 0 tests are frozen. Tree mutation behavior must follow tests, not the
other way around.

## Frozen tests to preserve

- [control.rs](../../../scraper/tests/control.rs)
- [scheduling.rs](../../../scraper/tests/scheduling.rs)
- [output.rs](../../../scraper/tests/output.rs)

## Target tests

- `queued_outputs_survive_target_and_generator_removal`
- `tree_changes_invalidate_stale_readiness`
- all existing control tests remain green

## Scope

- Removing a target removes attached generators.
- Removed generators are never queried again.
- Queued outputs survive target/generator removal.
- Pause/resume affects future scheduling.
- Tree changes invalidate or recompute affected readiness.

## Out of scope

- Cost fairness.
- Periodic lag.
- Runtime events.

## Target commands

```sh
cargo test -p nv-redfish-scraper --test control
cargo test -p nv-redfish-scraper --test scheduling
```

## Done

- Control and stale-readiness target tests pass.
- No stale scheduler entries can point at removed generators.
