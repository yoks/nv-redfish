# Scraper Phase 15: Firmware And Log-Service Discovery

This phase broadens standard discovery beyond sensors so additional health collector classes can move onto the shared scraper.

The focus is discovering and refreshing resource classes. Application-specific cursoring and log persistence remain outside scraper unless a generic cursor API is designed later.

## Guardrails

- Standard discovery must remain incremental.
- Firmware and log discovery must use scheduler-controlled BMC calls.
- Discovery must deduplicate resource ids.
- Log entry persistence policy must not enter scraper core.
- Firmware/log projections must remain application-owned.

## Public API

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

## Minimal Sources

Firmware inventory:

- service root update service
- firmware inventory collection
- software inventory collection if exposed separately

Log services:

- chassis log services
- system log services
- manager log services if already available from service root traversal

## TDD Test Plan

### 1. `standard_discovery_finds_firmware_inventory`

Mock update service with firmware inventory members.

Assert `query::<SoftwareInventory>().list()` returns inventory snapshots.

### 2. `firmware_query_emits_added_and_updated`

Subscribe to firmware inventory.

Assert initial snapshots emit `Added` and later refresh emits `Updated`.

### 3. `standard_discovery_finds_log_services_from_chassis`

Mock chassis-linked log services.

Assert log services are discovered and fetched.

### 4. `standard_discovery_finds_log_services_from_systems`

Mock system-linked log services.

Assert log services are discovered and fetched.

### 5. `log_service_discovery_deduplicates_services`

Expose the same log service through multiple parents.

Assert it is fetched once.

## Implementation Steps

1. Add standard discoverer for `SoftwareInventory`.
2. Add standard discoverer for `LogService`.
3. Reuse existing service root and collection traversal helpers.
4. Add source-specific candidate extraction helpers.
5. Add deduplication tests for shared resource ids.
6. Add health projection examples or tests for firmware and log service resources.
7. Keep log entry cursor state outside scraper.

## Acceptance Checklist

- Firmware inventory can be discovered and refreshed.
- Firmware subscriptions produce typed events.
- Log services can be discovered from chassis and systems.
- Duplicate log service ids are fetched once.
- Health can project firmware and log resources without scraper sink policy.

## Explicitly Out Of Scope

- durable log entry cursors
- log file writing
- firmware compliance policy
- update orchestration
- all possible Redfish inventory sources
