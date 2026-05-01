# Scraper phase 8: fair and cost-aware scheduling

## Purpose

Add fairness and cost-aware scheduling so expensive or low-rate work is not
starved.

## Required reading

- [Phase 0 contract](../phase-0.md)
- [Requirements](../requirements.md)
- [Scheduling overview](../scheduling.md)
- [Root scheduler](../scheduling-root.md)
- [Target scheduler](../scheduling-target.md)
- [Rust style guide](../rust-style-guide.md)

## Frozen test rule

The Phase 0 tests are frozen. Do not weaken fairness tests to match a simpler
scheduler.

## Frozen tests to preserve

- [scheduling.rs](../../../scraper/tests/scheduling.rs)
- [stats.rs](../../../scraper/tests/stats.rs)

## Target tests

- `cost_and_fairness_require_expensive_and_low_rate_work_to_run`
- `target_fairness_prevents_one_target_from_consuming_all_dispatches`
- cost/admission assertions in scheduling tests

## Scope

- Account for `CostUnits`.
- Avoid permanent starvation of expensive work.
- Provide fair target selection.
- Provide class service-share behavior or equivalent.

## Out of scope

- Redfish-specific priority policy.
- Application discovery policy.

## Target commands

```sh
cargo test -p nv-redfish-scraper --test scheduling
cargo test -p nv-redfish-scraper --test stats
```

## Done

- Fairness target tests pass.
- Scheduler still operates only on abstract metadata.
