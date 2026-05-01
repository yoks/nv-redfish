# Scraper phase 12: typed Redfish generator builders

## Purpose

Add typed Redfish generator builders that close over valid `nv-redfish` objects
and keep discovery policy in the application.

## Required reading

- [Phase 0 contract](../phase-0.md)
- [Requirements](../requirements.md)
- [Architecture](../architecture.md)
- [Redfish adapter](../redfish-adapter.md)
- [Rust style guide](../rust-style-guide.md)

## Frozen test rule

The Phase 0 tests are frozen. Do not add a detached command language to make
builder implementation easier.

## Frozen tests to preserve

- [redfish_adapter_api.rs](../../../scraper/tests/redfish_adapter_api.rs)
- [feature_gating.rs](../../../scraper/tests/feature_gating.rs)
- [no_detached_redfish_command.rs](../../../scraper/tests/trybuild/no_detached_redfish_command.rs)

## Target tests

- Typed builder shape tests.
- Detached command compile-fail tests.
- Future capability-gating tests added only with matching requirements.

## Scope

- Builders are generic over `B: nv_redfish::Bmc`.
- Builders close over typed objects such as `ServiceRoot<B>` and future resource
  wrappers.
- Disabled capabilities hide builders, config fields, payload variants, and
  fetch code.
- Applications choose which compiled generators to add.

## Out of scope

- Application discovery policy.
- Carbide-specific conversion.
- Generated `EntityPayload` enum integration.

## Target commands

```sh
cargo test -p nv-redfish-scraper --features redfish-adapter --test redfish_adapter_api
cargo test -p nv-redfish-scraper --features redfish-adapter --test feature_gating
```

## Done

- Typed builder tests pass.
- Invalid object/command pairings remain unrepresentable.
