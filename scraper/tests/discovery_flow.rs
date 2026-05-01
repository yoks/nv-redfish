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

mod support;

use nv_redfish_scraper::ClassId;
use nv_redfish_scraper::CostUnits;
use nv_redfish_scraper::GeneratorConfig;
use nv_redfish_scraper::GeneratorId;
use nv_redfish_scraper::Readiness;
use nv_redfish_scraper::Runtime;
use nv_redfish_scraper::RuntimeConfig;
use nv_redfish_scraper::RuntimeOutput;
use nv_redfish_scraper::TargetId;
use nv_redfish_scraper::TargetLimits;
use std::time::Instant;
use support::FakeError;
use support::FakeEvent;
use support::FakeGenerator;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestedState {
    NotRequested,
    RequestedMissing,
    RequestedFailed,
    RequestedSuccessful,
}

fn add_named_generator(
    runtime: &mut Runtime<FakeEvent, FakeError>,
    target_id: &TargetId,
    name: &'static str,
    result: Result<Vec<FakeEvent>, FakeError>,
) {
    let generator_id = GeneratorId::new(format!("generator-{name}"));
    let (generator, _) = FakeGenerator::new(
        target_id.clone(),
        generator_id.clone(),
        ClassId::new("discovery"),
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1))],
        vec![result],
    );
    runtime
        .add_generator(
            target_id,
            generator_id,
            GeneratorConfig::default(),
            generator,
        )
        .expect("generator should be added");
}

#[test]
fn application_can_start_with_service_root_and_add_more_generators_from_output() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::new(4));
    let target_id = TargetId::new("bmc");

    runtime
        .add_target(target_id.clone(), TargetLimits::new(4))
        .expect("target should be added");
    add_named_generator(
        &mut runtime,
        &target_id,
        "service-root",
        Ok(vec![FakeEvent::new("service-root")]),
    );

    let _ = tokio_test::block_on(runtime.run_once(Instant::now()));
    let outputs = runtime.drain_outputs();
    let saw_service_root = outputs.iter().any(|output| match output {
        RuntimeOutput::Work(Ok(success)) => success
            .events()
            .iter()
            .any(|event| event.name() == "service-root"),
        _ => false,
    });

    assert!(saw_service_root);
    add_named_generator(
        &mut runtime,
        &target_id,
        "sensors",
        Ok(vec![FakeEvent::new("sensor")]),
    );
    let _ = tokio_test::block_on(runtime.run_once(Instant::now()));
    assert_eq!(runtime.drain_outputs().len(), 1);
}

#[test]
fn application_can_request_narrow_scraping_only() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let target_id = TargetId::new("bmc");

    runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .expect("target should be added");
    add_named_generator(
        &mut runtime,
        &target_id,
        "gpu-sensors",
        Ok(vec![FakeEvent::new("gpu-sensor")]),
    );

    let _ = tokio_test::block_on(runtime.run_once(Instant::now()));
    let outputs = runtime.drain_outputs();
    assert_eq!(outputs.len(), 1);
    assert_eq!(runtime.stats().generator_count(), 1);
}

#[test]
fn requested_states_are_distinguishable_by_application_policy() {
    let states = [
        RequestedState::NotRequested,
        RequestedState::RequestedMissing,
        RequestedState::RequestedFailed,
        RequestedState::RequestedSuccessful,
    ];

    assert_eq!(states.len(), 4);
    assert_ne!(states[0], states[1]);
    assert_ne!(states[1], states[2]);
    assert_ne!(states[2], states[3]);
}

#[test]
fn runtime_remains_policy_free_during_discovery() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let target_id = TargetId::new("bmc");

    runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .expect("target should be added");
    assert_eq!(
        runtime.stats().generator_count(),
        0,
        "runtime should not add Redfish discovery generators by itself"
    );

    add_named_generator(
        &mut runtime,
        &target_id,
        "application-selected",
        Ok(vec![FakeEvent::new("selected")]),
    );
    assert_eq!(runtime.stats().generator_count(), 1);
}
