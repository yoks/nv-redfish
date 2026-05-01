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

//! Statistics snapshot tests.

mod support;

use core::task::Poll;

use nv_redfish_scraper::ClassId;
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
use support::lcg::Lcg;

fn rt() -> Runtime<FakeEvent, FakeError> {
    Runtime::new(RuntimeConfig::default())
}

#[test]
fn empty_runtime_snapshot_is_all_zeros() {
    let r = rt();
    let s = r.stats();
    assert_eq!(s.targets, 0);
    assert_eq!(s.generators, 0);
    assert_eq!(s.in_flight, 0);
    assert_eq!(s.dispatched, 0);
    assert!(s.per_target.is_empty());
    assert_eq!(s.output_queue.queued, 0);
    assert_eq!(s.output_queue.dropped, 0);
}

#[test]
fn snapshot_reflects_added_targets_and_generators() {
    let r = rt();
    let t1 = r.add_target(TargetLimits::default()).unwrap();
    let t2 = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(t1, Box::new(FakeGenerator::new([])), GeneratorConfig::default())
        .unwrap();
    r.add_generator(t2, Box::new(FakeGenerator::new([])), GeneratorConfig::default())
        .unwrap();
    r.add_generator(t2, Box::new(FakeGenerator::new([])), GeneratorConfig::default())
        .unwrap();
    let s = r.stats();
    assert_eq!(s.targets, 2);
    assert_eq!(s.generators, 3);
    assert_eq!(s.per_target.len(), 2);
    let pt2 = s
        .per_target
        .iter()
        .find(|t| t.target == Some(t2))
        .expect("target 2 stats present");
    assert_eq!(pt2.generators, 2);
    assert_eq!(pt2.per_generator.len(), 2);
}

#[test]
fn snapshot_dispatched_count_matches_observed_outputs() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([
            Step::Success(vec![FakeEvent::new(1)]),
            Step::Success(vec![FakeEvent::new(2)]),
            Step::Failure(FakeError::new(3)),
        ])),
        GeneratorConfig::default(),
    )
    .unwrap();

    let h = Harness::new();
    // Phase 5: budget for the extra `WorkStarted`/`WorkCompleted`/`WorkFailed`
    // events that interleave between work outputs under runtime-events.
    for _ in 0..12 {
        let mut next = r.next();
        match h.poll(&mut next) {
            Poll::Ready(_) => {}
            Poll::Pending => break,
        }
    }
    let s = r.stats();
    assert_eq!(s.dispatched, 3);
    assert_eq!(s.in_flight, 0);
    assert_eq!(s.per_target[0].dispatched, 3);
}

#[test]
fn class_stats_aggregate_per_class() {
    let r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let cls_a = ClassId::new("a");
    let cls_b = ClassId::new("b");
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([])),
        GeneratorConfig {
            class: Some(cls_a.clone()),
            weight: None,
        },
    )
    .unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([])),
        GeneratorConfig {
            class: Some(cls_a.clone()),
            weight: None,
        },
    )
    .unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([])),
        GeneratorConfig {
            class: Some(cls_b.clone()),
            weight: None,
        },
    )
    .unwrap();

    let cs = r.class_stats();
    let class_a = cs
        .iter()
        .find(|c| c.class.as_ref() == Some(&cls_a))
        .expect("class a");
    let class_b = cs
        .iter()
        .find(|c| c.class.as_ref() == Some(&cls_b))
        .expect("class b");
    assert_eq!(class_a.dispatched, 0);
    assert_eq!(class_b.dispatched, 0);
}

#[test]
fn output_queue_stats_track_queued_outputs() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    // Before any next() call, queue is empty.
    assert_eq!(r.stats().output_queue.queued, 0);
    let h = Harness::new();
    {
        let mut fut = r.next();
        let _ = h.poll(&mut fut);
    }
    // After draining, queue is empty again.
    assert_eq!(r.stats().output_queue.queued, 0);
}

#[test]
fn generator_stats_report_lag_missed_intervals_and_actual_interval() {
    // Phase 4: per-generator stats must surface lag/missed-intervals/
    // actual-interval after a few dispatches. Phase 0 leaves these zeroed,
    // so this test is red until Phase 4.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let gid = r
        .add_generator(
            t,
            Box::new(FakeGenerator::new([
                Step::Success(vec![FakeEvent::new(1)]),
                Step::Success(vec![FakeEvent::new(2)]),
                Step::Success(vec![FakeEvent::new(3)]),
            ])),
            GeneratorConfig::default(),
        )
        .unwrap();
    let h = Harness::new();
    for _ in 0..3 {
        let mut fut = r.next();
        let _ = h.poll(&mut fut);
    }
    let stats = r.stats();
    let target = stats
        .per_target
        .iter()
        .find(|t| t.target == Some(gid.target_id()))
        .expect("target stats");
    let (_, gen_stats) = target
        .per_generator
        .iter()
        .find(|(id, _)| *id == gid)
        .expect("gen stats");
    assert!(
        gen_stats.actual_interval.is_some(),
        "actual_interval must be populated after ≥2 dispatches"
    );
}

#[test]
fn periodic_overload_is_not_represented_as_stale_queued_job_depth() {
    // The runtime must not back up periodic generators into the output
    // queue. Even with many ready steps, queue depth stays small.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let steps: Vec<Step> = (0..1000)
        .map(|_| Step::Success(vec![FakeEvent::new(1)]))
        .collect();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new(steps)),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    for _ in 0..50 {
        let mut fut = r.next();
        let _ = h.poll(&mut fut);
    }
    let stats = r.stats().output_queue;
    assert!(
        stats.queued <= 1,
        "queued depth must reflect on-demand dispatch (got {})",
        stats.queued
    );
}

#[test]
fn bounded_queue_pressure_reports_dropped_or_rejected_outputs_not_unbounded_growth() {
    // Phase 4: with a bounded output queue, overflow must be reported as
    // OutputQueueStats.dropped, not by letting the queue grow unbounded.
    let cfg = RuntimeConfig {
        output_queue_capacity: Some(1),
        ..RuntimeConfig::default()
    };
    let mut r: Runtime<FakeEvent, FakeError> = Runtime::new(cfg);
    let t = r.add_target(TargetLimits::default()).unwrap();
    let steps: Vec<Step> = (0..10)
        .map(|i| Step::Success(vec![FakeEvent::new(i as u64)]))
        .collect();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new(steps)),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    for _ in 0..10 {
        let mut fut = r.next();
        let _ = h.poll(&mut fut);
    }
    let stats = r.stats().output_queue;
    assert!(stats.queued <= 1, "queue must respect capacity");
    assert!(
        stats.dropped >= 1,
        "bounded queue must surface dropped count (got {})",
        stats.dropped
    );
}

#[test]
fn stats_update_on_failure() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let gid = r
        .add_generator(
            t,
            Box::new(FakeGenerator::new([Step::Failure(FakeError::new(1))])),
            GeneratorConfig::default(),
        )
        .unwrap();
    let h = Harness::new();
    // Phase 5: drive the runtime forward enough times that the work future
    // both dispatches and finalizes; the extra iterations absorb the
    // transparent `WorkStarted` / `WorkFailed` events under runtime-events.
    for _ in 0..6 {
        let mut fut = r.next();
        let _ = h.poll(&mut fut);
    }
    let stats = r.stats();
    let target = stats
        .per_target
        .iter()
        .find(|t| t.target == Some(gid.target_id()))
        .expect("target stats");
    let (_, gs) = target
        .per_generator
        .iter()
        .find(|(id, _)| *id == gid)
        .expect("gen stats");
    assert_eq!(gs.dispatched, 1);
    assert_eq!(gs.failed, 1);
    assert_eq!(gs.succeeded, 0);
}

#[test]
fn stats_update_after_removal_and_shutdown() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let g_id = r
        .add_generator(
            t,
            Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])),
            GeneratorConfig::default(),
        )
        .unwrap();
    let h = Harness::new();
    {
        let mut fut = r.next();
        let _ = h.poll(&mut fut);
    }
    assert!(r.remove_generator(g_id));
    let stats_after_remove = r.stats();
    assert!(
        stats_after_remove
            .per_target
            .iter()
            .all(|t| t.per_generator.is_empty()),
        "generator stats record must disappear after removal"
    );

    r.graceful_shutdown();
    {
        let mut fut = r.next();
        let _ = h.poll(&mut fut);
    }
    let stats_after_shutdown = r.stats();
    // Targets remain visible until torn down explicitly; shutdown does not
    // wipe pre-existing target snapshots.
    assert!(stats_after_shutdown.targets >= 1);
}

#[test]
fn stats_snapshot_is_internally_consistent_under_generated_operation_sequences() {
    // For each LCG seed run a short operation sequence and assert that at
    // every checkpoint stats are internally consistent:
    //   - sum(per_target.dispatched) == top.dispatched
    //   - sum(per_target.in_flight) == top.in_flight
    //   - per_target.generators == per_target.per_generator.len()
    for seed in [1u64, 7, 31, 999] {
        let mut lcg = Lcg::new(seed);
        let mut r: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
        let mut targets = Vec::new();
        for _ in 0..3 {
            let id = r.add_target(TargetLimits::default()).unwrap();
            targets.push(id);
        }
        for _ in 0..6 {
            let t = targets[lcg.pick(targets.len())];
            let n = (lcg.next_u64() % 4) as usize + 1;
            let steps: Vec<Step> = (0..n)
                .map(|i| Step::Success(vec![FakeEvent::new(i as u64)]))
                .collect();
            r.add_generator(t, Box::new(FakeGenerator::new(steps)), GeneratorConfig::default())
                .unwrap();
        }
        let h = Harness::new();
        for _ in 0..40 {
            let mut fut = r.next();
            if let Poll::Ready(RuntimeOutput::Shutdown) = h.poll(&mut fut) {
                break;
            }
            let s = r.stats();
            let tot_disp: u64 = s.per_target.iter().map(|t| t.dispatched).sum();
            let tot_inf: u64 = s.per_target.iter().map(|t| t.in_flight).sum();
            assert_eq!(s.dispatched, tot_disp, "seed={}: dispatched must match sum of per-target", seed);
            assert_eq!(s.in_flight, tot_inf, "seed={}: in_flight must match sum of per-target", seed);
            for t in &s.per_target {
                assert_eq!(
                    t.generators as usize,
                    t.per_generator.len(),
                    "seed={}: per_target.generators count must match per_generator length",
                    seed
                );
            }
        }
    }
}
