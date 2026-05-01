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

#[cfg(feature = "runtime-events")]
use nv_redfish_scraper::ClassId;
#[cfg(feature = "runtime-events")]
use nv_redfish_scraper::CostUnits;
#[cfg(feature = "runtime-events")]
use nv_redfish_scraper::GeneratorConfig;
#[cfg(feature = "runtime-events")]
use nv_redfish_scraper::GeneratorId;
#[cfg(feature = "runtime-events")]
use nv_redfish_scraper::Readiness;
#[cfg(feature = "runtime-events")]
use nv_redfish_scraper::Runtime;
#[cfg(feature = "runtime-events")]
use nv_redfish_scraper::RuntimeConfig;
#[cfg(feature = "runtime-events")]
use nv_redfish_scraper::RuntimeEvent;
#[cfg(not(feature = "runtime-events"))]
use nv_redfish_scraper::RuntimeEventType;
use nv_redfish_scraper::RuntimeOutput;
#[cfg(feature = "runtime-events")]
use nv_redfish_scraper::TargetId;
#[cfg(feature = "runtime-events")]
use nv_redfish_scraper::TargetLimits;
#[cfg(feature = "runtime-events")]
use std::time::Instant;
#[cfg(feature = "runtime-events")]
use support::FakeError;
#[cfg(feature = "runtime-events")]
use support::FakeEvent;
#[cfg(feature = "runtime-events")]
use support::FakeGenerator;

#[test]
#[cfg(not(feature = "runtime-events"))]
fn disabled_runtime_event_type_is_infallible() {
    assert!(
        std::any::type_name::<RuntimeEventType>().contains("Infallible"),
        "runtime event payload must be Infallible when runtime-events is disabled"
    );
}

#[test]
#[cfg(not(feature = "runtime-events"))]
fn disabled_runtime_event_output_is_exhaustive_without_runtime_payload() {
    let output: RuntimeOutput<(), ()> = RuntimeOutput::Work(Ok(
        nv_redfish_scraper::WorkSuccess::new(Vec::new(), nv_redfish_scraper::WorkStats::default()),
    ));

    match output {
        RuntimeOutput::Work(_) => {}
        RuntimeOutput::Runtime(runtime_event) => match runtime_event {},
    }
}

#[test]
#[cfg(feature = "runtime-events")]
fn runtime_event_feature_exposes_runtime_event_variants() {
    let generator_id = GeneratorId::new("generator");
    let target_id = TargetId::new("target");

    let events = vec![
        RuntimeEvent::GeneratorLagging(generator_id.clone()),
        RuntimeEvent::GeneratorRecovered(generator_id.clone()),
        RuntimeEvent::GeneratorStarved(generator_id.clone()),
        RuntimeEvent::TargetThrottled(target_id),
        RuntimeEvent::GlobalThrottled,
        RuntimeEvent::EventQueuePressure,
        RuntimeEvent::WorkStarted(generator_id.clone()),
        RuntimeEvent::WorkCompleted(generator_id.clone()),
        RuntimeEvent::WorkFailed(generator_id),
    ];

    assert_eq!(events.len(), 9);
}

#[test]
#[cfg(feature = "runtime-events")]
fn work_started_completed_and_output_preserve_causal_order() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let target_id = TargetId::new("target");
    let generator_id = GeneratorId::new("generator");
    let (generator, _) = FakeGenerator::new(
        target_id.clone(),
        generator_id.clone(),
        ClassId::new("class"),
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1))],
        vec![Ok(vec![FakeEvent::new("event")])],
    );

    runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .expect("target should be added");
    runtime
        .add_generator(
            &target_id,
            generator_id,
            GeneratorConfig::default(),
            generator,
        )
        .expect("generator should be added");

    let _ = tokio_test::block_on(runtime.run_once(Instant::now()));
    let outputs = runtime.drain_outputs();

    assert_eq!(
        outputs.len(),
        3,
        "runtime events should bracket exactly one work output"
    );
    assert!(matches!(
        outputs.first(),
        Some(RuntimeOutput::Runtime(RuntimeEvent::WorkStarted(id))) if id == &GeneratorId::new("generator")
    ));
    assert!(matches!(
        outputs.get(1),
        Some(RuntimeOutput::Work(Ok(success))) if success.events()[0].name() == "event"
    ));
    assert!(matches!(
        outputs.get(2),
        Some(RuntimeOutput::Runtime(RuntimeEvent::WorkCompleted(id))) if id == &GeneratorId::new("generator")
    ));
}

#[test]
#[cfg(feature = "runtime-events")]
fn work_started_failed_and_output_preserve_causal_order() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let target_id = TargetId::new("target");
    let generator_id = GeneratorId::new("generator");
    let (generator, _) = FakeGenerator::new(
        target_id.clone(),
        generator_id.clone(),
        ClassId::new("class"),
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1))],
        vec![Err(FakeError::new("failed"))],
    );

    runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .expect("target should be added");
    runtime
        .add_generator(
            &target_id,
            generator_id,
            GeneratorConfig::default(),
            generator,
        )
        .expect("generator should be added");

    let _ = tokio_test::block_on(runtime.run_once(Instant::now()));
    let outputs = runtime.drain_outputs();

    assert!(matches!(
        outputs.as_slice(),
        [
            RuntimeOutput::Runtime(RuntimeEvent::WorkStarted(_)),
            RuntimeOutput::Work(Err(_)),
            RuntimeOutput::Runtime(RuntimeEvent::WorkFailed(_)),
        ]
    ));
}

#[test]
#[cfg(feature = "runtime-events")]
fn lag_and_queue_pressure_emit_ordered_runtime_events() {
    let mut runtime: Runtime<FakeEvent, FakeError> =
        Runtime::new(RuntimeConfig::default().with_output_queue_bound(1));
    let target_id = TargetId::new("target");
    let generator_id = GeneratorId::new("generator");
    let (generator, _) = FakeGenerator::new(
        target_id.clone(),
        generator_id.clone(),
        ClassId::new("periodic"),
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1)); 2],
        vec![Ok(vec![FakeEvent::new("event")])],
    );

    runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .expect("target should be added");
    runtime
        .add_generator(
            &target_id,
            generator_id,
            GeneratorConfig::default().with_requested_interval(std::time::Duration::from_secs(1)),
            generator,
        )
        .expect("generator should be added");

    let _ =
        tokio_test::block_on(runtime.run_once(Instant::now() + std::time::Duration::from_secs(5)));
    let outputs = runtime.drain_outputs();

    assert!(
        outputs.iter().any(|output| matches!(
            output,
            RuntimeOutput::Runtime(RuntimeEvent::GeneratorLagging(_))
        )),
        "lag should be reported as an ordered runtime event"
    );
    assert!(
        outputs.iter().any(|output| matches!(
            output,
            RuntimeOutput::Runtime(RuntimeEvent::EventQueuePressure)
        )),
        "queue pressure should be reported as an ordered runtime event"
    );
}
