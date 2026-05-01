# Scraper phase 11: Redfish adapter event boundary

## Purpose

Complete the Redfish adapter event and reconstruction boundary without adding
real fetch behavior yet.

## Required reading

- [Phase 0 contract](../phase-0.md)
- [Requirements](../requirements.md)
- [Architecture](../architecture.md)
- [Redfish adapter](../redfish-adapter.md)
- [Rust style guide](../rust-style-guide.md)

## Frozen test rule

The Phase 0 tests are frozen. Public Redfish event boundaries must satisfy the
tests rather than exposing execution handles.

## Frozen tests to preserve

- [redfish_adapter_api.rs](../../../scraper/tests/redfish_adapter_api.rs)
- [feature_gating.rs](../../../scraper/tests/feature_gating.rs)
- [no_detached_redfish_command.rs](../../../scraper/tests/trybuild/no_detached_redfish_command.rs)

## Target tests

- Redfish event identity tests.
- No-execution-handle tests.
- Reconstruction record tests.
- Serde serialization tests.

## Scope

- Preserve BMC id, `ODataId`, optional parent id, change kind, payload, metadata,
  and errors.
- Keep public events free of `B`, `ServiceRoot<B>`, `Chassis<B>`, and similar
  execution handles.
- Provide reconstruction records from read-side data.
- Support serialization with the `serde` feature.

## Out of scope

- Real BMC fetching.
- Concrete capability builders.
- Generated `EntityPayload` enum integration.

## Target commands

```sh
cargo test -p nv-redfish-scraper --features redfish-adapter --test redfish_adapter_api
cargo test -p nv-redfish-scraper --all-features --test redfish_adapter_api
cargo test -p nv-redfish-scraper --features redfish-adapter --test feature_gating
```

## Done

- Adapter event-boundary target tests pass.
- Adapter code remains behind `redfish-adapter`.
