# Scraper phase 3: ordered outputs

## Purpose

Implement the ordered output queue for successful and failed scheduled work.

## Required reading

- [Phase 0 contract](../phase-0.md)
- [Requirements](../requirements.md)
- [Runtime](../runtime.md)
- [Rust style guide](../rust-style-guide.md)

## Frozen test rule

The Phase 0 tests are frozen. Do not edit output tests to fit queue
implementation details.

## Frozen tests to preserve

- [output.rs](../../../scraper/tests/output.rs)
- [completion.rs](../../../scraper/tests/completion.rs)
- [control.rs](../../../scraper/tests/control.rs)
- [discovery_flow.rs](../../../scraper/tests/discovery_flow.rs)

## Target tests

- `successful_work_produces_ordered_work_output`
- `multiple_events_from_one_work_item_preserve_order`
- `failures_produce_ordered_work_error_output`
- `one_shot_drain_returns_all_available_outputs_in_fifo_order`
- `queue_pressure_is_reflected_in_stats`
- discovery-flow tests that consume outputs

## Scope

- Enqueue `RuntimeOutput::Work(Ok(_))`.
- Enqueue `RuntimeOutput::Work(Err(_))`.
- Preserve event ordering inside one work item.
- Preserve FIFO order across output queue operations.
- Implement `poll_output`, `drain_outputs`, and basic queue stats.

## Out of scope

- Runtime event ordering.
- Complex bounded queue drop/reject policies unless required by frozen tests.

## Target commands

```sh
cargo test -p nv-redfish-scraper --test output
cargo test -p nv-redfish-scraper --test discovery_flow
```

## Done

- Ordered output target tests pass.
- Output queue remains generic over work event type `E`.
