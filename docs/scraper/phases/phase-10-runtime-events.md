# Scraper phase 10: runtime events

## Purpose

Implement feature-gated runtime events while preserving ordered output causality.

## Required reading

- [Phase 0 contract](../phase-0.md)
- [Requirements](../requirements.md)
- [Runtime](../runtime.md)
- [Scheduling overview](../scheduling.md)
- [Rust style guide](../rust-style-guide.md)

## Frozen test rule

The Phase 0 tests are frozen. Do not change feature-gating or runtime-event
ordering tests.

## Frozen tests to preserve

- [runtime_events.rs](../../../scraper/tests/runtime_events.rs)
- [feature_gating.rs](../../../scraper/tests/feature_gating.rs)
- [default_no_runtime_event.rs](../../../scraper/tests/trybuild/default_no_runtime_event.rs)

## Target tests

- `work_started_completed_and_output_preserve_causal_order`
- `lag_and_queue_pressure_emit_ordered_runtime_events`
- disabled-feature runtime event tests remain green

## Scope

- Keep `RuntimeEventType = Infallible` without `runtime-events`.
- Compile runtime event emission only with `runtime-events`.
- Emit work started/completed/failed events.
- Emit lag and queue pressure events.
- Preserve causal ordering with work outputs.

## Out of scope

- Redfish work events.
- Application metrics/export mapping.

## Target commands

```sh
cargo test -p nv-redfish-scraper --test runtime_events
cargo test -p nv-redfish-scraper --features runtime-events --test runtime_events
cargo test -p nv-redfish-scraper --test feature_gating
```

## Done

- Runtime-event tests pass with and without the feature.
- Disabled builds cannot construct concrete runtime events.
