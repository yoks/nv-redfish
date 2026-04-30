# Redfish Scraper Requirements

This document defines guardrails for the proposed higher-level Redfish scraper crate. It is intentionally short and normative.

## Scope

The scraper crate maintains a typed, freshness-aware, BMC-safe view of Redfish resources.

It sits above `nv-redfish` and `nv-redfish-core`. It does not replace generated Redfish schema types, the transport-level `Bmc` trait, or domain-specific health/reporting logic.

## MUST

- The crate MUST treat resource discovery as a first-class problem.
- The crate MUST support resources that do not have stable global paths, including sensors, drives, log services, firmware inventory, and OEM resources.
- The crate MUST expose a typed query API for resource sets.
- The crate MUST expose direct typed access for known or explicitly addressed resources.
- The crate MUST keep discovery freshness separate from resource freshness.
- The crate MUST represent freshness honestly in returned snapshots.
- The crate MUST allow stale data when the BMC cannot satisfy desired freshness.
- The crate MUST avoid unbounded request backlogs.
- The crate MUST coalesce duplicate in-flight requests.
- The crate MUST ensure every BMC request passes through a shared scheduler.
- The scheduler MUST support bounded concurrency and bounded request rate.
- The scheduler MUST provide fair service across interactive, subscription, discovery, and maintenance work.
- Discovery MUST have a nonzero guaranteed service share when discovery work is pending.
- Polling MUST be demand-driven by queries, watches, subscriptions, or explicit refreshes.
- Polling MUST be coalesced so each resource has at most one pending refresh.
- The crate MUST support adaptive behavior when the BMC is slow or overloaded.
- The crate MUST allow users to subscribe to a single event stream for observability and integration.
- Typed subscriptions MUST be filtered views over the same internal resource/event stream.
- Store mutations MUST emit events after the store accepts the change.
- The crate MUST support vendor- or application-specific discoverers.
- The crate MUST provide escape hatches for raw JSON or unknown/OEM resources.

## MUST NOT

- The crate MUST NOT assume universal paths such as `/redfish/v1/Sensors`.
- The crate MUST NOT make individual collectors or queries call the BMC directly.
- The crate MUST NOT let high-frequency sensor polling starve discovery forever.
- The crate MUST NOT let discovery crawl the entire BMC as one large blocking operation.
- The crate MUST NOT treat requested freshness as a hard real-time guarantee.
- The crate MUST NOT hide staleness from callers.
- The crate MUST NOT accumulate missed poll ticks as work that must be replayed.
- The crate MUST NOT require users to know all resource URIs before using subscriptions.
- The crate MUST NOT duplicate BMC clients, caches, and rate limiters per resource class.
- The crate MUST NOT embed health-service-specific metric labels, sink behavior, or health report policy.
- The crate MUST NOT require full durable event sourcing for the first implementation.
- The crate MUST NOT make the simple API depend on queueing theory concepts.

## SHOULD

- The crate SHOULD start conservatively when BMC capacity is unknown.
- The crate SHOULD learn practical BMC capacity from observed latency, completion rate, and errors.
- The crate SHOULD reduce concurrency quickly on timeout, `429`, `503`, connection reset, or sharp latency increase.
- The crate SHOULD increase concurrency gradually after stable healthy windows.
- The crate SHOULD expose scheduler and freshness metrics.
- The crate SHOULD make built-in discovery incremental and resumable.
- The crate SHOULD let predicates provide discovery hints, while preserving correctness by filtering fetched snapshots.
- The crate SHOULD make common Redfish queries ergonomic, especially sensor kind, physical context, name, and relation filters.

## Non-Goals

- Replacing `nv-redfish-core::Bmc`.
- Replacing generated Redfish schema crates.
- Replacing application-specific sinks, health processors, or log persistence policy.
- Guaranteeing real-time telemetry delivery.
- Providing durable replay/event sourcing in the initial version.
