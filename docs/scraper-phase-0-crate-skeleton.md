# Scraper Phase 0: Crate Skeleton

This phase creates the crate shape and public entry points without useful Redfish behavior.

The goal is to make the future API compile early, prove construction is side-effect free, and create module ownership boundaries that later phases can fill in.

## Guardrails

- Building a scraper must not call the BMC.
- Registering discovery must not crawl the BMC.
- Public placeholders must be documented as placeholders.
- Internal modules must preserve the intended ownership boundaries.
- The crate must compile with the scraper style guide lints.

## Linter Settings

Add the crate lints in `src/lib.rs` during Phase 0. These settings are part of the crate skeleton, not a cleanup task for later phases.

```rust
#![deny(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::suspicious,
    clippy::complexity,
    clippy::perf
)]
#![deny(
    clippy::absolute_paths,
    clippy::todo,
    clippy::unimplemented,
    clippy::tests_outside_test_module,
    clippy::panic,
    clippy::unwrap_used,
    clippy::unwrap_in_result,
    clippy::unused_trait_names,
    clippy::print_stdout,
    clippy::print_stderr
)]
#![deny(missing_docs)]
#![allow(clippy::doc_markdown)]
```

These lints mean Phase 0 must include crate-level documentation, public item documentation, and test helpers that do not rely on `unwrap`, `panic!`, stdout, or placeholder `todo!`/`unimplemented!` bodies.

## Public API

```rust
let scraper = Scraper::builder(bmc)
    .capacity(BmcCapacity::adaptive())
    .discover(Discovery::standard())
    .build()
    .await?;

let resources = scraper.resources::<Sensor>();
let query = scraper.query::<Sensor>();
let events = scraper.subscribe_events();
```

`resources`, `query`, and `subscribe_events` may return inert handles in this phase.

## Internal Shape

Create the module layout:

```text
src/lib.rs
src/builder.rs
src/capacity.rs
src/discovery.rs
src/error.rs
src/event.rs
src/resources.rs
src/scheduler.rs
src/snapshot.rs
src/store.rs
```

Create one shared `Inner<B>` owned by `Scraper<B>`.

`Scraper<B>` should be cheap to clone.

## TDD Test Plan

### 1. `builder_creates_scraper`

Given a fake BMC, building a scraper returns `Ok(Scraper<_>)`.

The fake BMC request count remains zero.

### 2. `scraper_is_cloneable`

Clone the scraper and use both handles to create resource/query clients.

The test proves the public handle is cheap and shareable.

### 3. `subscribe_events_returns_stream`

Calling `subscribe_events()` returns a receiver without requiring background tasks or BMC access.

### 4. `discovery_registration_does_not_call_bmc`

Register `Discovery::standard()` during build.

Assert no BMC requests were made.

## Implementation Steps

1. Add the crate to the workspace.
2. Add the crate dependencies using workspace dependency style.
3. Define `Scraper<B>` and private `Inner<B>`.
4. Define `ScraperBuilder<B>`.
5. Add placeholder `BmcCapacity` and `Discovery` types.
6. Add placeholder `ResourceClient<B, T>` and `QueryBuilder<B, T>`.
7. Add an event bus that can create receivers.
8. Add crate-level docs and the linter settings from this document.

## Acceptance Checklist

- The crate builds.
- The public top-level API compiles.
- Scraper construction is side-effect free.
- Discovery registration is side-effect free.
- Event subscription can be created.

## Explicitly Out Of Scope

- BMC fetches
- typed snapshots
- cached access
- discovery execution
- scheduler limits
- query filtering
- subscriptions
- background tasks
