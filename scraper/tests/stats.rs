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
use nv_redfish_scraper::TargetId;
use nv_redfish_scraper::TargetLimits;
use std::time::Duration;
use std::time::Instant;
use support::FakeError;
use support::FakeEvent;
use support::FakeGenerator;

#[test]
fn runtime_stats_expose_per_target_class_and_generator_snapshots() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let target_id = TargetId::new("target");
    let generator_id = GeneratorId::new("generator");
    let (generator, _) = FakeGenerator::new(
        target_id.clone(),
        generator_id.clone(),
        ClassId::new("sensors"),
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1))],
        vec![Ok(vec![FakeEvent::new("sensor")])],
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

    let stats = runtime.stats();
    assert_eq!(stats.target_stats().len(), 1);
    assert_eq!(stats.class_stats().len(), 1);
    assert_eq!(stats.generator_stats().len(), 1);
}

#[test]
fn generator_stats_report_lag_missed_intervals_and_actual_interval() {
    let mut runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let target_id = TargetId::new("target");
    let generator_id = GeneratorId::new("generator");
    let (generator, _) = FakeGenerator::new(
        target_id.clone(),
        generator_id.clone(),
        ClassId::new("periodic"),
        CostUnits::new(1),
        vec![Readiness::ready(CostUnits::new(1)); 2],
        vec![
            Ok(vec![FakeEvent::new("first")]),
            Ok(vec![FakeEvent::new("second")]),
        ],
    );

    runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .expect("target should be added");
    runtime
        .add_generator(
            &target_id,
            generator_id,
            GeneratorConfig::default().with_requested_interval(Duration::from_secs(1)),
            generator,
        )
        .expect("generator should be added");

    let _ = tokio_test::block_on(runtime.run_once(Instant::now()));
    let _ = tokio_test::block_on(runtime.run_once(Instant::now() + Duration::from_secs(3)));
    let stats = runtime.stats();
    let generator_stats = stats
        .generator_stats()
        .first()
        .expect("generator stats should exist");

    assert!(
        generator_stats.lag().is_some(),
        "periodic overload should be observable as lag"
    );
    assert!(
        generator_stats.missed_intervals() > 0,
        "missed intervals should be counted"
    );
    assert_eq!(
        generator_stats.actual_interval(),
        Some(Duration::from_secs(3)),
        "actual interval should be reported separately from requested interval"
    );
}

#[test]
fn overload_is_not_reported_as_periodic_job_queue_depth() {
    let runtime: Runtime<FakeEvent, FakeError> =
        Runtime::new(RuntimeConfig::default().with_output_queue_bound(1));

    assert_eq!(
        runtime.output_queue_stats().len(),
        0,
        "periodic overload should be represented by lag and missed intervals, not stale job depth"
    );
}
