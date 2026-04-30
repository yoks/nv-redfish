# Scraper Phase 13: Relations

This phase adds relation-aware queries.

Users can ask for resources such as temperature sensors related to drives without manually knowing sensor paths.

## Guardrails

- Relations must be stored as typed indexes, not string-only conventions.
- Relation predicates must remain correctness checks, not only discovery hints.
- Discoverers may emit relations, but health-specific labels must stay outside scraper core.
- Relation updates must re-evaluate query membership.
- Relation removal must produce subscription membership removal when appropriate.

## Public API

```rust
let drive_temps = scraper
    .query::<Sensor>()
    .where_(sensor::reading_type().temperature())
    .where_(sensor::related_to::<Drive>())
    .list()
    .await?;
```

## Relation Model

Start with direct resource-to-resource relations:

```rust
pub struct Relation {
    pub from: ResourceRef,
    pub to: ResourceRef,
    pub kind: RelationKind,
}
```

`ResourceRef` should include `TypeId` and `ODataId`.

Initial relation kinds can be broad:

```rust
pub enum RelationKind {
    RelatedTo,
    MetricsFor,
}
```

## Internal Flow

```text
discoverer returns candidates plus relations
  |
  v
store records snapshots and relation index
  |
  v
relation predicate checks index membership
  |
  v
query membership updates when relation set changes
```

## TDD Test Plan

### 1. `store_records_relation_between_sensor_and_drive`

Insert a sensor snapshot and a relation to a drive.

Assert the relation index can find the sensor by related drive type.

### 2. `related_to_predicate_filters_by_relation_index`

Discover two sensors, only one related to a drive.

Assert `related_to::<Drive>()` returns only the related sensor.

### 3. `relation_discovery_hint_reaches_discoverer`

Use a relation predicate and a test discoverer.

Assert the discoverer receives a hint that relation-constrained discovery is useful.

### 4. `resource_update_re_evaluates_relation_based_query`

Change relation metadata for a resource.

Assert subscription membership updates.

### 5. `relation_removal_emits_removed_for_query`

Remove a relation that made a resource match.

Assert typed subscription emits `Removed`.

## Implementation Steps

1. Add `relation` module.
2. Add relation storage and indexes to `ResourceStore`.
3. Extend `DiscoveryBatch` to include relations.
4. Add relation mutation events or resource membership invalidation.
5. Add `related_to::<T>()` predicate.
6. Re-evaluate active query memberships on relation changes.

## Acceptance Checklist

- Relation-based queries work end to end.
- Relation predicates use store indexes.
- Discoverers can emit relation metadata.
- Relation changes update typed subscriptions.
- Health projections can consume relation data without scraper knowing health labels.

## Explicitly Out Of Scope

- full automatic Redfish graph inference
- multi-hop graph queries
- health label construction
- persistent graph database behavior
