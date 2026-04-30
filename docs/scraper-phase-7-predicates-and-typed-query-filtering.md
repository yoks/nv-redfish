# Scraper Phase 7: Predicates And Typed Query Filtering

This phase lets users filter query results with typed predicates.

Predicates have two roles: candidate hints can reduce discovery work, while snapshot predicates provide authoritative correctness after fetch.

## Guardrails

- Snapshot filtering must be authoritative.
- Discovery hints are optimization only.
- Multiple predicates must combine with logical AND.
- Predicate APIs must be typed and readable.
- Predicate evaluation must not call the BMC.

## Public API

```rust
let temps = scraper
    .query::<Sensor>()
    .where_(sensor::reading_type().temperature())
    .list()
    .await?;
```

Resource id filtering should also be possible:

```rust
let inlet = scraper
    .query::<Sensor>()
    .where_(resource::id().contains("Inlet"))
    .list()
    .await?;
```

## Predicate Shape

Use a trait or enum that can expose both stages:

```rust
pub trait Predicate<T>: Send + Sync + 'static {
    fn candidate_hint(&self) -> Option<DiscoveryHint>;
    fn matches_snapshot(&self, snapshot: &ResourceSnapshot<T>) -> bool;
}
```

The final code may use boxed predicates, typed predicate structs, or an enum. Prefer explicit typed structs over macros.

## Internal Flow

```text
query.where_(predicate).list()
  |
  v
collect discovery hints
  |
  v
discover candidates
  |
  v
apply candidate predicates when possible
  |
  v
fetch candidates
  |
  v
apply snapshot predicates
```

## TDD Test Plan

### 1. `list_applies_snapshot_predicate`

Discover two resources, one matching and one nonmatching.

Assert only the matching snapshot is returned.

### 2. `predicate_can_filter_by_resource_id`

Use an id predicate against discovered candidate ids.

Assert nonmatching ids are not returned.

### 3. `multiple_predicates_are_and_combined`

Combine two predicates where each alone would match a different subset.

Assert only resources matching both predicates are returned.

### 4. `predicate_failure_does_not_fetch_unneeded_candidates_when_candidate_stage_applies`

Use a candidate-stage id predicate.

Assert filtered-out candidate ids are not fetched.

### 5. `predicate_hints_are_passed_to_discoverer`

Register a test discoverer that records received hints.

Assert semantic hints from predicates are present.

## Implementation Steps

1. Add `predicate` module.
2. Add typed predicate containers to `QueryBuilder`.
3. Add resource-id predicates first because they are schema-independent.
4. Add initial sensor predicates required by examples.
5. Pass hints into discovery.
6. Apply snapshot predicates after every candidate fetch.
7. Add tests proving correctness without relying on hints.

## Acceptance Checklist

- `where_` works on one-shot `list()`.
- Multiple predicates use AND semantics.
- Candidate hints reach discoverers.
- Snapshot filtering remains authoritative.
- Predicate code is type safe and testable.

## Explicitly Out Of Scope

- OR groups
- user-provided async predicates
- full Redfish schema predicate coverage
- relation predicates
- subscriptions
