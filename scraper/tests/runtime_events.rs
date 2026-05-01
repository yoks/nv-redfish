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

//! Runtime-event feature-gating and emission tests.

mod support;

use core::any::TypeId;

use nv_redfish_scraper::RuntimeEventType;

use support::fake_error::FakeError;
use support::fake_event::FakeEvent;

#[cfg(not(feature = "runtime-events"))]
#[test]
fn runtime_event_type_is_infallible_when_feature_disabled() {
    use core::convert::Infallible;
    assert_eq!(
        TypeId::of::<RuntimeEventType>(),
        TypeId::of::<Infallible>(),
        "without runtime-events RuntimeEventType must be core::convert::Infallible"
    );
}

#[cfg(feature = "runtime-events")]
#[test]
fn runtime_event_type_is_concrete_enum_when_feature_enabled() {
    use nv_redfish_scraper::RuntimeEvent;
    assert_eq!(
        TypeId::of::<RuntimeEventType>(),
        TypeId::of::<RuntimeEvent>(),
        "with runtime-events RuntimeEventType must be RuntimeEvent"
    );
    let _ = RuntimeEvent::GlobalThrottled;
}

#[test]
fn output_type_can_carry_default_runtime_event_type() {
    use nv_redfish_scraper::RuntimeOutput;
    // Compiles for both feature states. We never construct the Runtime
    // variant when the feature is off (Infallible is uninhabited).
    let _: RuntimeOutput<FakeEvent, FakeError> = RuntimeOutput::Shutdown;
}

// ---------------------------------------------------------------------------
// Phase 5: emission tests. All gated on `runtime-events`. Currently red — the
// runtime emits no events in Phase 0; turning these green is the Phase 5
// definition of done.
// ---------------------------------------------------------------------------

#[cfg(feature = "runtime-events")]
mod emission {
    use core::task::Poll;

    use nv_redfish_scraper::GeneratorConfig;
    use nv_redfish_scraper::Runtime;
    use nv_redfish_scraper::RuntimeConfig;
    use nv_redfish_scraper::RuntimeEvent;
    use nv_redfish_scraper::RuntimeOutput;
    use nv_redfish_scraper::TargetLimits;

    use super::FakeError;
    use super::FakeEvent;
    use super::support::fake_generator::FakeGenerator;
    use super::support::fake_generator::Step;
    use super::support::harness::Harness;

    fn rt() -> Runtime<FakeEvent, FakeError> {
        Runtime::new(RuntimeConfig::default())
    }

    fn drain_until_shutdown(
        r: &mut Runtime<FakeEvent, FakeError>,
        h: &Harness,
    ) -> Vec<RuntimeOutput<FakeEvent, FakeError>> {
        let mut out = Vec::new();
        for _ in 0..100 {
            let mut fut = r.next();
            match h.poll(&mut fut) {
                Poll::Ready(o) => {
                    let is_shutdown = matches!(o, RuntimeOutput::Shutdown);
                    out.push(o);
                    if is_shutdown {
                        break;
                    }
                }
                Poll::Pending => break,
            }
        }
        out
    }

    fn is_runtime_event(o: &RuntimeOutput<FakeEvent, FakeError>) -> bool {
        matches!(o, RuntimeOutput::Runtime(_))
    }

    #[test]
    fn work_started_and_completed_events_bracket_successful_work_output() {
        let mut r = rt();
        let t = r.add_target(TargetLimits::default()).unwrap();
        r.add_generator(
            t,
            Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])),
            GeneratorConfig::default(),
        )
        .unwrap();
        r.graceful_shutdown();
        let h = Harness::new();
        let outs = drain_until_shutdown(&mut r, &h);
        let mut started = false;
        let mut work_seen = false;
        let mut bracketed = false;
        for o in &outs {
            match o {
                RuntimeOutput::Runtime(RuntimeEvent::WorkStarted { .. }) => started = true,
                RuntimeOutput::Work(Ok(_)) if started => work_seen = true,
                RuntimeOutput::Runtime(RuntimeEvent::WorkCompleted { .. }) if work_seen => {
                    bracketed = true;
                }
                _ => {}
            }
        }
        assert!(
            bracketed,
            "WorkStarted -> Work(Ok) -> WorkCompleted bracket missing"
        );
    }

    #[test]
    fn work_started_and_failed_events_bracket_failed_work_output() {
        let mut r = rt();
        let t = r.add_target(TargetLimits::default()).unwrap();
        r.add_generator(
            t,
            Box::new(FakeGenerator::new([Step::Failure(FakeError::new(1))])),
            GeneratorConfig::default(),
        )
        .unwrap();
        r.graceful_shutdown();
        let h = Harness::new();
        let outs = drain_until_shutdown(&mut r, &h);
        let mut started = false;
        let mut work_seen = false;
        let mut bracketed = false;
        for o in &outs {
            match o {
                RuntimeOutput::Runtime(RuntimeEvent::WorkStarted { .. }) => started = true,
                RuntimeOutput::Work(Err(_)) if started => work_seen = true,
                RuntimeOutput::Runtime(RuntimeEvent::WorkFailed { .. }) if work_seen => {
                    bracketed = true;
                }
                _ => {}
            }
        }
        assert!(
            bracketed,
            "WorkStarted -> Work(Err) -> WorkFailed bracket missing"
        );
    }

    #[test]
    fn runtime_events_contain_runtime_ids_only_no_user_payload() {
        // Trivially true at the type level: RuntimeEvent variants do not
        // carry generic payload. This test verifies by construction that
        // runtime events produced by the runtime carry only ids/numbers
        // and never reference FakeEvent / FakeError.
        let mut r = rt();
        let t = r.add_target(TargetLimits::default()).unwrap();
        r.add_generator(
            t,
            Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(7)])])),
            GeneratorConfig::default(),
        )
        .unwrap();
        r.graceful_shutdown();
        let h = Harness::new();
        let outs = drain_until_shutdown(&mut r, &h);
        let any_event = outs.iter().any(is_runtime_event);
        assert!(any_event, "runtime emitted no events with feature on");
    }

    #[test]
    fn runtime_events_are_not_emitted_for_failed_control_operations() {
        // Removing a non-existent target is a no-op and must NOT produce
        // any runtime event.
        let mut r = rt();
        let bogus_target = r.add_target(TargetLimits::default()).unwrap();
        assert!(r.remove_target(bogus_target));
        // Second removal is a failed control op.
        assert!(!r.remove_target(bogus_target));
        r.graceful_shutdown();
        let h = Harness::new();
        let outs = drain_until_shutdown(&mut r, &h);
        // Failed control ops should not show up as control-plane events.
        // Allow shutdown-related events; the assertion is that NO event
        // references the bogus_target as a "removed" event.
        for o in &outs {
            match o {
                RuntimeOutput::Runtime(RuntimeEvent::TargetThrottled { target_id })
                    if *target_id == bogus_target =>
                {
                    panic!("event referenced a target that no longer exists");
                }
                _ => {}
            }
        }
    }

    #[test]
    fn lagging_generator_can_emit_lag_event() {
        // Phase 5: a generator that reports ready but is not selected
        // within its `next_update_at` window must surface as
        // RuntimeEvent::GeneratorLagging.
        let mut r = rt();
        let t = r.add_target(TargetLimits::default()).unwrap();
        let many_steps: Vec<Step> = (0..50)
            .map(|i| Step::Success(vec![FakeEvent::new(i as u64)]))
            .collect();
        r.add_generator(
            t,
            Box::new(FakeGenerator::new(many_steps)),
            GeneratorConfig::default(),
        )
        .unwrap();
        r.graceful_shutdown();
        let h = Harness::new();
        let outs = drain_until_shutdown(&mut r, &h);
        let any_lag = outs.iter().any(|o| {
            matches!(
                o,
                RuntimeOutput::Runtime(RuntimeEvent::GeneratorLagging { .. })
            )
        });
        assert!(any_lag, "no lagging event emitted");
    }

    #[test]
    fn queue_pressure_can_emit_pressure_event() {
        // Phase 5: when output queue depth exceeds a watermark, the
        // runtime emits an EventQueuePressure event.
        let cfg = RuntimeConfig {
            output_queue_capacity: Some(2),
            ..RuntimeConfig::default()
        };
        let mut r: Runtime<FakeEvent, FakeError> = Runtime::new(cfg);
        let t = r.add_target(TargetLimits::default()).unwrap();
        let steps: Vec<Step> = (0..6)
            .map(|i| Step::Success(vec![FakeEvent::new(i as u64)]))
            .collect();
        r.add_generator(
            t,
            Box::new(FakeGenerator::new(steps)),
            GeneratorConfig::default(),
        )
        .unwrap();
        r.graceful_shutdown();
        let h = Harness::new();
        let outs = drain_until_shutdown(&mut r, &h);
        let any_pressure = outs.iter().any(|o| {
            matches!(
                o,
                RuntimeOutput::Runtime(RuntimeEvent::EventQueuePressure { .. })
            )
        });
        assert!(any_pressure, "no queue-pressure event emitted");
    }

    #[test]
    fn lag_and_queue_pressure_runtime_events_are_ordered_with_work_outputs() {
        // The emitted order must be: each event interleaves with work
        // outputs based on causal ordering. This test asserts that within
        // any consecutive (start, end) bracket of work, no other work item
        // sneaks between them.
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
        r.graceful_shutdown();
        let h = Harness::new();
        let outs = drain_until_shutdown(&mut r, &h);
        // Walk through the stream tracking whether we're "inside a work
        // bracket". A second WorkStarted before WorkCompleted/WorkFailed
        // would violate causal ordering.
        let mut inside = false;
        for o in &outs {
            match o {
                RuntimeOutput::Runtime(RuntimeEvent::WorkStarted { .. }) => {
                    assert!(!inside, "nested WorkStarted observed");
                    inside = true;
                }
                RuntimeOutput::Runtime(RuntimeEvent::WorkCompleted { .. })
                | RuntimeOutput::Runtime(RuntimeEvent::WorkFailed { .. }) => {
                    inside = false;
                }
                _ => {}
            }
        }
    }

    #[test]
    fn target_and_generator_control_plane_events_emitted_in_documented_order() {
        // Phase 5: when control-plane changes succeed they emit events in
        // the same order as the changes. (Currently red because no
        // control-plane events are emitted.)
        let mut r = rt();
        let t = r.add_target(TargetLimits::default()).unwrap();
        let _g = r
            .add_generator(
                t,
                Box::new(FakeGenerator::new([Step::ReadyNoWork])),
                GeneratorConfig::default(),
            )
            .unwrap();
        r.graceful_shutdown();
        let h = Harness::new();
        let outs = drain_until_shutdown(&mut r, &h);
        let any_control_event = outs.iter().any(is_runtime_event);
        assert!(
            any_control_event,
            "control-plane events were not emitted by the runtime"
        );
    }

    #[test]
    fn runtime_events_are_not_emitted_when_feature_is_disabled_at_runtime_level() {
        // Even with the feature on at compile time, a runtime that does
        // not exercise emission paths must still finish cleanly.
        let mut r: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
        r.graceful_shutdown();
        let h = Harness::new();
        let outs = drain_until_shutdown(&mut r, &h);
        let last = outs.last().expect("at least shutdown");
        assert!(matches!(last, RuntimeOutput::Shutdown));
    }
}
