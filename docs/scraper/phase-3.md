# Phase 3: Periodic generators and tree-change readiness

## Goal

Introduce real periodic scheduling: generators that report a
`Readiness { ready: false, next_update_at: Some(deadline) }` are dormant
until their deadline; on tree-change events any cached readiness must be
invalidated so the runtime never relies on stale state.

## Tests to turn green

| File | Test |
| ---- | ---- |
| `tests/scheduling.rs` | `tree_changes_invalidate_stale_readiness` |
| `tests/scheduling.rs` | `periodic_generators_do_not_accumulate_one_stale_job_per_missed_interval` |

## Design decisions

- **Cached readiness.** The runtime caches the most recent `Readiness`
  per generator, including its `next_update_at` (if any). Subsequent
  `select_candidate` calls before the deadline skip the generator
  without invoking `update_ready` again.
- **Cache invalidation triggers.**
  - Adding or removing a target.
  - Adding, removing, pausing, resuming a generator.
  - Updating target limits or generator config.
  - An explicit `trigger_generator` call.
  These already cause a waker wake; Phase 3 additionally clears the
  cached readiness so the next `select_candidate` re-queries
  `update_ready` from scratch.
- **No queued backlog.** The cache never accumulates "missed" work
  items. A generator that wakes from a missed deadline is simply
  considered ready *now*; the application is responsible for deciding
  whether to skip catch-up.
- **Stats hook.** A missed deadline increments
  `GeneratorStats::missed_intervals`. The actual numeric expectations
  are validated in Phase 4 stats tests; Phase 3 only wires the counter.

## Acceptance criteria

- Both periodic tests pass without flaky timing dependencies. The
  scheduling test driver remains synchronous (no real sleeps); the
  cache uses `Instant` values supplied by `Generator::update_ready`,
  which the test harness controls via the existing `Harness`.
- Phase 0–2 tests still green.
- `cargo clippy -p nv-redfish-scraper --all-features --all-targets -- -D warnings`
  passes.

## Out of scope (deferred)

- Numeric stats expansion — Phase 4.
- Runtime event emission for lag/recovery — Phase 5.
