# Phase 1: Scheduler admission, cost, and in-flight semantics

## Goal

Make the runtime cost-aware and clarify what happens to work that is
already in flight when its generator or target is removed. Phase 1 keeps
fairness simple (round-robin by generator) but introduces:

1. cost-aware admission against a per-target round budget;
2. anti-starvation for expensive work; and
3. completion delivery for in-flight work whose generator or target is
   removed.

## Tests to turn green

| File | Test |
| ---- | ---- |
| `tests/scheduling.rs` | `work_cost_participates_in_admission` |
| `tests/scheduling.rs` | `expensive_work_is_not_permanently_starved` (regression-only) |
| `tests/scheduling.rs` | `global_in_flight_limit_is_respected` (regression-only) |
| `tests/completion.rs` | `completion_is_still_called_once_when_removal_is_requested_while_work_is_in_flight` |
| `tests/control.rs` | `removing_a_generator_while_work_is_in_flight_does_not_cancel_that_work` (regression-only) |
| `tests/control.rs` | `removing_a_target_while_child_work_is_in_flight_waits_for_completion` (regression-only) |
| `tests/control.rs` | `graceful_shutdown_drains_already_selected_or_in_flight_work` (regression-only) |

## Design decisions

- **Round budget.** Every `select_candidate` invocation operates inside a
  short-lived "round". A round is bounded by `TargetLimits::max_cost_per_round`
  for each target; the runtime maintains a per-target cumulative cost
  counter that resets when the round ends.
- **Round boundary.** A round ends when the runtime exits its inner
  dispatch loop (no more eligible candidates this poll) or after a
  configured number of dispatches, whichever comes first. The boundary is
  internal; tests assert behaviour through admission and not directly
  through round counts.
- **Cost-aware admission.** Before calling `take_next` the scheduler
  checks `target.budget + work.cost <= TargetLimits::max_cost_per_round`.
  If the sum would overflow the budget, the generator stays "ready" and
  the scheduler moves on. If the cost is *individually* greater than the
  round budget, the scheduler stages the work for a fresh round in which
  the budget is reset by the standalone work item.
- **Anti-starvation.** A deficit counter per generator tracks rounds in
  which its work was admission-blocked. Once the deficit exceeds
  `GeneratorConfig::weight.unwrap_or(1)` rounds, the scheduler grants the
  generator a one-shot exception and admits its next work item even if
  the round budget would be exceeded.
- **In-flight survives removal.** Removal of a generator transfers
  ownership of the in-flight `Future` from the generator entry to a
  per-runtime "orphaned" set. Completion processing still calls the
  generator's `on_complete` if a captured `Arc<dyn Generator>` exists; if
  the generator object was moved at removal time, the runtime emits a
  `WorkCompletion` and updates stats but skips the user-facing callback.
- **Cursor invariants.** The dispatch cursor is clamped after every
  control-plane mutation that affects `generator_order`. A removed
  generator never appears in the candidate list of any subsequent
  `select_candidate` call. The state-machine test
  `removed_generator_is_never_picked_as_a_dispatch_candidate` verifies
  this from the outside.

## Acceptance criteria

- All "tests to turn green" listed above pass.
- No green test from Phase 0 regresses.
- `cargo clippy -p nv-redfish-scraper --all-features --all-targets -- -D warnings`
  passes.
- The runtime never silently drops dispatched work; either work
  completes and produces a `RuntimeOutput::Work(_)`, or shutdown drains
  it.

## Out of scope (deferred)

- Class weights and target fairness across multiple targets — Phase 2.
- Periodic scheduling — Phase 3.
- Stats lag/actual_interval — Phase 4.
- Runtime events emission — Phase 5.
