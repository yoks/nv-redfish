# Scraper Phase 14: Health Projection Adapter

This phase validates integration by replacing one existing Redfish health collector path with scraper-backed projection.

The adapter may live outside the scraper crate. Its purpose is to prove scraper events can feed the health service without putting health policy into scraper core.

## Guardrails

- Scraper core must remain domain-neutral.
- Metric labels, unit sanitization, thresholds, sinks, and health report policy stay in health code.
- The adapter must consume typed scraper events or snapshots.
- The adapter must not call the BMC directly.
- Existing health sink behavior must remain observable in tests.

## Public API

Health-side usage:

```rust
let mut sensors = scraper
    .query::<Sensor>()
    .freshness(sensor_cfg.sensor_fetch_interval)
    .discovery_freshness(sensor_cfg.state_refresh_interval)
    .subscribe()
    .await?;
```

Projection shape:

```text
TypedResourceEvent<Sensor>
  |
  v
SensorHealthProjection
  |
  v
CollectorEvent::Metric
```

## Integration Boundary

The scraper owns:

- Redfish discovery
- Redfish refresh
- BMC scheduling and coalescing
- snapshots and staleness
- resource and scheduler events

Health owns:

- endpoint lifecycle
- sink calls
- metric naming and labels
- thresholds and health report mapping
- persistent log policy
- non-Redfish collectors

## TDD Test Plan

### 1. `sensor_projection_converts_added_snapshot_to_metric_event`

Feed a typed `Added` sensor event into the projection.

Assert the expected health metric event is produced.

### 2. `sensor_projection_converts_updated_snapshot_to_metric_event`

Feed an `Updated` sensor event.

Assert a metric update is produced with the new reading.

### 3. `sensor_projection_preserves_threshold_fields`

Use a sensor snapshot with thresholds.

Assert threshold values reach the existing health event shape.

### 4. `sensor_projection_uses_relation_labels`

Add relation metadata for a sensor related to another resource.

Assert health labels are built in the projection layer.

### 5. `sensor_projection_does_not_embed_sink_logic_in_scraper`

Compile or unit-test boundaries so scraper crate has no dependency on health sink types.

## Implementation Steps

1. Identify the smallest existing Redfish health collector path to replace.
2. Add a projection module in health or an integration example.
3. Map `ResourceSnapshot<Sensor>` into existing health metric events.
4. Preserve existing threshold and unit behavior.
5. Drive projection from typed subscription events.
6. Keep all sink interaction outside scraper.
7. Add integration tests against fake scraper streams.

## Acceptance Checklist

- One health collector path can run from scraper events.
- Health sink behavior remains outside scraper.
- The adapter preserves existing metric semantics.
- No scraper module depends on health crate types.
- The integration proves scraper can replace collector-owned Redfish traversal.

## Explicitly Out Of Scope

- moving all collectors at once
- changing health report policy
- changing sink APIs
- persistent log cursor migration
- non-Redfish collector migration
