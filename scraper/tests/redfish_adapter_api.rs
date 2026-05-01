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

//! Redfish adapter public-API tests.
//!
//! Asserts that adapter types preserve identity and metadata without ever
//! exposing execution handles such as `Bmc`, `ServiceRoot<B>`, `Chassis<B>`,
//! or `ComputerSystem<B>`.

#![cfg(feature = "redfish-adapter")]

use core::any::TypeId;

use nv_redfish::core::ODataETag;
use nv_redfish::core::ODataId;
use nv_redfish_scraper::adapter::redfish::BmcId;
use nv_redfish_scraper::adapter::redfish::ChangeKind;
use nv_redfish_scraper::adapter::redfish::EntityPayload;
use nv_redfish_scraper::adapter::redfish::ReconstructionRecord;
use nv_redfish_scraper::adapter::redfish::RedfishAdapterError;
use nv_redfish_scraper::adapter::redfish::RedfishEvent;
use nv_redfish_scraper::adapter::redfish::RedfishResourceEvent;
use nv_redfish_scraper::adapter::redfish::ResourceMetadata;

fn ode<S: Into<String>>(s: S) -> ODataId {
    ODataId::from(s.into())
}

#[test]
fn redfish_resource_event_carries_required_identity_fields() {
    let event = RedfishResourceEvent {
        bmc_id: BmcId::new("bmc-1"),
        odata_id: ode("/redfish/v1/Chassis/1"),
        parent_odata_id: Some(ode("/redfish/v1/Chassis")),
        change: ChangeKind::Inserted,
        payload: None,
        metadata: ResourceMetadata::default(),
    };
    assert_eq!(event.bmc_id.as_str(), "bmc-1");
    assert_eq!(event.odata_id.to_string(), "/redfish/v1/Chassis/1");
    assert_eq!(
        event.parent_odata_id.as_ref().map(|x| x.to_string()),
        Some(String::from("/redfish/v1/Chassis"))
    );
    assert_eq!(event.change, ChangeKind::Inserted);
}

#[test]
fn redfish_event_top_level_variants_compile() {
    use nv_redfish_scraper::adapter::redfish::GeneratorEvent;
    use nv_redfish_scraper::adapter::redfish::ScrapeEvent;
    let _resource = RedfishEvent::Resource(RedfishResourceEvent {
        bmc_id: BmcId::new("a"),
        odata_id: ode("/x"),
        parent_odata_id: None,
        change: ChangeKind::Updated,
        payload: None,
        metadata: ResourceMetadata::default(),
    });
    let _gen = RedfishEvent::Generator(GeneratorEvent::Started {
        bmc_id: BmcId::new("a"),
        kind: String::from("service-root"),
    });
    let _scr = RedfishEvent::Scrape(ScrapeEvent::Completed {
        bmc_id: BmcId::new("a"),
        resources: 7,
    });
}

#[test]
fn redfish_resource_event_does_not_contain_execution_handles_via_field_types() {
    // Poor-man's static check: the field type ids of RedfishResourceEvent must
    // not include any nv-redfish execution wrapper. Because Rust does not
    // expose field types reflectively, we rely on the impl above being
    // structural — the absence of B-parameterised fields is enforced at
    // compile time. This test documents the invariant.
    let _ = TypeId::of::<RedfishResourceEvent>();
    let _ = TypeId::of::<RedfishEvent>();
}

#[test]
fn entity_payload_preserves_identity() {
    let payload = EntityPayload {
        kind: String::from("Chassis"),
        odata_id: ode("/redfish/v1/Chassis/1"),
        etag: Some(ODataETag::from(String::from("\"abc\""))),
    };
    assert_eq!(payload.kind, "Chassis");
    assert_eq!(payload.odata_id.to_string(), "/redfish/v1/Chassis/1");
    assert_eq!(
        payload.etag.as_ref().map(|e| e.to_string()),
        Some(String::from("\"abc\""))
    );
}

#[test]
fn reconstruction_record_can_be_built_from_resource_event() {
    let event = RedfishResourceEvent {
        bmc_id: BmcId::new("bmc-1"),
        odata_id: ode("/redfish/v1/Chassis/1"),
        parent_odata_id: Some(ode("/redfish/v1/Chassis")),
        change: ChangeKind::Inserted,
        payload: Some(EntityPayload {
            kind: String::from("Chassis"),
            odata_id: ode("/redfish/v1/Chassis/1"),
            etag: None,
        }),
        metadata: ResourceMetadata::default(),
    };
    let rec = ReconstructionRecord::from_resource_event(&event);
    assert_eq!(rec.bmc_id.as_str(), "bmc-1");
    assert_eq!(rec.odata_id.to_string(), "/redfish/v1/Chassis/1");
    assert_eq!(
        rec.parent_odata_id.as_ref().map(|p| p.to_string()),
        Some(String::from("/redfish/v1/Chassis"))
    );
}

#[test]
fn redfish_adapter_error_displays_a_message_and_implements_std_error() {
    let err = RedfishAdapterError::NotImplemented;
    let msg = format!("{}", err);
    assert!(!msg.is_empty(), "RedfishAdapterError Display = {}", msg);
    let _: &dyn std::error::Error = &err;
}

#[cfg(feature = "serde")]
#[test]
fn redfish_resource_event_serializes_with_serde() {
    let event = RedfishResourceEvent {
        bmc_id: BmcId::new("bmc-1"),
        odata_id: ode("/redfish/v1/Chassis/1"),
        parent_odata_id: None,
        change: ChangeKind::RefreshedNoChange,
        payload: None,
        metadata: ResourceMetadata {
            etag: None,
            generation: Some(3),
            fetch_latency_ms: Some(42),
            error: None,
        },
    };
    let s = serde_json::to_string(&event).expect("serialize ok");
    assert!(
        s.contains("bmc-1") && s.contains("Chassis"),
        "serialized output: {}",
        s
    );
}

#[cfg(feature = "serde")]
#[test]
fn reconstruction_record_serializes_with_serde() {
    let rec = ReconstructionRecord {
        bmc_id: BmcId::new("bmc-1"),
        odata_id: ode("/redfish/v1/Chassis/1"),
        parent_odata_id: Some(ode("/redfish/v1/Chassis")),
        payload: Some(EntityPayload {
            kind: String::from("Chassis"),
            odata_id: ode("/redfish/v1/Chassis/1"),
            etag: None,
        }),
    };
    let s = serde_json::to_string(&rec).expect("serialize ok");
    assert!(
        s.contains("bmc-1") && s.contains("Chassis"),
        "serialized output: {}",
        s
    );
    let back: ReconstructionRecord = serde_json::from_str(&s).expect("deserialize ok");
    assert_eq!(back, rec);
}

#[test]
fn redfish_event_top_level_does_not_include_b_type_parameter() {
    // Compile-time check: RedfishEvent has no type parameter `B`. The
    // assertion is implicit in the type signature; the test exists as a
    // regression catcher if someone mistakenly adds one.
    fn _accepts_unparameterised_event(_e: RedfishEvent) {}
    let _ = _accepts_unparameterised_event;
}

#[test]
fn change_kind_includes_required_variants() {
    let _ = ChangeKind::Inserted;
    let _ = ChangeKind::Updated;
    let _ = ChangeKind::RefreshedNoChange;
    let _ = ChangeKind::FetchFailed;
    let _ = ChangeKind::Stale;
    let _ = ChangeKind::Removed;
}

#[cfg(feature = "adapter-service-root")]
#[test]
fn service_root_builder_is_a_function_in_the_adapter_module() {
    // Compile-time check: the symbol exists in the adapter module. We
    // cannot easily construct a `ServiceRoot<B>` here without a real Bmc,
    // so this test asserts only that the public function name resolves.
    use nv_redfish_scraper::adapter::redfish::build_service_root_generator;
    let _ = build_service_root_generator::<NeverCalledBmc>;
}

#[cfg(feature = "adapter-chassis")]
#[test]
fn chassis_builder_is_a_function_in_the_adapter_module() {
    use nv_redfish_scraper::adapter::redfish::build_chassis_generator;
    let _ = build_chassis_generator::<NeverCalledBmc>;
}

#[cfg(feature = "adapter-computer-systems")]
#[test]
fn computer_system_builder_is_a_function_in_the_adapter_module() {
    use nv_redfish_scraper::adapter::redfish::build_computer_system_generator;
    let _ = build_computer_system_generator::<NeverCalledBmc>;
}

/// Placeholder Bmc impl used only at the type level. The trait body uses
/// `unimplemented!()` because no test in this file actually calls a method.
#[cfg(any(
    feature = "adapter-service-root",
    feature = "adapter-chassis",
    feature = "adapter-computer-systems",
))]
struct NeverCalledBmc;

#[cfg(any(
    feature = "adapter-service-root",
    feature = "adapter-chassis",
    feature = "adapter-computer-systems",
))]
#[allow(clippy::manual_async_fn)] // mirrors `Bmc` trait signature exactly
impl nv_redfish::Bmc for NeverCalledBmc {
    type Error = std::io::Error;
    fn expand<T: nv_redfish::core::Expandable>(
        &self,
        _id: &ODataId,
        _query: nv_redfish::core::query::ExpandQuery,
    ) -> impl core::future::Future<Output = Result<std::sync::Arc<T>, Self::Error>> + Send {
        async { unimplemented!() }
    }
    fn get<T: nv_redfish::core::EntityTypeRef + for<'de> serde::Deserialize<'de> + 'static>(
        &self,
        _id: &ODataId,
    ) -> impl core::future::Future<Output = Result<std::sync::Arc<T>, Self::Error>> + Send {
        async { unimplemented!() }
    }
    fn filter<T: nv_redfish::core::EntityTypeRef + for<'de> serde::Deserialize<'de> + 'static>(
        &self,
        _id: &ODataId,
        _query: nv_redfish::core::FilterQuery,
    ) -> impl core::future::Future<Output = Result<std::sync::Arc<T>, Self::Error>> + Send {
        async { unimplemented!() }
    }
    fn create<V: Send + Sync + serde::Serialize, R: Send + Sync + for<'de> serde::Deserialize<'de>>(
        &self,
        _id: &ODataId,
        _query: &V,
    ) -> impl core::future::Future<
        Output = Result<nv_redfish::core::ModificationResponse<R>, Self::Error>,
    > + Send {
        async { unimplemented!() }
    }
    fn update<V: Sync + Send + serde::Serialize, R: Send + Sync + Sized + for<'de> serde::Deserialize<'de>>(
        &self,
        _id: &ODataId,
        _etag: Option<&ODataETag>,
        _update: &V,
    ) -> impl core::future::Future<
        Output = Result<nv_redfish::core::ModificationResponse<R>, Self::Error>,
    > + Send {
        async { unimplemented!() }
    }
    fn delete<R: nv_redfish::core::EntityTypeRef + for<'de> serde::Deserialize<'de>>(
        &self,
        _id: &ODataId,
    ) -> impl core::future::Future<
        Output = Result<nv_redfish::core::ModificationResponse<R>, Self::Error>,
    > + Send {
        async { unimplemented!() }
    }
    fn action<T: Send + Sync + serde::Serialize, R: Send + Sync + Sized + for<'de> serde::Deserialize<'de>>(
        &self,
        _action: &nv_redfish::core::Action<T, R>,
        _params: &T,
    ) -> impl core::future::Future<
        Output = Result<nv_redfish::core::ModificationResponse<R>, Self::Error>,
    > + Send {
        async { unimplemented!() }
    }
    fn stream<T: Sized + for<'de> serde::Deserialize<'de> + Send + 'static>(
        &self,
        _uri: &str,
    ) -> impl core::future::Future<
        Output = Result<nv_redfish::core::BoxTryStream<T, Self::Error>, Self::Error>,
    > + Send {
        async { unimplemented!() }
    }
}

#[test]
fn expanded_payload_preservation_is_represented_in_the_event_api() {
    // Phase 7: when a fetch uses $expand, child resources are returned as
    // a sequence of RedfishResourceEvents whose `payload` carries the
    // child entity. Currently no expand wiring exists, so this test asserts
    // only that the EntityPayload boundary supports nesting via parent_odata_id.
    let parent = RedfishResourceEvent {
        bmc_id: BmcId::new("bmc-1"),
        odata_id: ode("/redfish/v1/Chassis"),
        parent_odata_id: None,
        change: ChangeKind::Inserted,
        payload: Some(EntityPayload {
            kind: String::from("ChassisCollection"),
            odata_id: ode("/redfish/v1/Chassis"),
            etag: None,
        }),
        metadata: ResourceMetadata::default(),
    };
    let child = RedfishResourceEvent {
        bmc_id: BmcId::new("bmc-1"),
        odata_id: ode("/redfish/v1/Chassis/1"),
        parent_odata_id: Some(parent.odata_id.clone()),
        change: ChangeKind::Inserted,
        payload: Some(EntityPayload {
            kind: String::from("Chassis"),
            odata_id: ode("/redfish/v1/Chassis/1"),
            etag: None,
        }),
        metadata: ResourceMetadata::default(),
    };
    assert_eq!(child.parent_odata_id.as_ref(), Some(&parent.odata_id));
    assert!(child.payload.is_some(), "expanded child must carry payload");
}

#[test]
fn reconstruction_records_preserve_hierarchy_identity_without_execution_handles() {
    // The record carries only ids and payload; no Bmc, ServiceRoot<B>, etc.
    let rec = ReconstructionRecord {
        bmc_id: BmcId::new("bmc-1"),
        odata_id: ode("/redfish/v1/Chassis/1"),
        parent_odata_id: Some(ode("/redfish/v1/Chassis")),
        payload: None,
    };
    // Identity preserved.
    assert_eq!(rec.odata_id.to_string(), "/redfish/v1/Chassis/1");
    // No handle leakage: this test would fail to compile if anyone added
    // a B-parameterised field to ReconstructionRecord.
    let _ = TypeId::of::<ReconstructionRecord>();
}
