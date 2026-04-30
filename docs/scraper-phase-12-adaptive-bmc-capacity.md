# Scraper Phase 12: Adaptive BMC Capacity

This phase lets the scheduler react to BMCs whose capacity is unknown and changes over time.

The scraper starts conservatively, learns from observed completions, and backs off quickly on overload signals.

## Guardrails

- The scheduler must not require prior latency knowledge.
- Adaptive mode must respect hard configured maximums.
- Capacity should increase slowly and decrease quickly.
- Overload must delay polling instead of creating unbounded backlog.
- Interactive requests must still respect adaptive and hard limits.
- Load state must be observable.

## Public API

```rust
BmcCapacity::adaptive()
    .initial_in_flight(1)
    .max_in_flight(16)
    .max_requests_per_second(30);
```

Optional later knobs can tune windows and thresholds, but defaults should be safe.

## Adaptive Policy

Start with AIMD:

```text
healthy observation window -> add one concurrency slot, up to max
timeout / 429 / 503 / connection reset -> halve concurrency, at least one
sharp latency increase -> reduce concurrency
stable recovery -> increase gradually
```

Keep the policy separate from queue fairness.

## TDD Test Plan

### 1. `adaptive_capacity_starts_conservative`

Configure adaptive mode with `initial_in_flight(1)`.

Assert only one request dispatches initially.

### 2. `adaptive_capacity_increases_after_healthy_window`

Complete enough requests quickly and successfully.

Assert allowed in-flight capacity increases.

### 3. `adaptive_capacity_decreases_after_timeout`

Simulate request timeout.

Assert capacity decreases.

### 4. `adaptive_capacity_decreases_after_503_or_429`

Simulate BMC overload responses.

Assert capacity decreases and load state changes.

### 5. `adaptive_capacity_marks_load_state_slow`

Increase fake BMC latency sharply without hard errors.

Assert scheduler emits slow/load-changed event.

### 6. `overload_delays_polling_instead_of_backlogging`

Run background freshness under constrained capacity.

Assert stale state is reported and missed ticks are not queued.

### 7. `interactive_refresh_still_respects_hard_limits`

Flood interactive refreshes.

Assert adaptive and hard limits still bound dispatch.

## Implementation Steps

1. Add scheduler observation records for latency and outcome.
2. Add load state enum and scheduler events.
3. Implement AIMD capacity controller as a pure component.
4. Connect controller output to concurrency admission.
5. Detect overload outcomes from BMC errors.
6. Add latency-window tests with a manual clock.
7. Ensure background reconcilers observe reduced capacity naturally.

## Acceptance Checklist

- Adaptive mode starts conservatively.
- Healthy BMC behavior increases useful capacity gradually.
- Overload decreases capacity quickly.
- Load state changes are observable.
- Polling becomes stale rather than backlogged under overload.

## Explicitly Out Of Scope

- machine-learning capacity prediction
- per-endpoint persistent capacity history
- per-resource adaptive intervals
- adaptive lane weights
