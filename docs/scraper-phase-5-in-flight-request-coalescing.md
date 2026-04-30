# Scraper Phase 5: In-Flight Request Coalescing

This phase makes duplicate concurrent work share one BMC request.

The public API does not change. The behavior protects the BMC when many callers request the same resource at once.

## Guardrails

- Coalescing must happen before BMC dispatch.
- Coalescing must not merge different resource types, ids, or operation shapes.
- All waiters must observe the same success or failure.
- Exactly one successful store mutation event should be emitted for one coalesced fetch.
- In-flight entries must be removed after completion.

## Public API

No new public API.

Existing code benefits automatically:

```rust
let a = scraper.resources::<Sensor>().refresh(id.clone());
let b = scraper.resources::<Sensor>().refresh(id.clone());
let (a, b) = tokio::try_join!(a, b)?;
```

## Operation Key

Start with plain typed `Get` operations:

```text
operation kind + TypeId + ODataId
```

Later phases can extend this to include query shape or expand/filter options.

Use a small typed key rather than concatenated strings.

## Internal Flow

```text
submit Get<Sensor>(id)
  |
  v
inflight.get(key)?
  | yes -> await existing shared result
  | no  -> insert owner task, dispatch through scheduler
```

The scheduler still owns BMC I/O. Coalescing can live in the scheduler admission path or directly above it, as long as no duplicate BMC request is admitted.

## TDD Test Plan

### 1. `concurrent_refresh_same_resource_uses_one_bmc_request`

Block the fake BMC so two refreshes overlap.

Assert both refresh futures complete successfully and fake BMC request count is one.

### 2. `coalesced_waiters_receive_same_snapshot`

Run concurrent identical refreshes.

Assert returned snapshots have the same id, value, fetched timestamp, and version metadata.

### 3. `coalesced_error_is_returned_to_all_waiters`

Configure the fake BMC to fail.

Run concurrent identical refreshes.

Assert both callers receive the failure and one resource error event is emitted.

### 4. `different_types_or_ids_do_not_coalesce`

Run concurrent refreshes for different ids and for the same id under different types.

Assert each distinct key causes its own BMC request.

### 5. `coalescing_removes_inflight_entry_after_completion`

Run one refresh to completion, then run another refresh for the same key.

Assert the second refresh performs a new BMC request.

## Implementation Steps

1. Add an `OperationKey` type.
2. Add an in-flight map keyed by `OperationKey`.
3. Represent shared completion with a cancellation-safe primitive.
4. Ensure the owner future removes the in-flight entry on success or error.
5. Ensure store mutation and event publication happen once.
6. Add deterministic overlap tests using fake BMC gates.

## Acceptance Checklist

- Concurrent identical refreshes produce one BMC request.
- All waiters receive the same outcome.
- Distinct work does not merge accidentally.
- In-flight state is cleaned after completion.
- Coalescing works before fairness and adaptive scheduling are added.

## Explicitly Out Of Scope

- coalescing sequential requests
- stale-while-revalidate
- request cancellation policy beyond safe cleanup
- query-level coalescing beyond plain typed `Get`
