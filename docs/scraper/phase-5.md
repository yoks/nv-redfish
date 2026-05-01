# Phase 5: Runtime events

## Goal

Emit the `RuntimeEvent` variants declared in Phase 0 in the correct
causal order. Runtime events bracket work outputs, surface lag/queue
pressure, and report control-plane changes — but only when the
`runtime-events` feature is enabled. Without the feature, the event
type is `core::convert::Infallible` and emission paths must not be
compiled.

## Tests to turn green

All tests live in `tests/runtime_events.rs::emission` and are gated on
`feature = "runtime-events"`. Phase 5 turns the following from red to
green:

- `work_started_and_completed_events_bracket_successful_work_output`
- `work_started_and_failed_events_bracket_failed_work_output`
- `runtime_events_contain_runtime_ids_only_no_user_payload`
- `runtime_events_are_not_emitted_for_failed_control_operations`
- `lagging_generator_can_emit_lag_event`
- `queue_pressure_can_emit_pressure_event`
- `target_and_generator_control_plane_events_emitted_in_documented_order`

## Design decisions

- **Bracketing.** `try_dispatch_one` emits a
  `RuntimeOutput::Runtime(RuntimeEvent::WorkStarted { generator_id })`
  before pushing the future into the in-flight set. `finalize_completion`
  emits the matching `WorkCompleted` or `WorkFailed` after the work
  output is enqueued.
- **No nested brackets.** Phase 5 does not enforce sequential
  execution; brackets may interleave when multiple work items are
  in flight, but each `WorkStarted` has exactly one matching
  `WorkCompleted`/`WorkFailed`.
- **Lag.** Phase 4 records `missed_intervals`; Phase 5 emits
  `GeneratorLagging { generator_id }` the first time
  `missed_intervals` transitions from 0 → ≥ 1, and `GeneratorRecovered`
  when it returns to 0.
- **Queue pressure.** Whenever `output_queue.len()` exceeds a
  high-water mark (initially configured as `cap / 2` for bounded
  queues, otherwise unset), the runtime emits a single
  `EventQueuePressure { queued }`; subsequent pressure events are
  rate-limited so the event stream itself does not amplify pressure.
- **Control-plane events.** `add_target`, `remove_target`,
  `pause_target`, `resume_target`, and the equivalent generator
  operations emit one event per *successful* mutation. Failed
  operations (e.g., removing a missing target) are silent.
- **Non-emission when feature off.** All emission paths are guarded by
  `#[cfg(feature = "runtime-events")]`. The `RuntimeOutput::Runtime`
  variant remains uninhabited (`Infallible`) when the feature is off,
  so this is enforced by the type system.

## Acceptance criteria

- All seven tests above pass under
  `cargo test -p nv-redfish-scraper --features runtime-events`.
- The two type-level tests
  (`runtime_event_type_is_infallible_when_feature_disabled`,
  `runtime_event_type_is_concrete_enum_when_feature_enabled`) and
  `output_type_can_carry_default_runtime_event_type` continue to pass.
- No regression in Phase 0–4 tests.
- `cargo clippy -p nv-redfish-scraper --all-features --all-targets -- -D warnings`
  passes.

## Out of scope (deferred)

- Adapter fetches — Phase 6/7.
- Reconstruction record derivation — Phase 8.
