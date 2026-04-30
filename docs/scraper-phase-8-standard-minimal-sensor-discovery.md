# Scraper Phase 8: Standard Minimal Sensor Discovery

This phase implements the smallest useful built-in sensor discovery for realistic BMCs.

The point is to prove the crate does not depend on a universal sensor path.

## Guardrails

- Standard discovery must not assume `/redfish/v1/Sensors`.
- Discovery must be incremental and resumable in shape.
- Discovery must deduplicate candidate ids.
- Discovery must use scheduler-controlled BMC calls.
- Discovery must support BMCs where sensors are reachable only through chassis or environment metrics links.

## Public API

```rust
let scraper = Scraper::builder(bmc)
    .discover(Discovery::standard())
    .build()
    .await?;

let sensors = scraper.query::<Sensor>().list().await?;
```

## Minimal Sources

Implement only these standard paths first:

- service root
- chassis collection
- chassis `Sensors` collection links
- chassis `EnvironmentMetrics` with `DataSourceUri`

Do not cover every Redfish sensor source yet.

## Internal Flow

```text
Sensor query demand
  |
  v
standard sensor discoverer
  |
  v
fetch service root
  |
  v
fetch chassis collection
  |
  v
fetch each chassis needed for links
  |
  v
fetch sensor collections or environment metrics links
  |
  v
return deduplicated Sensor candidate ids
```

Each fetch is a scheduler work item in the discovery lane.

## TDD Test Plan

### 1. `standard_discovery_finds_chassis_sensors`

Mock service root, chassis collection, one chassis, and a chassis sensor collection.

Assert `query::<Sensor>().list()` returns those sensors.

### 2. `standard_discovery_finds_environment_metric_sensor_uris`

Mock a BMC where sensor ids appear only in environment metric `DataSourceUri` fields.

Assert sensors are discovered and fetched.

### 3. `standard_discovery_deduplicates_sensor_ids`

Expose the same sensor through both a sensor collection and environment metrics.

Assert only one candidate fetch is made.

### 4. `standard_discovery_is_incremental`

Use scheduler instrumentation to prove discovery is multiple bounded work items, not one unbounded blocking crawl.

### 5. `standard_discovery_does_not_assume_global_sensor_path`

Make `/redfish/v1/Sensors` absent.

Assert discovery still succeeds through chassis-linked paths.

## Implementation Steps

1. Add standard discovery registration for `Sensor`.
2. Add helpers for fetching service root and collections through `DiscoveryContext`.
3. Parse chassis links into candidate crawl work.
4. Extract sensor ids from chassis sensor collections.
5. Extract sensor ids from environment metrics `DataSourceUri`.
6. Deduplicate candidates using `BTreeSet<ODataId>`.
7. Keep source-specific code small and unit-tested.

## Acceptance Checklist

- Standard sensor discovery works without a global sensor path.
- Chassis-linked sensors are discovered.
- Environment metric sensor URIs are discovered.
- Duplicate candidate ids are fetched once.
- Discovery BMC calls use the discovery lane.

## Explicitly Out Of Scope

- drive metrics sensor discovery
- power supply metrics discovery
- processor metrics discovery
- telemetry service discovery
- OEM sensor sources
- relation inference
