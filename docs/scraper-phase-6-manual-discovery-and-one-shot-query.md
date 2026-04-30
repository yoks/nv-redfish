# Scraper Phase 6: Manual Discovery And One-Shot Query

This phase introduces explicit discoverers and `query::<T>().list()`.

Users can ask for a typed resource set without knowing every URI. Discovery is still test/manual only; standard Redfish crawling comes later.

## Guardrails

- Registering a discoverer must be side-effect free.
- Discovery must be demand-driven by a query.
- Discoverers must not call the BMC directly; they use `DiscoveryContext`.
- Temporary query demand must be removed after `list()` returns.
- Fetched snapshots must be filtered before returning.

## Public API

```rust
let sensors = scraper.query::<Sensor>().list().await?;
```

Manual discoverer shape:

```rust
#[async_trait::async_trait]
pub trait Discoverer<T>: Send + Sync + 'static {
    async fn discover(
        &self,
        cx: &mut DiscoveryContext<'_>,
        hint: DiscoveryHint,
    ) -> Result<DiscoveryBatch, Error>;
}
```

## Discovery Batch

Start with candidate ids:

```rust
pub struct DiscoveryBatch {
    pub candidates: Vec<ODataId>,
}
```

Later phases can add cursors, relations, and source metadata.

## Internal Flow

```text
QueryBuilder<T>::list()
  |
  v
register temporary demand
  |
  v
DiscoveryRegistry::discover::<T>()
  |
  v
fetch candidates through scheduler
  |
  v
apply query predicates
  |
  v
remove temporary demand
  |
  v
return Vec<ResourceSnapshot<T>>
```

## TDD Test Plan

### 1. `list_uses_registered_discoverer`

Register a test discoverer for `TestResource`.

Call `query::<TestResource>().list()`.

Assert the discoverer was invoked.

### 2. `list_fetches_discovered_candidates`

The discoverer returns two ids and the fake BMC has both resources.

Assert `list()` returns two snapshots and both requests went through the scheduler.

### 3. `list_returns_matching_snapshots`

With no predicates yet, all successfully fetched candidates match.

Assert returned snapshots are typed and stored.

### 4. `list_removes_temporary_demand_after_return`

Call `list()` and then inspect query manager test state.

Temporary demand must be gone.

### 5. `list_emits_discovered_and_added_events`

Subscribe before calling `list()`.

Assert discovery and resource events appear in order that preserves store mutation rules.

### 6. `list_with_no_discoverer_returns_empty_or_error_by_policy`

Choose one policy and encode it in the public docs.

Prefer `Ok(Vec::new())` for absent optional discovery, unless missing discoverer is a configuration error for this crate.

## Implementation Steps

1. Add `query` module and `QueryBuilder<B, T>`.
2. Add `DiscoveryRegistry`.
3. Add `Discoverer<T>`, `DiscoveryContext`, `DiscoveryHint`, and `DiscoveryBatch`.
4. Add a temporary query demand owner.
5. Make `list()` run discoverers and fetch candidates through the scheduler.
6. Emit discovery events.
7. Remove temporary demand on success, error, and cancellation-safe paths where practical.

## Acceptance Checklist

- One-shot query works end to end.
- Discovery is demand-driven.
- All BMC work goes through the scheduler.
- Discoverer registration remains side-effect free.
- Temporary demand does not leak after `list()`.

## Explicitly Out Of Scope

- standard Redfish discovery
- predicate DSL
- subscriptions
- background freshness
- relation indexes
- adaptive scheduling
