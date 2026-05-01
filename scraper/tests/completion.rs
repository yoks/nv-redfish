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
use nv_redfish_scraper::CompletionOutcome;
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
use support::FakeGeneratorHandle;

fn runtime_with_generator(
    result: Result<Vec<FakeEvent>, FakeError>,
) -> (
    Runtime<FakeEvent, FakeError>,
    FakeGeneratorHandle<FakeEvent, FakeError>,
) {
    let mut runtime = Runtime::new(RuntimeConfig::default());
    let target_id = TargetId::new("target");
    let generator_id = GeneratorId::new("generator");
    let (generator, handle) = FakeGenerator::new(
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
    (runtime, handle)
}

#[test]
fn completion_is_reported_exactly_once_after_success() {
    let (mut runtime, handle) = runtime_with_generator(Ok(vec![FakeEvent::new("event")]));

    let _ = tokio_test::block_on(runtime.run_once(Instant::now()));

    assert_eq!(handle.completion_count(), 1);
    assert_eq!(
        handle.completion_outcomes(),
        vec![CompletionOutcome::Success]
    );
}

#[test]
fn completion_is_reported_exactly_once_after_failure() {
    let (mut runtime, handle) = runtime_with_generator(Err(FakeError::new("failed")));

    let _ = tokio_test::block_on(runtime.run_once(Instant::now()));

    assert_eq!(handle.completion_count(), 1);
    assert_eq!(
        handle.completion_outcomes(),
        vec![CompletionOutcome::Failure]
    );
}

#[test]
fn output_is_enqueued_before_generator_completion_callback() {
    let (mut runtime, handle) = runtime_with_generator(Ok(vec![FakeEvent::new("event")]));

    let _ = tokio_test::block_on(runtime.run_once(Instant::now()));

    assert_eq!(handle.completion_count(), 1);
    assert_eq!(
        runtime.output_queue_stats().len(),
        1,
        "work output must be observable after completion is reported"
    );
}

#[test]
fn in_flight_counters_are_released_after_completion() {
    let (mut runtime, _) = runtime_with_generator(Ok(vec![FakeEvent::new("event")]));

    let _ = tokio_test::block_on(runtime.run_once(Instant::now()));

    assert_eq!(runtime.stats().global_in_flight(), 0);
}

#[test]
fn failed_work_keeps_runtime_owned_stats() {
    let (mut runtime, _) = runtime_with_generator(Err(FakeError::new("failed")));

    let _ = tokio_test::block_on(runtime.run_once(Instant::now()));

    match runtime.poll_output() {
        Some(RuntimeOutput::Work(Err(error))) => {
            assert_eq!(error.stats().started_count(), 1);
            assert_eq!(error.stats().completed_count(), 1);
        }
        _ => {
            panic!("expected failed work output with runtime stats");
        }
    }
}
