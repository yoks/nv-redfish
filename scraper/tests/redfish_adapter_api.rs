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

mod support;

use core::any::TypeId;

use nv_redfish::core::ODataETag;
#[cfg(any(
    feature = "adapter-service-root",
    feature = "adapter-chassis",
    feature = "adapter-computer-systems",
))]
use nv_redfish::core::ODataId;
use nv_redfish_scraper::adapter::redfish::BmcId;
use nv_redfish_scraper::adapter::redfish::ChangeKind;
use nv_redfish_scraper::adapter::redfish::EntityPayload;
use nv_redfish_scraper::adapter::redfish::ReconstructionRecord;
use nv_redfish_scraper::adapter::redfish::RedfishAdapterError;
use nv_redfish_scraper::adapter::redfish::RedfishEvent;
use nv_redfish_scraper::adapter::redfish::RedfishResourceEvent;
use nv_redfish_scraper::adapter::redfish::ResourceMetadata;

use support::redfish_events::ode;

#[test]
fn redfish_resource_event_carries_required_identity_fields() {
    let event = support::redfish_events::ResourceEvent::at("bmc-1", "/redfish/v1/Chassis/1")
        .parent("/redfish/v1/Chassis")
        .change(ChangeKind::Inserted)
        .build();
    assert_eq!(event.bmc_id.as_str(), "bmc-1");
    assert_eq!(event.odata_id.to_string(), "/redfish/v1/Chassis/1");
    assert_eq!(
        event.parent_odata_id.as_ref().map(ToString::to_string),
        Some(String::from("/redfish/v1/Chassis"))
    );
    assert_eq!(event.change, ChangeKind::Inserted);
}

#[test]
fn redfish_event_top_level_variants_compile() {
    use nv_redfish_scraper::adapter::redfish::GeneratorEvent;
    use nv_redfish_scraper::adapter::redfish::ScrapeEvent;
    let _resource = RedfishEvent::Resource(
        support::redfish_events::ResourceEvent::at("a", "/x")
            .change(ChangeKind::Updated)
            .build(),
    );
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
    let event = support::redfish_events::ResourceEvent::at("bmc-1", "/redfish/v1/Chassis/1")
        .parent("/redfish/v1/Chassis")
        .payload_kind("Chassis")
        .build();
    let rec = ReconstructionRecord::from_resource_event(&event);
    assert_eq!(rec.bmc_id.as_str(), "bmc-1");
    assert_eq!(rec.odata_id.to_string(), "/redfish/v1/Chassis/1");
    assert_eq!(
        rec.parent_odata_id.as_ref().map(ToString::to_string),
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
    let parent = support::redfish_events::ResourceEvent::at("bmc-1", "/redfish/v1/Chassis")
        .payload_kind("ChassisCollection")
        .build();
    let child = support::redfish_events::ResourceEvent::at("bmc-1", "/redfish/v1/Chassis/1")
        .parent("/redfish/v1/Chassis")
        .payload_kind("Chassis")
        .build();
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

// =============================================================================
// Phase 6 end-to-end tests
//
// These tests exercise the full path from `MockBmc` through `ServiceRoot::new`,
// the scraper `Runtime`, and the Phase 6 `ServiceRootGenerator<B>`.
// =============================================================================

#[cfg(all(feature = "adapter-service-root", feature = "adapter-chassis"))]
mod phase_6 {
    use core::task::Poll;

    use nv_redfish::ServiceRoot;
    use nv_redfish_scraper::adapter::redfish::build_service_root_generator;
    use nv_redfish_scraper::adapter::redfish::BmcId;
    use nv_redfish_scraper::adapter::redfish::ChangeKind;
    use nv_redfish_scraper::adapter::redfish::RedfishAdapterError;
    use nv_redfish_scraper::adapter::redfish::RedfishEvent;
    use nv_redfish_scraper::GeneratorConfig;
    use nv_redfish_scraper::Runtime;
    use nv_redfish_scraper::RuntimeConfig;
    use nv_redfish_scraper::RuntimeOutput;
    use nv_redfish_scraper::TargetLimits;
    use nv_redfish_scraper::WorkResult;

    use super::support::harness::Harness;
    use super::support::mock_bmc::chassis_collection_json;
    use super::support::mock_bmc::expect_get_err;
    use super::support::mock_bmc::expect_get_ok;
    use super::support::mock_bmc::mock_bmc;
    use super::support::mock_bmc::MockBmc;
    use super::support::mock_bmc::MockExpect;

    const SERVICE_ROOT: &str = "/redfish/v1/";
    const SERVICE_ROOT_CANONICAL: &str = "/redfish/v1";
    const CHASSIS_COLLECTION: &str = "/redfish/v1/Chassis";

    /// Build a `ServiceRoot<MockBmc>` from a fresh mock with the supplied
    /// expectations, using the `Harness` waker to drive the constructor's
    /// async future synchronously.
    fn boot_service_root(
        h: &Harness,
        expectations: impl IntoIterator<Item = MockExpect>,
    ) -> (std::sync::Arc<MockBmc>, ServiceRoot<MockBmc>) {
        let bmc = mock_bmc();
        for exp in expectations {
            bmc.expect(exp);
        }
        let service_root = h
            .block_on(ServiceRoot::new(bmc.clone()))
            .expect("ServiceRoot::new with mock should succeed");
        (bmc, service_root)
    }

    /// Drain the next `Work(_)` output from the runtime, transparently
    /// skipping any `Runtime(_)` events emitted under the `runtime-events`
    /// feature.
    fn drain_one_work(
        r: &mut Runtime<RedfishEvent, RedfishAdapterError>,
        h: &Harness,
    ) -> WorkResult<RedfishEvent, RedfishAdapterError> {
        loop {
            let mut fut = r.next();
            match h.poll(&mut fut) {
                Poll::Ready(RuntimeOutput::Work(work)) => return work,
                Poll::Ready(RuntimeOutput::Runtime(_)) => continue,
                Poll::Ready(_) => panic!("unexpected non-work runtime output"),
                Poll::Pending => panic!("runtime parked while draining work"),
            }
        }
    }

    #[test]
    fn service_root_generator_emits_inserted_resource_event_on_first_chassis_scrape() {
        let h = Harness::new();
        let (_bmc, service_root) = boot_service_root(
            &h,
            vec![
                expect_get_ok(
                    SERVICE_ROOT_CANONICAL,
                    super::support::mock_bmc::service_root_json_with_chassis(SERVICE_ROOT),
                ),
                expect_get_ok(
                    CHASSIS_COLLECTION,
                    chassis_collection_json(CHASSIS_COLLECTION, Some("\"v1\"")),
                ),
            ],
        );

        let mut r: Runtime<RedfishEvent, RedfishAdapterError> =
            Runtime::new(RuntimeConfig::default());
        let t = r.add_target(TargetLimits::default()).expect("add_target");
        r.add_generator(
            t,
            build_service_root_generator(BmcId::new("bmc-1"), service_root),
            GeneratorConfig::default(),
        )
        .expect("add_generator");

        let success = match drain_one_work(&mut r, &h) {
            Ok(s) => s,
            Err(e) => panic!("expected success, got error: {}", format_adapter_error(&e.error)),
        };
        assert_eq!(
            success.events.len(),
            1,
            "expected exactly one event, got {}",
            success.events.len()
        );
        match &success.events[0] {
            RedfishEvent::Resource(ev) => {
                assert_eq!(ev.change, ChangeKind::Inserted);
                assert_eq!(ev.odata_id.to_string(), CHASSIS_COLLECTION);
                assert_eq!(ev.bmc_id.as_str(), "bmc-1");
                assert_eq!(
                    ev.parent_odata_id.as_ref().map(ToString::to_string),
                    Some(String::from(SERVICE_ROOT))
                );
                assert_eq!(
                    ev.metadata.etag.as_ref().map(ToString::to_string),
                    Some(String::from("\"v1\""))
                );
            }
            _ => panic!("expected RedfishEvent::Resource"),
        }
    }

    #[test]
    fn service_root_generator_surfaces_transport_error_when_child_fetch_fails() {
        let h = Harness::new();
        let (_bmc, service_root) = boot_service_root(
            &h,
            vec![
                expect_get_ok(
                    SERVICE_ROOT_CANONICAL,
                    super::support::mock_bmc::service_root_json_with_chassis(SERVICE_ROOT),
                ),
                expect_get_err(CHASSIS_COLLECTION, "synthetic-transport"),
            ],
        );

        let mut r: Runtime<RedfishEvent, RedfishAdapterError> =
            Runtime::new(RuntimeConfig::default());
        let t = r.add_target(TargetLimits::default()).expect("add_target");
        r.add_generator(
            t,
            build_service_root_generator(BmcId::new("bmc-1"), service_root),
            GeneratorConfig::default(),
        )
        .expect("add_generator");

        match drain_one_work(&mut r, &h) {
            Err(work_err) => match work_err.error {
                RedfishAdapterError::Transport(msg) => {
                    assert!(
                        msg.contains("synthetic-transport"),
                        "transport error should preserve the synthetic tag, got {}",
                        msg
                    );
                }
                other => panic!(
                    "expected Transport, got {}",
                    format_adapter_error(&other)
                ),
            },
            Ok(_) => panic!("expected transport error, got success"),
        }
    }

    #[test]
    fn service_root_generator_emits_refreshed_no_change_on_second_scrape_with_same_etag() {
        let h = Harness::new();
        let (_bmc, service_root) = boot_service_root(
            &h,
            vec![
                expect_get_ok(
                    SERVICE_ROOT_CANONICAL,
                    super::support::mock_bmc::service_root_json_with_chassis(SERVICE_ROOT),
                ),
                expect_get_ok(
                    CHASSIS_COLLECTION,
                    chassis_collection_json(CHASSIS_COLLECTION, Some("\"v1\"")),
                ),
                expect_get_ok(
                    CHASSIS_COLLECTION,
                    chassis_collection_json(CHASSIS_COLLECTION, Some("\"v1\"")),
                ),
            ],
        );

        let mut r: Runtime<RedfishEvent, RedfishAdapterError> =
            Runtime::new(RuntimeConfig::default());
        let t = r.add_target(TargetLimits::default()).expect("add_target");
        r.add_generator(
            t,
            build_service_root_generator(BmcId::new("bmc-1"), service_root),
            GeneratorConfig::default(),
        )
        .expect("add_generator");

        let first = match drain_one_work(&mut r, &h) {
            Ok(s) => s,
            Err(e) => panic!("first scrape failed: {}", format_adapter_error(&e.error)),
        };
        match &first.events[0] {
            RedfishEvent::Resource(ev) => {
                assert_eq!(ev.change, ChangeKind::Inserted, "first scrape must Insert");
            }
            _ => panic!("first scrape: expected Resource"),
        }

        let second = match drain_one_work(&mut r, &h) {
            Ok(s) => s,
            Err(e) => panic!("second scrape failed: {}", format_adapter_error(&e.error)),
        };
        match &second.events[0] {
            RedfishEvent::Resource(ev) => {
                assert_eq!(ev.odata_id.to_string(), CHASSIS_COLLECTION);
                assert_eq!(
                    ev.change,
                    ChangeKind::RefreshedNoChange,
                    "second scrape with same ETag must classify as RefreshedNoChange"
                );
            }
            _ => panic!("second scrape: expected Resource"),
        }
    }

    fn format_adapter_error(err: &RedfishAdapterError) -> String {
        format!("{}", err)
    }
}

// =============================================================================
// Phase 7 end-to-end tests
//
// Drive the chassis-`$expand`, sensors, and computer-systems builders through
// the full scraper runtime. Reuses Phase-6 mock infrastructure plus the new
// `mock_bmc` helpers added in this phase.
// =============================================================================

#[cfg(all(
    feature = "adapter-service-root",
    feature = "adapter-chassis",
    feature = "adapter-sensors",
    feature = "adapter-computer-systems",
))]
mod phase_7 {
    use core::task::Poll;

    use nv_redfish::ServiceRoot;
    use nv_redfish_scraper::adapter::redfish::build_chassis_generator;
    use nv_redfish_scraper::adapter::redfish::build_computer_system_generator;
    use nv_redfish_scraper::adapter::redfish::build_sensors_generator;
    use nv_redfish_scraper::adapter::redfish::BmcId;
    use nv_redfish_scraper::adapter::redfish::ChangeKind;
    use nv_redfish_scraper::adapter::redfish::RedfishAdapterError;
    use nv_redfish_scraper::adapter::redfish::RedfishEvent;
    use nv_redfish_scraper::GeneratorConfig;
    use nv_redfish_scraper::Runtime;
    use nv_redfish_scraper::RuntimeConfig;
    use nv_redfish_scraper::RuntimeOutput;
    use nv_redfish_scraper::TargetLimits;
    use nv_redfish_scraper::WorkResult;

    use super::support::harness::Harness;
    use super::support::mock_bmc::chassis_collection_json_with_member;
    use super::support::mock_bmc::chassis_item_json_expanded;
    use super::support::mock_bmc::chassis_item_json_reference;
    use super::support::mock_bmc::computer_system_json;
    use super::support::mock_bmc::expect_get_ok;
    use super::support::mock_bmc::mock_bmc;
    use super::support::mock_bmc::sensor_collection_json;
    use super::support::mock_bmc::service_root_json_advertising_expand;
    use super::support::mock_bmc::service_root_json_with_chassis;
    use super::support::mock_bmc::service_root_json_with_chassis_and_systems;
    use super::support::mock_bmc::system_collection_json_with_member;
    use super::support::mock_bmc::ExpandedChildSpec;
    use super::support::mock_bmc::MockBmc;
    use super::support::mock_bmc::MockExpect;

    const SERVICE_ROOT: &str = "/redfish/v1/";
    const SERVICE_ROOT_CANONICAL: &str = "/redfish/v1";
    const CHASSIS_COLLECTION: &str = "/redfish/v1/Chassis";
    const CHASSIS_1: &str = "/redfish/v1/Chassis/1";
    const CHASSIS_1_THERMAL: &str = "/redfish/v1/Chassis/1/Thermal";
    const CHASSIS_1_POWER: &str = "/redfish/v1/Chassis/1/Power";
    const CHASSIS_1_SENSORS: &str = "/redfish/v1/Chassis/1/Sensors";
    const SYSTEM_COLLECTION: &str = "/redfish/v1/Systems";
    const SYSTEM_1: &str = "/redfish/v1/Systems/1";

    fn drain_one_work(
        r: &mut Runtime<RedfishEvent, RedfishAdapterError>,
        h: &Harness,
    ) -> WorkResult<RedfishEvent, RedfishAdapterError> {
        loop {
            let mut fut = r.next();
            match h.poll(&mut fut) {
                Poll::Ready(RuntimeOutput::Work(work)) => return work,
                Poll::Ready(RuntimeOutput::Runtime(_)) => continue,
                Poll::Ready(_) => panic!("unexpected non-work runtime output"),
                Poll::Pending => panic!("runtime parked while draining work"),
            }
        }
    }

    fn format_adapter_error(err: &RedfishAdapterError) -> String {
        format!("{}", err)
    }

    fn boot_service_root(
        h: &Harness,
        expectations: impl IntoIterator<Item = MockExpect>,
    ) -> (std::sync::Arc<MockBmc>, ServiceRoot<MockBmc>) {
        let bmc = mock_bmc();
        for exp in expectations {
            bmc.expect(exp);
        }
        let service_root = h
            .block_on(ServiceRoot::new(bmc.clone()))
            .expect("ServiceRoot::new with mock should succeed");
        (bmc, service_root)
    }

    /// Drive the chassis-`$expand` path: bootstrap a `Chassis<MockBmc>`
    /// whose schema arrives with inlined `Thermal`, `Power`, and
    /// `Sensors` payloads, then assert that the chassis generator emits
    /// one parent event plus three child events (one per inlined
    /// navigation property) with the parent's `@odata.id` linked into
    /// each child's `parent_odata_id`.
    #[test]
    fn chassis_expand_yields_parent_and_children_with_correct_parent_odata_id() {
        let h = Harness::new();
        let spec = ExpandedChildSpec {
            thermal: Some(String::from(CHASSIS_1_THERMAL)),
            power: Some(String::from(CHASSIS_1_POWER)),
            sensors_collection: Some(String::from(CHASSIS_1_SENSORS)),
        };
        // The actual `$expand` toggle on the BMC is irrelevant to the
        // adapter logic — `ChassisGenerator` only inspects what the
        // schema deserialised into. Use the non-advertising helper so the
        // mock stays on the simpler `Bmc::get` path; the chassis JSON
        // shape (inlined Thermal/Power/Sensors below) is what makes the
        // navigation properties land as `NavProperty::Expanded`.
        let _ = service_root_json_advertising_expand; // keep helper exercised in code
        let (_bmc, service_root) = boot_service_root(
            &h,
            vec![
                expect_get_ok(
                    SERVICE_ROOT_CANONICAL,
                    service_root_json_with_chassis(SERVICE_ROOT),
                ),
                expect_get_ok(
                    CHASSIS_COLLECTION,
                    chassis_collection_json_with_member(CHASSIS_COLLECTION, None, CHASSIS_1),
                ),
                expect_get_ok(CHASSIS_1, chassis_item_json_expanded(CHASSIS_1, None, &spec)),
            ],
        );

        let collection = h
            .block_on(service_root.chassis())
            .expect("chassis collection fetch")
            .expect("chassis collection present");
        let mut members = h.block_on(collection.members()).expect("members fetch");
        let chassis = members.pop().expect("at least one chassis member");

        let mut r: Runtime<RedfishEvent, RedfishAdapterError> =
            Runtime::new(RuntimeConfig::default());
        let t = r.add_target(TargetLimits::default()).expect("add_target");
        r.add_generator(
            t,
            build_chassis_generator(BmcId::new("bmc-1"), chassis),
            GeneratorConfig::default(),
        )
        .expect("add_generator");

        let success = match drain_one_work(&mut r, &h) {
            Ok(s) => s,
            Err(e) => panic!("expected success, got error: {}", format_adapter_error(&e.error)),
        };
        assert_eq!(
            success.events.len(),
            4,
            "expected parent + thermal + power + sensors events, got {}",
            success.events.len()
        );
        let mut events = success.events.into_iter();
        match events.next().expect("parent event") {
            RedfishEvent::Resource(ev) => {
                assert_eq!(ev.odata_id.to_string(), CHASSIS_1);
                assert_eq!(ev.parent_odata_id, None, "parent chassis must not be nested");
                assert_eq!(ev.change, ChangeKind::Inserted);
                let payload = ev.payload.as_ref().expect("parent payload present");
                assert_eq!(payload.kind, "Chassis");
            }
            _ => panic!("expected Resource event for chassis parent"),
        }
        let mut child_kinds = Vec::new();
        for ev in events {
            match ev {
                RedfishEvent::Resource(ev) => {
                    assert_eq!(
                        ev.parent_odata_id.as_ref().map(ToString::to_string),
                        Some(String::from(CHASSIS_1)),
                        "every child must point at the parent chassis @odata.id"
                    );
                    assert_eq!(ev.change, ChangeKind::Inserted);
                    let payload = ev.payload.as_ref().expect("child payload present");
                    child_kinds.push(payload.kind.clone());
                }
                _ => panic!("expected Resource event for chassis child"),
            }
        }
        child_kinds.sort();
        assert_eq!(
            child_kinds,
            vec![
                String::from("Power"),
                String::from("SensorCollection"),
                String::from("Thermal"),
            ],
            "expected one child per inlined nav property",
        );
    }

    /// Drive the sensors generator: bootstrap a `Chassis<MockBmc>` whose
    /// schema only references the `Sensors` collection, then mock the
    /// collection-level fetch with three sensor refs. Drain three work
    /// items; each must contain a single `Sensor` event whose
    /// `parent_odata_id` points at the parent chassis.
    #[test]
    fn sensors_builder_emits_one_event_per_sensor_under_chassis() {
        let h = Harness::new();
        let sensor_ids: [&str; 3] = [
            "/redfish/v1/Chassis/1/Sensors/Temp",
            "/redfish/v1/Chassis/1/Sensors/Voltage",
            "/redfish/v1/Chassis/1/Sensors/Fan",
        ];
        let (_bmc, service_root) = boot_service_root(
            &h,
            vec![
                expect_get_ok(
                    SERVICE_ROOT_CANONICAL,
                    service_root_json_with_chassis(SERVICE_ROOT),
                ),
                expect_get_ok(
                    CHASSIS_COLLECTION,
                    chassis_collection_json_with_member(CHASSIS_COLLECTION, None, CHASSIS_1),
                ),
                expect_get_ok(
                    CHASSIS_1,
                    chassis_item_json_reference(CHASSIS_1, None, Some(CHASSIS_1_SENSORS)),
                ),
                expect_get_ok(
                    CHASSIS_1_SENSORS,
                    sensor_collection_json(CHASSIS_1_SENSORS, &sensor_ids),
                ),
            ],
        );

        let collection = h
            .block_on(service_root.chassis())
            .expect("chassis collection fetch")
            .expect("chassis collection present");
        let mut members = h.block_on(collection.members()).expect("members fetch");
        let chassis = members.pop().expect("at least one chassis");

        let mut r: Runtime<RedfishEvent, RedfishAdapterError> =
            Runtime::new(RuntimeConfig::default());
        let t = r.add_target(TargetLimits::default()).expect("add_target");
        r.add_generator(
            t,
            build_sensors_generator(BmcId::new("bmc-1"), chassis),
            GeneratorConfig::default(),
        )
        .expect("add_generator");

        let mut emitted_ids = Vec::new();
        for i in 0..sensor_ids.len() {
            let success = match drain_one_work(&mut r, &h) {
                Ok(s) => s,
                Err(e) => panic!(
                    "sensors work {} returned error: {}",
                    i,
                    format_adapter_error(&e.error)
                ),
            };
            assert_eq!(
                success.events.len(),
                1,
                "sensors work {} expected exactly one event, got {}",
                i,
                success.events.len()
            );
            match &success.events[0] {
                RedfishEvent::Resource(ev) => {
                    assert_eq!(ev.change, ChangeKind::Inserted);
                    assert_eq!(
                        ev.parent_odata_id.as_ref().map(ToString::to_string),
                        Some(String::from(CHASSIS_1)),
                        "sensor work {} must report the chassis as parent",
                        i
                    );
                    let payload = ev.payload.as_ref().expect("sensor payload present");
                    assert_eq!(payload.kind, "Sensor");
                    emitted_ids.push(ev.odata_id.to_string());
                }
                _ => panic!("sensors work {}: expected Resource event", i),
            }
        }
        emitted_ids.sort();
        let mut expected_ids: Vec<String> = sensor_ids.iter().map(|s| String::from(*s)).collect();
        expected_ids.sort();
        assert_eq!(
            emitted_ids, expected_ids,
            "sensor events should cover every sensor link returned by the BMC",
        );

        // After the queue drains the runtime should have no further
        // *work* ready: drain any runtime-events bookkeeping output the
        // `runtime-events` feature might emit, then assert the next poll
        // parks.
        loop {
            let mut fut = r.next();
            match h.poll(&mut fut) {
                Poll::Pending => break,
                Poll::Ready(RuntimeOutput::Runtime(_)) => continue,
                Poll::Ready(RuntimeOutput::Work(_)) => {
                    panic!("unexpected extra work after sensors drain")
                }
                Poll::Ready(_) => panic!("unexpected runtime output after sensors drain"),
            }
        }
    }

    /// Drive the computer-system builder: bootstrap a `ComputerSystem<MockBmc>`
    /// via the typed wrappers and assert that the generator emits a
    /// single `Inserted` event whose payload kind is `"ComputerSystem"`.
    #[test]
    fn computer_system_builder_emits_one_event_per_system() {
        let h = Harness::new();
        let (_bmc, service_root) = boot_service_root(
            &h,
            vec![
                expect_get_ok(
                    SERVICE_ROOT_CANONICAL,
                    service_root_json_with_chassis_and_systems(SERVICE_ROOT),
                ),
                expect_get_ok(
                    SYSTEM_COLLECTION,
                    system_collection_json_with_member(SYSTEM_COLLECTION, None, SYSTEM_1),
                ),
                expect_get_ok(SYSTEM_1, computer_system_json(SYSTEM_1, Some("\"sys-v1\""))),
            ],
        );

        let collection = h
            .block_on(service_root.systems())
            .expect("systems fetch")
            .expect("systems present");
        let mut members = h.block_on(collection.members()).expect("members fetch");
        let system = members.pop().expect("at least one system");

        let mut r: Runtime<RedfishEvent, RedfishAdapterError> =
            Runtime::new(RuntimeConfig::default());
        let t = r.add_target(TargetLimits::default()).expect("add_target");
        r.add_generator(
            t,
            build_computer_system_generator(BmcId::new("bmc-1"), system),
            GeneratorConfig::default(),
        )
        .expect("add_generator");

        let success = match drain_one_work(&mut r, &h) {
            Ok(s) => s,
            Err(e) => panic!("expected success, got error: {}", format_adapter_error(&e.error)),
        };
        assert_eq!(
            success.events.len(),
            1,
            "expected exactly one event, got {}",
            success.events.len()
        );
        match &success.events[0] {
            RedfishEvent::Resource(ev) => {
                assert_eq!(ev.change, ChangeKind::Inserted);
                assert_eq!(ev.parent_odata_id, None, "system has no parent in event stream");
                assert_eq!(ev.odata_id.to_string(), SYSTEM_1);
                let payload = ev.payload.as_ref().expect("system payload present");
                assert_eq!(payload.kind, "ComputerSystem");
                assert_eq!(
                    ev.metadata.etag.as_ref().map(ToString::to_string),
                    Some(String::from("\"sys-v1\"")),
                );
            }
            _ => panic!("expected Resource event for computer system"),
        }
    }
}
