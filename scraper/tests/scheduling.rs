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

//! Scheduler behavior tests.

mod support;

use core::task::Poll;

use nv_redfish_scraper::CostUnits;
use nv_redfish_scraper::GeneratorConfig;
use nv_redfish_scraper::Runtime;
use nv_redfish_scraper::RuntimeConfig;
use nv_redfish_scraper::RuntimeOutput;
use nv_redfish_scraper::TargetLimits;

use support::controlled::ControlledGen;
use support::controlled::Trigger;
use support::fake_error::FakeError;
use support::fake_event::FakeEvent;
use support::fake_generator::FakeGenerator;
use support::fake_generator::Step;
use support::harness::Harness;

fn rt() -> Runtime<FakeEvent, FakeError> {
    Runtime::new(RuntimeConfig::default())
}

fn drain_one(r: &mut Runtime<FakeEvent, FakeError>, h: &Harness) -> Option<RuntimeOutput<FakeEvent, FakeError>> {
    // Phase 5: transparently skip `RuntimeOutput::Runtime(_)` so existing
    // scenarios that match on `Work(...)` / `Shutdown` are unaffected by
    // emission of bracket / lag / pressure events.
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

fn unwrap_event_id(out: RuntimeOutput<FakeEvent, FakeError>) -> u64 {
    match out {
        RuntimeOutput::Work(Ok(s)) => {
            assert_eq!(s.events.len(), 1, "expected exactly one event");
            s.events[0].id()
        }
        RuntimeOutput::Work(Err(_)) => panic!("expected success, got failure"),
        RuntimeOutput::Runtime(_) | RuntimeOutput::Shutdown => panic!("expected work output"),
    }
}

#[test]
fn no_work_when_no_target_is_ready() {
    let mut r = rt();
    let h = Harness::new();
    let mut next = r.next();
    let polled = h.poll(&mut next);
    assert!(matches!(polled, Poll::Pending));
}

#[test]
fn ready_generator_produces_one_work_output() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])),
        GeneratorConfig::default(),
    )
    .unwrap();

    let h = Harness::new();
    let out = drain_one(&mut r, &h).expect("expected work output");
    assert_eq!(unwrap_event_id(out), 1);
}

#[test]
fn next_returns_at_most_one_work_item_per_call() {
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
    assert_eq!(unwrap_event_id(drain_one(&mut r, &h).unwrap()), 1);
    assert_eq!(unwrap_event_id(drain_one(&mut r, &h).unwrap()), 2);
}

#[test]
fn not_ready_generators_are_skipped_without_calling_take_next() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let lazy = FakeGenerator::new([Step::NotReady]);
    let lazy_counters = lazy.counters();
    r.add_generator(t, Box::new(lazy), GeneratorConfig::default())
        .unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(9)])])),
        GeneratorConfig::default(),
    )
    .unwrap();

    let h = Harness::new();
    let out = drain_one(&mut r, &h).expect("expected work");
    assert_eq!(unwrap_event_id(out), 9);

    assert!(lazy_counters.update_ready() >= 1);
    assert_eq!(
        lazy_counters.take_next(),
        0,
        "take_next should not be called on a not-ready generator"
    );
}

#[test]
fn ready_with_no_work_continues_scanning_in_same_next_call() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::ReadyNoWork])),
        GeneratorConfig::default(),
    )
    .unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(7)])])),
        GeneratorConfig::default(),
    )
    .unwrap();

    let h = Harness::new();
    let out = drain_one(&mut r, &h).expect("expected work output in same call");
    assert_eq!(unwrap_event_id(out), 7);
}

#[test]
fn round_robin_cursor_resumes_after_the_generator_that_produced_work() {
    // With three always-ready generators producing distinct ids, observed
    // outputs over three consecutive next() calls should rotate through all
    // three rather than always picking the first-inserted generator.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([
            Step::Success(vec![FakeEvent::new(1)]),
            Step::Success(vec![FakeEvent::new(1)]),
            Step::Success(vec![FakeEvent::new(1)]),
        ])),
        GeneratorConfig::default(),
    )
    .unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([
            Step::Success(vec![FakeEvent::new(2)]),
            Step::Success(vec![FakeEvent::new(2)]),
            Step::Success(vec![FakeEvent::new(2)]),
        ])),
        GeneratorConfig::default(),
    )
    .unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([
            Step::Success(vec![FakeEvent::new(3)]),
            Step::Success(vec![FakeEvent::new(3)]),
            Step::Success(vec![FakeEvent::new(3)]),
        ])),
        GeneratorConfig::default(),
    )
    .unwrap();

    let h = Harness::new();
    let a = unwrap_event_id(drain_one(&mut r, &h).unwrap());
    let b = unwrap_event_id(drain_one(&mut r, &h).unwrap());
    let c = unwrap_event_id(drain_one(&mut r, &h).unwrap());
    let mut seen = vec![a, b, c];
    seen.sort_unstable();
    assert_eq!(
        seen,
        vec![1, 2, 3],
        "round-robin should visit each generator once before repeating, got {:?}",
        [a, b, c]
    );
}

#[test]
fn round_robin_cursor_advances_when_generator_returns_no_work() {
    // Two generators: A returns ReadyNoWork once, B returns Success.
    // The first dispatch should pick B. After A has returned None, the
    // cursor should ensure A is not retried in the next dispatch before B.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([
            Step::ReadyNoWork,
            Step::Success(vec![FakeEvent::new(11)]),
        ])),
        GeneratorConfig::default(),
    )
    .unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([
            Step::Success(vec![FakeEvent::new(22)]),
            Step::Success(vec![FakeEvent::new(22)]),
        ])),
        GeneratorConfig::default(),
    )
    .unwrap();

    let h = Harness::new();
    let first = unwrap_event_id(drain_one(&mut r, &h).unwrap());
    let second = unwrap_event_id(drain_one(&mut r, &h).unwrap());
    assert_eq!(first, 22, "B should be chosen first since A returned None");
    // After B was picked, cursor should resume after B; next scan starts at A.
    // A's next step is Success(11); B's next step is Success(22). Round-robin
    // means A is preferred this round.
    assert_eq!(
        second, 11,
        "round-robin should prefer A this round after B already won"
    );
}

#[test]
fn target_in_flight_limit_is_respected() {
    // Use a target with max_in_flight = 1. With two ready generators behind
    // it, the second never starts until the first completes.
    let limits = TargetLimits {
        max_in_flight: Some(1),
        ..TargetLimits::default()
    };
    let mut r = rt();
    let t = r.add_target(limits).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(2)])])),
        GeneratorConfig::default(),
    )
    .unwrap();

    let h = Harness::new();
    // The first next() call drains the first work to completion within one
    // poll because the future is immediately ready, so the in-flight count
    // returns to zero before the second dispatch is attempted.
    let _ = drain_one(&mut r, &h).expect("first work output");
    let _ = drain_one(&mut r, &h).expect("second work output");

    let stats = r.stats();
    assert_eq!(stats.dispatched, 2);
    assert_eq!(stats.in_flight, 0);
    let target = stats
        .per_target
        .first()
        .expect("at least one target stats record");
    assert_eq!(target.dispatched, 2);
    assert_eq!(target.in_flight, 0);
}

#[test]
fn shutdown_drains_existing_work_then_emits_shutdown() {
    // Phase 5: graceful_shutdown is a *drain* signal. Already-ready work
    // produced by existing generators is still dispatched and observed,
    // and only after the runtime is fully idle does the sticky `Shutdown`
    // output appear.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let g = FakeGenerator::new([
        Step::Success(vec![FakeEvent::new(1)]),
        Step::Success(vec![FakeEvent::new(2)]),
    ]);
    let counters = g.counters();
    r.add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();

    r.graceful_shutdown();

    let h = Harness::new();
    let mut work_count = 0u64;
    let mut got_shutdown = false;
    for _ in 0..32 {
        match drain_one(&mut r, &h) {
            Some(RuntimeOutput::Work(_)) => work_count += 1,
            Some(RuntimeOutput::Shutdown) => {
                got_shutdown = true;
                break;
            }
            Some(_) | None => {}
        }
    }
    assert_eq!(work_count, 2, "drain mode delivered both work outputs");
    assert!(got_shutdown, "Shutdown observed after drain");
    assert!(counters.update_ready() >= 1);
    assert!(counters.take_next() >= 2);
}

#[test]
fn cost_is_carried_through_to_completion() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let g = FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])
        .with_cost(CostUnits::new(7));
    r.add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();

    let h = Harness::new();
    let _ = drain_one(&mut r, &h).expect("work output");
    // Phase 0 records dispatch counts, not per-cost; this test ensures the
    // dispatch path accepts non-zero cost without panicking.
    assert_eq!(r.stats().dispatched, 1);
}

#[test]
fn next_parks_when_no_output_and_no_ready_work() {
    // With a target but no ready generator, next() must park. Subsequent
    // poll without any wake source must continue to park (no busy poll).
    let mut r = rt();
    let _t = r.add_target(TargetLimits::default()).unwrap();
    let h = Harness::new();
    {
        let mut next = r.next();
        assert!(matches!(h.poll(&mut next), Poll::Pending));
    }
    let wakes_before = h.wakes();
    {
        let mut next = r.next();
        assert!(matches!(h.poll(&mut next), Poll::Pending));
    }
    assert_eq!(
        h.wakes(),
        wakes_before,
        "second poll must not have triggered an additional wake"
    );
}

#[test]
fn generator_creates_work_only_after_selection() {
    // Two ready peers: only the selected generator's `take_next` is invoked.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let a = FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])]);
    let b = FakeGenerator::new([Step::Success(vec![FakeEvent::new(2)])]);
    let a_counters = a.counters();
    let b_counters = b.counters();
    r.add_generator(t, Box::new(a), GeneratorConfig::default())
        .unwrap();
    r.add_generator(t, Box::new(b), GeneratorConfig::default())
        .unwrap();

    let h = Harness::new();
    let _ = drain_one(&mut r, &h).expect("work");
    let total = a_counters.take_next() + b_counters.take_next();
    assert_eq!(total, 1, "exactly one generator should have take_next called");
}

#[test]
fn stale_or_removed_scheduler_entries_are_skipped() {
    // Add three generators, remove the middle one before any next() call.
    // The scheduler must skip the removed entry without querying it.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let a = FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])]);
    let b = FakeGenerator::new([Step::Success(vec![FakeEvent::new(2)])]);
    let c = FakeGenerator::new([Step::Success(vec![FakeEvent::new(3)])]);
    let b_counters = b.counters();
    r.add_generator(t, Box::new(a), GeneratorConfig::default())
        .unwrap();
    let bid = r
        .add_generator(t, Box::new(b), GeneratorConfig::default())
        .unwrap();
    r.add_generator(t, Box::new(c), GeneratorConfig::default())
        .unwrap();
    assert!(r.remove_generator(bid));

    let h = Harness::new();
    let _ = drain_one(&mut r, &h).expect("work output");
    let _ = drain_one(&mut r, &h).expect("work output");

    assert_eq!(b_counters.update_ready(), 0);
    assert_eq!(b_counters.take_next(), 0);
}

#[test]
fn round_robin_order_is_deterministic_across_two_full_cycles() {
    // Four always-ready generators each reporting their own id; ten
    // dispatches must visit each generator exactly the expected number of
    // times across two round-robin cycles.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    for id in 1..=4u64 {
        let steps: Vec<Step> = (0..3)
            .map(move |_| Step::Success(vec![FakeEvent::new(id)]))
            .collect();
        r.add_generator(
            t,
            Box::new(FakeGenerator::new(steps)),
            GeneratorConfig::default(),
        )
        .unwrap();
    }

    let h = Harness::new();
    let mut counts = [0u64; 5];
    for _ in 0..8 {
        let id = unwrap_event_id(drain_one(&mut r, &h).expect("work"));
        counts[id as usize] += 1;
    }
    for (id, c) in counts.iter().enumerate().skip(1) {
        assert_eq!(
            *c, 2,
            "generator {} should be visited exactly twice across two RR cycles",
            id
        );
    }
}

#[test]
fn insertion_during_operation_participates_per_documented_semantics() {
    // Add one generator, drain it. Add a second after the first dispatches.
    // The second must be eligible on subsequent next() calls.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    let first = unwrap_event_id(drain_one(&mut r, &h).expect("first"));
    assert_eq!(first, 1);

    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(2)])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    let second = unwrap_event_id(drain_one(&mut r, &h).expect("second"));
    assert_eq!(second, 2);
}

#[test]
fn removal_during_operation_does_not_corrupt_cursor() {
    // Three always-ready generators. Drain one, then remove the next one
    // the cursor would have selected, and ensure dispatching continues with
    // the remaining one.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let g1 = FakeGenerator::new([
        Step::Success(vec![FakeEvent::new(1)]),
        Step::Success(vec![FakeEvent::new(1)]),
    ]);
    let g2 = FakeGenerator::new([
        Step::Success(vec![FakeEvent::new(2)]),
        Step::Success(vec![FakeEvent::new(2)]),
    ]);
    let g3 = FakeGenerator::new([
        Step::Success(vec![FakeEvent::new(3)]),
        Step::Success(vec![FakeEvent::new(3)]),
    ]);
    let id1 = r
        .add_generator(t, Box::new(g1), GeneratorConfig::default())
        .unwrap();
    let id2 = r
        .add_generator(t, Box::new(g2), GeneratorConfig::default())
        .unwrap();
    let id3 = r
        .add_generator(t, Box::new(g3), GeneratorConfig::default())
        .unwrap();

    let h = Harness::new();
    let first = unwrap_event_id(drain_one(&mut r, &h).expect("first"));
    // Remove whichever generator was *not* the one that produced first; the
    // remaining two must still dispatch successfully.
    let to_remove = if first == 1 {
        id2
    } else if first == 2 {
        id3
    } else {
        id1
    };
    assert!(r.remove_generator(to_remove));

    let second = unwrap_event_id(drain_one(&mut r, &h).expect("second"));
    let third = unwrap_event_id(drain_one(&mut r, &h).expect("third"));
    assert_ne!(second, 0, "must keep dispatching after removal");
    assert_ne!(third, 0, "must keep dispatching after removal");
}

#[test]
fn global_in_flight_limit_is_respected() {
    // Use ControlledGen that pends until fired, plus a global cap of 1.
    // After one dispatch the next() call must NOT dispatch a second item.
    let cfg = RuntimeConfig {
        global_max_in_flight: Some(1),
        ..RuntimeConfig::default()
    };
    let mut r: Runtime<FakeEvent, FakeError> = Runtime::new(cfg);
    let t1 = r.add_target(TargetLimits::default()).unwrap();
    let t2 = r.add_target(TargetLimits::default()).unwrap();
    let trig1 = Trigger::new();
    let trig2 = Trigger::new();
    let g1 = ControlledGen::new(trig1.clone(), Ok(vec![FakeEvent::new(1)]));
    let g2 = ControlledGen::new(trig2.clone(), Ok(vec![FakeEvent::new(2)]));
    let g1_counters = g1.counters();
    let g2_counters = g2.counters();
    r.add_generator(t1, Box::new(g1), GeneratorConfig::default())
        .unwrap();
    r.add_generator(t2, Box::new(g2), GeneratorConfig::default())
        .unwrap();

    let h = Harness::new();
    {
        let mut fut = r.next();
        let _ = h.poll(&mut fut);
    }
    // Exactly one generator should have started work.
    let started = g1_counters.take_next() + g2_counters.take_next();
    assert_eq!(started, 1, "global cap should permit only one in-flight item");

    // Polling again without firing must not dispatch a second one.
    {
        let mut fut = r.next();
        let _ = h.poll(&mut fut);
    }
    let started_now = g1_counters.take_next() + g2_counters.take_next();
    assert_eq!(
        started_now, 1,
        "global cap should still only permit one in-flight item"
    );
}

#[test]
fn work_cost_participates_in_admission() {
    // With per-target max_cost_per_round = 5 and a generator producing
    // cost-10 work, cost-aware admission must REJECT the work because cost
    // exceeds the round budget. The current runtime ignores cost and
    // dispatches anyway, so this test is red until Phase 1.
    let mut r = rt();
    let limits = TargetLimits {
        max_cost_per_round: Some(CostUnits::new(5)),
        ..TargetLimits::default()
    };
    let t = r.add_target(limits).unwrap();
    let g = FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])
        .with_cost(CostUnits::new(10));
    let g_counters = g.counters();
    r.add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();

    let h = Harness::new();
    let _ = drain_one(&mut r, &h);
    assert_eq!(
        g_counters.take_next(),
        0,
        "cost-aware admission must reject work whose cost exceeds the round budget"
    );
}

#[test]
fn expensive_work_is_not_permanently_starved() {
    // A cheap (cost 1) generator and an expensive (cost 100) generator share
    // a target with a generous per-round budget but a small per-tick budget.
    // Across many next() calls, the expensive generator must eventually be
    // dispatched at least once. (Phase 1, deficit-style fairness.)
    let mut r = rt();
    let t = r.add_target(TargetLimits {
        max_cost_per_round: Some(CostUnits::new(10)),
        ..TargetLimits::default()
    })
    .unwrap();
    let cheap_steps: Vec<Step> = (0..50)
        .map(|i| Step::Success(vec![FakeEvent::new(i as u64)]))
        .collect();
    let expensive_steps: Vec<Step> = (0..3)
        .map(|i| Step::Success(vec![FakeEvent::new(1000 + i as u64)]))
        .collect();
    let cheap = FakeGenerator::new(cheap_steps).with_cost(CostUnits::new(1));
    let expensive = FakeGenerator::new(expensive_steps).with_cost(CostUnits::new(100));
    let cheap_counters = cheap.counters();
    let expensive_counters = expensive.counters();
    r.add_generator(t, Box::new(cheap), GeneratorConfig::default())
        .unwrap();
    r.add_generator(t, Box::new(expensive), GeneratorConfig::default())
        .unwrap();

    let h = Harness::new();
    for _ in 0..200 {
        let _ = drain_one(&mut r, &h);
    }
    assert!(
        expensive_counters.on_complete_total() >= 1,
        "expensive work was starved across 200 scheduling rounds (cheap completions: {})",
        cheap_counters.on_complete_total()
    );
}

#[test]
fn class_weights_or_service_shares_affect_selection() {
    // Two classes A and B with weights 3 and 1 respectively. Across many
    // dispatches A's share of completions should approach 75%. Phase 2
    // implements weighted selection; until then this fails.
    use nv_redfish_scraper::ClassId;

    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let class_a = ClassId::new("A");
    let class_b = ClassId::new("B");

    let steps_a: Vec<Step> = (0..40)
        .map(|i| Step::Success(vec![FakeEvent::new(i as u64)]))
        .collect();
    let steps_b: Vec<Step> = (0..40)
        .map(|i| Step::Success(vec![FakeEvent::new(1000 + i as u64)]))
        .collect();
    let ga = FakeGenerator::new(steps_a);
    let gb = FakeGenerator::new(steps_b);
    let ga_counters = ga.counters();
    let gb_counters = gb.counters();
    r.add_generator(
        t,
        Box::new(ga),
        GeneratorConfig {
            class: Some(class_a.clone()),
            weight: Some(3),
        },
    )
    .unwrap();
    r.add_generator(
        t,
        Box::new(gb),
        GeneratorConfig {
            class: Some(class_b.clone()),
            weight: Some(1),
        },
    )
    .unwrap();

    let h = Harness::new();
    for _ in 0..40 {
        let _ = drain_one(&mut r, &h);
    }
    let a = ga_counters.on_complete_total() as f64;
    let b = gb_counters.on_complete_total() as f64;
    let total = a + b;
    assert!(total > 0.0, "no work observed");
    let share_a = a / total;
    assert!(
        (0.6..=0.85).contains(&share_a),
        "class A weight=3 vs B weight=1 should yield ~75% share, got {:.2}",
        share_a
    );
}

#[test]
fn target_fairness_prevents_one_target_from_consuming_all_dispatches() {
    // Asymmetric load: target T_heavy has 5 always-ready generators, target
    // T_light has 1. Generator-level RR alone would give T_heavy ~83% of
    // dispatches. True target-level fairness should equalize their share
    // closer to 50/50. (Phase 2.)
    let mut r = rt();
    let t_heavy = r.add_target(TargetLimits::default()).unwrap();
    let t_light = r.add_target(TargetLimits::default()).unwrap();
    let mut heavy_counters = Vec::new();
    for _ in 0..5 {
        let steps: Vec<Step> = (0..40)
            .map(|i| Step::Success(vec![FakeEvent::new(i as u64)]))
            .collect();
        let g = FakeGenerator::new(steps);
        heavy_counters.push(g.counters());
        r.add_generator(t_heavy, Box::new(g), GeneratorConfig::default())
            .unwrap();
    }
    let light_steps: Vec<Step> = (0..40)
        .map(|i| Step::Success(vec![FakeEvent::new(1000 + i as u64)]))
        .collect();
    let light = FakeGenerator::new(light_steps);
    let light_c = light.counters();
    r.add_generator(t_light, Box::new(light), GeneratorConfig::default())
        .unwrap();

    let h = Harness::new();
    for _ in 0..60 {
        let _ = drain_one(&mut r, &h);
    }
    let heavy_total: u64 = heavy_counters.iter().map(|c| c.on_complete_total()).sum();
    let light_total = light_c.on_complete_total();
    let total = heavy_total + light_total;
    assert!(total > 0);
    let light_share = light_total as f64 / total as f64;
    assert!(
        light_share >= 0.30,
        "target fairness: light target only got {:.2} share (heavy={}, light={}); should be ~0.5",
        light_share,
        heavy_total,
        light_total
    );
}

#[test]
fn tree_changes_invalidate_stale_readiness() {
    // A generator returns ready then NotReady. After the runtime has cached
    // ready, removing and re-adding the target should invalidate any stale
    // readiness state. Phase 3 — readiness invalidation.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let g = FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])]);
    r.add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();

    let h = Harness::new();
    let _ = drain_one(&mut r, &h);
    assert!(r.remove_target(t));
    let t2 = r.add_target(TargetLimits::default()).unwrap();
    let g2 = FakeGenerator::new([Step::NotReady]);
    let g2_counters = g2.counters();
    r.add_generator(t2, Box::new(g2), GeneratorConfig::default())
        .unwrap();

    // Phase 5: skip residual transparent runtime events from the prior
    // dispatch+completion before driving the runtime forward so that
    // update_ready on the new generator is exercised.
    for _ in 0..8 {
        let mut fut = r.next();
        let _ = h.poll(&mut fut);
    }
    // The new generator must have been queried for readiness on its own
    // schedule, not satisfied by stale prior ready state.
    assert!(
        g2_counters.update_ready() >= 1,
        "readiness on new tree was not invalidated, update_ready never called"
    );
}

#[test]
fn periodic_generators_do_not_accumulate_one_stale_job_per_missed_interval() {
    // A generator that always reports ready but produces only a single
    // work-stream MUST NOT accumulate one queued job per missed interval.
    // The runtime must dispatch on demand, not in arrears. (Phase 3 / 4.)
    use nv_redfish_scraper::OutputQueueStats;

    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let steps: Vec<Step> = (0..1000)
        .map(|_| Step::Success(vec![FakeEvent::new(1)]))
        .collect();
    let g = FakeGenerator::new(steps);
    r.add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();

    // Do not consume outputs. Drive the runtime via next() but the harness
    // immediately drops each future — the runtime must not silently
    // accumulate hundreds of outputs into the queue.
    let h = Harness::new();
    for _ in 0..200 {
        let mut fut = r.next();
        let _ = h.poll(&mut fut);
    }
    let stats: OutputQueueStats = r.stats().output_queue;
    assert!(
        stats.queued <= 1,
        "periodic overload must not accumulate stale jobs in the queue (queued = {})",
        stats.queued
    );
}
