// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Phase-8 tests: reconstruction-record derivation and replay.
//!
//! These tests exercise the public surface added in Phase 8:
//!
//! - `reconstruction_iter` derives one `ReconstructionRecord` per
//!   `RedfishResourceEvent` whose `change` is in
//!   `Inserted`, `Updated`, `RefreshedNoChange`, `Stale` or `Removed`,
//!   skips `FetchFailed`, and is idempotent.
//! - `replay_records` dispatches an application-supplied
//!   `ReplayDecision` per record against a live `Runtime`, returning
//!   counters in `ReplayStats`.

#![cfg(feature = "redfish-adapter")]

mod support;

use nv_redfish_scraper::adapter::redfish::BmcId;
use nv_redfish_scraper::adapter::redfish::ChangeKind;
use nv_redfish_scraper::adapter::redfish::ReconstructionRecord;
use nv_redfish_scraper::adapter::redfish::RedfishAdapterError;
use nv_redfish_scraper::adapter::redfish::RedfishEvent;
use nv_redfish_scraper::reconstruction_iter;
use nv_redfish_scraper::replay_records;
use nv_redfish_scraper::GeneratorConfig;
use nv_redfish_scraper::ReplayDecision;
use nv_redfish_scraper::Runtime;
use nv_redfish_scraper::RuntimeConfig;
use nv_redfish_scraper::TargetLimits;

use support::redfish_events::ode;
use support::redfish_events::payload_at;
use support::redfish_events::NoopRedfishGenerator;
use support::redfish_events::ResourceEvent;

#[test]
fn reconstruction_iterator_produces_records_in_event_order() {
    let events = [
        ResourceEvent::at("bmc-1", "/redfish/v1/Chassis/A")
            .parent("/redfish/v1/Chassis")
            .change(ChangeKind::Inserted)
            .payload_kind("Chassis")
            .build(),
        ResourceEvent::at("bmc-1", "/redfish/v1/Chassis/A")
            .parent("/redfish/v1/Chassis")
            .change(ChangeKind::RefreshedNoChange)
            .payload_kind("Chassis")
            .build(),
        ResourceEvent::at("bmc-1", "/redfish/v1/Chassis/B")
            .parent("/redfish/v1/Chassis")
            .change(ChangeKind::Updated)
            .payload_kind("Chassis")
            .build(),
        ResourceEvent::at("bmc-1", "/redfish/v1/Chassis/A/Thermal")
            .parent("/redfish/v1/Chassis/A")
            .change(ChangeKind::Stale)
            .payload_kind("Thermal")
            .build(),
    ];

    let expected: Vec<ReconstructionRecord> = events
        .iter()
        .map(ReconstructionRecord::from_resource_event)
        .collect();

    let first: Vec<ReconstructionRecord> = reconstruction_iter(events.iter()).collect();
    assert_eq!(first, expected);

    let second: Vec<ReconstructionRecord> = reconstruction_iter(events.iter()).collect();
    assert_eq!(
        first, second,
        "reconstruction_iter must be idempotent over the same event slice"
    );
}

#[test]
fn reconstruction_iterator_skips_failed_fetches() {
    let events = [
        ResourceEvent::at("bmc-1", "/redfish/v1/Chassis/A")
            .payload_kind("Chassis")
            .build(),
        ResourceEvent::at("bmc-1", "/redfish/v1/Chassis/B")
            .fetch_failed()
            .build(),
        ResourceEvent::at("bmc-1", "/redfish/v1/Chassis/C")
            .payload_kind("Chassis")
            .build(),
    ];

    let records: Vec<ReconstructionRecord> = reconstruction_iter(events.iter()).collect();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].odata_id.to_string(), "/redfish/v1/Chassis/A");
    assert_eq!(records[1].odata_id.to_string(), "/redfish/v1/Chassis/C");

    assert!(
        records.iter().all(|rec| rec.payload.is_some()),
        "successful fetch records must preserve their payload"
    );
}

#[test]
fn reconstruction_iterator_emits_removal_records_for_removed_change_kind() {
    let events = [ResourceEvent::at("bmc-1", "/redfish/v1/Chassis/A")
        .parent("/redfish/v1/Chassis")
        .change(ChangeKind::Removed)
        .payload_kind("Chassis")
        .build()];

    let records: Vec<ReconstructionRecord> = reconstruction_iter(events.iter()).collect();
    assert_eq!(records.len(), 1);
    let rec = &records[0];
    assert_eq!(rec.bmc_id.as_str(), "bmc-1");
    assert_eq!(rec.odata_id.to_string(), "/redfish/v1/Chassis/A");
    assert_eq!(
        rec.parent_odata_id.as_ref().map(ToString::to_string),
        Some(String::from("/redfish/v1/Chassis"))
    );
    assert!(
        rec.payload.is_none(),
        "removal records must clear payload while preserving identity"
    );
}

#[test]
fn reconstruction_records_can_be_replayed_to_rebuild_runtime_tree() {
    let parent_record = ReconstructionRecord {
        bmc_id: BmcId::new("bmc-1"),
        odata_id: ode("/redfish/v1/Chassis/A"),
        parent_odata_id: None,
        payload: Some(payload_at("Chassis", "/redfish/v1/Chassis/A")),
    };
    // Two sibling children of the parent chassis. Generated via an
    // iterator so the test reads as data + transformation rather than
    // copy-pasted struct literals.
    let child_records: Vec<ReconstructionRecord> =
        IntoIterator::into_iter(["Thermal", "Power"])
            .map(|kind| {
                let path = format!("/redfish/v1/Chassis/A/{kind}");
                ReconstructionRecord {
                    bmc_id: BmcId::new("bmc-1"),
                    odata_id: ode(&path),
                    parent_odata_id: Some(ode("/redfish/v1/Chassis/A")),
                    payload: Some(payload_at(kind, &path)),
                }
            })
            .collect();

    let runtime: Runtime<RedfishEvent, RedfishAdapterError> =
        Runtime::new(RuntimeConfig::default());

    // Pass 1: feed the root record so the helper exercises the AddTarget
    // dispatch path. The newly allocated TargetId comes back through
    // `ReplayStats::added_targets` rather than via runtime stats scanning.
    let parent_stats = replay_records(&runtime, [parent_record], |_rec| {
        ReplayDecision::AddTarget {
            limits: TargetLimits::default(),
        }
    });
    assert_eq!(parent_stats.targets_added(), 1);
    assert_eq!(parent_stats.generators_added(), 0);
    assert_eq!(parent_stats.skipped, 0);
    assert_eq!(parent_stats.failed, 0);
    let target = *parent_stats
        .added_targets
        .first()
        .expect("added_targets must contain the new target id");

    // Pass 2: feed the children, attaching one no-op generator per record to
    // the captured target. Exercises the AddGenerator dispatch path and the
    // generator-id surfacing.
    let child_stats = replay_records(&runtime, child_records, |_rec| {
        ReplayDecision::AddGenerator {
            target,
            generator: Box::new(NoopRedfishGenerator),
            config: GeneratorConfig::default(),
        }
    });
    assert_eq!(child_stats.targets_added(), 0);
    assert_eq!(child_stats.generators_added(), 2);
    assert_eq!(child_stats.skipped, 0);
    assert_eq!(child_stats.failed, 0);

    let runtime_stats = runtime.stats();
    assert_eq!(runtime_stats.targets, 1);
    assert_eq!(runtime_stats.generators, 2);

    // The surfaced ids must match what the runtime reports.
    let runtime_target_ids: Vec<_> = runtime_stats
        .per_target
        .iter()
        .filter_map(|s| s.target)
        .collect();
    assert_eq!(parent_stats.added_targets, runtime_target_ids);
    let runtime_generator_ids: Vec<_> = runtime_stats
        .per_target
        .iter()
        .flat_map(|t| t.per_generator.iter().map(|(id, _)| *id))
        .collect();
    assert_eq!(child_stats.added_generators, runtime_generator_ids);

    // Surfaced ids are usable: pause one of them through the runtime control
    // surface to prove they are not synthetic placeholders.
    let first_generator = *child_stats
        .added_generators
        .first()
        .expect("added_generators must contain the new generator id");
    assert!(
        runtime.pause_generator(first_generator),
        "newly-added generator id must be controllable through the runtime",
    );
}
