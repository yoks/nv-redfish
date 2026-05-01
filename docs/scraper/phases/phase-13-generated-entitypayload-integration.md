# Scraper phase 13: generated EntityPayload integration

## Purpose

Integrate the Redfish adapter with the generated `EntityPayload` enum once CSDL
compiler support exists.

## Required reading

- [Phase 0 contract](../phase-0.md)
- [Requirements](../requirements.md)
- [Architecture](../architecture.md)
- [Redfish adapter](../redfish-adapter.md)
- [Rust style guide](../rust-style-guide.md)

## Frozen test rule

Existing Phase 0 tests are frozen. New generated-payload tests may be added only
with the corresponding compiler/codegen requirement change.

## Frozen tests to preserve

- [redfish_adapter_api.rs](../../../scraper/tests/redfish_adapter_api.rs)
- [feature_gating.rs](../../../scraper/tests/feature_gating.rs)
- future generated-payload tests added with the codegen work

## Target tests

- Generated payload identity tests.
- `@odata.id`, `@odata.etag`, and entity kind tests.
- Expanded `NavProperty<T>` payload preservation tests.
- Capability-specific payload variant feature-gating tests.

## Scope

- Use generated `EntityPayload` instead of a parallel scraper domain model.
- Preserve generated schema data.
- Preserve expanded payload data.
- Keep serialized events read-side only: BMC id, resource id, parent id, change
  metadata, payload, scrape metadata, and errors.

## Out of scope

- Carbide model conversion.
- Durable replay policy.
- Required reconstruction of execution handles by event consumers.

## Target commands

```sh
cargo test -p nv-redfish-scraper --all-features --test redfish_adapter_api
```

## Done

- Generated payload tests pass.
- Adapter still does not expose execution handles in public event payloads.
