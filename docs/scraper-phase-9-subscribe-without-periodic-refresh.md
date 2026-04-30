# Scraper Phase 9: Subscribe Without Periodic Refresh

This phase adds typed subscriptions as filtered views over the global event stream.

Subscriptions run an initial one-shot query and then observe future matching changes. They do not create timers yet.

## Guardrails

- Typed subscriptions must be derived from the global event stream.
- Subscribing must register long-lived query demand.
- Dropping a subscription must remove that demand.
- Initial results must be emitted to the subscriber.
- Subscription filtering must use the same predicate semantics as `list()`.

## Public API

```rust
let mut sub = scraper
    .query::<Sensor>()
    .where_(sensor::reading_type().temperature())
    .subscribe()
    .await?;
```

The returned type should implement `Stream<Item = Result<TypedResourceEvent<T>, Error>>` or expose an equivalent async receive API.

## Event Shape

Typed events may be richer than global metadata events:

```rust
pub enum TypedResourceEvent<T> {
    Added(ResourceSnapshot<T>),
    Updated {
        previous: Option<ResourceSnapshot<T>>,
        new: ResourceSnapshot<T>,
    },
    Removed(ODataId),
    Error {
        id: ODataId,
        error: Arc<Error>,
    },
}
```

Keep the exact shape compatible with later freshness events.

## Internal Flow

```text
subscribe()
  |
  v
register query demand
  |
  v
run initial list()
  |
  v
emit Added for initial matches
  |
  v
listen to global events
  |
  v
filter by type, id membership, and predicates
```

## TDD Test Plan

### 1. `subscribe_runs_initial_list`

Register a test discoverer and subscribe.

Assert the discoverer and candidate fetches ran.

### 2. `subscribe_emits_added_for_initial_matches`

Subscribe to a query with existing matching resources.

Assert the subscription emits `Added` events for initial matches.

### 3. `subscribe_filters_global_events_by_query`

Trigger events for matching and nonmatching resources.

Assert the typed subscription receives only matching events.

### 4. `subscribe_emits_updated_for_matching_resource`

Refresh a matching resource after subscription starts.

Assert an `Updated` event is emitted.

### 5. `subscribe_emits_removed_when_resource_no_longer_matches`

Update a resource so it no longer satisfies predicates.

Assert `Removed` is emitted for that subscription's membership.

### 6. `dropping_subscription_removes_query_demand`

Drop the subscription handle.

Assert query demand is removed.

## Implementation Steps

1. Add a query demand manager for long-lived owners.
2. Add typed subscription stream handle.
3. Reuse `list()` for initial membership.
4. Track per-subscription membership ids.
5. Filter global resource events into typed events.
6. Remove demand in `Drop` for the subscription handle.
7. Add cancellation and drop tests.

## Acceptance Checklist

- Subscriptions are filtered views over global events.
- Initial query results are delivered as typed events.
- Membership changes produce added/removed behavior.
- Dropping the subscription removes desired state.
- No periodic refresh is added yet.

## Explicitly Out Of Scope

- freshness timers
- watch handles
- adaptive scheduling
- relation predicates
- durable replay
