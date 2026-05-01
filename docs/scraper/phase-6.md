# Phase 6: Redfish adapter — service root and chassis fetch

## Goal

Replace the Phase 0 `NotImplementedGenerator` body for the service-root
and chassis builders with real fetch logic that consumes the
application-supplied `nv-redfish` typed objects (`ServiceRoot<B>`,
`Chassis<B>`). The adapter remains the only place that talks to
`nv-redfish`; the runtime stays Redfish-free.

## Tests to turn green

| File | Test |
| ---- | ---- |
| `tests/redfish_adapter_api.rs` | An end-to-end success path that lands a `RuntimeOutput::Work(Ok(_))` carrying a `RedfishEvent::Resource` for service root |
| `tests/redfish_adapter_api.rs` | An end-to-end failure path that surfaces `RuntimeOutput::Work(Err(RedfishAdapterError::*))` |
| `tests/redfish_adapter_api.rs` | A regression test ensuring the success path remains green when transport returns `RefreshedNoChange` |

These specific test names are not present in Phase 0 because Phase 0
cannot construct a real `Bmc` without network access. Phase 6 introduces
a `MockBmc` that synthesises typed `Arc<T>` payloads in-memory and
exposes the success/failure surface to the scraper.

## Design decisions

- **MockBmc fixture.** A test-only `MockBmc` implements `nv-redfish::Bmc`
  using static `Arc<T>` instances for `get`/`expand` and explicit
  `Result::Err` values for the failure path. It lives under
  `scraper/tests/support/mock_bmc.rs` and is gated on
  `feature = "redfish-adapter"`.
- **Service-root fetch.** `build_service_root_generator` clones the
  passed-in `ServiceRoot<B>` into the generator. On `take_next` the
  generator returns a future that calls `service_root.bmc().get(...)`
  for the discoverable child collections (Chassis, Systems,
  EventService, etc.) and emits one `RedfishResourceEvent` per
  collection root.
- **Chassis fetch.** `build_chassis_generator` walks the chassis
  collection one entity per work item. Each fetched chassis becomes a
  `RedfishResourceEvent` with `change = Inserted` (first observation)
  or `Updated`/`RefreshedNoChange` (subsequent observations).
- **Error mapping.** Transport errors map to
  `RedfishAdapterError::Transport(String)` (introduced in Phase 6 as a
  new non-exhaustive variant). Parse errors map to
  `RedfishAdapterError::Parse(String)`. The `NotImplemented` variant
  remains for capabilities that are still stubbed.
- **No detached fetch.** All fetches must go through the
  `nv-redfish` typed wrapper; the trybuild fixture
  `no_detached_redfish_command.rs` continues to enforce this at compile
  time.

## Acceptance criteria

- The new end-to-end tests pass under
  `cargo test -p nv-redfish-scraper --features "redfish-adapter,adapter-service-root,adapter-chassis"`.
- The Phase 0 type-only tests in `redfish_adapter_api.rs` and
  `discovery_flow.rs` continue to pass without modification.
- The trybuild feature-gating fixtures remain unchanged.
- Phase 0–5 scheduler tests remain green.

## Out of scope (deferred)

- Sensors, computer systems, expand handling, parent linkage — Phase 7.
- Reconstruction records — Phase 8.
