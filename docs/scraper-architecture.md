# Redfish Scraper Architecture

The scraper is a demand-driven materialized view of a BMC Redfish graph.

Queries create desired state. Discovery expands desired state into candidate resources. Reconcilers decide what must be refreshed. The scheduler protects the BMC. The store materializes typed snapshots. Events describe every accepted change.

## High-Level Flow

```text
User API
  |
  v
Query / Watch / Refresh
  |
  v
Desired State
  |
  +--------------------+
  |                    |
  v                    v
Discovery          Refresh
Reconciler         Reconciler
  |                    |
  +---------+----------+
            |
            v
       Work Planner
            |
            v
Adaptive BMC Scheduler
            |
            v
           BMC
            |
            v
     Resource Store
            |
            v
       Event Stream
            |
            v
Typed Subscriptions / Application Projections
```

## Components

### Scraper

The public entry point. It is cheap to clone and owns shared state.

```rust
scraper.query::<Sensor>()
scraper.resources::<Sensor>()
scraper.subscribe_events()
```

The scraper owns one BMC client, one resource store, one scheduler, one discovery registry, and one event stream per BMC endpoint.

### Desired State

Desired state records active interest.

Examples:

- a one-shot `list()` request
- a long-lived `subscribe()`
- a background `watch()`
- an explicit `refresh(id)`

Desired state includes:

- resource type
- predicates
- desired resource freshness
- desired discovery freshness
- owner/query id
- scheduling lane and priority

### Discovery Registry

The discovery registry stores discoverers. Registering a discoverer does not imply an eager crawl.

```rust
Scraper::builder(bmc)
    .discover(Discovery::standard())
    .discover(MyVendorDiscovery)
    .build()
    .await?;
```

Active queries demand discovery. For example, a `Sensor` query can use discoverers for:

- `Chassis.Sensors`
- `Chassis.EnvironmentMetrics -> DataSourceUri`
- `Drive.Metrics -> DataSourceUri`
- `PowerSupply.Metrics -> DataSourceUri`
- `Processor.Metrics` and `Processor.EnvironmentMetrics`
- legacy `Chassis.Power` and `Chassis.Thermal`
- `TelemetryService`
- OEM/vendor extensions

Discovery is incremental. A large Redfish graph is crawled as small work units, not as one blocking operation.

### Query Manager

The query manager owns active query plans.

```rust
QueryPlan<T> {
    predicates,
    freshness,
    discovery_freshness,
    priority,
}
```

Queries use predicates both for filtering and for discovery hints.

Predicate examples:

- sensor reading type is temperature
- sensor name contains `GPU`
- sensor is related to a drive
- resource path matches a prefix

Hints optimize discovery, but correctness comes from filtering fetched snapshots.

### Reconcilers

Reconcilers compare desired state with observed state.

The discovery reconciler asks:

- which queries need membership refresh?
- which discoverers can produce candidates?
- which discovery cursor should advance next?

The refresh reconciler asks:

- which demanded resources are stale?
- which refreshes are already queued or in flight?
- which resources can be refreshed now?

Reconcilers produce work items. They do not call the BMC directly.

### Work Planner

The work planner converts reconciler decisions into scheduler work.

```rust
WorkItem {
    lane,
    owner,
    resource_type,
    id,
    operation,
}
```

Work items are deduplicated before dispatch. Identical in-flight requests share one BMC request and fan out the result.

### Adaptive BMC Scheduler

The scheduler is the only component allowed to call `Bmc::get`, `Bmc::expand`, `Bmc::filter`, or related BMC operations.

It controls:

- maximum in-flight requests
- request rate
- weighted fair queues
- discovery minimum share
- request coalescing
- adaptive backoff
- timeout/error handling

Default lanes:

```text
Interactive
Subscription
Discovery
Maintenance
```

Interactive work gets lower latency, but it cannot consume all capacity forever. Discovery has a reserved nonzero service share when discovery work is pending.

The scheduler treats the BMC as an unknown, time-varying service system. It starts conservatively, learns from observed completions, and adapts with congestion-control-like behavior.

Overload becomes stale data, not an infinite request backlog.

### Resource Store

The store is the materialized view.

Snapshots are typed:

```rust
pub struct ResourceSnapshot<T> {
    pub id: ODataId,
    pub value: Arc<T>,
    pub etag: Option<ODataETag>,
    pub fetched_at: SystemTime,
    pub staleness: Staleness,
}
```

The store maintains indexes:

```text
Type -> resource ids
Query -> matching resource ids
Relation -> related resource ids
Discovery source -> candidate resource ids
```

These indexes let relation-based queries, such as drive temperature sensors, avoid repeated full-graph scans.

### Event Stream

All accepted store changes emit events into one ordered stream.

```rust
pub struct EventEnvelope<E> {
    pub seq: u64,
    pub timestamp: SystemTime,
    pub event: E,
}
```

Events are emitted after the store accepts the mutation.

Event families:

- resource events
- discovery events
- scheduler/load events
- query lifecycle events
- freshness events

Typed subscriptions are filtered views over this same event stream.

## Query Lifecycle

### `list()`

```text
1. Build temporary query plan.
2. Run relevant discovery through the scheduler.
3. Fetch candidate resources through the scheduler.
4. Apply predicates.
5. Return matching snapshots.
6. Remove temporary demand.
```

### `subscribe()`

```text
1. Register long-lived query demand.
2. Run initial discovery and fetch.
3. Emit Added events for matching resources.
4. Refresh matching resources according to desired freshness.
5. Re-run discovery according to discovery freshness.
6. Emit Added, Updated, Removed, Error, and FreshnessMissed events.
7. Remove demand when the subscription is dropped.
```

### `watch()`

`watch()` is like `subscribe()`, but it keeps the materialized view warm without returning resource events to the caller. Dropping the watch handle removes the demand.

### `refresh(id)`

`refresh(id)` creates interactive demand for one resource. It bypasses freshness checks, but still goes through scheduler limits.

## Interaction With Health Service

In `bare-metal-manager-core/crates/health`, the scraper would replace Redfish polling collectors, not the whole health service.

Today the health crate owns:

- endpoint discovery and sharding
- collector lifecycle
- Redfish traversal
- per-collector periodic loops
- per-collector BMC clients and caches
- per-collector concurrency
- sink and health-report conversion

With the scraper, health should keep:

- endpoint discovery and sharding
- credential/proxy setup
- data sinks
- Prometheus/tracing/override sinks
- health report processors
- log file persistence policy
- non-Redfish collectors such as NVUE/NMX-T

The scraper should own:

- Redfish discovery
- Redfish resource refresh
- shared BMC cache
- shared adaptive scheduler
- request coalescing
- resource freshness and staleness
- typed Redfish event stream

Health then becomes a projection layer:

```text
Scraper Resource Events
  |
  +--> SensorHealthProjection -> CollectorEvent::Metric
  +--> FirmwareProjection     -> CollectorEvent::Firmware
  +--> LogProjection          -> CollectorEvent::Log
```

For sensors, health-specific label building, threshold handling, unit sanitization, and health-report mapping should remain outside the scraper.

For firmware, health can project `SoftwareInventory` snapshots into existing firmware events.

For logs, the scraper can discover log services and schedule Redfish reads. Health should initially keep persistent high-water state and log file writing, because those are application policies.

## Migration Path

1. Add the scraper crate and BMC scheduler.
2. Implement standard discovery for service root, chassis, systems, sensors, firmware inventory, and log services.
3. Add one scraper runner per health endpoint.
4. Convert sensor metrics from scraper events while keeping existing health sinks.
5. Move firmware collection onto scraper queries.
6. Move log service discovery onto scraper queries.
7. Migrate log entry cursoring after the generic cursor API is mature.

## Design Summary

Queries define desired state.

Reconcilers convert desired state into BMC work.

The scheduler protects the BMC.

The store materializes the latest known Redfish graph.

Events expose every accepted change.

Applications consume typed filtered views or project the event stream into their own domain.
