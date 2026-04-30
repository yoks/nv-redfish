# Scraper Phase 3: Global Event Stream

This phase makes every accepted scraper change observable through one ordered stream.

Phase 1 already emits minimal resource events. This phase completes the event model and hardens ordering, subscription, and error behavior.

## Guardrails

- Store mutation events must be published after the store accepts the mutation.
- Failed BMC work may emit error events but must not mutate the store.
- Cached reads must not emit events.
- Event sequence numbers must be monotonic within one scraper instance.
- Event publication must not block store locks.
- Dropping all subscribers must not break scraper operation.

## Public API

```rust
let mut events = scraper.subscribe_events();

while let Ok(envelope) = events.recv().await {
    match envelope.event {
        ScraperEvent::Resource(event) => {
            tracing::debug!(seq = envelope.seq, ?event);
        }
        _ => {}
    }
}
```

Initial implementation can use `tokio::sync::broadcast::Receiver`.

## Event Types

```rust
pub struct EventEnvelope<E> {
    pub seq: EventSeq,
    pub timestamp: SystemTime,
    pub event: E,
}

pub enum ScraperEvent<B>
where
    B: Bmc,
{
    Resource(ResourceEvent<B>),
}
```

Use a typed `EventSeq` newtype if the current code does not already have one. Keep payloads small.

## Internal Flow

```text
refresh succeeds
  |
  v
store.insert(snapshot) -> MutationKind
  |
  v
event_bus.publish(ResourceEvent::Added or Updated)
  |
  v
return snapshot
```

For a failed fetch:

```text
scheduler.get<T>(id) fails
  |
  v
event_bus.publish(ResourceEvent::Error)
  |
  v
return error
```

## TDD Test Plan

### 1. `events_have_monotonic_sequence_numbers`

Perform two successful refreshes and read two events.

Assert the second sequence is greater than the first.

### 2. `events_are_emitted_after_store_mutation`

Subscribe, refresh a resource, receive the event, then inspect the store.

The snapshot must already be present when the event is observed.

### 3. `new_subscriber_receives_future_events`

Create one event, then create a new subscriber.

Trigger another event. The new subscriber receives only the future event.

### 4. `event_stream_includes_resource_errors`

Configure the fake BMC to fail a refresh.

Assert a resource error event is emitted and no snapshot is stored.

### 5. `dropping_event_subscriber_does_not_stop_scraper`

Create and drop a subscriber, then refresh a resource.

The refresh still succeeds and future subscribers can receive later events.

## Implementation Steps

1. Make event sequence generation the responsibility of `EventBus`.
2. Ensure `EventBus::publish` creates the envelope.
3. Ensure successful resource events are emitted after store locks are released.
4. Ensure refresh errors produce error events.
5. Add event tests that avoid real sleeps.
6. Keep replay and persistence out of the event bus.

## Acceptance Checklist

- One global stream exists per scraper.
- Events have monotonic sequence numbers.
- Successful resource events correspond to already-mutated store state.
- Error events do not imply store mutation.
- Subscribers receive future events without owning scraper progress.

## Explicitly Out Of Scope

- durable event sourcing
- replay from sequence number
- typed subscription filtering
- scheduler/load events
- query lifecycle events
