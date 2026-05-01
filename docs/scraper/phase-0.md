# Scraper phase 0

Phase 0 establishes the public shape of the `scraper` crate and the complete
test plan for the architecture before runtime behavior is implemented.

This is a special phase. Later phases should be small implementation passes that
make one focused group of tests pass at a time.

## Goals

- Create the `scraper` crate in the workspace.
- Define the external API for the generic runtime.
- Define the external API for the Redfish adapter boundary.
- Define feature gates for runtime events and Redfish adapter capabilities.
- Add dependency-free tests that describe all required runtime behavior using
  fake generators, fake events, fake errors, and fake schedulable work.
- Add compile-time API tests for generic bounds, feature gating, and public
  usability.
- Add behavior tests that cover every architectural requirement at the runtime
  and adapter-boundary level before implementation phases start.
- Keep implementation minimal enough that Phase 0 does not accidentally decide
  scheduling policy details that belong in later phases.

## Non-goals

- Do not implement real Redfish fetching.
- Do not implement full scheduler fairness.
- Do not implement durable persistence, replay, or reconstruction.
- Do not add Carbide-specific reports, models, metrics, DB/API/Vault logic, or
  mutations.
- Do not add placeholder runtime knobs that are not exercised by tests.

## Deliverable shape

Phase 0 should leave the repository with a compiling `nv-redfish-scraper` crate
and a complete failing TDD test suite.

The crate may contain minimal implementations where needed for compilation, but
the behavior tests should be the authoritative plan for later phases.

Behavior tests that describe unimplemented functionality should not be ignored.
They should fail until the implementation satisfies them. Later phases make
tests green by changing production code only.

After Phase 0, tests are frozen unless a requirement document changes or a test
is proven to contradict the documented architecture. Later implementation
phases must not rewrite tests to fit the implementation.

Compile-time API tests should run in Phase 0. If an API contract depends on
generated Redfish support that does not exist yet, add a compile-fail or
feature-specific test that captures the expected contract and documents the
blocker in the test module.

## Fake test meaning

In this plan, fake tests are real tests with real assertions. "Fake" only means
the tests use local test doubles instead of external systems.

Fake test inputs may include:

- fake generators,
- fake scheduled work futures,
- fake work events,
- fake errors,
- fake clocks or explicit timestamps,
- fake Redfish-like payloads when generated `EntityPayload` support is not ready.

Fake tests must not:

- depend on HTTP,
- depend on BMC mocks,
- depend on real Redfish generated schemas unless the test explicitly targets the
  Redfish adapter API,
- depend on Carbide crates,
- use sleeps or timing races,
- encode implementation internals instead of public behavior.

These tests are the contract for the crate. They should be sufficient to cover
the requirements in `requirements.md`, `runtime.md`, `scheduling.md`,
`scheduling-root.md`, `scheduling-target.md`, and `redfish-adapter.md`.

## Crate layout

Initial crate layout:

```text
scraper/
  Cargo.toml
  src/
    lib.rs
    ids.rs
    runtime.rs
    control.rs
    output.rs
    generator.rs
    scheduler.rs
    stats.rs
    event.rs
    adapter/
      mod.rs
      redfish.rs
  tests/
    api_bounds.rs
    control.rs
    scheduling.rs
    output.rs
    completion.rs
    stats.rs
    runtime_events.rs
    feature_gating.rs
    redfish_adapter_api.rs
    discovery_flow.rs
    trybuild/
      default_no_redfish_adapter.rs
      default_no_redfish_adapter.stderr
      default_no_runtime_event.rs
      default_no_runtime_event.stderr
      no_detached_redfish_command.rs
      no_detached_redfish_command.stderr
    support/
      mod.rs
      fake_event.rs
      fake_error.rs
      fake_generator.rs
```

Modules may be merged if the implementation stays clearer, but the public API
should still read as these domains:

- ids and opaque identifiers,
- control API,
- generators and scheduled work,
- scheduler metadata,
- runtime outputs,
- runtime events,
- statistics,
- Redfish adapter API.

## Public API to establish

Phase 0 should establish names, ownership, and generic boundaries. Exact private
data structures can remain internal.

### Identifiers

Required public identifier types:

```rust
TargetId
GeneratorId
ClassId
```

Requirements:

- ids are opaque,
- ids do not expose Redfish semantics,
- generator ids can optionally carry or recover their parent target id if the
  chosen API requires it,
- public constructors and accessors are intentional,
- formatting behavior is explicit.

### Runtime

Required public runtime shape:

```rust
Runtime<E, Err>
RuntimeConfig
TargetLimits
GeneratorConfig
RuntimeHandle<E, Err>
```

The runtime API should support:

- `add_target`,
- `remove_target`,
- `update_target_limits`,
- `pause_target`,
- `resume_target`,
- `add_generator`,
- `remove_generator`,
- `update_generator`,
- `pause_generator`,
- `resume_generator`,
- `trigger_generator`,
- one-shot execution,
- polling or draining ordered outputs,
- statistics snapshots.

The runtime must not mention Redfish, BMCs, transports, generated schema types,
or application domain models.

### Generator and scheduled work

Required public concepts:

```rust
Generator<E, Err>
ScheduledWork<E, Err>
ScheduledWorkResult<E, Err>
WorkMeta
Readiness
CostUnits
WorkCompletion
```

Requirements:

- generators are stateful,
- readiness is pull-based,
- work is created only after selection,
- cost is reported before dispatch,
- completion is reported exactly once for dispatched work,
- generic event and error payloads avoid unnecessary trait bounds.

### Output

Required public concepts:

```rust
RuntimeOutput<E, Err, R = RuntimeEventType>
WorkResult<E, Err>
WorkSuccess<E>
WorkError<Err>
OutputQueueStats
```

Requirements:

- output order is FIFO across work success, work failure, and runtime events,
- successful work can contain multiple events and preserves per-work event
  ordering,
- failed work carries the generic error value without requiring formatting
  traits,
- output queue behavior is observable through stats,
- bounded output queues report pressure through bounded length plus dropped or
  rejected counts.

### Runtime events

Required public concepts:

```rust
RuntimeEvent
RuntimeEventType
```

Requirements:

- runtime events are behind a Cargo feature,
- when disabled, `RuntimeEventType` is `core::convert::Infallible`,
- runtime event emission code is not compiled when disabled,
- `RuntimeOutput::Runtime(_)` is not constructible by normal users when events
  are disabled.

Runtime event variants to reserve in the API:

- generator lagging,
- generator recovered,
- generator starved,
- target throttled,
- global throttled,
- event queue pressure,
- work started,
- work completed,
- work failed,
- scheduler statistics snapshot.

### Redfish adapter boundary

Phase 0 should define the adapter-facing public shape without implementing real
fetching.

Required public concepts:

```rust
RedfishEvent
RedfishResourceEvent
RedfishAdapterError
BmcId
ChangeKind
ResourceMetadata
GeneratorEvent
ScrapeEvent
ReconstructionRecord
```

Requirements:

- adapter APIs are feature-gated,
- fetch-side builders are generic over `B: nv_redfish::Bmc`,
- public Redfish events do not expose `B`, `ServiceRoot<B>`, `Chassis<B>`,
  `ComputerSystem<B>`, or other execution handles,
- resource events include BMC id, `ODataId`, optional parent `ODataId`, change
  kind, optional payload, metadata, and errors when applicable,
- generated `EntityPayload` integration is represented by a narrow type alias or
  trait boundary until the CSDL compiler support exists.
- Redfish events and reconstruction records are serializable when the `serde`
  feature is enabled.

### Statistics

Required public concepts:

```rust
RuntimeStats
TargetStats
ClassStats
GeneratorStats
WorkStats
OutputQueueStats
```

Requirements:

- runtime statistics expose global counters,
- runtime statistics expose per-target snapshots,
- runtime statistics expose per-class snapshots,
- runtime statistics expose per-generator snapshots,
- generator statistics expose lag, missed intervals, and actual interval,
- work statistics expose runtime-owned completion and latency metadata,
- overload is observable through lag, starvation, throttling, saturation, and
  queue pressure, not stale periodic job depth.

## Test inventory

Phase 0 should add tests by behavior domain, not implementation phase.

### API bounds tests

File: `tests/api_bounds.rs`

Tests:

- runtime output works with event payloads that are not `Clone`,
- runtime output works with event payloads that are not `Debug`,
- runtime output works with event payloads that are not `Eq` or `PartialEq`,
- work errors carry error values that are not formatting-friendly,
- public ids remain opaque,
- common API examples compile without Redfish dependencies.

### Control tests

File: `tests/control.rs`

Tests:

- add target,
- remove target,
- pause and resume target,
- update target limits,
- add generator under target,
- remove generator,
- pause and resume generator,
- trigger generator,
- removing a target removes attached generators,
- removed generators are never queried again,
- queued outputs survive target or generator removal.

### Scheduling tests

File: `tests/scheduling.rs`

Tests:

- no work is dispatched when no target is ready,
- ready generator dispatches one work item,
- `run_once` dispatches at most one selected work item,
- generator readiness is queried before selection,
- generator creates work only after selection,
- target in-flight limits are respected,
- global in-flight limits are respected,
- work cost participates in admission,
- expensive work is not permanently starved,
- class weights or service shares affect selection,
- target fairness prevents one target from consuming all dispatches,
- tree changes invalidate stale readiness,
- periodic generators do not accumulate one stale job per missed interval.

### Output tests

File: `tests/output.rs`

Tests:

- successful work produces ordered `RuntimeOutput::Work`,
- multiple events from one work item preserve order,
- failures produce ordered `RuntimeOutput::Work(Err(_))`,
- runtime events are ordered with work outputs when enabled,
- one-shot drain returns all available outputs,
- stream or polling API observes FIFO order,
- bounded queue pressure is reflected in stats.

### Completion tests

File: `tests/completion.rs`

Tests:

- completion is reported exactly once after success,
- completion is reported exactly once after failure,
- output is enqueued before generator completion callback,
- in-flight counters are released after completion,
- generator lag state can be updated from completion,
- failed work does not lose runtime-owned stats.

### Runtime event tests

File: `tests/runtime_events.rs`

Tests:

- runtime event feature exposes `RuntimeEvent`,
- disabled runtime event feature uses `Infallible`,
- disabled runtime event build does not compile event emission paths,
- lagging generator can emit lag event when enabled,
- queue pressure can emit pressure event when enabled,
- work started/completed events exactly bracket successful work output when
  enabled,
- work started/failed events exactly bracket failed work output when enabled,
- lag and queue pressure runtime events are ordered with work outputs.

### Stats tests

File: `tests/stats.rs`

Tests:

- runtime exposes per-target, per-class, and per-generator stats,
- generator stats report lag, missed intervals, and actual interval,
- periodic overload is not represented as stale queued job depth,
- bounded output queue pressure reports dropped or rejected outputs instead of
  unbounded queue growth.

### Feature-gating tests

File: `tests/feature_gating.rs`

Tests:

- disabled Redfish capability hides related builders,
- disabled capability hides related event payload variants,
- disabled capability hides related config fields,
- enabled capability exposes only its typed builders,
- runtime-only build does not depend on `nv-redfish`,
- `redfish-adapter` module is absent without the feature,
- concrete `RuntimeEvent` type is absent without the runtime event feature,
- detached Redfish command language types are absent.

These use `trybuild` where normal integration tests cannot express compile-fail
expectations.

### Redfish adapter API tests

File: `tests/redfish_adapter_api.rs`

Tests:

- adapter generator builders close over typed `nv-redfish` objects,
- service-root builders produce runtime generators, not just marker objects,
- invalid object and command pairings are not expressible,
- Redfish resource event contains required identity fields,
- Redfish event payload does not contain execution handles,
- generated `EntityPayload` boundary preserves schema payload identity,
- generated `EntityPayload` implements the scraper payload boundary and serde
  when generated support is enabled,
- expanded payload preservation is represented in the event API,
- child events can carry `parent_odata_id`,
- reconstruction records preserve hierarchy identity without execution handles,
- reconstruction records can be derived from resource events,
- serialized Redfish resource events contain read-side fields and metadata but
  not execution handles,
- Redfish events and reconstruction records are serializable when `serde` is
  enabled.

Tests may initially use fake payloads or a placeholder trait boundary if
generated `EntityPayload` is not available yet.

### Discovery flow tests

File: `tests/discovery_flow.rs`

Tests:

- application can start with one service-root-like generator,
- application can consume output and add more generators,
- application can request narrow scraping only,
- not requested, requested missing, requested failed, and requested successful
  states are distinguishable,
- runtime remains policy-free during application-driven discovery.

## Phase 0 acceptance criteria

- `scraper` is listed in workspace members.
- `cargo check -p nv-redfish-scraper` succeeds.
- `cargo test -p nv-redfish-scraper` runs the full test suite and is expected to
  fail on unimplemented behavior.
- `cargo test -p nv-redfish-scraper --no-run` succeeds.
- `cargo test -p nv-redfish-scraper --all-features --no-run` succeeds.
- Feature-gating `trybuild` tests pass for their configured features.
- Failing tests clearly identify the missing behavior they require.
- Public Rust files contain the required license header.
- Crate root uses the scraper lint posture from the style guide.
- Runtime modules compile without `nv-redfish`.
- Redfish adapter code is isolated behind adapter features.
- Tests are grouped by behavior domain.
- No behavior tests are ignored merely because implementation is missing.
- No public API contains Carbide-specific types or concepts.
- The frozen tests cover the requirements in `requirements.md`, `runtime.md`,
  `scheduling.md`, `scheduling-root.md`, `scheduling-target.md`, and
  `redfish-adapter.md` at the public behavior/API level.

## Later phases

Later implementation work is described in
[Scraper implementation phases](implementation-phases.md).

Each phase must make its target failing tests pass by changing implementation
code only. Tests should remain unchanged unless the documented requirements
change or the test is demonstrably wrong.
