# Phase 7: Sensors, computer systems, expand handling, parent linkage

## Goal

Extend the adapter from "service root + chassis" (Phase 6) to the rest
of the Redfish capabilities the scraper needs out of the gate. Phase 7
also wires real `$expand` handling so a single fetch can yield an
`Inserted` parent and zero or more `Inserted` children in a single
work item.

## Tests to turn green

The Phase 0 type-only tests already exercise the public surface:

- `redfish_adapter_api.rs::expanded_payload_preservation_is_represented_in_the_event_api`
- `redfish_adapter_api.rs::reconstruction_records_preserve_hierarchy_identity_without_execution_handles`
- `discovery_flow.rs::child_resource_events_carry_parent_odata_id`
- `discovery_flow.rs::expanded_payload_preservation_is_representable_via_event_api`

Phase 7 adds end-to-end tests to `redfish_adapter_api.rs`:

- `chassis_$expand_yields_parent_and_children_with_correct_parent_odata_id`
- `sensors_builder_emits_one_event_per_sensor_under_chassis`
- `computer_system_builder_emits_one_event_per_system`

## Design decisions

- **Expand expressed via `nv-redfish::query::ExpandQuery`.** The Phase 7
  generators construct an `ExpandQuery` with depth ≥ 1 when the caller
  opts in via a per-builder configuration toggle. The default remains
  unexpanded fetch (one event per fetch).
- **Parent linkage.** When the fetch produces a parent and children in
  one response, each child's `parent_odata_id` is filled with the
  parent's `@odata.id`. The parent's own `parent_odata_id` continues to
  reflect its real parent (e.g., `Chassis` collection points at
  `/redfish/v1`).
- **EntityPayload boundary.** Each event's optional
  `EntityPayload` carries the kind, `@odata.id`, and `@odata.etag` of
  the child. Until the CSDL compiler exposes a generated
  `EntityPayload` enum, payloads remain the Phase 0 struct.
- **Sensors.** `build_sensors_generator` takes a `Chassis<B>` and walks
  the chassis's sensor sub-tree using the standard Redfish links
  (`/redfish/v1/Chassis/{id}/Thermal`, `.../Power`, `.../Sensors`).
  Each sensor becomes one `RedfishResourceEvent`.
- **Computer systems.** `build_computer_system_generator` mirrors the
  chassis builder for the systems collection.

## Acceptance criteria

- The three new end-to-end tests pass under
  `cargo test -p nv-redfish-scraper --features
   "redfish-adapter,adapter-service-root,adapter-chassis,adapter-sensors,adapter-computer-systems"`.
- Phase 0–6 tests remain green.
- The trybuild "adapter-with-one-cap-hides-others" fixture still
  enforces feature gating.

## Out of scope (deferred)

- Reconstruction record derivation from the resource-event stream —
  Phase 8.
