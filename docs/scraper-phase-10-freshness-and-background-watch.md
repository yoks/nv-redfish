# Scraper Phase 10: Freshness And Background Watch

This phase adds background reconciliation for desired freshness.

Subscriptions and watches express desired resource freshness and discovery freshness. The scraper keeps data as fresh as the BMC can safely support.

## Guardrails

- Requested freshness is not a hard real-time guarantee.
- Discovery freshness and resource freshness must be independent.
- Missed ticks must not accumulate as queued work.
- Each resource may have at most one pending refresh.
- Overload must produce stale snapshots, not unbounded backlog.
- Tests must use paused time or a manual clock.

## Public API

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

`watch()` keeps matching resources warm without returning typed events to the caller.

## Internal Model

Add reconcilers:

```text
desired query demand
  |
  +--> discovery reconciler decides membership refresh work
  |
  +--> refresh reconciler decides resource refresh work
```

Reconcilers enqueue work only when the current state is stale and no equivalent work is queued or in flight.

## TDD Test Plan

### 1. `subscribe_refreshes_matching_resource_when_stale`

Use paused time.

Subscribe with short freshness, advance time, and assert a refresh is scheduled.

### 2. `watch_refreshes_without_returning_typed_events`

Create a watch and advance time.

Assert refreshes happen and no typed subscription stream is required.

### 3. `dropping_watch_stops_background_demand`

Drop the watch handle.

Advance time and assert no further refresh work is scheduled for that watch.

### 4. `resource_freshness_and_discovery_freshness_are_independent`

Use different intervals.

Assert resource refresh can occur without discovery refresh and discovery refresh can occur without refetching all fresh resources.

### 5. `stale_snapshot_reports_age_and_desired_freshness`

Advance time beyond desired freshness without allowing BMC progress.

Assert cached snapshots report `Staleness::Stale`.

### 6. `missed_poll_ticks_do_not_accumulate`

Block the scheduler, advance time by many intervals, then unblock.

Assert at most one refresh per demanded resource is queued.

### 7. `resource_has_at_most_one_pending_refresh`

Create overlapping demands for the same resource.

Assert pending refresh work is coalesced by resource.

## Implementation Steps

1. Add freshness fields to query demand.
2. Add `watch()` and watch handle drop cleanup.
3. Add clock abstraction for tests.
4. Add discovery reconciler loop.
5. Add refresh reconciler loop.
6. Add staleness recomputation on snapshot reads.
7. Add freshness-related events.
8. Ensure reconciler work goes through scheduler and coalescing.

## Acceptance Checklist

- Background freshness works for subscriptions and watches.
- Staleness is visible to cached readers and subscribers.
- Discovery and resource freshness are separate.
- Missed polling intervals do not become backlog.
- One resource has at most one pending refresh.

## Explicitly Out Of Scope

- adaptive capacity changes
- fair lane scheduling
- durable demand persistence
- complex retry policy
