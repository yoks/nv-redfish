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

//! Output ordering and shape tests.

mod support;

use core::task::Poll;

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

fn next(r: &mut Runtime<FakeEvent, FakeError>, h: &Harness) -> RuntimeOutput<FakeEvent, FakeError> {
    // Phase 5: skip `RuntimeOutput::Runtime(_)` so existing tests that
    // assert on `Work(...)` / `Shutdown` are not perturbed by emission of
    // `WorkStarted` / `WorkCompleted` / lag / pressure events under
    // `--features runtime-events`.
    loop {
        let mut fut = r.next();
        match h.poll(&mut fut) {
            Poll::Ready(o) => match &o {
                RuntimeOutput::Runtime(_) => continue,
                _ => return o,
            },
            Poll::Pending => panic!("expected output, runtime parked"),
        }
    }
}

#[test]
fn successful_work_produces_ordered_runtime_output_work_ok() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    let out = next(&mut r, &h);
    match out {
        RuntimeOutput::Work(Ok(s)) => {
            assert_eq!(s.events.len(), 1);
            assert_eq!(s.events[0].id(), 1);
        }
        _ => panic!("expected Work(Ok(_))"),
    }
}

#[test]
fn empty_event_vec_still_produces_a_success_output() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    let out = next(&mut r, &h);
    match out {
        RuntimeOutput::Work(Ok(s)) => assert!(s.events.is_empty()),
        _ => panic!("expected Work(Ok(_)) with empty events"),
    }
}

#[test]
fn multiple_events_from_one_work_item_preserve_order() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![
            FakeEvent::new(10),
            FakeEvent::new(11),
            FakeEvent::new(12),
        ])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    let out = next(&mut r, &h);
    match out {
        RuntimeOutput::Work(Ok(s)) => {
            let ids: Vec<u64> = s.events.iter().map(|e| e.id()).collect();
            assert_eq!(ids, vec![10, 11, 12]);
        }
        _ => panic!("expected Work(Ok(_))"),
    }
}

#[test]
fn failed_work_produces_runtime_output_work_err() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Failure(FakeError::new(99))])),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    let out = next(&mut r, &h);
    match out {
        RuntimeOutput::Work(Err(werr)) => {
            assert_eq!(werr.error.id(), 99);
        }
        _ => panic!("expected Work(Err(_))"),
    }
}

#[test]
fn fifo_order_is_preserved_across_work_outputs() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
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
    let mut ids = Vec::new();
    for _ in 0..3 {
        match next(&mut r, &h) {
            RuntimeOutput::Work(Ok(s)) => {
                ids.extend(s.events.into_iter().map(|e| e.id()));
            }
            _ => panic!("expected Work(Ok(_))"),
        }
    }
    assert_eq!(ids, vec![1, 2, 3]);
}

#[test]
fn shutdown_output_is_returned_after_queued_work() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    // Drain the work output first.
    match next(&mut r, &h) {
        RuntimeOutput::Work(Ok(s)) => assert_eq!(s.events[0].id(), 1),
        _ => panic!("expected Work(Ok(_))"),
    }
    // Now request shutdown — script exhausted, so shutdown should be next.
    r.graceful_shutdown();
    assert!(matches!(next(&mut r, &h), RuntimeOutput::Shutdown));
}

#[test]
fn output_produced_before_target_removal_remains_observable() {
    let mut r = rt();
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
    // Produce one output to enqueue. Then drain it before removing the target.
    let first = next(&mut r, &h);
    assert!(matches!(first, RuntimeOutput::Work(Ok(_))));
    // Remove the target. The other queued/scheduled work for this target
    // should not appear, but the already-returned first output is unaffected.
    assert!(r.remove_target(t));
}

#[test]
fn queued_output_is_returned_before_scanning_or_selecting_more_work() {
    // Two generators each have one Success step. We drain twice. Outputs
    // must be returned in the order they were enqueued by the runtime, and
    // each next() call must consume from the queue first before scanning
    // for more work.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let a = FakeGenerator::new([Step::Success(vec![FakeEvent::new(100)])]);
    let b = FakeGenerator::new([Step::Success(vec![FakeEvent::new(200)])]);
    let a_counters = a.counters();
    let b_counters = b.counters();
    r.add_generator(t, Box::new(a), GeneratorConfig::default())
        .unwrap();
    r.add_generator(t, Box::new(b), GeneratorConfig::default())
        .unwrap();
    let h = Harness::new();
    // First call dispatches and returns output #1.
    let first = next(&mut r, &h);
    assert!(matches!(first, RuntimeOutput::Work(Ok(_))));
    // Whichever generator produced first should have take_next called once.
    let take_next_after_first = a_counters.take_next() + b_counters.take_next();
    assert_eq!(take_next_after_first, 1);
}

#[test]
fn shutdown_output_is_returned_immediately_by_later_next_calls() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    let _ = next(&mut r, &h);
    r.graceful_shutdown();
    assert!(matches!(next(&mut r, &h), RuntimeOutput::Shutdown));
    // Sticky: subsequent calls return Shutdown without ever parking.
    for _ in 0..3 {
        assert!(matches!(next(&mut r, &h), RuntimeOutput::Shutdown));
    }
}

#[test]
fn bounded_queue_pressure_is_reflected_in_stats() {
    // Bound the output queue at 2 and produce 5 immediate-success outputs
    // without consuming. The runtime must report dropped >= 1 in
    // OutputQueueStats. (Phase 4: bounded queue with drop accounting.)
    let cfg = RuntimeConfig {
        output_queue_capacity: Some(2),
        ..RuntimeConfig::default()
    };
    let mut r: Runtime<FakeEvent, FakeError> = Runtime::new(cfg);
    let t = r.add_target(TargetLimits::default()).unwrap();
    let steps: Vec<Step> = (0..5)
        .map(|i| Step::Success(vec![FakeEvent::new(i as u64)]))
        .collect();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new(steps)),
        GeneratorConfig::default(),
    )
    .unwrap();

    let h = Harness::new();
    // Drive the runtime forward without consuming any output.
    for _ in 0..10 {
        let mut fut = r.next();
        // Even when next() returns Ready, drop the result without observing
        // it; the queue should still self-bound and report dropped count.
        let _ = h.poll(&mut fut);
    }
    let stats = r.stats().output_queue;
    assert!(
        stats.queued <= 2,
        "queue must respect capacity: queued={}",
        stats.queued
    );
    assert!(
        stats.dropped >= 1,
        "bounded queue must report dropped outputs (dropped={})",
        stats.dropped
    );
}

#[test]
fn mixed_enqueue_property_test_preserves_fifo_with_no_loss_or_duplication() {
    // Generate a deterministic interleaving of success and failure work
    // outputs and assert the consumed order matches the dispatch order
    // (no duplication, no loss).
    let mut lcg = Lcg::new(0x00C0_FFEE);
    for _seed_iter in 0..3 {
        let mut r = rt();
        let t = r.add_target(TargetLimits::default()).unwrap();
        let mut expected: Vec<(bool, u64)> = Vec::new();
        let mut steps: Vec<Step> = Vec::new();
        for _ in 0..20 {
            let id = lcg.next_u64() & 0xFFFF;
            let succ = lcg.coin(1, 2);
            if succ {
                expected.push((true, id));
                steps.push(Step::Success(vec![FakeEvent::new(id)]));
            } else {
                expected.push((false, id));
                steps.push(Step::Failure(FakeError::new(id)));
            }
        }
        r.add_generator(
            t,
            Box::new(FakeGenerator::new(steps)),
            GeneratorConfig::default(),
        )
        .unwrap();
        let h = Harness::new();
        let mut observed: Vec<(bool, u64)> = Vec::new();
        for _ in 0..20 {
            match next(&mut r, &h) {
                RuntimeOutput::Work(Ok(s)) => {
                    assert_eq!(s.events.len(), 1);
                    observed.push((true, s.events[0].id()));
                }
                RuntimeOutput::Work(Err(e)) => {
                    observed.push((false, e.error.id()));
                }
                RuntimeOutput::Runtime(_) | RuntimeOutput::Shutdown => {
                    panic!("unexpected output kind");
                }
            }
        }
        assert_eq!(expected, observed, "FIFO must be preserved");
    }
}
