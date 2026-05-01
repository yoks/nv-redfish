# Scraper phase 2: basic dispatch

## Purpose

Implement the first real runtime scheduling path for one `run_once` call.

## Required reading

- [Phase 0 contract](../phase-0.md)
- [Implementation phases](../implementation-phases.md)
- [Requirements](../requirements.md)
- [Runtime](../runtime.md)
- [Scheduling overview](../scheduling.md)
- [Root scheduler](../scheduling-root.md)
- [Target scheduler](../scheduling-target.md)
- [Rust style guide](../rust-style-guide.md)

## Frozen test rule

The Phase 0 tests are frozen. Make tests green by changing implementation code
only.

## Frozen tests to preserve

- [scheduling.rs](../../../scraper/tests/scheduling.rs)
- [control.rs](../../../scraper/tests/control.rs)
- [discovery_flow.rs](../../../scraper/tests/discovery_flow.rs)
- [output.rs](../../../scraper/tests/output.rs)
- [fake_generator.rs](../../../scraper/tests/support/fake_generator.rs)

## Target tests

- `no_work_is_dispatched_when_no_generator_is_ready`
- `ready_generator_dispatches_one_work_item`
- `run_once_dispatches_at_most_one_selected_work_item`
- discovery-flow tests that require only basic dispatch

## Scope

- Traverse target/generator tree.
- Query readiness before selection.
- Select one ready generator.
- Call `take_next` only after selection.
- Execute at most one selected work item per `run_once`.
- Return `RunOutcome::Idle` or `RunOutcome::Dispatched` correctly.

## Out of scope

- Fair scheduling.
- Full cost-aware admission.
- Runtime events.
- Rich stats.

## Target commands

```sh
cargo test -p nv-redfish-scraper --test scheduling
cargo test -p nv-redfish-scraper --test discovery_flow
```

## Done

- Basic dispatch target tests pass.
- Already-green control/API/feature tests remain green.
- Runtime still contains no Redfish types.
