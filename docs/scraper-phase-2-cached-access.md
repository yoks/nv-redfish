# Scraper Phase 2: Cached Direct Access

This phase exposes read-only access to the materialized view built by Phase 1 refreshes.

```rust
let cached = scraper
    .resources::<Sensor>()
    .cached("/redfish/v1/Chassis/1/Sensors/InletTemp");

let all = scraper.resources::<Sensor>().list_cached();
```

The user can inspect known resources without causing BMC I/O.

This phase does not implement discovery, one-shot queries, subscriptions, polling, request coalescing, or cache expiration.

## Guardrails

- Cached access must never call the BMC.
- Cached access must never enqueue scheduler work.
- Cached access must be scoped by resource type.
- Cached access must preserve snapshot metadata, including `fetched_at`, `etag`, and `staleness`.
- Missing resources must return `None`, not trigger discovery.
- `list_cached()` must return only resources already present in the local store.

## Public API

```rust
impl<B, T> ResourceClient<B, T> {
    pub fn cached(&self, id: impl Into<ODataId>) -> Option<ResourceSnapshot<T>>;

    pub fn list_cached(&self) -> Vec<ResourceSnapshot<T>>;
}
```

These methods are synchronous because they perform only local store reads.

If the store uses an async lock, keep the public API ergonomic. Prefer a synchronous lock for the initial store unless there is a strong reason not to.

## Internal Flow

### `cached(id)`

```text
ResourceClient<T>::cached(id)
  |
  v
ResourceStore::get::<T>(id)
  |
  v
return Option<ResourceSnapshot<T>>
```

### `list_cached()`

```text
ResourceClient<T>::list_cached()
  |
  v
ResourceStore::list::<T>()
  |
  v
return Vec<ResourceSnapshot<T>>
```

No scheduler, BMC, discovery, or event stream work occurs.

## Store Requirements

Phase 1 can get away with insertion only. Phase 2 must complete typed lookup and listing.

Use two indexes:

```text
resources: HashMap<ResourceKey, ErasedSnapshot>
by_type: HashMap<TypeId, BTreeSet<ODataId>>
```

`BTreeSet` gives deterministic `list_cached()` ordering. If using `HashSet`, tests should not depend on order.

### Resource Key

```rust
struct ResourceKey {
    type_id: TypeId,
    id: ODataId,
}
```

### Insert Invariant

When Phase 1 inserts or updates a snapshot:

- `resources[(type_id, id)]` must be updated
- `by_type[type_id]` must contain `id`

If store insertion fails, no success event should be emitted.

### Get Invariant

`get::<T>(id)`:

- looks up `(TypeId::of::<T>(), id)`
- downcasts the erased value to `Arc<T>`
- returns a typed snapshot copy
- returns `None` if key is absent
- returns `None` or an internal error if the stored value has the wrong type

Wrong-type storage should be impossible if insertion is correct. In tests, prefer asserting type scoping rather than manufacturing corrupted store state.

## Snapshot Clone Semantics

`ResourceSnapshot<T>` should be cheap to clone.

Expected shape:

```rust
pub struct ResourceSnapshot<T> {
    pub id: ODataId,
    pub value: Arc<T>,
    pub etag: Option<ODataETag>,
    pub fetched_at: SystemTime,
    pub staleness: Staleness,
}
```

Returning snapshots from cache should clone metadata and clone the `Arc<T>`, not clone the full resource.

## TDD Test Plan

### 1. `cached_returns_none_for_unknown_resource`

Given:

- empty store

When:

```rust
scraper.resources::<TestResource>().cached("/redfish/v1/Test/1")
```

Then:

- result is `None`
- fake BMC request count remains `0`
- scheduler executed work count remains `0`

This test proves missing cache entries do not trigger discovery or refresh.

### 2. `cached_returns_snapshot_after_refresh`

Given:

- Phase 1 refresh has stored `/redfish/v1/Test/1`

When:

```rust
let cached = scraper.resources::<TestResource>().cached("/redfish/v1/Test/1");
```

Then:

- result is `Some`
- snapshot id matches
- snapshot value matches refreshed value
- snapshot `fetched_at` equals the stored snapshot timestamp
- snapshot `staleness` is preserved

This test should not assert that `fetched_at` is "now"; it should compare against the refresh result.

### 3. `cached_does_not_call_bmc`

Given:

- one refresh has already happened
- fake BMC request count is recorded

When:

- call `cached(id)` many times

Then:

- fake BMC request count does not increase
- scheduler executed work count does not increase
- no new events are emitted

Cached reads are observations, not mutations.

### 4. `list_cached_returns_all_snapshots_for_type`

Given:

- refresh two resources of the same type

When:

```rust
let all = scraper.resources::<TestResource>().list_cached();
```

Then:

- result length is `2`
- both ids are present
- values match
- no BMC calls occur during listing

### 5. `list_cached_is_type_scoped`

Given:

- refresh `/redfish/v1/Test/1` as `TestResource`
- refresh `/redfish/v1/Other/1` as `OtherResource`

When:

```rust
let test_resources = scraper.resources::<TestResource>().list_cached();
let other_resources = scraper.resources::<OtherResource>().list_cached();
```

Then:

- `test_resources` contains only `TestResource`
- `other_resources` contains only `OtherResource`
- neither list includes the other type

This test protects the `(TypeId, ODataId)` model.

### 6. `same_id_different_type_is_separate`

Given:

- store contains the same `ODataId` under two different Rust types

When:

- call `cached` for each type

Then:

- each call returns the value for its own type

This is unusual in real Redfish usage, but it proves the store key is typed.

### 7. `cached_snapshot_arc_is_shared`

Given:

- refresh stores a resource

When:

- call `cached(id)` twice

Then:

- both returned snapshots point to the same `Arc<T>` allocation if that is easy to assert

This is optional but useful. It protects cheap clone semantics.

## Implementation Steps

1. Add `ResourceStore::get::<T>(&self, id: &ODataId)`.
2. Add `ResourceStore::list::<T>(&self)`.
3. Ensure Phase 1 insertion updates both `resources` and `by_type`.
4. Add `ResourceClient::cached`.
5. Add `ResourceClient::list_cached`.
6. Add tests proving no BMC/scheduler/event activity during cached reads.
7. Keep all methods local-store-only.

## Locking Guidance

The initial store can use:

```rust
std::sync::RwLock<ResourceStoreInner>
```

or:

```rust
tokio::sync::RwLock<ResourceStoreInner>
```

Prefer the simplest approach that does not hold locks across `.await`.

Cached methods should not be async unless the chosen lock makes that unavoidable. The API examples assume synchronous cached reads.

## Event Behavior

Cached reads must not emit events.

Only accepted store mutations emit events. Since `cached` and `list_cached` do not mutate the store, they are invisible to `subscribe_events()`.

## Error Handling

Public cached reads should use `Option`, not `Result`, for normal absence.

```rust
pub fn cached(&self, id: impl Into<ODataId>) -> Option<ResourceSnapshot<T>>;
```

Internal downcast failure indicates a bug. Reasonable options:

- return `None` and emit a debug assertion
- return `None` and log internally
- keep the store API private enough that wrong-type insertion is impossible

Do not expose downcast errors in the Phase 2 public API.

## Acceptance Checklist

- `cached(id)` returns `None` for unknown resources.
- `cached(id)` returns a typed snapshot after refresh.
- `cached(id)` never calls the BMC.
- `cached(id)` never enqueues scheduler work.
- `cached(id)` emits no events.
- `list_cached()` returns all stored snapshots for exactly one type.
- same id under different types remains distinct.
- cached snapshots preserve metadata.

## Explicitly Out Of Scope

- cache expiration
- stale recomputation over time
- automatic refresh on stale cached reads
- discovery
- `query::<T>().list()`
- subscriptions
- watches
- relation indexes
- request coalescing
- adaptive scheduling
- health projections
