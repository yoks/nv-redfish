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
use nv_redfish_scraper::RunOutcome;
use nv_redfish_scraper::Runtime;
use nv_redfish_scraper::RuntimeConfig;
use nv_redfish_scraper::TargetId;
use nv_redfish_scraper::TargetLimits;
use std::time::Duration;
use std::time::Instant;
use support::FakeError;
use support::FakeEvent;
use support::FakeGenerator;
use support::FakeGeneratorHandle;

fn add_generator(
    runtime: &mut Runtime<FakeEvent, FakeError>,
    target_id: &TargetId,
    generator_id: &GeneratorId,
    class_id: &str,
    cost: CostUnits,
    readiness: Vec<Readiness>,
    events: Vec<FakeEvent>,
) -> FakeGeneratorHandle<FakeEvent, FakeError> {
    let (generator, handle) = FakeGenerator::new(
        target_id.clone(),
        generator_id.clone(),
        ClassId::new(class_id),
        cost,
        readiness,
        vec![Ok(events)],
    );

    runtime
        .add_generator(
            target_id,
            generator_id.clone(),
            GeneratorConfig::default(),
            generator,
        )
        .expect("generator should be added");
    handle
}

#[test]
fn no_work_is_dispatched_when_no_generator_is_ready() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let target_id = TargetId::new("target");
    let generator_id = GeneratorId::new("generator");

    runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .expect("target should be added");
    let handle = add_generator(
        &mut runtime,
        &target_id,
        &generator_id,
        "class",
        CostUnits::new(1),
        vec![Readiness::not_ready(None)],
        vec![FakeEvent::new("event")],
    );

    let outcome = tokio_test::block_on(runtime.run_once(Instant::now()));
    assert_eq!(outcome, Ok(RunOutcome::Idle));
    assert_eq!(
        handle.update_ready_count(),
        1,
        "scheduler should query readiness even when no work is dispatched"
    );
    assert_eq!(handle.take_next_count(), 0);
}

#[test]
fn ready_generator_dispatches_one_work_item() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let target_id = TargetId::new("target");
    let generator_id = GeneratorId::new("generator");

    runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .expect("target should be added");
    let handle = add_generator(
        &mut runtime,
        &target_id,
        &generator_id,
        "class",
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1))],
        vec![FakeEvent::new("event")],
    );

    let outcome = tokio_test::block_on(runtime.run_once(Instant::now()));
    assert_eq!(outcome, Ok(RunOutcome::Dispatched));
    assert_eq!(handle.update_ready_count(), 1);
    assert_eq!(handle.take_next_count(), 1);
    assert_eq!(runtime.drain_outputs().len(), 1);
}

#[test]
fn run_once_dispatches_at_most_one_selected_work_item() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::new(4));
    let target_id = TargetId::new("target");

    runtime
        .add_target(target_id.clone(), TargetLimits::new(4))
        .expect("target should be added");
    let first = add_generator(
        &mut runtime,
        &target_id,
        &GeneratorId::new("first"),
        "class",
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1))],
        vec![FakeEvent::new("first")],
    );
    let second = add_generator(
        &mut runtime,
        &target_id,
        &GeneratorId::new("second"),
        "class",
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1))],
        vec![FakeEvent::new("second")],
    );

    let outcome = tokio_test::block_on(runtime.run_once(Instant::now()));
    assert_eq!(outcome, Ok(RunOutcome::Dispatched));
    assert_eq!(first.take_next_count() + second.take_next_count(), 1);
}

#[test]
fn target_and_global_in_flight_limits_are_respected() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::new(1));
    let target_id = TargetId::new("target");

    runtime
        .add_target(target_id.clone(), TargetLimits::new(1))
        .expect("target should be added");
    add_generator(
        &mut runtime,
        &target_id,
        &GeneratorId::new("generator"),
        "class",
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1))],
        vec![FakeEvent::new("event")],
    );

    let first = tokio_test::block_on(runtime.run_once(Instant::now()));
    let second = tokio_test::block_on(runtime.run_once(Instant::now()));
    assert_eq!(first, Ok(RunOutcome::Dispatched));
    assert_eq!(
        second,
        Ok(RunOutcome::Idle),
        "runtime should not dispatch stale duplicate work after one ready item is consumed"
    );
}

#[test]
fn cost_and_fairness_require_expensive_and_low_rate_work_to_run() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::new(4));
    let target_id = TargetId::new("target");

    runtime
        .add_target(target_id.clone(), TargetLimits::new(4))
        .expect("target should be added");
    let cheap = add_generator(
        &mut runtime,
        &target_id,
        &GeneratorId::new("cheap"),
        "foreground",
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1)); 8],
        vec![FakeEvent::new("cheap")],
    );
    let expensive = add_generator(
        &mut runtime,
        &target_id,
        &GeneratorId::new("expensive"),
        "background",
        CostUnits::new(8),
        vec![Readiness::ready(CostUnits::new(8)); 8],
        vec![FakeEvent::new("expensive")],
    );

    for _ in 0..4 {
        let _ = tokio_test::block_on(runtime.run_once(Instant::now()));
    }

    assert!(
        cheap.take_next_count() > 0,
        "cheap work should receive service"
    );
    assert!(
        expensive.take_next_count() > 0,
        "expensive work must not be permanently starved"
    );
}

#[test]
fn target_fairness_prevents_one_target_from_consuming_all_dispatches() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::new(4));
    let first_target = TargetId::new("first-target");
    let second_target = TargetId::new("second-target");

    runtime
        .add_target(first_target.clone(), TargetLimits::new(4))
        .expect("first target should be added");
    runtime
        .add_target(second_target.clone(), TargetLimits::new(4))
        .expect("second target should be added");
    let first = add_generator(
        &mut runtime,
        &first_target,
        &GeneratorId::new("first-generator"),
        "class",
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1)); 4],
        vec![FakeEvent::new("first")],
    );
    let second = add_generator(
        &mut runtime,
        &second_target,
        &GeneratorId::new("second-generator"),
        "class",
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1)); 4],
        vec![FakeEvent::new("second")],
    );

    for _ in 0..2 {
        let _ = tokio_test::block_on(runtime.run_once(Instant::now()));
    }

    assert!(first.take_next_count() > 0);
    assert!(second.take_next_count() > 0);
}

#[test]
fn tree_changes_invalidate_stale_readiness() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let target_id = TargetId::new("target");
    let stale_generator_id = GeneratorId::new("stale");
    let fresh_generator_id = GeneratorId::new("fresh");

    runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .expect("target should be added");
    let stale = add_generator(
        &mut runtime,
        &target_id,
        &stale_generator_id,
        "class",
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1))],
        vec![FakeEvent::new("stale")],
    );
    runtime
        .remove_generator(&stale_generator_id)
        .expect("stale generator should be removed");
    let fresh = add_generator(
        &mut runtime,
        &target_id,
        &fresh_generator_id,
        "class",
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1))],
        vec![FakeEvent::new("fresh")],
    );

    let outcome = tokio_test::block_on(runtime.run_once(Instant::now()));
    assert_eq!(outcome, Ok(RunOutcome::Dispatched));
    assert_eq!(stale.take_next_count(), 0);
    assert_eq!(fresh.take_next_count(), 1);
}

#[test]
fn periodic_generators_do_not_accumulate_stale_jobs() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::new(4));
    let target_id = TargetId::new("target");
    let generator_id = GeneratorId::new("periodic");
    let (generator, handle) = FakeGenerator::new(
        target_id.clone(),
        generator_id.clone(),
        ClassId::new("periodic"),
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1)); 16],
        vec![
            Ok(vec![FakeEvent::new("first")]),
            Ok(vec![FakeEvent::new("second")]),
            Ok(vec![FakeEvent::new("third")]),
        ],
    );

    runtime
        .add_target(target_id.clone(), TargetLimits::new(4))
        .expect("target should be added");
    runtime
        .add_generator(
            &target_id,
            generator_id,
            GeneratorConfig::default().with_requested_interval(Duration::from_secs(1)),
            generator,
        )
        .expect("generator should be added");

    let _ = tokio_test::block_on(runtime.run_once(Instant::now() + Duration::from_secs(30)));

    assert_eq!(
        handle.take_next_count(),
        1,
        "one scheduler pass should create at most one fresh work item even after a long delay"
    );
    assert_eq!(
        runtime.drain_outputs().len(),
        1,
        "runtime must not enqueue one stale periodic job per missed interval"
    );
}
