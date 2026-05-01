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

fn runtime_with_generator(
    result: Result<Vec<FakeEvent>, FakeError>,
) -> Runtime<FakeEvent, FakeError> {
    let mut runtime = Runtime::new(RuntimeConfig::default());
    let target_id = TargetId::new("target");
    let generator_id = GeneratorId::new("generator");
    let (generator, _) = FakeGenerator::new(
        target_id.clone(),
        generator_id.clone(),
        ClassId::new("class"),
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1))],
        vec![result],
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
    runtime
}

#[test]
fn successful_work_produces_ordered_work_output() {
    let mut runtime = runtime_with_generator(Ok(vec![FakeEvent::new("first")]));

    let _ = tokio_test::block_on(runtime.run_once(Instant::now()));
    let output = runtime.poll_output();

    match output {
        Some(RuntimeOutput::Work(Ok(success))) => {
            assert_eq!(success.events()[0].name(), "first");
        }
        _ => {
            panic!("expected ordered successful work output");
        }
    }
}

#[test]
fn multiple_events_from_one_work_item_preserve_order() {
    let mut runtime = runtime_with_generator(Ok(vec![
        FakeEvent::new("first"),
        FakeEvent::new("second"),
        FakeEvent::new("third"),
    ]));

    let _ = tokio_test::block_on(runtime.run_once(Instant::now()));
    let output = runtime.poll_output();

    match output {
        Some(RuntimeOutput::Work(Ok(success))) => {
            let names = success
                .events()
                .iter()
                .map(FakeEvent::name)
                .collect::<Vec<_>>();
            assert_eq!(names, vec!["first", "second", "third"]);
        }
        _ => {
            panic!("expected successful work output");
        }
    }
}

#[test]
fn failures_produce_ordered_work_error_output() {
    let mut runtime = runtime_with_generator(Err(FakeError::new("failed")));

    let _ = tokio_test::block_on(runtime.run_once(Instant::now()));
    let output = runtime.poll_output();

    match output {
        Some(RuntimeOutput::Work(Err(error))) => {
            assert_eq!(error.error().name(), "failed");
        }
        _ => {
            panic!("expected ordered failed work output");
        }
    }
}

#[test]
fn one_shot_drain_returns_all_available_outputs_in_fifo_order() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::new(4));
    let target_id = TargetId::new("target");

    runtime
        .add_target(target_id.clone(), TargetLimits::new(4))
        .expect("target should be added");
    for name in ["first", "second"] {
        let generator_id = GeneratorId::new(format!("generator-{name}"));
        let (generator, _) = FakeGenerator::new(
            target_id.clone(),
            generator_id.clone(),
            ClassId::new("class"),
            CostUnits::new(1),
            vec![Readiness::ready(CostUnits::new(1))],
            vec![Ok(vec![FakeEvent::new(name)])],
        );
        runtime
            .add_generator(
                &target_id,
                generator_id,
                GeneratorConfig::default(),
                generator,
            )
            .expect("generator should be added");
        let _ = tokio_test::block_on(runtime.run_once(Instant::now()));
    }

    let outputs = runtime.drain_outputs();
    assert_eq!(outputs.len(), 2);
}

#[test]
fn queue_pressure_is_reflected_in_stats() {
    let mut runtime = runtime_with_generator(Ok(vec![FakeEvent::new("event")]));

    let _ = tokio_test::block_on(runtime.run_once(Instant::now()));
    let stats = runtime.output_queue_stats();

    assert_eq!(stats.len(), 1);
    assert_eq!(stats.dropped(), 0);
    assert_eq!(stats.rejected(), 0);
}
