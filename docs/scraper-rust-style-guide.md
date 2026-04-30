# Redfish Scraper Rust Style Guide

This document defines the Rust style expected for the scraper crate.

The goal is open-source-quality code: functional, strongly typed, readable, testable, and boring in the best way.

## Core Principles

- Prefer simple, explicit code over clever code.
- Prefer small composable functions over large procedures.
- Prefer typed domain concepts over strings, booleans, and loosely structured maps.
- Prefer immutable data and pure transformations where practical.
- Prefer dependency injection over hidden globals.
- Prefer deterministic tests over timing-dependent tests.
- Prefer narrow modules with clear ownership.
- Prefer compiler-enforced invariants over runtime comments.
- Avoid speculative abstraction. Add abstraction only when it removes real duplication or clarifies ownership.

## Function Size And Shape

- A function SHOULD fit on one screen.
- A function MUST have one clear reason to change.
- A function MUST NOT mix unrelated responsibilities such as scheduling, BMC I/O, store mutation, and event formatting.
- A function MUST NOT hide important side effects behind a vague name.
- A function that performs I/O SHOULD do little besides I/O orchestration.
- A function that transforms data SHOULD be pure when practical.
- Deeply nested control flow MUST be split into named helpers.
- Repeated `match` or `if let` chains SHOULD become typed helper functions when they encode domain rules.

Good shape:

```rust
pub async fn refresh(&self, id: ODataId) -> Result<ResourceSnapshot<T>, Error> {
    let fetched = self.fetch_through_scheduler(id).await?;
    let event = self.store_snapshot(fetched)?;
    self.events.publish(event);
    self.store.get_required::<T>(&id)
}
```

Bad shape:

```rust
pub async fn refresh(&self, id: ODataId) -> Result<ResourceSnapshot<T>, Error> {
    // builds work item, calls BMC, handles retry, mutates store,
    // computes event, manages subscriptions, records metrics,
    // and contains a large nested error branch
}
```

## Strong Typing

- Use newtypes for ids, owners, query ids, discovery source ids, and scheduler work ids.
- Do not pass raw `String` where `ODataId`, `ResourceKey`, `QueryId`, or `DiscoverySourceId` is intended.
- Do not use booleans for mode selection when an enum communicates intent.
- Do not use `usize` or `u64` for unrelated concepts without a wrapper when they cross module boundaries.
- Prefer typed structs over tuple-heavy return values.
- Public APIs MUST communicate intent through names and types.

Examples:

```rust
pub struct QueryId(u64);
pub struct WorkId(u64);
pub struct DiscoverySourceId(u64);

pub enum Lane {
    Interactive,
    Subscription,
    Discovery,
    Maintenance,
}
```

Avoid:

```rust
fn enqueue(id: u64, priority: u8, is_discovery: bool)
```

Prefer:

```rust
fn enqueue(owner: WorkOwner, lane: Lane, priority: Priority)
```

## Ownership Boundaries

- `Scheduler` owns BMC I/O.
- `ResourceStore` owns materialized snapshots and indexes.
- `EventBus` owns event sequencing and publication.
- `DiscoveryRegistry` owns discoverer registration and lookup.
- `QueryManager` owns active query demand.
- Reconcilers produce work; they do not call BMC directly.
- Application projections, such as health metrics, MUST live outside the scraper core.

Code MUST preserve these boundaries even in tests.

## Side Effects

Side effects must be visible in function names or module ownership.

Allowed side-effect verbs:

- `fetch`
- `insert`
- `update`
- `publish`
- `enqueue`
- `dispatch`
- `record`
- `register`
- `remove`

Avoid vague names for side-effecting functions:

- `handle`
- `process`
- `do_work`
- `run_stuff`
- `manage`

Generic names are acceptable only at trait boundaries where the trait itself defines the meaning.

## Error Handling

- Use `Result` for recoverable failures.
- Do not use `unwrap` or `expect` in library code.
- Do not panic for BMC, scheduler, discovery, or store data issues.
- Preserve source errors where possible.
- Add context at module boundaries.
- Keep error enums small and meaningful.
- Tests may use `expect` when it makes failures clearer.

Error variants should identify the failing subsystem:

```rust
pub enum Error {
    Bmc(BmcError),
    Scheduler(SchedulerError),
    Store(StoreError),
    Discovery(DiscoveryError),
    Query(QueryError),
}
```

Avoid stringly typed catch-all errors in core implementation. A temporary catch-all is acceptable in early scaffolding only if a later phase removes it.

## Async Code

- Do not hold synchronous locks across `.await`.
- Do not hold async locks across BMC calls unless the lock specifically protects that operation.
- Keep async functions narrow.
- Prefer cancellation-safe operations.
- Background tasks MUST have explicit shutdown paths.
- Tests MUST NOT rely on real sleep for correctness when paused time or explicit notifications can be used.

## Locks And Shared State

- Keep lock scopes short and obvious.
- Prefer immutable snapshots outside locks.
- Do not publish events while holding the store write lock unless the event bus is proven nonblocking.
- Do not call user code while holding internal locks.
- Store mutation and event emission MUST preserve this order:

```text
compute mutation
acquire store lock
apply mutation
release store lock
publish event
```

If the implementation needs atomic store/event behavior, introduce a small commit object rather than widening lock scope.

## DRY Without Over-Abstracting

- Remove meaningful duplication.
- Keep harmless test setup duplication if abstraction would hide intent.
- Do not introduce generic frameworks before at least two real call sites need them.
- Do not make discovery, scheduling, and store code share abstractions just because they all process "work."
- Prefer small helper functions over trait hierarchies.
- Prefer traits for extension points, not internal convenience.

Good extension points:

- `Discoverer<T>`
- test fake BMC
- scheduler clock

Questionable early abstractions:

- generic manager traits for every subsystem
- macro-generated boilerplate before public API stabilizes
- over-general event filters before typed subscriptions exist

## Module Design

- Each module should expose a small public surface.
- Use `pub(crate)` by default inside the crate.
- Public API must be intentional and documented.
- Avoid circular conceptual dependencies.
- Module names should reflect ownership, not implementation mechanics.

Suggested ownership:

```text
scraper       public handle and shared inner state
builder       construction and configuration
resources     direct typed resource access
query         query builders and active query demand
store         snapshots and indexes
event         event envelopes and event bus
scheduler     BMC request admission and execution
discovery     discoverer traits and registry
reconcile     discovery and refresh reconcilers
predicate     typed predicates and hints
relation      relation indexes and relation predicates
```

## Public API Style

- Public API should read like user intent.
- Builder methods should be chainable.
- Method names must distinguish BMC I/O from local reads.
- Local reads should use names like `cached` and `list_cached`.
- BMC I/O should use names like `refresh`, `list`, `subscribe`, or `watch`.
- Do not hide BMC I/O behind innocuous getters.

Examples:

```rust
resources.cached(id)       // no BMC I/O
resources.refresh(id)      // BMC I/O
query.list().await         // discovery/fetch may occur
query.subscribe().await    // discovery/fetch and ongoing demand
```

## Events

- Store mutations emit events after successful mutation.
- Cached reads do not emit events.
- Failed BMC work may emit error events but must not mutate the store.
- Events must have monotonically increasing sequence numbers within one scraper instance.
- Event payloads should be small enough for broad subscribers.
- Typed subscriptions should be filtered views over the global event stream.

## Scheduler Code

- Scheduler code must be deterministic under test.
- Separate admission decisions from execution.
- Separate fixed scheduling from adaptive policy.
- Keep lane fairness logic isolated and unit-tested.
- Every scheduler decision should be explainable from input state.
- Avoid mixing metrics recording with scheduling decisions.

Good split:

```text
admission.rs       accepts work and applies hard bounds
fair.rs            selects next lane/work owner
executor.rs        calls BMC
adaptive.rs        updates capacity from observations
stats.rs           reports observations
```

## Discovery Code

- Discovery must be incremental.
- Discoverers should return candidate ids and relation hints, not mutate the store directly.
- Discoverers must not bypass the scheduler.
- Discovery hints are optimizations only.
- Snapshot predicates remain authoritative.
- Vendor-specific discovery belongs behind explicit discoverers.

## Testability

Code is not done until it is easy to test.

Required test seams:

- fake BMC
- fake or manual clock
- scheduler instrumentation
- event subscriber helpers
- store inspection helpers for crate-private tests
- deterministic discovery fixtures

Tests should assert behavior, not implementation accidents.

Prefer:

```rust
assert_eq!(fake_bmc.request_count(), 1);
assert_event!(events, ResourceEvent::Added { id, .. });
assert!(snapshot.staleness.is_fresh());
```

Avoid:

```rust
tokio::time::sleep(Duration::from_millis(50)).await;
assert_eq!(debug_string, "...exact internal layout...");
```

## Test Categories

Every phase should include:

- unit tests for pure logic
- async tests for scheduler/store/event interactions
- integration-style tests with a fake BMC
- regression tests for each guardrail the phase touches

Examples:

- store type separation
- no BMC call during cached reads
- duplicate request coalescing
- discovery does not assume global sensor paths
- discovery lane progresses under subscription load
- stale snapshots report desired freshness misses

## Documentation

- Every public type must have useful docs.
- Public docs should describe behavior and side effects.
- Public docs should state whether BMC I/O may occur.
- Internal comments should explain why, not what.
- Complex invariants should be documented near the type that owns them.

Example:

```rust
/// Returns the local snapshot for `id` without scheduling BMC I/O.
///
/// Returns `None` when the resource has not been discovered or refreshed.
pub fn cached(&self, id: impl Into<ODataId>) -> Option<ResourceSnapshot<T>>;
```

## Formatting And Lints

- Code must pass `cargo fmt`.
- Code should pass strict Clippy before merge.
- Avoid allow attributes unless narrowly scoped and justified.
- Avoid module-level lint suppressions.
- Keep imports explicit and organized by formatter.

## Review Checklist

Before considering code ready:

- Is every BMC call routed through the scheduler?
- Are public methods clear about local read vs BMC I/O?
- Are functions small and single-purpose?
- Are domain concepts strongly typed?
- Are locks short and not held across `.await`?
- Are events emitted after store mutation?
- Are tests deterministic?
- Can the behavior be tested without a real BMC?
- Is there any health-specific policy in scraper core?
- Is the abstraction justified by real call sites?
