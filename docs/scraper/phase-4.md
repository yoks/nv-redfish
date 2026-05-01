# Phase 4: Stats expansion and bounded queue accounting

## Goal

Populate the per-generator stats fields that Phase 0 left zeroed and
make the bounded output queue surface drop accounting in
`OutputQueueStats::dropped` rather than letting outputs vanish silently.

## Tests to turn green

| File | Test |
| ---- | ---- |
| `tests/stats.rs` | `generator_stats_report_lag_missed_intervals_and_actual_interval` |
| `tests/stats.rs` | `bounded_queue_pressure_reports_dropped_or_rejected_outputs_not_unbounded_growth` |
| `tests/output.rs` | `bounded_queue_pressure_is_reflected_in_stats` |
| `tests/completion.rs` | `generator_lag_state_can_be_updated_from_completion` |

## Design decisions

- **`actual_interval`.** Tracked between consecutive `WorkCompletion`
  observations for a given `GeneratorId`. The runtime stores the
  previous completion `Instant` per generator and writes the delta into
  `GeneratorStats::actual_interval` on every new completion. Phase 0's
  `Duration::default()` becomes a `Some(Duration)` after ≥ 2
  completions.
- **`missed_intervals`.** Incremented on each scheduling pass that
  observes `now > generator.cached_next_update_at` (Phase 3 wires the
  cache; Phase 4 wires the counter).
- **Bounded queue drop counter.** `RuntimeState::enqueue_output` already
  increments `output_dropped` when the queue is full; Phase 4 surfaces
  this counter through `RuntimeStats::output_queue.dropped`. The
  invariant under test: with a capacity of 1 and 10 immediate outputs,
  `dropped >= 1`.
- **No reordering.** Phase 4 changes accounting only. The FIFO order of
  delivered outputs and their content remain unchanged.

## Acceptance criteria

- All four tests above pass.
- The state-machine "internally consistent" test
  (`stats_snapshot_is_internally_consistent_under_generated_operation_sequences`)
  still passes.
- No regression in Phase 0–3 tests.

## Out of scope (deferred)

- Lag-trigger runtime events — Phase 5.
- Adapter fetches — Phases 6/7/8.
