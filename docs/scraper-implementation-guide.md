# Redfish Scraper Implementation Guide

This document guides implementation of the scraper crate in small, end-to-end, tested phases.

The goal is not to build all architecture pieces at once. Each phase must expose one complete behavior through the public API, prove it with tests, and preserve the requirements guardrails.

All implementation work must follow [Redfish Scraper Rust Style Guide](scraper-rust-style-guide.md).

## Development Rules

- Start each phase by writing failing tests.
- Keep each phase end-to-end: user API, internal behavior, and observable result.
- Prefer a narrow vertical slice over a broad partial subsystem.
- Every BMC operation in tests must go through the scheduler, even if the scheduler is simple in early phases.
- Do not add background polling until direct refresh and one-shot query behavior are correct.
- Do not add adaptive scheduling until fixed bounded scheduling is correct.
- Do not add health-service projections until scraper events and snapshots are stable.
- Do not implement durable event sourcing in the initial crate.

## Test Harness

Build a small test harness before feature work:

- a fake `Bmc` implementation with typed responses by `ODataId`
- request recording with lane, resource type, id, and operation
- controllable latency and errors
- a manual clock or paused Tokio time for freshness tests
- helpers for awaiting events without sleeping in real time

The harness must allow tests to assert:

- which BMC requests were made
- how many BMC requests were made
- whether duplicate requests were coalesced
- which events were emitted
- whether snapshots are fresh or stale
- whether request lanes receive service under contention

## Phase 0: Crate Skeleton

Detailed guide: [Scraper Phase 0: Crate Skeleton](scraper-phase-0-crate-skeleton.md)

### User Value

The crate builds and exposes the top-level public shape without useful Redfish behavior yet.

### Public Surface

```rust
Scraper::builder(bmc)
    .capacity(BmcCapacity::adaptive())
    .discover(Discovery::standard())
    .build()
    .await?;

scraper.resources::<T>();
scraper.query::<T>();
scraper.subscribe_events();
```

### Tests First

- `builder_creates_scraper`
- `scraper_is_cloneable`
- `subscribe_events_returns_stream`
- `discovery_registration_does_not_call_bmc`

### Implementation Notes

Create the crate, module layout, builder, `Scraper`, `Inner`, placeholder `DiscoveryRegistry`, placeholder `ResourceStore`, placeholder scheduler, and event stream.

No Redfish requests should be made in this phase.

### Done When

- The crate compiles.
- The builder works.
- Registering discovery is side-effect free.
- Event subscription can be created.

## Phase 1: Direct Typed Refresh

Detailed guide: [Scraper Phase 1: Direct Typed Refresh](scraper-phase-1-direct-refresh.md)

### User Value

Users can fetch one known resource by URI and receive a typed snapshot.

### Public Surface

```rust
let snapshot = scraper
    .resources::<Sensor>()
    .refresh("/redfish/v1/Chassis/1/Sensors/InletTemp")
    .await?;
```

### Tests First

- `refresh_fetches_known_resource`
- `refresh_stores_snapshot`
- `refresh_emits_resource_added_event`
- `refresh_emits_resource_updated_event_on_second_value`
- `refresh_uses_scheduler`
- `refresh_error_emits_resource_error`

### Implementation Notes

Implement:

- `ResourceSnapshot<T>`
- `Staleness`
- typed store insertion
- `ResourceEvent::Added`
- `ResourceEvent::Updated`
- `ResourceEvent::Error`
- scheduler API for one `Get` operation

The scheduler can be simple FIFO with max in-flight `1`, but all BMC calls must go through it.

### Done When

- Direct refresh works end to end.
- Snapshots are stored.
- Events are emitted only after the store is updated.
- No direct BMC calls bypass the scheduler.

## Phase 2: Cached Direct Access

Detailed guide: [Scraper Phase 2: Cached Direct Access](scraper-phase-2-cached-access.md)

### User Value

Users can read the materialized view without BMC I/O.

### Public Surface

```rust
let cached = scraper
    .resources::<Sensor>()
    .cached("/redfish/v1/Chassis/1/Sensors/InletTemp");

let all = scraper.resources::<Sensor>().list_cached();
```

### Tests First

- `cached_returns_none_for_unknown_resource`
- `cached_returns_snapshot_after_refresh`
- `cached_does_not_call_bmc`
- `list_cached_returns_all_snapshots_for_type`
- `list_cached_is_type_scoped`

### Implementation Notes

Finish type-indexed store APIs:

- lookup by `(TypeId, ODataId)`
- list by type
- typed downcast from erased snapshot

### Done When

- Cached access never calls the BMC.
- Type separation is enforced.
- Snapshots returned from cache preserve staleness metadata.

## Phase 3: Global Event Stream

Detailed guide: [Scraper Phase 3: Global Event Stream](scraper-phase-3-global-event-stream.md)

### User Value

Users and integrations can observe all scraper changes from one stream.

### Public Surface

```rust
let mut events = scraper.subscribe_events();
```

### Tests First

- `events_have_monotonic_sequence_numbers`
- `events_are_emitted_after_store_mutation`
- `new_subscriber_receives_future_events`
- `event_stream_includes_resource_errors`
- `dropping_event_subscriber_does_not_stop_scraper`

### Implementation Notes

Add:

- `EventEnvelope`
- `ScraperEvent`
- `ResourceEvent`
- ordered sequence generation
- broadcast behavior

Do not add replay or persistence.

### Done When

- Resource refresh behavior is observable through the global stream.
- Event order is deterministic within one scraper instance.

## Phase 4: Fixed Bounded Scheduler

Detailed guide: [Scraper Phase 4: Fixed Bounded Scheduler](scraper-phase-4-fixed-bounded-scheduler.md)

### User Value

BMC requests are centrally controlled with explicit bounds.

### Public Surface

```rust
BmcCapacity::fixed()
    .max_in_flight(4)
    .max_requests_per_second(10);
```

### Tests First

- `scheduler_limits_in_flight_requests`
- `scheduler_limits_request_rate`
- `scheduler_records_lane_for_each_request`
- `interactive_request_completes_through_scheduler`
- `scheduler_emits_basic_stats_event`

### Implementation Notes

Implement fixed scheduling before fair or adaptive scheduling.

Required lanes:

- `Interactive`
- `Subscription`
- `Discovery`
- `Maintenance`

At this phase, lanes may share FIFO order. Lane fairness comes later.

### Done When

- Limits are enforced under concurrent refreshes.
- Tests can prove no request bypasses the scheduler.
- Scheduler stats are observable.

## Phase 5: In-Flight Request Coalescing

Detailed guide: [Scraper Phase 5: In-Flight Request Coalescing](scraper-phase-5-in-flight-request-coalescing.md)

### User Value

Duplicate requests share one BMC call.

### Public Surface

No new public API.

### Tests First

- `concurrent_refresh_same_resource_uses_one_bmc_request`
- `coalesced_waiters_receive_same_snapshot`
- `coalesced_error_is_returned_to_all_waiters`
- `different_types_or_ids_do_not_coalesce`
- `coalescing_removes_inflight_entry_after_completion`

### Implementation Notes

Coalesce by operation key:

```text
operation kind + resource type + ODataId + query shape
```

For the first implementation, support plain `Get` by type and id.

### Done When

- Concurrent identical refreshes produce one BMC request.
- All waiters observe the same outcome.
- The store emits one accepted mutation event.

## Phase 6: Manual Discovery And One-Shot Query

Detailed guide: [Scraper Phase 6: Manual Discovery And One-Shot Query](scraper-phase-6-manual-discovery-and-one-shot-query.md)

### User Value

Users can ask for a resource set without knowing all URIs.

### Public Surface

```rust
let sensors = scraper
    .query::<Sensor>()
    .list()
    .await?;
```

### Tests First

- `list_uses_registered_discoverer`
- `list_fetches_discovered_candidates`
- `list_returns_matching_snapshots`
- `list_removes_temporary_demand_after_return`
- `list_emits_discovered_and_added_events`
- `list_with_no_discoverer_returns_empty_or_error_by_policy`

### Implementation Notes

Add:

- `Discoverer<T>`
- `DiscoveryBatch`
- `DiscoveryContext`
- `DiscoveryRegistry`
- `DiscoveryEvent::Discovered`

Start with explicit test discoverers that return fixed candidate IDs. Do not implement standard Redfish crawling yet.

### Done When

- `query::<T>().list()` exercises discovery, scheduler fetch, store, predicates, and return value.
- Discovery is demand-driven.
- Registering discoverers remains side-effect free.

## Phase 7: Predicates And Typed Query Filtering

Detailed guide: [Scraper Phase 7: Predicates And Typed Query Filtering](scraper-phase-7-predicates-and-typed-query-filtering.md)

### User Value

Users can filter resource sets.

### Public Surface

```rust
let temps = scraper
    .query::<Sensor>()
    .where_(sensor::reading_type().temperature())
    .list()
    .await?;
```

### Tests First

- `list_applies_snapshot_predicate`
- `predicate_can_filter_by_resource_id`
- `multiple_predicates_are_and_combined`
- `predicate_failure_does_not_fetch_unneeded_candidates_when_candidate_stage_applies`
- `predicate_hints_are_passed_to_discoverer`

### Implementation Notes

Support two predicate stages:

- candidate predicates that can run before fetch
- snapshot predicates that require fetched data

Hints are optimization only. Tests must prove correctness does not depend on hints.

### Done When

- Common query filtering works.
- Predicate hints reach discoverers.
- Snapshot filtering remains authoritative.

## Phase 8: Standard Minimal Sensor Discovery

Detailed guide: [Scraper Phase 8: Standard Minimal Sensor Discovery](scraper-phase-8-standard-minimal-sensor-discovery.md)

### User Value

The crate can find sensors on realistic Redfish shapes without a universal sensor path.

### Public Surface

```rust
Scraper::builder(bmc)
    .discover(Discovery::standard())
    .build()
    .await?;

let sensors = scraper.query::<Sensor>().list().await?;
```

### Tests First

- `standard_discovery_finds_chassis_sensors`
- `standard_discovery_finds_environment_metric_sensor_uris`
- `standard_discovery_deduplicates_sensor_ids`
- `standard_discovery_is_incremental`
- `standard_discovery_does_not_assume_global_sensor_path`

### Implementation Notes

Implement the smallest useful standard sensor discovery:

- service root
- chassis collection
- chassis sensor collection
- environment metrics `DataSourceUri`

Do not implement every sensor source in this phase.

### Done When

- A mock BMC with only chassis-linked sensors works.
- A mock BMC with only environment metric sensor URIs works.
- No test depends on `/redfish/v1/Sensors`.

## Phase 9: Subscribe Without Periodic Refresh

Detailed guide: [Scraper Phase 9: Subscribe Without Periodic Refresh](scraper-phase-9-subscribe-without-periodic-refresh.md)

### User Value

Typed subscriptions observe changes from the global stream.

### Public Surface

```rust
let mut sub = scraper
    .query::<Sensor>()
    .where_(sensor::reading_type().temperature())
    .subscribe()
    .await?;
```

### Tests First

- `subscribe_runs_initial_list`
- `subscribe_emits_added_for_initial_matches`
- `subscribe_filters_global_events_by_query`
- `subscribe_emits_updated_for_matching_resource`
- `subscribe_emits_removed_when_resource_no_longer_matches`
- `dropping_subscription_removes_query_demand`

### Implementation Notes

Do not add timers yet. The subscription should:

- register query demand
- run initial discovery/fetch
- return a typed stream
- filter global resource events by query membership

### Done When

- Subscriptions are filtered views over the global stream.
- Dropping a subscription removes desired state.

## Phase 10: Freshness And Background Watch

Detailed guide: [Scraper Phase 10: Freshness And Background Watch](scraper-phase-10-freshness-and-background-watch.md)

### User Value

Users can keep matching resources fresh while subscribed or watched.

### Public Surface

```rust
let mut sub = scraper
    .query::<Sensor>()
    .freshness(Duration::from_secs(5))
    .discovery_freshness(Duration::from_secs(60))
    .subscribe()
    .await?;

let watch = scraper
    .query::<Drive>()
    .freshness(Duration::from_secs(30))
    .watch()
    .await?;
```

### Tests First

- `subscribe_refreshes_matching_resource_when_stale`
- `watch_refreshes_without_returning_typed_events`
- `dropping_watch_stops_background_demand`
- `resource_freshness_and_discovery_freshness_are_independent`
- `stale_snapshot_reports_age_and_desired_freshness`
- `missed_poll_ticks_do_not_accumulate`
- `resource_has_at_most_one_pending_refresh`

### Implementation Notes

Add refresh reconciler and discovery reconciler loops with controllable time.

Polling must express desired freshness. It must not enqueue every missed tick.

### Done When

- Background freshness works.
- Staleness is visible.
- Polling backlog is bounded by resource, not by time.

## Phase 11: Fair Scheduler Lanes

Detailed guide: [Scraper Phase 11: Fair Scheduler Lanes](scraper-phase-11-fair-scheduler-lanes.md)

### User Value

Discovery cannot be starved by subscriptions or interactive refreshes.

### Public Surface

```rust
BmcCapacity::fixed()
    .interactive_share(50)
    .subscription_share(30)
    .discovery_share(15)
    .maintenance_share(5);
```

### Tests First

- `discovery_lane_makes_progress_under_subscription_load`
- `interactive_lane_has_lower_wait_than_background_work`
- `unused_lane_capacity_can_be_borrowed`
- `borrowed_capacity_returns_when_lane_has_work`
- `per_query_subscription_work_is_fair`

### Implementation Notes

Use a simple weighted fair algorithm, such as deficit round robin.

Keep hard bounds from Phase 4.

### Done When

- Continuous subscription demand cannot starve discovery.
- Interactive work receives better latency without infinite capacity priority.

## Phase 12: Adaptive BMC Capacity

Detailed guide: [Scraper Phase 12: Adaptive BMC Capacity](scraper-phase-12-adaptive-bmc-capacity.md)

### User Value

The scraper reacts to unexpectedly slow or overloaded BMCs.

### Public Surface

```rust
BmcCapacity::adaptive()
    .initial_in_flight(1)
    .max_in_flight(16);
```

### Tests First

- `adaptive_capacity_starts_conservative`
- `adaptive_capacity_increases_after_healthy_window`
- `adaptive_capacity_decreases_after_timeout`
- `adaptive_capacity_decreases_after_503_or_429`
- `adaptive_capacity_marks_load_state_slow`
- `overload_delays_polling_instead_of_backlogging`
- `interactive_refresh_still_respects_hard_limits`

### Implementation Notes

Use observed behavior, not prior latency knowledge.

Start with AIMD:

```text
healthy window -> increase slowly
timeout/error/sharp slowdown -> decrease quickly
```

Expose load state through scheduler events.

### Done When

- Tests can simulate a slow BMC and observe reduced concurrency.
- Data becomes stale under overload instead of creating unbounded work.

## Phase 13: Relations

Detailed guide: [Scraper Phase 13: Relations](scraper-phase-13-relations.md)

### User Value

Users can query resources by relationship.

### Public Surface

```rust
let drive_temps = scraper
    .query::<Sensor>()
    .where_(sensor::reading_type().temperature())
    .where_(sensor::related_to::<Drive>())
    .list()
    .await?;
```

### Tests First

- `store_records_relation_between_sensor_and_drive`
- `related_to_predicate_filters_by_relation_index`
- `relation_discovery_hint_reaches_discoverer`
- `resource_update_re_evaluates_relation_based_query`
- `relation_removal_emits_removed_for_query`

### Implementation Notes

Add relation indexes to the store.

Start with relations emitted by discoverers. Do not try to infer every Redfish relationship automatically in this phase.

### Done When

- Relation-based sensor queries work end to end.
- Relations participate in query membership updates.

## Phase 14: Health Projection Adapter

Detailed guide: [Scraper Phase 14: Health Projection Adapter](scraper-phase-14-health-projection-adapter.md)

### User Value

The health service can replace a Redfish collector with a scraper-backed projection.

### Public Surface

This may live outside the scraper crate, but it validates integration.

```rust
let mut sensors = scraper
    .query::<Sensor>()
    .freshness(sensor_cfg.sensor_fetch_interval)
    .discovery_freshness(sensor_cfg.state_refresh_interval)
    .subscribe()
    .await?;
```

### Tests First

- `sensor_projection_converts_added_snapshot_to_metric_event`
- `sensor_projection_converts_updated_snapshot_to_metric_event`
- `sensor_projection_preserves_threshold_fields`
- `sensor_projection_uses_relation_labels`
- `sensor_projection_does_not_embed_sink_logic_in_scraper`

### Implementation Notes

Keep health-specific behavior outside the scraper:

- metric labels
- unit sanitization
- thresholds
- health-report policy
- sink calls

This phase can be implemented in an integration example or in the health crate.

### Done When

- One existing health collector path can be represented as scraper query plus projection.
- The scraper remains domain-neutral.

## Phase 15: Firmware And Log-Service Discovery

Detailed guide: [Scraper Phase 15: Firmware And Log-Service Discovery](scraper-phase-15-firmware-and-log-service-discovery.md)

### User Value

Additional Redfish collector classes can move onto the shared scraper.

### Public Surface

```rust
let firmware = scraper
    .query::<SoftwareInventory>()
    .freshness(Duration::from_secs(60 * 60 * 2))
    .subscribe()
    .await?;

let log_services = scraper
    .query::<LogService>()
    .freshness(Duration::from_secs(1800))
    .watch()
    .await?;
```

### Tests First

- `standard_discovery_finds_firmware_inventory`
- `firmware_query_emits_added_and_updated`
- `standard_discovery_finds_log_services_from_chassis`
- `standard_discovery_finds_log_services_from_systems`
- `log_service_discovery_deduplicates_services`

### Implementation Notes

Implement resource discovery first. Keep log entry cursor persistence out of the scraper unless a generic cursor design is added later.

### Done When

- Firmware inventory can be projected from scraper events.
- Log services can be discovered and kept warm by the scraper.

## Release Candidate Gate

Before calling the crate usable, verify:

- every BMC request goes through the scheduler
- direct refresh works
- cached access works
- one-shot query works through discovery
- subscription works through the global event stream
- freshness and discovery freshness are independent
- duplicate requests coalesce
- polling cannot create unbounded backlog
- discovery cannot starve under continuous subscription load
- adaptive mode reacts to slow or failing BMC behavior
- standard discovery does not assume universal resource paths
- health projection can replace at least one Redfish collector path

## Suggested Module Order

```text
src/lib.rs
src/scraper.rs
src/builder.rs
src/snapshot.rs
src/event.rs
src/store.rs
src/scheduler/mod.rs
src/scheduler/fixed.rs
src/scheduler/adaptive.rs
src/discovery/mod.rs
src/query/mod.rs
src/resources.rs
src/reconcile/mod.rs
src/predicate/mod.rs
src/relation.rs
```

Module names are provisional. Keep public API stable only once the early phases prove the shape.
