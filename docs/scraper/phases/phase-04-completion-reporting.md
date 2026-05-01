# Scraper phase 4: completion reporting

## Purpose

Report completion exactly once for dispatched work and attach runtime-owned work
metadata and stats.

## Required reading

- [Phase 0 contract](../phase-0.md)
- [Requirements](../requirements.md)
- [Runtime](../runtime.md)
- [Scheduling overview](../scheduling.md)
- [Rust style guide](../rust-style-guide.md)

## Frozen test rule

The Phase 0 tests are frozen. Do not change fake generator callbacks or
completion assertions to match implementation shortcuts.

## Frozen tests to preserve

- [completion.rs](../../../scraper/tests/completion.rs)
- [output.rs](../../../scraper/tests/output.rs)
- [scheduling.rs](../../../scraper/tests/scheduling.rs)

## Target tests

- `completion_is_reported_exactly_once_after_success`
- `completion_is_reported_exactly_once_after_failure`
- `output_is_enqueued_before_generator_completion_callback`
- `in_flight_counters_are_released_after_completion`
- `failed_work_keeps_runtime_owned_stats`

## Scope

- Report success and failure completion outcomes.
- Enqueue work output before calling `Generator::on_complete`.
- Release in-flight counters after completion.
- Attach runtime-owned `WorkStats` to success and failure output.

## Out of scope

- Full lag/missed interval calculation.
- Runtime events.

## Target commands

```sh
cargo test -p nv-redfish-scraper --test completion
cargo test -p nv-redfish-scraper --test output
```

## Done

- Completion target tests pass.
- Scheduled work does not fabricate runtime stats itself.
