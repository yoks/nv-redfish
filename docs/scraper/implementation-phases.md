# Scraper implementation phases

This plan follows Phase 0. Phase 0 created the crate API and the frozen TDD
contract tests.

The implementation phases below are intentionally smaller than whole test
suites and larger than one assertion at a time. Each phase targets a coherent
runtime or adapter primitive and may make tests in several files pass.

## Frozen test rule

The tests created in Phase 0 are frozen.

During these implementation phases:

- do not rewrite tests to fit implementation,
- do not weaken assertions,
- do not mark behavior tests ignored,
- do not delete compile-fail tests or update `.stderr` files just to make a
  build pass,
- do not add broad placeholder APIs unless a frozen test or requirement requires
  them.

Tests may change only if:

- a requirement document changes,
- a test is proven to contradict the documented architecture,
- a test has a mechanical issue unrelated to the behavior it specifies, such as
  a typo in a symbol name.

When a test changes for one of those reasons, update the relevant requirement or
phase document in the same change.

## Phase 1: Core API Cleanup

Detailed runbook: [phase 1](phases/phase-01-core-api-cleanup.md).

Purpose:

- finish any small API refinements needed before behavior implementation,
- keep already-green API-bound and feature-gating tests green,
- remove accidental placeholder shape that is not required by tests or docs.

Frozen tests to preserve:

- `tests/api_bounds.rs`
- `tests/feature_gating.rs`
- `tests/trybuild/default_no_redfish_adapter.rs`
- `tests/trybuild/default_no_runtime_event.rs`
- `tests/trybuild/no_detached_redfish_command.rs`

Expected outcome:

- `cargo check -p nv-redfish-scraper` passes,
- `cargo clippy -p nv-redfish-scraper` passes,
- `cargo test -p nv-redfish-scraper --test api_bounds` passes,
- `cargo test -p nv-redfish-scraper --test feature_gating` passes.

## Phase 2: Basic Dispatch

Detailed runbook: [phase 2](phases/phase-02-basic-dispatch.md).

Purpose:

- implement enough scheduler traversal for one `run_once` call,
- query generator readiness,
- select a ready generator,
- call `take_next` only after selection,
- return `RunOutcome::Idle` or `RunOutcome::Dispatched` correctly.

Frozen tests to preserve:

- `tests/scheduling.rs`
- `tests/control.rs`
- `tests/discovery_flow.rs`
- `tests/output.rs`

Target tests:

- `no_work_is_dispatched_when_no_generator_is_ready`
- `ready_generator_dispatches_one_work_item`
- `run_once_dispatches_at_most_one_selected_work_item`
- the first discovery-flow tests that depend only on basic dispatch

Expected implementation scope:

- root and target tree traversal may be simple,
- fairness and cost policy can remain minimal,
- completion and rich stats can remain red until later phases.

## Phase 3: Ordered Outputs

Detailed runbook: [phase 3](phases/phase-03-ordered-outputs.md).

Purpose:

- enqueue successful work output,
- enqueue failed work output,
- preserve event ordering within one work item,
- preserve FIFO output ordering,
- implement `poll_output`, `drain_outputs`, and basic queue stats behavior.

Frozen tests to preserve:

- `tests/output.rs`
- `tests/completion.rs`
- `tests/control.rs`
- `tests/discovery_flow.rs`

Target tests:

- `successful_work_produces_ordered_work_output`
- `multiple_events_from_one_work_item_preserve_order`
- `failures_produce_ordered_work_error_output`
- `one_shot_drain_returns_all_available_outputs_in_fifo_order`
- `queue_pressure_is_reflected_in_stats`
- discovery-flow tests that consume outputs

Expected implementation scope:

- output queue remains generic over `E`,
- runtime event ordering can remain red until Phase 10,
- bounded queue drop/reject policy can stay minimal unless needed by tests.

## Phase 4: Completion Reporting

Detailed runbook: [phase 4](phases/phase-04-completion-reporting.md).

Purpose:

- report completion exactly once for every dispatched work item,
- report both success and failure outcomes,
- enqueue output before calling generator completion callback,
- release in-flight counters after completion,
- attach runtime-owned `WorkStats` to success and failure outputs.

Frozen tests to preserve:

- `tests/completion.rs`
- `tests/output.rs`
- `tests/scheduling.rs`

Target tests:

- `completion_is_reported_exactly_once_after_success`
- `completion_is_reported_exactly_once_after_failure`
- `output_is_enqueued_before_generator_completion_callback`
- `in_flight_counters_are_released_after_completion`
- `failed_work_keeps_runtime_owned_stats`

Expected implementation scope:

- completion metadata should be runtime-owned,
- scheduled work should not fabricate runtime statistics,
- no Redfish-specific logic enters the runtime.

## Phase 5: Control Safety And Tree Mutation

Detailed runbook: [phase 5](phases/phase-05-control-safety-tree-mutation.md).

Purpose:

- make target and generator control operations interact safely with scheduling,
- ensure removed targets remove attached generators,
- ensure removed generators are never queried again,
- preserve already queued outputs across removal,
- invalidate stale readiness after tree changes.

Frozen tests to preserve:

- `tests/control.rs`
- `tests/scheduling.rs`
- `tests/output.rs`

Target tests:

- `queued_outputs_survive_target_and_generator_removal`
- `tree_changes_invalidate_stale_readiness`
- all control tests should remain green.

Expected implementation scope:

- tree mutation should remove all scheduler references to removed generators,
- queued outputs should not be deleted by removal,
- pause/resume should affect future scheduling, not historical outputs.

## Phase 6: Capacity Limits

Detailed runbook: [phase 6](phases/phase-06-capacity-limits.md).

Purpose:

- enforce global maximum in-flight work,
- enforce per-target maximum in-flight work,
- account for admission and release around dispatch,
- expose in-flight saturation as runtime state for later stats/events.

Frozen tests to preserve:

- `tests/scheduling.rs`
- `tests/completion.rs`
- `tests/stats.rs`

Target tests:

- `target_and_global_in_flight_limits_are_respected`
- in-flight portions of completion and stats tests.

Expected implementation scope:

- limits can be conservative,
- cost-aware budget policy can wait until Phase 8,
- runtime-wide and per-target counters must not drift.

## Phase 7: Periodic Semantics

Detailed runbook: [phase 7](phases/phase-07-periodic-semantics.md).

Purpose:

- support requested generator intervals,
- ensure periodic generators create fresh work only when selected,
- avoid accumulating stale queued jobs for missed periods,
- calculate generator lag, missed intervals, and actual interval.

Frozen tests to preserve:

- `tests/scheduling.rs`
- `tests/stats.rs`
- `tests/runtime_events.rs`

Target tests:

- `periodic_generators_do_not_accumulate_stale_jobs`
- `generator_stats_report_lag_missed_intervals_and_actual_interval`
- `overload_is_not_reported_as_periodic_job_queue_depth`

Expected implementation scope:

- generator lag is the overload signal,
- periodic job queue depth must not become the overload signal,
- runtime event emission can remain red until Phase 10.

## Phase 8: Fair And Cost-Aware Scheduling

Detailed runbook: [phase 8](phases/phase-08-fair-cost-aware-scheduling.md).

Purpose:

- account for work costs,
- avoid permanent starvation of expensive work,
- provide target fairness,
- honor class service shares or a compatible fair scheduling policy.

Frozen tests to preserve:

- `tests/scheduling.rs`
- `tests/stats.rs`

Target tests:

- `cost_and_fairness_require_expensive_and_low_rate_work_to_run`
- `target_fairness_prevents_one_target_from_consuming_all_dispatches`
- cost/admission parts of scheduling tests.

Expected implementation scope:

- a simple DRR-like or equivalent policy is acceptable,
- implementation should leave room for richer class weights,
- scheduler metadata must remain Redfish-independent.

## Phase 9: Stats And Observability

Detailed runbook: [phase 9](phases/phase-09-stats-observability.md).

Purpose:

- expose global, per-target, per-class, and per-generator stats,
- report queue pressure,
- report throttling/starvation/saturation counts,
- keep stats deterministic and snapshot-based.

Frozen tests to preserve:

- `tests/stats.rs`
- `tests/output.rs`
- `tests/scheduling.rs`

Target tests:

- `runtime_stats_expose_per_target_class_and_generator_snapshots`
- `queue_pressure_is_reflected_in_stats`
- any remaining stats assertions in scheduling/completion tests.

Expected implementation scope:

- stats should be generic runtime facts,
- stats must not contain Redfish or application domain semantics,
- runtime event snapshots can be implemented in Phase 10.

## Phase 10: Runtime Events

Detailed runbook: [phase 10](phases/phase-10-runtime-events.md).

Purpose:

- implement `runtime-events` feature behavior,
- emit ordered scheduler/executor/queue runtime events,
- preserve causal ordering between runtime events and work outputs,
- keep runtime event emission code out of builds without the feature.

Frozen tests to preserve:

- `tests/runtime_events.rs`
- `tests/feature_gating.rs`
- `tests/trybuild/default_no_runtime_event.rs`

Target tests:

- `work_started_completed_and_output_preserve_causal_order`
- `lag_and_queue_pressure_emit_ordered_runtime_events`
- disabled-feature runtime event tests must remain green.

Expected implementation scope:

- `RuntimeEventType` remains `Infallible` without the feature,
- runtime event emission is compiled only with the feature,
- event ordering uses the same ordered output path as work results.

## Phase 11: Redfish Adapter Event Boundary

Detailed runbook: [phase 11](phases/phase-11-redfish-adapter-event-boundary.md).

Purpose:

- complete Redfish work event types,
- preserve BMC id, resource id, parent id, change kind, payload, metadata, and
  errors,
- support reconstruction records without execution handles,
- support serialization when `serde` is enabled.

Frozen tests to preserve:

- `tests/redfish_adapter_api.rs`
- `tests/feature_gating.rs`
- `tests/trybuild/no_detached_redfish_command.rs`

Target tests:

- Redfish event identity tests,
- no-execution-handle tests,
- reconstruction record tests,
- serde serialization tests.

Expected implementation scope:

- public events do not expose `B`, `ServiceRoot<B>`, `Chassis<B>`, or similar
  execution handles,
- generated payload integration remains represented by the `EntityPayload`
  boundary until compiler support exists,
- adapter code remains behind `redfish-adapter`.

## Phase 12: Typed Redfish Generator Builders

Detailed runbook: [phase 12](phases/phase-12-typed-redfish-generator-builders.md).

Purpose:

- add typed adapter generator builders for compiled capabilities,
- make builders close over valid `nv-redfish` objects,
- ensure invalid object/command pairings remain unrepresentable,
- keep application discovery policy outside the adapter.

Frozen tests to preserve:

- `tests/redfish_adapter_api.rs`
- `tests/feature_gating.rs`
- future capability-specific tests added only when the corresponding
  requirement and builder are introduced.

Target tests:

- typed builder shape tests,
- detached command compile-fail tests,
- future capability-gating tests.

Expected implementation scope:

- builders are feature-gated per capability,
- disabled capabilities hide builders, config fields, event payload variants,
  and fetch code,
- applications choose which compiled generators to add.

## Phase 13: Generated EntityPayload Integration

Detailed runbook: [phase 13](phases/phase-13-generated-entitypayload-integration.md).

Purpose:

- integrate with generated `EntityPayload` once CSDL compiler support exists,
- preserve generated Redfish schema data,
- preserve `@odata.id`, `@odata.etag`, and entity kind,
- preserve expanded `NavProperty<T>` payloads.

Frozen tests to preserve:

- existing adapter API tests,
- future generated-payload tests added with the codegen requirement change.

Target tests:

- generated payload identity tests,
- expanded payload preservation tests using real generated types,
- capability-specific payload variant feature-gating tests.

Expected implementation scope:

- do not create a parallel scraper domain model,
- serialized Redfish events contain read-side data and metadata only,
- execution-handle reconstruction remains optional helper behavior.
