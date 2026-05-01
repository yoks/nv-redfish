# Phase 8: Reconstruction records derivation from resource events

## Goal

Provide an automatic derivation path from a stream of
`RedfishResourceEvent`s to a stream of `ReconstructionRecord`s, plus a
public iterator helper applications can use to persist scraper state
without rerunning discovery.

## Tests to turn green

Phase 0 already exercises identity preservation and serde
serializability:

- `redfish_adapter_api.rs::reconstruction_record_can_be_built_from_resource_event`
- `redfish_adapter_api.rs::reconstruction_record_serializes_with_serde`
- `redfish_adapter_api.rs::reconstruction_records_preserve_hierarchy_identity_without_execution_handles`

Phase 8 adds:

- `reconstruction.rs::reconstruction_iterator_produces_records_in_event_order`
- `reconstruction.rs::reconstruction_iterator_skips_failed_fetches`
- `reconstruction.rs::reconstruction_iterator_emits_removal_records_for_removed_change_kind`
- `reconstruction.rs::reconstruction_records_can_be_replayed_to_rebuild_runtime_tree`

## Design decisions

- **Pull-based iterator.** A new `pub fn reconstruction_iter<I>(events:
  I) -> impl Iterator<Item = ReconstructionRecord>` lives in the
  adapter module. It consumes `&RedfishResourceEvent` references and
  emits one record per `Inserted`/`Updated`/`RefreshedNoChange` event;
  `FetchFailed` events are skipped.
- **Removal records.** A `ChangeKind::Removed` event yields a
  `ReconstructionRecord` with `payload = None` so consumers can mark
  the entity as deleted in their persisted store without losing parent
  linkage.
- **Replay helper.** A companion helper accepts an iterator of records
  and rebuilds the scheduler tree by calling `add_target` and
  `add_generator`. Phase 8 ships a thin wrapper; the policy of *which*
  generator builders to invoke for a given record is application-owned.
- **Idempotency.** Re-running the iterator over the same event stream
  produces the same record stream. Replay is idempotent: applying the
  same record twice is a no-op at the application level.

## Acceptance criteria

- The four new tests pass under
  `cargo test -p nv-redfish-scraper --features "redfish-adapter,serde"`.
- Phase 0–7 tests remain green.
- `cargo clippy -p nv-redfish-scraper --all-features --all-targets -- -D warnings`
  passes.

## Out of scope (forever, in this crate)

- Persistence: writing/reading records to/from disk is application
  concern.
- Cross-version compatibility of records: an application may version
  its records independently of the scraper crate.
