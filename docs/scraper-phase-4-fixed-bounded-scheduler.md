# Scraper Phase 4: Fixed Bounded Scheduler

This phase turns the scheduler into the only bounded admission point for BMC work.

The scheduler does not need fairness or adaptive behavior yet. It must enforce fixed hard limits for concurrency and request rate.

## Guardrails

- Every BMC request must pass through the scheduler.
- Fixed hard limits must be enforced even for interactive requests.
- Scheduler tests must be deterministic.
- Scheduler code must separate admission decisions from BMC execution.
- Lane metadata must be recorded for every work item.

## Public API

```rust
let scraper = Scraper::builder(bmc)
    .capacity(
        BmcCapacity::fixed()
            .max_in_flight(4)
            .max_requests_per_second(10),
    )
    .build()
    .await?;
```

Initial lanes:

```rust
pub enum Lane {
    Interactive,
    Subscription,
    Discovery,
    Maintenance,
}
```

## Internal Model

Use a single FIFO queue in this phase.

```text
ResourceClient::refresh
  |
  v
Scheduler::submit(WorkItem { lane: Interactive, operation: Get<T>(id) })
  |
  v
admission waits for concurrency and rate permits
  |
  v
Bmc::get<T>(id)
```

The implementation may execute inline if the hard bounds are real. Prefer a shape that can evolve into worker tasks.

## TDD Test Plan

### 1. `scheduler_limits_in_flight_requests`

Use a fake BMC that blocks requests on a test-controlled gate.

Start more refreshes than `max_in_flight`.

Assert the fake BMC sees no more than the configured in-flight count.

### 2. `scheduler_limits_request_rate`

Use paused Tokio time or a manual scheduler clock.

Set a low request rate and assert dispatches occur only when permits are available.

### 3. `scheduler_records_lane_for_each_request`

Execute one request per lane through test-only scheduler entry points.

Assert each recorded request includes the expected lane.

### 4. `interactive_request_completes_through_scheduler`

Call public `resources::<T>().refresh(id)`.

Assert the fake BMC request was observed by scheduler instrumentation.

### 5. `scheduler_emits_basic_stats_event`

Submit enough work to change queue or in-flight stats.

Assert a scheduler stats event is visible on the global event stream.

## Implementation Steps

1. Promote capacity configuration from placeholder to real values.
2. Add scheduler work item and operation types.
3. Add a concurrency limiter.
4. Add a deterministic request-rate limiter.
5. Add scheduler stats snapshots.
6. Route `refresh` through the bounded scheduler.
7. Add tests with blocked fake BMC calls.

## Acceptance Checklist

- In-flight requests never exceed the configured bound.
- Request rate never exceeds the configured bound.
- Interactive refresh still works through the public API.
- Tests can prove no request bypasses the scheduler.
- Scheduler stats are observable.

## Explicitly Out Of Scope

- lane fairness
- adaptive capacity
- request coalescing
- retry policy
- BMC latency learning
