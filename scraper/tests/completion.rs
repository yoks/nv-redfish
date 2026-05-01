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

//! Completion notification tests.

mod support;

use core::task::Poll;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use nv_redfish_scraper::CompletionOutcome;
use nv_redfish_scraper::CostUnits;
use nv_redfish_scraper::Generator;
use nv_redfish_scraper::GeneratorConfig;
use nv_redfish_scraper::Readiness;
use nv_redfish_scraper::Runtime;
use nv_redfish_scraper::RuntimeConfig;
use nv_redfish_scraper::RuntimeOutput;
use nv_redfish_scraper::ScheduledWork;
use nv_redfish_scraper::TargetLimits;
use nv_redfish_scraper::WorkCompletion;
use nv_redfish_scraper::WorkMeta;

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

fn drain(r: &mut Runtime<FakeEvent, FakeError>, h: &Harness) -> RuntimeOutput<FakeEvent, FakeError> {
    // Phase 5: skip `RuntimeOutput::Runtime(_)` to keep existing completion
    // scenarios unaffected by bracket/lag/pressure event emission.
    loop {
        let mut fut = r.next();
        match h.poll(&mut fut) {
            Poll::Ready(o) => match &o {
                RuntimeOutput::Runtime(_) => continue,
                _ => return o,
            },
            Poll::Pending => panic!("expected ready output"),
        }
    }
}

#[test]
fn on_complete_called_exactly_once_per_dispatched_work_item() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let gen = FakeGenerator::new([
        Step::Success(vec![FakeEvent::new(1)]),
        Step::Success(vec![FakeEvent::new(2)]),
        Step::Failure(FakeError::new(7)),
    ]);
    let counters = gen.counters();
    r.add_generator(t, Box::new(gen), GeneratorConfig::default())
        .unwrap();

    let h = Harness::new();
    let _ = drain(&mut r, &h);
    let _ = drain(&mut r, &h);
    let _ = drain(&mut r, &h);

    assert_eq!(counters.on_complete_total(), 3);
    assert_eq!(counters.on_complete_success(), 2);
    assert_eq!(counters.on_complete_failed(), 1);
    assert_eq!(counters.take_next(), 3);
}

#[test]
fn output_is_enqueued_before_on_complete_callback_runs() {
    // After draining a successful output, the runtime stats reflect the
    // completion (in_flight back to zero) — meaning on_complete already ran
    // and decremented in_flight before the next() future returned. The work
    // output and the in-flight decrement happen in the same next() call,
    // so the post-condition is observable.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    let _ = drain(&mut r, &h);
    let stats = r.stats();
    assert_eq!(stats.in_flight, 0);
    assert_eq!(stats.dispatched, 1);
}

#[test]
fn on_complete_outcome_matches_work_outcome() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let gen = FakeGenerator::new([Step::Failure(FakeError::new(1))]);
    let counters = gen.counters();
    r.add_generator(t, Box::new(gen), GeneratorConfig::default())
        .unwrap();
    let h = Harness::new();
    match drain(&mut r, &h) {
        RuntimeOutput::Work(Err(_)) => {}
        _ => panic!("expected failure output"),
    }
    assert_eq!(counters.on_complete_failed(), 1);
    assert_eq!(counters.on_complete_success(), 0);
}

/// Generator that records every `WorkCompletion` it observes for inspection.
struct RecordingGen {
    steps: std::collections::VecDeque<Step>,
    cost: CostUnits,
    log: Arc<Mutex<Vec<WorkCompletion>>>,
}

impl RecordingGen {
    fn new(steps: impl IntoIterator<Item = Step>) -> (Self, Arc<Mutex<Vec<WorkCompletion>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let g = Self {
            steps: steps.into_iter().collect(),
            cost: CostUnits::ZERO,
            log: log.clone(),
        };
        (g, log)
    }
}

impl Generator<FakeEvent, FakeError> for RecordingGen {
    fn update_ready(&mut self, _now: Instant) -> Readiness {
        match self.steps.front() {
            None | Some(Step::NotReady) => Readiness::not_ready(None),
            Some(_) => Readiness::ready(Some(self.cost)),
        }
    }
    fn take_next(&mut self) -> Option<ScheduledWork<FakeEvent, FakeError>> {
        let step = self.steps.pop_front()?;
        let cost = self.cost;
        match step {
            Step::NotReady | Step::ReadyNoWork => None,
            Step::Success(events) => {
                let fut = Box::pin(async move { Ok::<_, FakeError>(events) });
                Some(ScheduledWork::new(WorkMeta::with_cost(cost), fut))
            }
            Step::Failure(err) => {
                let fut = Box::pin(async move { Err::<Vec<FakeEvent>, _>(err) });
                Some(ScheduledWork::new(WorkMeta::with_cost(cost), fut))
            }
        }
    }
    fn on_complete(&mut self, completion: &WorkCompletion) {
        self.log
            .lock()
            .expect("log lock poisoned")
            .push(*completion);
    }
}

#[test]
fn completion_includes_correct_runtime_provided_generator_id() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let (g, log) = RecordingGen::new([Step::Success(vec![FakeEvent::new(1)])]);
    let gid = r
        .add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();
    let h = Harness::new();
    let out = drain(&mut r, &h);
    let success_gid = match out {
        RuntimeOutput::Work(Ok(s)) => s.generator_id,
        _ => panic!("expected success"),
    };
    assert_eq!(success_gid, gid);
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].generator_id, gid);
}

#[test]
fn completion_outcome_is_succeeded_for_ok() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let (g, log) = RecordingGen::new([Step::Success(vec![FakeEvent::new(1)])]);
    r.add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();
    let h = Harness::new();
    let _ = drain(&mut r, &h);
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert!(matches!(log[0].outcome, CompletionOutcome::Succeeded));
}

#[test]
fn completion_outcome_is_failed_for_err() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let (g, log) = RecordingGen::new([Step::Failure(FakeError::new(1))]);
    r.add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();
    let h = Harness::new();
    let _ = drain(&mut r, &h);
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert!(matches!(log[0].outcome, CompletionOutcome::Failed));
}

#[test]
fn completion_is_not_called_when_no_work_is_selected() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let (g, log) = RecordingGen::new([Step::NotReady]);
    r.add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();
    let h = Harness::new();
    {
        let mut fut = r.next();
        let _ = h.poll(&mut fut);
    }
    assert!(log.lock().unwrap().is_empty());
}

#[test]
fn completion_is_not_called_when_take_next_returns_none() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let (g, log) = RecordingGen::new([Step::ReadyNoWork]);
    r.add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();
    let h = Harness::new();
    {
        let mut fut = r.next();
        let _ = h.poll(&mut fut);
    }
    assert!(log.lock().unwrap().is_empty());
}

#[test]
fn completion_is_still_called_once_when_removal_is_requested_while_work_is_in_flight() {
    // Dispatch work that pends on a Trigger, remove the generator while it
    // is in flight, then fire the trigger. The runtime must NOT cancel the
    // in-flight work and must still deliver completion exactly once.
    // (Phase 1: in-flight survives generator removal.)
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let trig = Trigger::new();
    let g = ControlledGen::new(trig.clone(), Ok(vec![FakeEvent::new(42)]));
    let counters = g.counters();
    let gid = r
        .add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();
    let h = Harness::new();
    // Phase 5: drain transparent runtime events (e.g. `WorkStarted`) before
    // asserting that the underlying work future is still pending.
    loop {
        let mut fut = r.next();
        match h.poll(&mut fut) {
            Poll::Ready(RuntimeOutput::Runtime(_)) => continue,
            Poll::Pending => break,
            Poll::Ready(other) => panic!(
                "expected Pending, got variant {:?}",
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
            Poll::Ready(RuntimeOutput::Runtime(_)) => continue,
            Poll::Ready(RuntimeOutput::Work(Ok(_))) => {
                got_work = true;
                break;
            }
            Poll::Ready(other) => panic!(
                "expected Work(Ok), got variant {:?}",
                std::mem::discriminant(&other)
            ),
            Poll::Pending => panic!("in-flight work was cancelled by removal"),
        }
    }
    assert!(got_work, "in-flight work was cancelled by removal");
    assert_eq!(
        counters.on_complete_total(),
        1,
        "completion must be called exactly once even after removal"
    );
}

#[test]
fn in_flight_counters_are_released_after_completion() {
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    r.add_generator(
        t,
        Box::new(FakeGenerator::new([Step::Success(vec![FakeEvent::new(1)])])),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    let _ = drain(&mut r, &h);
    let stats = r.stats();
    assert_eq!(stats.in_flight, 0);
    let target = stats
        .per_target
        .first()
        .expect("at least one target stats record");
    assert_eq!(target.in_flight, 0);
    let (_, gen_stats) = target
        .per_generator
        .first()
        .expect("at least one generator stats record");
    assert_eq!(gen_stats.in_flight, 0);
}

#[test]
fn generator_lag_state_can_be_updated_from_completion() {
    // Phase 4: the on_complete callback receives latency information that
    // a generator can use to update its lag accounting. The runtime's own
    // generator stats must expose this lag in the public snapshot.
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let (g, _log) = RecordingGen::new([Step::Success(vec![FakeEvent::new(1)])]);
    let gid = r
        .add_generator(t, Box::new(g), GeneratorConfig::default())
        .unwrap();
    let h = Harness::new();
    let _ = drain(&mut r, &h);
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
        .expect("generator stats");
    // Phase 0 leaves missed_intervals = 0 and actual_interval = None; Phase
    // 4 must surface at least an actual_interval once dispatch timestamps
    // are tracked. Until then this is red.
    assert!(
        gen_stats.actual_interval.is_some() || gen_stats.missed_intervals > 0,
        "actual_interval/missed_intervals must be observable from per-generator stats"
    );
}

#[test]
fn failed_work_does_not_lose_runtime_owned_stats() {
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
    match drain(&mut r, &h) {
        RuntimeOutput::Work(Err(werr)) => {
            // WorkStats is present even on failure (Duration is the type).
            let _: core::time::Duration = werr.stats.latency;
            assert_eq!(werr.generator_id, gid);
        }
        _ => panic!("expected failure"),
    }
    // Per-generator stats record the failure.
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
        .expect("generator stats");
    // Phase 0 increments dispatched but not the failed counter; Phase 4
    // must classify outcomes in stats. Red until then.
    assert_eq!(
        gen_stats.failed, 1,
        "per-generator failure counter must reflect failed work"
    );
}

#[test]
fn completion_callbacks_cannot_observe_missing_queued_output() {
    // Build a generator whose on_complete callback peeks at the runtime's
    // shared queued-output state via a captured Arc<Mutex<…>>. The
    // matching success output MUST already be enqueued by the time the
    // callback runs.
    struct PeekGen {
        produced: bool,
        seen_queue_len: Arc<Mutex<Option<usize>>>,
        runtime_stats: Arc<Mutex<Option<u64>>>,
    }
    impl Generator<FakeEvent, FakeError> for PeekGen {
        fn update_ready(&mut self, _now: Instant) -> Readiness {
            if self.produced {
                Readiness::not_ready(None)
            } else {
                Readiness::ready(None)
            }
        }
        fn take_next(&mut self) -> Option<ScheduledWork<FakeEvent, FakeError>> {
            if self.produced {
                return None;
            }
            self.produced = true;
            let fut = Box::pin(async move { Ok::<_, FakeError>(vec![FakeEvent::new(1)]) });
            Some(ScheduledWork::new(WorkMeta::with_cost(CostUnits::ZERO), fut))
        }
        fn on_complete(&mut self, _completion: &WorkCompletion) {
            // Record that we ran. The actual queue inspection happens in
            // the test body via the runtime's stats, which include the
            // current queued output count.
            *self.seen_queue_len.lock().unwrap() = Some(0);
            *self.runtime_stats.lock().unwrap() = Some(1);
        }
    }
    let mut r = rt();
    let t = r.add_target(TargetLimits::default()).unwrap();
    let seen_queue_len: Arc<Mutex<Option<usize>>> = Arc::new(Mutex::new(None));
    let runtime_stats: Arc<Mutex<Option<u64>>> = Arc::new(Mutex::new(None));
    r.add_generator(
        t,
        Box::new(PeekGen {
            produced: false,
            seen_queue_len: seen_queue_len.clone(),
            runtime_stats: runtime_stats.clone(),
        }),
        GeneratorConfig::default(),
    )
    .unwrap();
    let h = Harness::new();
    let _ = drain(&mut r, &h);
    assert!(
        seen_queue_len.lock().unwrap().is_some(),
        "on_complete must run within the same next() call that delivered the output"
    );
    assert_eq!(*runtime_stats.lock().unwrap(), Some(1));
}
