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

#![cfg(feature = "redfish-adapter")]

use nv_redfish::core::ODataETag;
use nv_redfish::core::ODataId;
use nv_redfish_scraper::adapter::redfish::BmcId;
use nv_redfish_scraper::adapter::redfish::ChangeKind;
use nv_redfish_scraper::adapter::redfish::EntityPayload;
use nv_redfish_scraper::adapter::redfish::ReconstructionRecord;
use nv_redfish_scraper::adapter::redfish::RedfishEvent;
use nv_redfish_scraper::adapter::redfish::RedfishResourceEvent;
use nv_redfish_scraper::adapter::redfish::ResourceMetadata;
use nv_redfish_scraper::adapter::redfish::TypedRedfishBuilder;
use std::time::Duration;
use std::time::SystemTime;

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
struct FakePayload {
    kind: &'static str,
    odata_id: ODataId,
    etag: ODataETag,
}

impl FakePayload {
    fn new(kind: &'static str, odata_id: &'static str, etag: &'static str) -> Self {
        Self {
            kind,
            odata_id: ODataId::from(odata_id.to_owned()),
            etag: ODataETag::from(etag.to_owned()),
        }
    }
}

impl EntityPayload for FakePayload {
    fn entity_kind(&self) -> &str {
        self.kind
    }

    fn odata_id(&self) -> Option<&ODataId> {
        Some(&self.odata_id)
    }

    fn etag(&self) -> Option<&ODataETag> {
        Some(&self.etag)
    }
}

fn metadata() -> ResourceMetadata {
    ResourceMetadata::new(SystemTime::UNIX_EPOCH, Duration::from_millis(5), 1, None)
}

#[test]
fn redfish_resource_event_contains_required_identity_fields() {
    let event = RedfishResourceEvent::new(
        BmcId::new("bmc-a"),
        ODataId::from("/redfish/v1/Chassis/1".to_owned()),
        Some(ODataId::from("/redfish/v1/Chassis".to_owned())),
        ChangeKind::Inserted,
        Some(FakePayload::new(
            "Chassis",
            "/redfish/v1/Chassis/1",
            "etag-1",
        )),
        metadata(),
    );

    assert_eq!(event.bmc_id().as_str(), "bmc-a");
    assert_eq!(event.odata_id().to_string(), "/redfish/v1/Chassis/1");
    assert_eq!(
        event
            .parent_odata_id()
            .expect("parent id should be present")
            .to_string(),
        "/redfish/v1/Chassis"
    );
    assert_eq!(event.change(), &ChangeKind::Inserted);
    assert_eq!(
        event
            .payload()
            .expect("payload should be present")
            .entity_kind(),
        "Chassis"
    );
}

#[test]
fn redfish_event_payload_does_not_require_execution_handles() {
    let event: RedfishEvent<FakePayload> = RedfishEvent::Resource(RedfishResourceEvent::new(
        BmcId::new("bmc-a"),
        ODataId::from("/redfish/v1/Systems/1".to_owned()),
        None,
        ChangeKind::Refreshed,
        Some(FakePayload::new(
            "ComputerSystem",
            "/redfish/v1/Systems/1",
            "etag-2",
        )),
        metadata(),
    ));

    match event {
        RedfishEvent::Resource(resource) => {
            assert_eq!(resource.bmc_id().as_str(), "bmc-a");
            assert!(resource.payload().is_some());
        }
        _ => {
            panic!("expected resource event");
        }
    }
}

#[test]
fn expanded_payload_preservation_is_represented_in_event_api() {
    let event = RedfishResourceEvent::new(
        BmcId::new("bmc-a"),
        ODataId::from("/redfish/v1/Chassis/1".to_owned()),
        None,
        ChangeKind::Updated,
        Some(FakePayload::new(
            "ChassisWithExpandedSensors",
            "/redfish/v1/Chassis/1",
            "etag-expanded",
        )),
        metadata(),
    );

    assert_eq!(
        event
            .payload()
            .expect("expanded payload should be preserved")
            .entity_kind(),
        "ChassisWithExpandedSensors"
    );
}

#[test]
fn child_events_can_carry_parent_odata_id() {
    let child = RedfishResourceEvent::new(
        BmcId::new("bmc-a"),
        ODataId::from("/redfish/v1/Chassis/1/Sensors/GPU0".to_owned()),
        Some(ODataId::from("/redfish/v1/Chassis/1".to_owned())),
        ChangeKind::Inserted,
        Some(FakePayload::new(
            "Sensor",
            "/redfish/v1/Chassis/1/Sensors/GPU0",
            "etag-sensor",
        )),
        metadata(),
    );

    assert_eq!(
        child
            .parent_odata_id()
            .expect("child should carry parent id")
            .to_string(),
        "/redfish/v1/Chassis/1"
    );
}

#[test]
fn typed_builder_api_is_parameterized_by_bmc_and_object_type() {
    fn assert_builder_shape<B, O>()
    where
        B: nv_redfish::Bmc,
    {
        let _ = std::any::type_name::<TypedRedfishBuilder<B, O>>();
    }

    assert_builder_shape::<nv_redfish_bmc_mock::Bmc<std::io::Error>, ()>();
}

#[test]
fn reconstruction_record_preserves_hierarchy_identity_without_execution_handles() {
    let record = ReconstructionRecord::new(
        BmcId::new("bmc-a"),
        ODataId::from("/redfish/v1/Chassis/1/Sensors/GPU0".to_owned()),
        Some(ODataId::from("/redfish/v1/Chassis/1".to_owned())),
        Some(FakePayload::new(
            "Sensor",
            "/redfish/v1/Chassis/1/Sensors/GPU0",
            "etag-sensor",
        )),
    );

    assert_eq!(record.bmc_id().as_str(), "bmc-a");
    assert_eq!(
        record
            .parent_odata_id()
            .expect("parent id should be available")
            .to_string(),
        "/redfish/v1/Chassis/1"
    );
    assert_eq!(
        record
            .payload()
            .expect("payload should be available")
            .entity_kind(),
        "Sensor"
    );
}

#[test]
fn reconstruction_record_can_be_derived_from_resource_event() {
    let tests = trybuild::TestCases::new();

    tests.pass("tests/trybuild/reconstruction_record_from_event.rs");
}

#[test]
fn typed_service_root_builder_produces_runtime_generator() {
    let tests = trybuild::TestCases::new();

    tests.pass("tests/trybuild/typed_service_root_builder_produces_generator.rs");
}

#[test]
#[cfg(feature = "serde")]
fn redfish_events_are_serializable_when_serde_feature_is_enabled() {
    fn assert_serialize<T: serde::Serialize>() {}

    assert_serialize::<BmcId>();
    assert_serialize::<ResourceMetadata>();
    assert_serialize::<RedfishResourceEvent<FakePayload>>();
    assert_serialize::<RedfishEvent<FakePayload>>();
    assert_serialize::<ReconstructionRecord<FakePayload>>();
}

#[test]
#[cfg(feature = "serde")]
fn serialized_redfish_resource_event_contains_only_read_side_fields() {
    let event = RedfishEvent::Resource(RedfishResourceEvent::new(
        BmcId::new("bmc-a"),
        ODataId::from("/redfish/v1/Chassis/1".to_owned()),
        Some(ODataId::from("/redfish/v1/Chassis".to_owned())),
        ChangeKind::FetchFailed,
        Some(FakePayload::new(
            "Chassis",
            "/redfish/v1/Chassis/1",
            "etag-1",
        )),
        ResourceMetadata::new(
            SystemTime::UNIX_EPOCH,
            Duration::from_millis(7),
            42,
            Some("timeout".to_owned()),
        ),
    ));

    let json = serde_json::to_value(&event).expect("event should serialize");

    assert_eq!(json["Resource"]["bmc_id"], "bmc-a");
    assert_eq!(json["Resource"]["odata_id"], "/redfish/v1/Chassis/1");
    assert_eq!(json["Resource"]["parent_odata_id"], "/redfish/v1/Chassis");
    assert_eq!(json["Resource"]["change"], "FetchFailed");
    assert_eq!(json["Resource"]["metadata"]["generation"], 42);
    assert_eq!(json["Resource"]["metadata"]["error"], "timeout");
    assert_eq!(json["Resource"]["payload"]["kind"], "Chassis");
    assert!(
        json.get("bmc").is_none(),
        "serialized events must not expose execution handles"
    );
}

#[test]
#[cfg(feature = "serde")]
fn generated_entity_payload_contract_is_available() {
    let tests = trybuild::TestCases::new();

    tests.pass("tests/trybuild/generated_entity_payload_contract.rs");
}
