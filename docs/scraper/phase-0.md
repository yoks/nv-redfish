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
- Do not add a separate one-shot execution API or batch drain API when
  `next().await` is the runtime driver and output consumer.

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
Runtime<Ev, Err>
RuntimeConfig
TargetLimits
GeneratorConfig
RuntimeHandle<Ev, Err>
```

The runtime API should support:

- `Runtime::next(&mut self).await` as the ordered output consumer and runtime
  driver,
- graceful shutdown,
- adding targets,
- removing targets,
- updating target limits,
- pausing and resuming targets,
- adding generators,
- removing generators,
- updating generators,
- pausing and resuming generators,
- triggering generators,
- statistics snapshots.

The runtime must not mention Redfish, BMCs, transports, generated schema types,
or application domain models.

### `next().await` Runtime Driver

`Runtime::next(&mut self).await` is the Phase 0 runtime step and output
interface.

Required behavior:

1. If the output queue already contains an item, return the oldest item
   immediately.
2. Otherwise, scan schedulable generators in scheduler order.
3. Skip generators that are not ready.
4. Call `take_next` only on the selected ready generator.
5. If `take_next` returns `None`, continue scanning during the same `next` call.
6. If a work item is returned, execute at most that one work item.
7. Enqueue the resulting `RuntimeOutput::Work`.
8. Report completion to the originating generator exactly once.
9. Return the oldest queued output.
10. If no output is queued and no work can be selected, wait for a wake source
    instead of returning an idle value or spinning.

Phase 0 exposes `next().await` as the public runtime execution and output API.
Tests should consume runtime output by awaiting `next()`.

Wake sources include control-plane changes and newly enqueued outputs. Later
phases may add timer wakeups, shutdown wakeups, or other control triggers.

### Async And Sync Interaction

Control APIs remain synchronous. They may briefly lock runtime state, but they
must not wait for work futures.

`next().await` must not hold runtime-state locks while awaiting scheduled work.
Control-plane changes must be able to occur while selected work is in flight.

When `next().await` cannot make progress, it should park the current task using
executor-friendly waker logic rather than blocking an executor thread.

### Graceful shutdown

Phase 0 should expose a graceful shutdown API on the runtime control surface:

```rust
graceful_shutdown()
```

The exact receiver follows the chosen control API shape, but the operation must
be synchronous and idempotent.

Required behavior:

- the first call starts graceful shutdown,
- later calls do nothing,
- after shutdown starts, mutating control APIs reject new target and generator
  changes,
- active generators are removed from scheduler selection,
- no new work is selected after shutdown starts,
- already selected or in-flight work is allowed to complete,
- work output from already selected or in-flight work remains ordered before
  shutdown completion,
- completion is still reported exactly once for completed in-flight work,
- `Runtime::next(&mut self).await` continues returning already queued output
  before shutdown completion,
- once shutdown has drained in-flight work and prior queued output, `next()`
  returns the runtime shutdown output,
- after shutdown output has been returned once, later `next()` calls return the
  shutdown output immediately.

Shutdown is a runtime lifecycle output, not an application work event. If runtime
events are enabled, do not also emit a separate runtime event just to say
shutdown completed unless a later requirements document adds that event.

### Generator and scheduled work

Required public concepts:

```rust
Generator<Ev, Err>
ScheduledWork<Ev, Err>
ScheduledWorkResult<Ev, Err>
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
WorkResult<Ev, Err>
WorkSuccess<E>
WorkError<Err>
OutputQueueStats
```

Requirements:

- output order is FIFO across work success, work failure, and runtime events,
- runtime shutdown is observable through the ordered output API,
- successful work can contain multiple events and preserves per-work event
  ordering,
- failed work carries the generic error value without requiring formatting
  traits,
- `next().await` is the consumer-facing ordered output API,
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

The test suite is the implementation contract. It must cover the full public
behavior envelope, not only the happy path. If behavior has observable variants,
there should be tests for success, failure, empty state, missing object, removed
object, feature-disabled, feature-enabled, and ordering/concurrency edge cases
where applicable.

The suite should use whatever test technique best captures the requirement:

- deterministic fake generators for normal runtime behavior,
- test-controlled futures for in-flight and wakeup behavior,
- fake events and fake errors for payload behavior,
- mocks or spy objects when call counts and call ordering matter,
- compile-fail tests for feature gates and unavailable APIs,
- property-based tests for scheduler/order/state-machine invariants when
  enumerating examples would leave gaps.

Tests must assert invariants directly. A test that only proves "something was
returned" is not sufficient when the requirement is about identity, order,
single execution, callback count, feature gating, or absence of accidental trait
bounds.

Minimum cross-cutting invariants:

- every emitted work output has the runtime-provided generator id that produced
  it,
- `generator_id.target_id()` is correct for every generated id,
- output order is FIFO and causal across work, runtime events, and shutdown,
- `take_next` is called only for selected generators,
- completion is called exactly once per executed work item,
- output is enqueued before completion callback,
- removed generators are never queried for readiness again,
- no runtime state lock is held while awaiting scheduled work,
- pending `next().await` calls wake only from real wake sources,
- feature-disabled code paths do not compile hidden runtime-event or adapter
  APIs.

### API bounds tests

File: `tests/api_bounds.rs`

Tests:

- runtime output works with event payloads that are not `Clone`,
- runtime output works with event payloads that are not `Debug`,
- runtime output works with event payloads that are not `Eq` or `PartialEq`,
- work errors carry error values that are not formatting-friendly,
- scheduled work can use non-`'static` futures when the runtime lifetime permits
  it,
- public payload types do not gain accidental `Send`, `Sync`, `Clone`, `Debug`,
  `Eq`, `PartialEq`, `Display`, or `Error` bounds unless a documented API
  requires them,
- `Runtime` is not accidentally cloneable if the chosen public API requires one
  consumer,
- runtime control handles are cloneable if the chosen public API includes a
  handle,
- public ids remain opaque,
- raw id internals are not constructible through public API,
- common API examples compile without Redfish dependencies,
- runtime-only tests compile with default features,
- runtime-only tests compile with `--all-features`.

### Control tests

File: `tests/control.rs`

Tests:

- add target,
- add multiple targets and verify monotonic id allocation,
- remove missing target returns the documented false/error shape,
- remove target,
- remove target twice returns the documented false/error shape on the second
  call,
- pause and resume target,
- update target limits,
- add generator under target,
- add generator under missing target fails with the documented error,
- add generator under removed target fails with the documented error,
- remove generator,
- remove missing generator returns the documented false/error shape,
- remove generator twice returns the documented false/error shape on the second
  call,
- pause and resume generator,
- trigger generator,
- removing a target removes attached generators,
- removed generators are never queried again,
- removed targets remove every attached generator in deterministic order,
- queued outputs survive target or generator removal,
- graceful shutdown drains already selected or in-flight work,
- graceful shutdown rejects later mutating control operations,
- graceful shutdown with no targets produces the documented shutdown output,
- graceful shutdown is idempotent,
- graceful shutdown does not drop already queued work outputs,
- graceful shutdown does not select new work after shutdown starts,
- removing a generator while work is in flight does not cancel that work,
- removing a target while child work is in flight waits internally for child
  completion/finalization,
- control-plane changes wake a pending `next().await` when they make progress
  possible,
- control-plane changes that cannot make progress do not cause busy-polling.

### Scheduling tests

File: `tests/scheduling.rs`

Tests:

- no work is produced when no target is ready,
- `next().await` parks when no output and no ready work exist,
- ready generator produces one work output through `next().await`,
- `next().await` executes at most one selected work item before returning output,
- generator readiness is queried before selection,
- generator creates work only after selection,
- not-ready generators are skipped without calling `take_next`,
- if a ready generator returns `None` from `take_next`, scanning continues in
  the same `next` call,
- stale or removed scheduler entries are skipped and not queried,
- round-robin order is deterministic across at least two full cycles,
- round-robin cursor advances when a generator is not ready,
- round-robin cursor advances when `take_next` returns `None`,
- round-robin cursor resumes after the generator that produced work,
- insertion during runtime operation participates only according to documented
  scheduler semantics,
- removal during runtime operation does not corrupt scheduler cursor state,
- target in-flight limits are respected,
- global in-flight limits are respected,
- work cost participates in admission,
- expensive work is not permanently starved,
- class weights or service shares affect selection,
- target fairness prevents one target from consuming all dispatches,
- tree changes invalidate stale readiness,
- periodic generators do not accumulate one stale job per missed interval,
- property-based scheduler tests generate add/remove/ready/not-ready/none/work
  operation sequences and assert deterministic order, no duplicate candidates in
  one scan, no removed ids returned, and cursor progress.

### Output tests

File: `tests/output.rs`

Tests:

- successful work produces ordered `RuntimeOutput::Work`,
- successful work with an empty event vector still produces a success output,
- multiple events from one work item preserve order,
- failures produce ordered `RuntimeOutput::Work(Err(_))`,
- failed work carries the original generic error value,
- failed work still counts as executed work for completion and ordering,
- runtime events are ordered with work outputs when enabled,
- `next().await` observes FIFO order,
- queued output is returned before scanning/selecting more work,
- output produced before generator removal remains observable,
- output produced before target removal remains observable,
- shutdown output is returned only after older queued output and in-flight work
  output,
- shutdown output is returned immediately by later `next()` calls,
- bounded queue pressure is reflected in stats,
- property-based output tests generate mixed work success/failure/runtime-event
  enqueue sequences and assert FIFO preservation and no lost/duplicated output.

### Completion tests

File: `tests/completion.rs`

Tests:

- completion is reported exactly once after success,
- completion is reported exactly once after failure,
- output is enqueued before generator completion callback,
- completion includes the correct runtime-provided generator id,
- completion outcome is `Succeeded` for `Ok(Vec<_>)`,
- completion outcome is `Failed` for `Err(_)`,
- completion is not called when no work is selected,
- completion is not called when `take_next` returns `None`,
- completion is still called once when removal is requested while work is in
  flight,
- in-flight counters are released after completion,
- generator lag state can be updated from completion,
- failed work does not lose runtime-owned stats,
- completion callbacks cannot observe missing queued output when they inspect
  shared test state.

### Runtime event tests

File: `tests/runtime_events.rs`

Tests:

- runtime event feature exposes `RuntimeEvent`,
- disabled runtime event feature uses `Infallible`,
- disabled runtime event build does not compile event emission paths,
- default-feature builds cannot name or construct concrete runtime events,
- all-features builds emit target/generator control-plane events in the
  documented order when those events are part of the public API,
- runtime events contain runtime ids only and do not carry user work payloads,
- runtime events are not emitted for failed control operations,
- runtime events are not emitted when the feature is disabled,
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
- stats for missing/removed targets and generators follow the documented
  behavior,
- generator stats report lag, missed intervals, and actual interval,
- periodic overload is not represented as stale queued job depth,
- bounded output queue pressure reports dropped or rejected outputs instead of
  unbounded queue growth,
- stats update on success and failure,
- stats update after removal and shutdown,
- stats snapshots are internally consistent under generated operation sequences.

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
- detached Redfish command language types are absent,
- default features do not pull `nv-redfish` or generated schema dependencies,
- `runtime-events` alone does not enable Redfish adapter APIs,
- Redfish adapter features do not expose disabled capability builders,
- mutually independent feature combinations compile or fail exactly as
  documented.

These use `trybuild` where normal integration tests cannot express compile-fail
expectations.

### Redfish adapter API tests

File: `tests/redfish_adapter_api.rs`

Tests:

- adapter generator builders close over typed `nv-redfish` objects,
- service-root builders produce runtime generators, not just marker objects,
- invalid object and command pairings are not expressible,
- builders cannot be called when their capability feature is disabled,
- Redfish resource event contains required identity fields,
- Redfish event payload does not contain execution handles,
- public Redfish events do not expose `B`, `ServiceRoot<B>`, `Chassis<B>`, or
  other execution handles,
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
- application can consume output through `next().await` and add more generators,
- application can consume a failed discovery work output and choose not to add
  more generators,
- application can remove discovery generators after consuming their output,
- application can shut down after partial discovery and still receive queued
  outputs first,
- application can request narrow scraping only,
- not requested, requested missing, requested failed, and requested successful
  states are distinguishable,
- runtime remains policy-free during application-driven discovery,
- final fake report is built from consumed outputs in deterministic order,
- fake events do not include target ids solely to satisfy runtime bookkeeping.

### State-machine and property tests

File: `tests/state_machine.rs` or behavior-domain files when clearer.

Use property-based tests when a small number of example tests can miss ordering
or lifecycle bugs. These tests should generate valid sequences of public
operations and assert invariants after every step.

Operation families:

- add target,
- remove target,
- add generator,
- remove generator,
- mark fake generator ready/not ready,
- make fake generator return work/none/error,
- call `next().await` with a bounded test executor step,
- complete a test-controlled in-flight future,
- request graceful shutdown,
- enable or disable runtime event expectations through feature-gated test
  builds.

Required invariants:

- ids are unique and never reused,
- removed generators are never selected,
- removed targets cannot receive new generators,
- at most one work item is executed per `next().await`,
- queued outputs are never lost or duplicated,
- completion count equals executed work count per generator,
- shutdown output is last and sticky,
- scheduler candidates never contain removed ids,
- the model's expected output log matches consumed runtime output.

Property tests must be deterministic on failure: record or print the random seed
and minimized operation sequence.

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
- Runtime output is consumed through `Runtime::next(&mut self).await`.
- Graceful shutdown is exposed and observed through the ordered output API.
- There is no separate public one-shot execution or batch drain API in Phase 0.
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
