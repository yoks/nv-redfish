# Scraper Phase 1: Direct Typed Refresh

This phase implements the smallest useful BMC-backed behavior:

```rust
let snapshot = scraper
    .resources::<Sensor>()
    .refresh("/redfish/v1/Chassis/1/Sensors/InletTemp")
    .await?;
```

The user already knows the resource URI. The scraper fetches that typed resource through the scheduler, stores a typed snapshot, and emits a resource event.

This phase does not implement discovery, cached reads, subscriptions, request coalescing, adaptive scheduling, or polling.

## Guardrails

- Every BMC request must go through the scheduler.
- Store mutations must emit events after the store accepts the change.
- Direct refresh must work for any generated Redfish type that satisfies the BMC `get<T>` bounds.
- The phase must not assume any well-known path beyond the caller-provided URI.
- The phase must not introduce health-service-specific concepts.
- The scheduler may be simple, but it must already be the only BMC I/O path.

## Public API

```rust
impl Scraper<B> {
    pub fn resources<T>(&self) -> ResourceClient<B, T>;
}

impl<B, T> ResourceClient<B, T> {
    pub async fn refresh(&self, id: impl Into<ODataId>) -> Result<ResourceSnapshot<T>, Error>;
}
```

The returned snapshot should be owned and cheap to clone.

```rust
pub struct ResourceSnapshot<T> {
    pub id: ODataId,
    pub value: Arc<T>,
    pub etag: Option<ODataETag>,
    pub fetched_at: SystemTime,
    pub staleness: Staleness,
}
```

For Phase 1, successful `refresh` returns `Staleness::Fresh`.

## Internal Flow

```text
ResourceClient<T>::refresh(id)
  |
  v
WorkItem {
  lane: Interactive,
  owner: ImmediateRequest,
  operation: Get<T>(id),
}
  |
  v
Scheduler::execute(work)
  |
  v
Bmc::get::<T>(&id)
  |
  v
ResourceStore::insert(snapshot)
  |
  v
EventBus::publish(ResourceEvent::Added or Updated)
  |
  v
return snapshot
```

## Minimal Types

### Resource Identity

Use `(TypeId, ODataId)` as the store key.

```rust
struct ResourceKey {
    type_id: TypeId,
    id: ODataId,
}
```

Phase 1 does not need public `ResourceType` names. Human-readable type names can come later.

### Erased Snapshot

The store needs to hold typed snapshots behind type erasure.

```rust
struct ErasedSnapshot {
    type_id: TypeId,
    id: ODataId,
    etag: Option<ODataETag>,
    fetched_at: SystemTime,
    staleness: Staleness,
    value: Arc<dyn Any + Send + Sync>,
}
```

The public `ResourceSnapshot<T>` should be produced at the API boundary.

### Staleness

Keep this small in Phase 1.

```rust
pub enum Staleness {
    Fresh,
    Stale {
        age: Duration,
        desired: Option<Duration>,
    },
}
```

Only `Fresh` is required in this phase.

### Events

Phase 1 needs only resource events.

```rust
pub enum ScraperEvent {
    Resource(ResourceEvent),
}

pub enum ResourceEvent {
    Added {
        type_id: TypeId,
        id: ODataId,
    },
    Updated {
        type_id: TypeId,
        id: ODataId,
    },
    Error {
        type_id: TypeId,
        id: ODataId,
        error: Arc<Error>,
    },
}
```

The event can start as metadata-only. Typed filtered events can be added later. The important invariant is that successful events correspond to state already accepted by the store.

### Scheduler

Implement the narrowest scheduler that can execute one `Get<T>` operation.

```rust
pub enum Lane {
    Interactive,
    Subscription,
    Discovery,
    Maintenance,
}
```

Phase 1 behavior:

- accept a work item
- record the lane
- call `Bmc::get::<T>`
- return the result

It can execute inline or via a worker task. Prefer a shape that can evolve into a queue later.

## Test Harness Requirements

Create a fake BMC that can:

- return typed resources by `ODataId`
- return an error for a specific `ODataId`
- count requests
- record requested ids
- record that requests came through scheduler instrumentation

The fake type can be simple and test-local. It should implement enough of the BMC trait for `get<T>`.

Use small generated/test resource types where possible. If using real generated types is too heavy for early unit tests, create a local test entity that implements the required entity traits.

## TDD Test Plan

### 1. `refresh_fetches_known_resource`

Given:

- fake BMC has a typed resource at `/redfish/v1/Test/1`

When:

```rust
scraper.resources::<TestResource>().refresh("/redfish/v1/Test/1").await
```

Then:

- result is `Ok`
- snapshot id is `/redfish/v1/Test/1`
- snapshot value is the fake resource
- fake BMC request count is `1`

Write this test first. It should fail until `ResourceClient::refresh`, scheduler execution, and BMC get are wired.

### 2. `refresh_stores_snapshot`

Given:

- refresh succeeds

Then:

- internal store contains one snapshot under `(TypeId::of::<TestResource>(), id)`

This test may use a crate-private test helper to inspect the store. Do not add public cached APIs yet; that is Phase 2.

### 3. `refresh_emits_resource_added_event`

Given:

- event subscriber is created before refresh
- store does not contain the resource

When refresh succeeds

Then:

- subscriber receives `ResourceEvent::Added`
- the event sequence is greater than zero
- inspecting the store after receiving the event finds the snapshot

This proves "emit after store mutation."

### 4. `refresh_emits_resource_updated_event_on_second_value`

Given:

- first refresh stores resource version A
- fake BMC response changes to version B

When:

- second refresh runs

Then:

- subscriber receives `ResourceEvent::Updated`
- store contains version B

Do not require deep diffing in this phase. Existing key versus absent key is enough to choose `Added` vs `Updated`.

### 5. `refresh_uses_scheduler`

Given:

- fake scheduler instrumentation records executed work

When refresh runs

Then:

- exactly one scheduler work item is recorded
- lane is `Interactive`
- operation is `Get`
- id and type match the request

The test should fail if `ResourceClient` calls `Bmc::get` directly.

### 6. `refresh_error_emits_resource_error`

Given:

- fake BMC returns an error for the id

When refresh runs

Then:

- result is `Err`
- store does not contain the snapshot
- event stream receives `ResourceEvent::Error`

For Phase 1, error events may be emitted before or after returning the error. Successful mutation events must still be after store mutation.

## Implementation Steps

1. Define `ResourceSnapshot<T>` and `Staleness`.
2. Define `ScraperEvent`, `ResourceEvent`, and `EventEnvelope`.
3. Add an event bus to `Scraper::Inner`.
4. Add a minimal `ResourceStore`.
5. Add `ResourceClient<T>` returned by `scraper.resources::<T>()`.
6. Add `Scheduler::get<T>(lane, id)`.
7. Implement `ResourceClient::refresh`.
8. Make successful refresh insert into store before emitting `Added` or `Updated`.
9. Make failed refresh emit `Error`.
10. Keep all APIs and tests narrow; do not add cached public methods yet.

## Error Handling

Use one scraper error type that can wrap BMC errors.

```rust
pub enum Error {
    Bmc(Box<dyn std::error::Error + Send + Sync>),
    Store(String),
    Scheduler(String),
}
```

The exact shape can change later. For this phase, prioritize preserving source error information enough for tests and logs.

## Acceptance Checklist

- `refresh(id)` fetches through the scheduler.
- `refresh(id)` stores a typed snapshot.
- first success emits `Added`.
- later success for the same `(type, id)` emits `Updated`.
- failure emits `Error`.
- events have monotonically increasing sequence numbers if Phase 0 already introduced envelopes.
- no public cached read API exists yet unless Phase 2 is also complete.

## Explicitly Out Of Scope

- `cached(id)`
- `list_cached()`
- `query::<T>().list()`
- discovery
- subscriptions
- polling/freshness timers
- request coalescing
- fair scheduling
- adaptive capacity
- relation indexes
- health projections
