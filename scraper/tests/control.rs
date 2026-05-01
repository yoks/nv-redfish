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

//! Control-plane behavior tests.

mod support;

use core::task::Poll;

use nv_redfish_scraper::AddGeneratorError;
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

#[test]
fn add_target_returns_a_target_id() {
    let r = rt();
    let id = r.add_target(TargetLimits::default());
    assert!(id.is_some());
}

#[test]
fn add_multiple_targets_have_strictly_monotonic_ids() {
    let r = rt();
    let a = r.add_target(TargetLimits::default()).unwrap();
    let b = r.add_target(TargetLimits::default()).unwrap();
    let c = r.add_target(TargetLimits::default()).unwrap();
    assert!(a < b);
    assert!(b < c);
    assert_ne!(a, c);
}

#[test]
fn remove_existing_target_returns_true() {
    let r = rt();
    let id = r.add_target(TargetLimits::default()).unwrap();
    assert!(r.remove_target(id));
}

#[test]
fn remove_missing_target_returns_false() {
    let r = rt();
    let id = r.add_target(TargetLimits::default()).unwrap();
    let _ = r.remove_target(id);
    assert!(!r.remove_target(id));
}

#[test]
fn add_generator_under_missing_target_fails() {
    let r = rt();
    let phantom = {
        let r2 = rt();
        r2.add_target(TargetLimits::default()).unwrap()
    };
    let err = r
        .add_generator(
            phantom,
            Box::new(FakeGenerator::new([])),
            GeneratorConfig::default(),
        )
        .unwrap_err();
    assert_eq!(err, AddGeneratorError::TargetNotFound);
}

#[test]
fn add_generator_under_removed_target_fails() {
    let r = rt();
    let id = r.add_target(TargetLimits::default()).unwrap();
    assert!(r.remove_target(id));
    let err = r
        .add_generator(
            id,
            Box::new(FakeGenerator::new([])),
            GeneratorConfig::default(),
        )
        .unwrap_err();
    assert_eq!(err, AddGeneratorError::TargetNotFound);
}

#[test]
fn remove_generator_returns_false_when_missing() {
    let r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let g = r
        .add_generator(t, Box::new(FakeGenerator::new([])), GeneratorConfig::default())
        .unwrap();
    assert!(r.remove_generator(g));
    assert!(!r.remove_generator(g));
}

#[test]
fn pause_and_resume_target_round_trip() {
    let r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    assert!(r.pause_target(t));
    assert!(r.resume_target(t));
}

#[test]
fn pause_and_resume_generator_round_trip() {
    let r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let g = r
        .add_generator(t, Box::new(FakeGenerator::new([])), GeneratorConfig::default())
        .unwrap();
    assert!(r.pause_generator(g));
    assert!(r.resume_generator(g));
}

#[test]
fn update_target_limits_returns_true_for_known_target() {
    let r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    assert!(r.update_target_limits(
        t,
        TargetLimits {
            max_in_flight: Some(4),
            ..TargetLimits::default()
        }
    ));
}

#[test]
fn removing_target_removes_attached_generators() {
    let r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let g = r
        .add_generator(t, Box::new(FakeGenerator::new([])), GeneratorConfig::default())
        .unwrap();
    assert!(r.remove_target(t));
    // generator no longer exists
    assert!(!r.remove_generator(g));
    let stats = r.stats();
    assert_eq!(stats.targets, 0);
    assert_eq!(stats.generators, 0);
}

#[test]
fn graceful_shutdown_is_idempotent() {
    let r = rt();
    r.graceful_shutdown();
    r.graceful_shutdown(); // second call is no-op, no panic
}

#[test]
fn graceful_shutdown_with_no_targets_emits_shutdown_output() {
    let mut r = rt();
    r.graceful_shutdown();
    let h = Harness::new();
    let mut next = r.next();
    let polled = h.poll(&mut next);
    let out = match polled {
        Poll::Ready(o) => o,
        Poll::Pending => panic!("expected shutdown output, got pending"),
    };
    assert!(matches!(out, RuntimeOutput::Shutdown));
}

#[test]
fn graceful_shutdown_rejects_later_mutating_control_operations() {
    let r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.graceful_shutdown();

    // Adding a target after shutdown returns None.
    assert!(r.add_target(TargetLimits::default()).is_none());

    // Adding a generator after shutdown returns ShutdownStarted.
    let err = r
        .add_generator(t, Box::new(FakeGenerator::new([])), GeneratorConfig::default())
        .unwrap_err();
    assert_eq!(err, AddGeneratorError::ShutdownStarted);
}

#[test]
fn shutdown_is_sticky_across_repeated_next_calls() {
    let mut r = rt();
    r.graceful_shutdown();
    let h = Harness::new();

    {
        let mut next = r.next();
        assert!(matches!(h.poll(&mut next), Poll::Ready(RuntimeOutput::Shutdown)));
    }
    {
        let mut next = r.next();
        assert!(matches!(h.poll(&mut next), Poll::Ready(RuntimeOutput::Shutdown)));
    }
    {
        let mut next = r.next();
        assert!(matches!(h.poll(&mut next), Poll::Ready(RuntimeOutput::Shutdown)));
    }
}

#[test]
fn graceful_shutdown_drains_already_queued_outputs_before_shutdown() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let _g = r
        .add_generator(
            t,
            Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])),
            GeneratorConfig::default(),
        )
        .unwrap();

    let h = Harness::new();

    // Step 1: produce one work output. Phase 5: skip transparent runtime
    // events (`WorkStarted`, `WorkCompleted`, ...) that may interleave.
    {
        let out = loop {
            let mut next = r.next();
            match h.poll(&mut next) {
                Poll::Ready(RuntimeOutput::Runtime(_)) => continue,
                Poll::Ready(o) => break o,
                Poll::Pending => panic!("first next() should produce a work output"),
            }
        };
        match out {
            RuntimeOutput::Work(Ok(s)) => {
                assert_eq!(s.events.len(), 1);
                assert_eq!(s.events[0].id(), 1);
            }
            _ => panic!("expected work output"),
        }
    }

    // Step 2: request shutdown. Generator script is exhausted, so no further
    // dispatch occurs. Shutdown output should be observable (after any
    // residual runtime events have drained).
    r.graceful_shutdown();
    let mut got_shutdown = false;
    for _ in 0..8 {
        let mut next = r.next();
        match h.poll(&mut next) {
            Poll::Ready(RuntimeOutput::Runtime(_)) => continue,
            Poll::Ready(RuntimeOutput::Shutdown) => {
                got_shutdown = true;
                break;
            }
            Poll::Ready(other) => panic!(
                "expected Shutdown, got {:?}",
                std::mem::discriminant(&other)
            ),
            Poll::Pending => panic!("runtime parked instead of emitting shutdown"),
        }
    }
    assert!(got_shutdown, "shutdown output never observed");
}

#[test]
fn pending_next_wakes_after_control_plane_change() {
    // 1. Empty runtime: next() must park (Pending).
    // 2. add_target+add_generator should wake the parked task.
    let mut r = rt();
    let h = Harness::new();
    {
        let mut next = r.next();
        let polled = h.poll(&mut next);
        assert!(matches!(polled, Poll::Pending), "expected Pending");
    }
    let pre = h.wakes();

    // Control plane mutation via the handle.
    let t = r.add_target(TargetLimits::default()).unwrap();
    let _g = r
        .add_generator(
            t,
            Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(42)])])),
            GeneratorConfig::default(),
        )
        .unwrap();

    let post = h.wakes();
    assert!(
        post > pre,
        "control-plane changes did not wake parked next() (pre={}, post={})",
        pre,
        post
    );

    // Polling again should now produce a work output. Phase 5: skip
    // transparent runtime events that may interleave.
    let out = loop {
        let mut next = r.next();
        match h.poll(&mut next) {
            Poll::Ready(RuntimeOutput::Runtime(_)) => continue,
            Poll::Ready(o) => break o,
            Poll::Pending => panic!("expected work output after wake"),
        }
    };
    match out {
        RuntimeOutput::Work(Ok(s)) => {
            assert_eq!(s.events.len(), 1);
            assert_eq!(s.events[0].id(), 42);
        }
        _ => panic!("expected work output"),
    }
}

#[test]
fn removed_generator_is_never_queried_again() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let gen = FakeGenerator::new([
        Step::Success(vec![FakeEvent::new(1)]),
        Step::Success(vec![FakeEvent::new(2)]),
    ]);
    let counters = gen.counters();
    let g = r
        .add_generator(t, Box::new(gen), GeneratorConfig::default())
        .unwrap();
    assert!(r.remove_generator(g));

    let h = Harness::new();
    // After removal next() should never call update_ready/take_next on the
    // removed generator; so any progress only comes from queued outputs.
    {
        let mut next = r.next();
        let _ = h.poll(&mut next);
    }

    // Counters should still be at zero.
    assert_eq!(counters.update_ready(), 0);
    assert_eq!(counters.take_next(), 0);
    assert_eq!(counters.on_complete_total(), 0);
}

#[test]
fn removing_target_removes_attached_generators_in_deterministic_order() {
    // Add generators in a known order under a single target. After
    // remove_target the runtime must remove them in insertion order; the
    // public side-effect we can observe is that subsequent re-adds get ids
    // larger than any prior id (no slot reuse).
    let r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let g1 = r
        .add_generator(t, Box::new(FakeGenerator::new([])), GeneratorConfig::default())
        .unwrap();
    let g2 = r
        .add_generator(t, Box::new(FakeGenerator::new([])), GeneratorConfig::default())
        .unwrap();
    let g3 = r
        .add_generator(t, Box::new(FakeGenerator::new([])), GeneratorConfig::default())
        .unwrap();
    assert!(g1 < g2);
    assert!(g2 < g3);
    assert!(r.remove_target(t));

    // Add a fresh target and one generator; its id MUST NOT clash with any
    // of the removed ones.
    let t2 = r.add_target(TargetLimits::default()).unwrap();
    let g4 = r
        .add_generator(t2, Box::new(FakeGenerator::new([])), GeneratorConfig::default())
        .unwrap();
    assert!(t2 > t);
    // Generator id is target-scoped; check it does not collide via formatted
    // representation (Display is intentional).
    assert_ne!(format!("{}", g4), format!("{}", g1));
    assert_ne!(format!("{}", g4), format!("{}", g2));
    assert_ne!(format!("{}", g4), format!("{}", g3));
}

#[test]
fn queued_outputs_survive_target_or_generator_removal() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(7)])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    // Phase 5: skip transparent runtime events.
    let out = loop {
        let mut fut = r.next();
        match h.poll(&mut fut) {
            Poll::Ready(RuntimeOutput::Runtime(_)) => continue,
            Poll::Ready(o) => break o,
            Poll::Pending => panic!("expected work"),
        }
    };
    match out {
        RuntimeOutput::Work(Ok(s)) => assert_eq!(s.events[0].id(), 7),
        _ => panic!("expected work"),
    }
    assert!(r.remove_target(t));
    // Even after removal, a subsequent next() must not crash; it parks.
    // Phase 5: drain residual transparent runtime events first.
    loop {
        let mut fut = r.next();
        match h.poll(&mut fut) {
            Poll::Ready(RuntimeOutput::Runtime(_)) => continue,
            Poll::Pending => break,
            Poll::Ready(other) => panic!(
                "expected Pending, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }
}

#[test]
fn graceful_shutdown_drains_already_selected_or_in_flight_work() {
    // Phase 1: start a Trigger-controlled work item, request shutdown,
    // then fire the trigger. The runtime must deliver the work output
    // before the sticky Shutdown output.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let trig = Trigger::new();
    let g = ControlledGen::new(trig.clone(), Ok(vec![FakeEvent::new(1)]));
    r.add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();
    let h = Harness::new();
    // Phase 5: drain transparent runtime events first; the controlled
    // future itself should remain pending.
    loop {
        let mut fut = r.next();
        match h.poll(&mut fut) {
            Poll::Ready(RuntimeOutput::Runtime(_)) => continue,
            Poll::Pending => break,
            Poll::Ready(other) => panic!(
                "expected Pending, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }
    r.graceful_shutdown();
    trig.fire();
    let mut got_work = false;
    let mut got_shutdown = false;
    for _ in 0..8 {
        let mut fut = r.next();
        match h.poll(&mut fut) {
            Poll::Ready(RuntimeOutput::Work(Ok(_))) => got_work = true,
            Poll::Ready(RuntimeOutput::Shutdown) => {
                got_shutdown = true;
                break;
            }
            Poll::Ready(_) | Poll::Pending => {}
        }
    }
    assert!(
        got_work,
        "graceful shutdown must drain in-flight work before Shutdown"
    );
    assert!(got_shutdown, "Shutdown must eventually be delivered");
}

#[test]
fn graceful_shutdown_drains_then_blocks_new_control_plane_mutations() {
    // Phase 5: graceful_shutdown drains existing ready work but **rejects**
    // new control-plane mutations (add_target / add_generator) once
    // shutdown has started.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let g = FakeGenerator::new([
        Step::Success(vec![FakeEvent::new(1)]),
        Step::Success(vec![FakeEvent::new(2)]),
    ]);
    r.add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();
    r.graceful_shutdown();

    // New mutations after shutdown are rejected.
    assert!(r.add_target(TargetLimits::default()).is_none());
    let extra = FakeGenerator::new([Step::Success(vec![FakeEvent::new(99)])]);
    assert!(r
        .add_generator(t, Box::new(extra), GeneratorConfig::default())
        .is_err());

    // Pre-shutdown work still drains.
    let h = Harness::new();
    let mut work_count = 0u64;
    let mut got_shutdown = false;
    for _ in 0..32 {
        let mut fut = r.next();
        match h.poll(&mut fut) {
            Poll::Ready(RuntimeOutput::Work(_)) => work_count += 1,
            Poll::Ready(RuntimeOutput::Shutdown) => {
                got_shutdown = true;
                break;
            }
            Poll::Ready(_) | Poll::Pending => {}
        }
    }
    assert_eq!(work_count, 2);
    assert!(got_shutdown);
}

#[test]
fn removing_a_generator_while_work_is_in_flight_does_not_cancel_that_work() {
    // Phase 1: Start a Trigger-controlled work item, then remove the
    // generator. The in-flight work must still complete and produce output.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let trig = Trigger::new();
    let g = ControlledGen::new(trig.clone(), Ok(vec![FakeEvent::new(1)]));
    let gid = r
        .add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();
    let h = Harness::new();
    // Phase 5: drain transparent runtime events first.
    loop {
        let mut fut = r.next();
        match h.poll(&mut fut) {
            Poll::Ready(RuntimeOutput::Runtime(_)) => continue,
            Poll::Pending => break,
            Poll::Ready(other) => panic!(
                "expected Pending, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }
    assert!(r.remove_generator(gid));
    trig.fire();
    let mut got_work = false;
    for _ in 0..8 {
        let mut fut = r.next();
        match h.poll(&mut fut) {
            Poll::Ready(RuntimeOutput::Work(Ok(_))) => {
                got_work = true;
                break;
            }
            Poll::Ready(_) | Poll::Pending => {}
        }
    }
    assert!(got_work, "in-flight work was cancelled by generator removal");
}

#[test]
fn removing_a_target_while_child_work_is_in_flight_waits_for_completion() {
    // Phase 1: Same as the previous test but at the target level.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let trig = Trigger::new();
    let g = ControlledGen::new(trig.clone(), Ok(vec![FakeEvent::new(1)]));
    r.add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();
    let h = Harness::new();
    // Phase 5: drain transparent runtime events first.
    loop {
        let mut fut = r.next();
        match h.poll(&mut fut) {
            Poll::Ready(RuntimeOutput::Runtime(_)) => continue,
            Poll::Pending => break,
            Poll::Ready(other) => panic!(
                "expected Pending, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }
    assert!(r.remove_target(t));
    trig.fire();
    let mut got_work = false;
    for _ in 0..8 {
        let mut fut = r.next();
        match h.poll(&mut fut) {
            Poll::Ready(RuntimeOutput::Work(Ok(_))) => {
                got_work = true;
                break;
            }
            Poll::Ready(_) | Poll::Pending => {}
        }
    }
    assert!(
        got_work,
        "in-flight work was cancelled by target removal"
    );
}

#[test]
fn control_plane_changes_that_cannot_make_progress_do_not_cause_busy_polling() {
    // Pause the only generator and drive next(); the runtime must NOT
    // wake itself or busy-poll. Subsequent Pending polls observe no
    // additional wakes beyond the initial register.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let g = r
        .add_generator(
            t,
            Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])),
            GeneratorConfig::default(),
        )
        .unwrap();
    assert!(r.pause_generator(g));
    let h = Harness::new();
    {
        let mut fut = r.next();
        assert!(matches!(h.poll(&mut fut), Poll::Pending));
    }
    let pre = h.wakes();
    // Resume + immediately pause: net no progress, but each call wakes the
    // task at most once. We don't assert "exactly zero" because resume
    // legitimately wakes; we assert "bounded".
    assert!(r.resume_generator(g));
    assert!(r.pause_generator(g));
    let post = h.wakes();
    assert!(
        post - pre <= 2,
        "control-plane churn caused busy wake (pre={}, post={})",
        pre,
        post
    );
}
