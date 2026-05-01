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
use std::time::Instant;
use support::FakeError;
use support::FakeEvent;
use support::FakeGenerator;

fn runtime() -> Runtime<FakeEvent, FakeError> {
    Runtime::new(RuntimeConfig::default())
}

fn ready_generator(
    target_id: &TargetId,
    generator_id: &GeneratorId,
) -> FakeGenerator<FakeEvent, FakeError> {
    let (generator, _) = FakeGenerator::new(
        target_id.clone(),
        generator_id.clone(),
        ClassId::new("control"),
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1))],
        vec![Ok(vec![FakeEvent::new("event")])],
    );
    generator
}

#[test]
fn add_and_remove_target_updates_tree_state() {
    let mut runtime = runtime();
    let target_id = TargetId::new("target");

    assert!(runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .is_ok());
    assert_eq!(runtime.stats().target_count(), 1);

    assert!(runtime.remove_target(&target_id).is_ok());
    assert_eq!(runtime.stats().target_count(), 0);
}

#[test]
fn pause_resume_and_update_target_limits_are_control_operations() {
    let mut runtime = runtime();
    let target_id = TargetId::new("target");

    assert!(runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .is_ok());
    assert!(runtime.pause_target(&target_id).is_ok());
    assert!(runtime.resume_target(&target_id).is_ok());
    assert!(runtime
        .update_target_limits(&target_id, TargetLimits::new(2))
        .is_ok());
}

#[test]
fn add_remove_pause_resume_and_trigger_generator_are_control_operations() {
    let mut runtime = runtime();
    let target_id = TargetId::new("target");
    let generator_id = GeneratorId::new("generator");

    assert!(runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .is_ok());
    assert!(runtime
        .add_generator(
            &target_id,
            generator_id.clone(),
            GeneratorConfig::default(),
            ready_generator(&target_id, &generator_id),
        )
        .is_ok());
    assert_eq!(runtime.stats().generator_count(), 1);

    assert!(runtime.pause_generator(&generator_id).is_ok());
    assert!(runtime.resume_generator(&generator_id).is_ok());
    assert!(runtime.trigger_generator(&generator_id).is_ok());
    assert!(runtime.remove_generator(&generator_id).is_ok());
    assert_eq!(runtime.stats().generator_count(), 0);
}

#[test]
fn removing_target_removes_attached_generators() {
    let mut runtime = runtime();
    let target_id = TargetId::new("target");
    let generator_id = GeneratorId::new("generator");

    assert!(runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .is_ok());
    assert!(runtime
        .add_generator(
            &target_id,
            generator_id.clone(),
            GeneratorConfig::default(),
            ready_generator(&target_id, &generator_id),
        )
        .is_ok());

    assert!(runtime.remove_target(&target_id).is_ok());
    assert_eq!(runtime.stats().target_count(), 0);
    assert_eq!(runtime.stats().generator_count(), 0);
    assert!(runtime.remove_generator(&generator_id).is_err());
}

#[test]
fn removed_generators_are_never_queried_again() {
    let mut runtime = runtime();
    let target_id = TargetId::new("target");
    let generator_id = GeneratorId::new("generator");
    let (generator, handle) = FakeGenerator::new(
        target_id.clone(),
        generator_id.clone(),
        ClassId::new("control"),
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1))],
        vec![Ok(vec![FakeEvent::new("event")])],
    );

    assert!(runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .is_ok());
    assert!(runtime
        .add_generator(
            &target_id,
            generator_id.clone(),
            GeneratorConfig::default(),
            generator,
        )
        .is_ok());
    assert!(runtime.remove_generator(&generator_id).is_ok());

    let outcome = tokio_test::block_on(runtime.run_once(Instant::now()));
    assert_eq!(outcome, Ok(RunOutcome::Idle));
    assert_eq!(handle.update_ready_count(), 0);
    assert_eq!(handle.take_next_count(), 0);
}

#[test]
fn queued_outputs_survive_target_and_generator_removal() {
    let mut runtime = runtime();
    let target_id = TargetId::new("target");
    let generator_id = GeneratorId::new("generator");

    assert!(runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .is_ok());
    assert!(runtime
        .add_generator(
            &target_id,
            generator_id.clone(),
            GeneratorConfig::default(),
            ready_generator(&target_id, &generator_id),
        )
        .is_ok());

    let outcome = tokio_test::block_on(runtime.run_once(Instant::now()));
    assert_eq!(
        outcome,
        Ok(RunOutcome::Dispatched),
        "run_once should execute the ready generator before removal"
    );
    assert!(runtime.remove_target(&target_id).is_ok());
    assert_eq!(
        runtime.drain_outputs().len(),
        1,
        "queued work output must survive generator and target removal"
    );
}
