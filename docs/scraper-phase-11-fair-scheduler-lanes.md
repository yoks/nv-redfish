# Scraper Phase 11: Fair Scheduler Lanes

This phase prevents one class of work from starving another.

The fixed scheduler already enforces hard bounds. This phase adds lane shares so discovery, subscriptions, interactive requests, and maintenance all make progress.

## Guardrails

- Discovery must have a nonzero guaranteed service share when it has work.
- Interactive work can receive lower latency but cannot create infinite priority.
- Unused lane capacity may be borrowed.
- Borrowed capacity must return when the original lane has work.
- Fairness logic must be isolated and unit-tested.

## Public API

```rust
BmcCapacity::fixed()
    .interactive_share(50)
    .subscription_share(30)
    .discovery_share(15)
    .maintenance_share(5);
```

Shares are relative weights. Validate that configured shares are nonzero for required lanes or normalize safely.

## Scheduling Algorithm

Use a simple weighted fair algorithm.

Deficit round robin is a good first choice:

```text
each lane has a queue and deficit
each scheduling round adds lane weight to deficit
dispatch while lane has work and deficit allows it
charge one cost per request initially
```

Do not mix adaptive capacity policy into fairness.

## TDD Test Plan

### 1. `discovery_lane_makes_progress_under_subscription_load`

Keep subscription work continuously queued.

Add discovery work and assert it dispatches within a bounded number of scheduler turns.

### 2. `interactive_lane_has_lower_wait_than_background_work`

Queue background work, then interactive work.

Assert interactive wait is lower under configured shares without violating hard limits.

### 3. `unused_lane_capacity_can_be_borrowed`

Queue only subscription work.

Assert it can use available capacity beyond its nominal share.

### 4. `borrowed_capacity_returns_when_lane_has_work`

After subscription borrows capacity, add discovery work.

Assert discovery receives service again.

### 5. `per_query_subscription_work_is_fair`

Queue work for multiple subscription owners.

Assert one subscription cannot monopolize the subscription lane forever.

## Implementation Steps

1. Replace single FIFO with per-lane queues.
2. Add lane weight configuration.
3. Implement fairness policy as a pure, unit-tested component.
4. Keep hard concurrency and rate limiters outside the fairness policy.
5. Add owner-level fairness inside the subscription lane.
6. Add scheduler stats that expose per-lane queue and dispatch counts.

## Acceptance Checklist

- Discovery cannot starve under continuous subscription load.
- Interactive work receives better latency.
- Unused capacity can be borrowed.
- Borrowed capacity returns when a lane becomes active.
- Hard limits from Phase 4 still hold.

## Explicitly Out Of Scope

- adaptive capacity
- dynamic lane weights
- deadline scheduling
- request cost estimation beyond equal cost
