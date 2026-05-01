# Phase 2: Class weights and target fairness

## Goal

Replace the trivial generator-level round-robin with a fair scheduler
that honours both per-class weights and per-target shares. Phase 2 does
not change cost admission or in-flight semantics.

## Tests to turn green

| File | Test |
| ---- | ---- |
| `tests/scheduling.rs` | `class_weights_or_service_shares_affect_selection` |
| `tests/scheduling.rs` | `target_fairness_prevents_one_target_from_consuming_all_dispatches` |

## Design decisions

- **Class weights.** `GeneratorConfig::weight` (default `1`) feeds a
  weighted deficit-round-robin (DRR) among generators that share a
  class. Generators without an explicit class share an implicit
  `<unclassified>` bucket whose weight is the per-config default.
- **Class share, not per-generator share.** Two generators in the same
  class with weight 3 split that class's quantum equally; they do not
  each get a 3-share. This matches the "service shares" reading of the
  scheduling document.
- **Target fairness.** A second DRR layer sits above class selection.
  Each target receives an equal quantum per round; classes only compete
  with classes inside their target. With three targets each holding a
  single generator, the asymmetric example in the test
  (`heavy=5 generators, light=1`) yields ≥ 30% share to the light
  target despite generator-level RR alone giving it 1/6.
- **Two-level scheduler shape.** `select_candidate` first picks a target
  using the target DRR, then within that target picks a class using the
  class DRR. The second-level DRR consults the same `GeneratorConfig`
  weights surfaced in Phase 0; no new public API is required.
- **Determinism.** DRR with integer quanta is fully deterministic for
  a given history of (target, class, dispatched-cost) triples. The
  state-machine and discovery tests already assert determinism for
  fixed seeds; Phase 2 must keep those green.

## Acceptance criteria

- Both class-weights and target-fairness tests pass.
- No regression in Phase 0/1 tests.
- The scheduler still satisfies the round-robin tests in `scheduling.rs`
  for the single-class, single-target case (DRR with equal weights
  collapses to RR).
- `cargo clippy -p nv-redfish-scraper --all-features --all-targets -- -D warnings`
  passes.

## Out of scope (deferred)

- Periodic scheduling — Phase 3.
- Lag/actual-interval reporting — Phase 4.
- Lag-trigger runtime events — Phase 5.
