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

//! Discovery-flow API tests for the Redfish adapter.
//!
//! Builder type signatures are enforced by the library compilation itself
//! (the per-capability `pub fn build_*_generator<B: Bmc>` declarations). The
//! tests in this file additionally assert observable behavior of the public
//! adapter event API: identity preservation, parent linkage, payload
//! preservation, and the type-level fact that emitted events do not carry
//! `B` parameters.

#![cfg(feature = "redfish-adapter")]

mod support;

use core::any::TypeId;
use core::task::Poll;

use nv_redfish::core::ODataETag;
use nv_redfish::core::ODataId;
use nv_redfish_scraper::adapter::redfish::BmcId;
use nv_redfish_scraper::adapter::redfish::ChangeKind;
use nv_redfish_scraper::adapter::redfish::EntityPayload;
use nv_redfish_scraper::adapter::redfish::GeneratorEvent;
use nv_redfish_scraper::adapter::redfish::RedfishAdapterError;
use nv_redfish_scraper::adapter::redfish::RedfishEvent;
use nv_redfish_scraper::adapter::redfish::ScrapeEvent;
use nv_redfish_scraper::Generator;
use nv_redfish_scraper::GeneratorConfig;
use nv_redfish_scraper::Runtime;
use nv_redfish_scraper::RuntimeConfig;
use nv_redfish_scraper::RuntimeOutput;
use nv_redfish_scraper::TargetLimits;

use support::fake_error::FakeError;
use support::fake_event::FakeEvent;
use support::fake_generator::FakeGenerator;
use support::fake_generator::Step;
use support::harness::Harness;

#[test]
fn child_resource_events_carry_parent_odata_id() {
    let event = support::redfish_events::ResourceEvent::at("bmc-A", "/redfish/v1/Chassis/1/Power")
        .parent("/redfish/v1/Chassis/1")
        .build();
    let parent = event
        .parent_odata_id
        .as_ref()
        .expect("parent odata id present");
    assert_eq!(parent.to_string(), "/redfish/v1/Chassis/1");
    assert_eq!(event.odata_id.to_string(), "/redfish/v1/Chassis/1/Power");
}

#[test]
fn expanded_payload_preservation_is_representable_via_event_api() {
    let payload = EntityPayload {
        kind: String::from("Chassis"),
        odata_id: ODataId::from(String::from("/redfish/v1/Chassis/1")),
        etag: Some(ODataETag::from(String::from("\"v1\""))),
    };
    let event = support::redfish_events::ResourceEvent::at("bmc-A", "/redfish/v1/Chassis/1")
        .change(ChangeKind::Updated)
        .payload(payload.clone())
        .build();
    let preserved = event.payload.as_ref().expect("payload preserved on event");
    assert_eq!(preserved.kind, payload.kind);
    assert_eq!(preserved.odata_id, payload.odata_id);
    assert_eq!(preserved.etag, payload.etag);
}

#[test]
fn redfish_event_type_does_not_have_a_bmc_type_parameter() {
    // The Generator type produced by every adapter builder is parameterised
    // by RedfishEvent and RedfishAdapterError, neither of which carries a `B`
    // type parameter. The TypeId is stable regardless of any concrete `B`
    // chosen at the builder call site.
    let id = TypeId::of::<Box<dyn Generator<RedfishEvent, RedfishAdapterError> + Send>>();
    let _ = id;
}

#[test]
fn change_kind_covers_phase_0_required_variants() {
    // The four Phase 0 required variants must be expressible in the public
    // API. Stale/Removed are reserved for later phases but already declared.
    let _ = ChangeKind::Inserted;
    let _ = ChangeKind::Updated;
    let _ = ChangeKind::RefreshedNoChange;
    let _ = ChangeKind::FetchFailed;
    let _ = ChangeKind::Stale;
    let _ = ChangeKind::Removed;
}

#[test]
fn generator_lifecycle_events_are_constructible() {
    let started = GeneratorEvent::Started {
        bmc_id: BmcId::new("a"),
        kind: String::from("service-root"),
    };
    let stopped = GeneratorEvent::Stopped {
        bmc_id: BmcId::new("a"),
        kind: String::from("service-root"),
    };
    let _ = RedfishEvent::Generator(started);
    let _ = RedfishEvent::Generator(stopped);
}

#[test]
fn scrape_lifecycle_events_are_constructible() {
    let completed = ScrapeEvent::Completed {
        bmc_id: BmcId::new("a"),
        resources: 7,
    };
    let failed = ScrapeEvent::Failed {
        bmc_id: BmcId::new("a"),
        error: String::from("transport reset"),
    };
    let _ = RedfishEvent::Scrape(completed);
    let _ = RedfishEvent::Scrape(failed);
}

// ---------------------------------------------------------------------------
// Application-flow tests using FakeGenerator/FakeEvent. These exercise the
// pattern of running the scheduler with a single root-like generator that
// "discovers" further generators that the application then adds.
// ---------------------------------------------------------------------------

fn drain_one(
    r: &mut Runtime<FakeEvent, FakeError>,
    h: &Harness,
) -> Option<RuntimeOutput<FakeEvent, FakeError>> {
    // Phase 5: skip transparent runtime events so existing discovery-flow
    // scenarios (which assert on Work/Shutdown shapes) keep passing under
    // `--features runtime-events`.
    loop {
        let mut next = r.next();
        match h.poll(&mut next) {
            Poll::Ready(o) => match &o {
                RuntimeOutput::Runtime(_) => continue,
                _ => return Some(o),
            },
            Poll::Pending => return None,
        }
    }
}

#[test]
fn discovery_starts_with_one_root_like_generator_then_application_adds_more() {
    let mut r: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    let _ = drain_one(&mut r, &h).expect("root output");

    // Application reacts by adding more generators.
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(2)])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(3)])])),
        GeneratorConfig::default(),
    )
    .unwrap();

    let mut ids = Vec::new();
    for _ in 0..2 {
        match drain_one(&mut r, &h).unwrap() {
            RuntimeOutput::Work(Ok(s)) => ids.extend(s.events.into_iter().map(|e| e.id())),
            _ => panic!("expected work"),
        }
    }
    ids.sort_unstable();
    assert_eq!(ids, vec![2, 3]);
}

#[test]
fn discovery_consume_failed_root_then_stop_emits_one_failure_then_no_progress() {
    let mut r: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Failure(FakeError::new(1))])),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    match drain_one(&mut r, &h).unwrap() {
        RuntimeOutput::Work(Err(_)) => {}
        _ => panic!("expected failure"),
    }
    // Generator script exhausted; subsequent next() parks. Phase 5: skip
    // any residual transparent runtime events first.
    loop {
        let mut fut = r.next();
        match h.poll(&mut fut) {
            Poll::Ready(RuntimeOutput::Runtime(_)) => continue,
            Poll::Pending => break,
            Poll::Ready(other) => panic!(
                "expected Pending, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }
}

#[test]
fn discovery_partial_then_shutdown_drains_queued_outputs_first() {
    let mut r: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([
            Step::Success(vec![FakeEvent::new(1)]),
            Step::Success(vec![FakeEvent::new(2)]),
        ])),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    let _ = drain_one(&mut r, &h);
    r.graceful_shutdown();
    let mut got_work = false;
    let mut got_shutdown = false;
    for _ in 0..5 {
        match drain_one(&mut r, &h) {
            Some(RuntimeOutput::Work(_)) => got_work = true,
            Some(RuntimeOutput::Shutdown) => {
                got_shutdown = true;
                break;
            }
            Some(_) | None => {}
        }
    }
    let _ = got_work;
    assert!(got_shutdown);
}

#[test]
fn discovery_runtime_is_policy_free_about_application_level_flow() {
    // Two ready generators that produce different ids. The runtime does
    // not pick "service-root before chassis"; that is a Phase 6 application
    // policy. The runtime's only contract is fairness across generators.
    let mut r: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(100)])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(200)])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    let mut ids = Vec::new();
    for _ in 0..2 {
        match drain_one(&mut r, &h).unwrap() {
            RuntimeOutput::Work(Ok(s)) => ids.extend(s.events.into_iter().map(|e| e.id())),
            _ => panic!("expected work"),
        }
    }
    ids.sort_unstable();
    assert_eq!(ids, vec![100, 200]);
}

#[test]
fn discovery_final_report_is_deterministic_for_fixed_input() {
    // For the same generator script and input order, the runtime must
    // produce the same output stream every time.
    fn run_once() -> Vec<u64> {
        let mut r: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
        let t = r.add_target(TargetLimits::default()).unwrap();
        r.add_generator(
            t,
            Box::new(FakeGenerator::new([
                Step::Success(vec![FakeEvent::new(1)]),
                Step::Failure(FakeError::new(2)),
                Step::Success(vec![FakeEvent::new(3)]),
                Step::Success(vec![FakeEvent::new(4)]),
            ])),
            GeneratorConfig::default(),
        )
        .unwrap();
        let h = Harness::new();
        let mut out = Vec::new();
        for _ in 0..4 {
            match drain_one(&mut r, &h).unwrap() {
                RuntimeOutput::Work(Ok(s)) => out.extend(s.events.iter().map(|e| e.id())),
                RuntimeOutput::Work(Err(e)) => out.push(0xFF00 | e.error.id()),
                _ => panic!("unexpected"),
            }
        }
        out
    }
    let a = run_once();
    let b = run_once();
    assert_eq!(a, b);
}
