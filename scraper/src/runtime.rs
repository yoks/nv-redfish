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

//! Generic runtime entry point.

use crate::control::ControlError;
use crate::control::GeneratorConfig;
use crate::control::RuntimeConfig;
use crate::control::RuntimeError;
use crate::control::TargetLimits;
#[cfg(feature = "runtime-events")]
use crate::event::RuntimeEvent;
use crate::generator::CompletionOutcome;
use crate::generator::Generator;
use crate::generator::ScheduledWork;
use crate::generator::WorkCompletion;
use crate::generator::WorkMeta;
use crate::ids::ClassId;
use crate::ids::GeneratorId;
use crate::ids::TargetId;
use crate::output::OutputQueueStats;
use crate::output::RuntimeOutput;
use crate::output::WorkError;
use crate::output::WorkSuccess;
use crate::stats::ClassStats;
use crate::stats::GeneratorStats;
use crate::stats::RuntimeStats;
use crate::stats::TargetStats;
use crate::stats::WorkStats;
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::convert::TryFrom as _;
use std::marker::PhantomData;
use std::time::Duration;
use std::time::Instant;

/// Result of one runtime scheduling pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunOutcome {
    /// No work was ready or admitted.
    Idle,
    /// One work item was dispatched and completed.
    Dispatched,
}

struct GeneratorState<E, Err> {
    generator: Box<dyn Generator<E, Err>>,
    config: GeneratorConfig,
    paused: bool,
    triggered: bool,
    class_id: Option<ClassId>,
    dispatched_count: u64,
    dispatched_cost: u64,
    registered_at: Instant,
    last_dispatch_at: Option<Instant>,
    lag: Option<Duration>,
    missed_intervals: u64,
    actual_interval: Option<Duration>,
}

struct TargetState<E, Err> {
    limits: TargetLimits,
    paused: bool,
    in_flight: usize,
    throttled_count: u64,
    generator_cursor: usize,
    generators: BTreeMap<GeneratorId, GeneratorState<E, Err>>,
}

impl<E, Err> TargetState<E, Err> {
    const fn new(limits: TargetLimits) -> Self {
        Self {
            limits,
            paused: false,
            in_flight: 0,
            throttled_count: 0,
            generator_cursor: 0,
            generators: BTreeMap::new(),
        }
    }
}

/// Handle type reserved for future split control/output usage.
pub struct RuntimeHandle<E, Err> {
    _events: PhantomData<E>,
    _errors: PhantomData<Err>,
}

impl<E, Err> RuntimeHandle<E, Err> {
    const fn new() -> Self {
        Self {
            _events: PhantomData,
            _errors: PhantomData,
        }
    }
}

/// Generic Redfish-independent scraping runtime.
pub struct Runtime<E, Err> {
    config: RuntimeConfig,
    targets: BTreeMap<TargetId, TargetState<E, Err>>,
    outputs: VecDeque<RuntimeOutput<E, Err>>,
    output_dropped: u64,
    output_rejected: u64,
    global_in_flight: usize,
    target_cursor: usize,
}

impl<E, Err> Runtime<E, Err> {
    /// Creates a runtime from configuration.
    #[must_use]
    pub const fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            targets: BTreeMap::new(),
            outputs: VecDeque::new(),
            output_dropped: 0,
            output_rejected: 0,
            global_in_flight: 0,
            target_cursor: 0,
        }
    }

    /// Returns a lightweight runtime handle.
    #[must_use]
    pub const fn handle(&self) -> RuntimeHandle<E, Err> {
        RuntimeHandle::new()
    }

    /// Adds a target to the scheduler tree.
    ///
    /// # Errors
    ///
    /// Returns [`ControlError::TargetAlreadyExists`] when the target id exists.
    pub fn add_target(
        &mut self,
        target_id: TargetId,
        limits: TargetLimits,
    ) -> Result<(), ControlError> {
        if self.targets.contains_key(&target_id) {
            return Err(ControlError::TargetAlreadyExists);
        }

        self.targets.insert(target_id, TargetState::new(limits));
        Ok(())
    }

    /// Removes a target and all attached generators.
    ///
    /// # Errors
    ///
    /// Returns [`ControlError::TargetNotFound`] when the target id is unknown.
    pub fn remove_target(&mut self, target_id: &TargetId) -> Result<(), ControlError> {
        self.targets
            .remove(target_id)
            .map(|_| ())
            .ok_or(ControlError::TargetNotFound)
    }

    /// Updates limits for a target.
    ///
    /// # Errors
    ///
    /// Returns [`ControlError::TargetNotFound`] when the target id is unknown.
    pub fn update_target_limits(
        &mut self,
        target_id: &TargetId,
        limits: TargetLimits,
    ) -> Result<(), ControlError> {
        let target = self
            .targets
            .get_mut(target_id)
            .ok_or(ControlError::TargetNotFound)?;

        target.limits = limits;
        Ok(())
    }

    /// Pauses a target.
    ///
    /// # Errors
    ///
    /// Returns [`ControlError::TargetNotFound`] when the target id is unknown.
    pub fn pause_target(&mut self, target_id: &TargetId) -> Result<(), ControlError> {
        let target = self
            .targets
            .get_mut(target_id)
            .ok_or(ControlError::TargetNotFound)?;

        target.paused = true;
        Ok(())
    }

    /// Resumes a target.
    ///
    /// # Errors
    ///
    /// Returns [`ControlError::TargetNotFound`] when the target id is unknown.
    pub fn resume_target(&mut self, target_id: &TargetId) -> Result<(), ControlError> {
        let target = self
            .targets
            .get_mut(target_id)
            .ok_or(ControlError::TargetNotFound)?;

        target.paused = false;
        Ok(())
    }

    /// Adds a generator under a target.
    ///
    /// # Errors
    ///
    /// Returns target or generator existence errors.
    pub fn add_generator<G>(
        &mut self,
        target_id: &TargetId,
        generator_id: GeneratorId,
        config: GeneratorConfig,
        generator: G,
    ) -> Result<(), ControlError>
    where
        G: Generator<E, Err> + 'static,
    {
        let target = self
            .targets
            .get_mut(target_id)
            .ok_or(ControlError::TargetNotFound)?;

        if target.generators.contains_key(&generator_id) {
            return Err(ControlError::GeneratorAlreadyExists);
        }

        target.generators.insert(
            generator_id,
            GeneratorState {
                generator: Box::new(generator),
                config,
                paused: false,
                triggered: false,
                class_id: None,
                dispatched_count: 0,
                dispatched_cost: 0,
                registered_at: Instant::now(),
                last_dispatch_at: None,
                lag: None,
                missed_intervals: 0,
                actual_interval: None,
            },
        );
        Ok(())
    }

    /// Removes a generator.
    ///
    /// # Errors
    ///
    /// Returns [`ControlError::GeneratorNotFound`] when the id is unknown.
    pub fn remove_generator(&mut self, generator_id: &GeneratorId) -> Result<(), ControlError> {
        self.targets
            .values_mut()
            .find_map(|target| target.generators.remove(generator_id))
            .map(|_| ())
            .ok_or(ControlError::GeneratorNotFound)
    }

    /// Updates generator configuration.
    ///
    /// # Errors
    ///
    /// Returns [`ControlError::GeneratorNotFound`] when the id is unknown.
    pub fn update_generator(
        &mut self,
        generator_id: &GeneratorId,
        config: GeneratorConfig,
    ) -> Result<(), ControlError> {
        let generator = self
            .generator_state_mut(generator_id)
            .ok_or(ControlError::GeneratorNotFound)?;

        generator.config = config;
        Ok(())
    }

    /// Pauses a generator.
    ///
    /// # Errors
    ///
    /// Returns [`ControlError::GeneratorNotFound`] when the id is unknown.
    pub fn pause_generator(&mut self, generator_id: &GeneratorId) -> Result<(), ControlError> {
        let generator = self
            .generator_state_mut(generator_id)
            .ok_or(ControlError::GeneratorNotFound)?;

        generator.paused = true;
        Ok(())
    }

    /// Resumes a generator.
    ///
    /// # Errors
    ///
    /// Returns [`ControlError::GeneratorNotFound`] when the id is unknown.
    pub fn resume_generator(&mut self, generator_id: &GeneratorId) -> Result<(), ControlError> {
        let generator = self
            .generator_state_mut(generator_id)
            .ok_or(ControlError::GeneratorNotFound)?;

        generator.paused = false;
        Ok(())
    }

    /// Triggers a generator for immediate scheduling consideration.
    ///
    /// # Errors
    ///
    /// Returns [`ControlError::GeneratorNotFound`] when the id is unknown.
    pub fn trigger_generator(&mut self, generator_id: &GeneratorId) -> Result<(), ControlError> {
        let generator = self
            .generator_state_mut(generator_id)
            .ok_or(ControlError::GeneratorNotFound)?;

        generator.triggered = true;
        Ok(())
    }

    /// Executes at most one selected work item.
    ///
    /// # Errors
    ///
    /// Returns a runtime error when execution cannot proceed.
    pub async fn run_once(&mut self, now: Instant) -> Result<RunOutcome, RuntimeError> {
        let Some(work) = self.take_next_ready_work(now) else {
            return Ok(RunOutcome::Idle);
        };

        let (meta, future) = work.into_parts();
        #[cfg(feature = "runtime-events")]
        self.enqueue_work_started(meta.generator_id().clone());
        self.global_in_flight += 1;
        if let Some(target) = self.targets.get_mut(meta.target_id()) {
            target.in_flight += 1;
        }

        let result = future.await;
        let stats = WorkStats::new(1, 1);
        let outcome = if result.is_ok() {
            CompletionOutcome::Success
        } else {
            CompletionOutcome::Failure
        };

        match result {
            Ok(events) => {
                self.enqueue_output(RuntimeOutput::Work(Ok(WorkSuccess::new(
                    events,
                    stats.clone(),
                ))));
                #[cfg(feature = "runtime-events")]
                self.enqueue_work_completed(meta.generator_id().clone());
            }
            Err(error) => {
                self.enqueue_output(RuntimeOutput::Work(Err(WorkError::new(
                    error,
                    stats.clone(),
                ))));
                #[cfg(feature = "runtime-events")]
                self.enqueue_work_failed(meta.generator_id().clone());
            }
        }

        self.report_completion(meta, outcome, stats);
        Ok(RunOutcome::Dispatched)
    }

    /// Polls one ordered output item.
    #[must_use]
    pub fn poll_output(&mut self) -> Option<RuntimeOutput<E, Err>> {
        self.outputs.pop_front()
    }

    /// Drains all currently available ordered output items.
    #[must_use]
    pub fn drain_outputs(&mut self) -> Vec<RuntimeOutput<E, Err>> {
        self.outputs.drain(..).collect::<Vec<_>>()
    }

    /// Returns output queue statistics.
    #[must_use]
    pub fn output_queue_stats(&self) -> OutputQueueStats {
        OutputQueueStats::new(
            self.outputs.len(),
            self.output_dropped,
            self.output_rejected,
        )
    }

    /// Returns a runtime statistics snapshot.
    #[must_use]
    pub fn stats(&self) -> RuntimeStats {
        RuntimeStats::with_details(
            self.targets.len(),
            self.generator_count(),
            self.global_in_flight,
            self.outputs.len(),
            self.target_stats(),
            self.class_stats(),
            self.generator_stats(),
        )
    }

    /// Returns the runtime configuration.
    #[must_use]
    pub const fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    fn generator_state_mut(
        &mut self,
        generator_id: &GeneratorId,
    ) -> Option<&mut GeneratorState<E, Err>> {
        self.targets
            .values_mut()
            .find_map(|target| target.generators.get_mut(generator_id))
    }

    fn generator_count(&self) -> usize {
        self.targets
            .values()
            .map(|target| target.generators.len())
            .sum()
    }

    fn enqueue_output(&mut self, output: RuntimeOutput<E, Err>) {
        if self
            .config
            .output_queue_bound()
            .is_some_and(|bound| self.outputs.len() >= bound)
        {
            self.output_rejected += 1;
            #[cfg(feature = "runtime-events")]
            self.enqueue_queue_pressure();
            return;
        }

        self.outputs.push_back(output);
    }

    #[cfg(feature = "runtime-events")]
    fn enqueue_runtime_event(&mut self, event: RuntimeEvent) {
        self.outputs.push_back(RuntimeOutput::Runtime(event));
    }

    #[cfg(feature = "runtime-events")]
    fn enqueue_work_started(&mut self, generator_id: GeneratorId) {
        self.enqueue_runtime_event(RuntimeEvent::WorkStarted(generator_id));
    }

    #[cfg(feature = "runtime-events")]
    fn enqueue_work_completed(&mut self, generator_id: GeneratorId) {
        self.enqueue_runtime_event(RuntimeEvent::WorkCompleted(generator_id));
    }

    #[cfg(feature = "runtime-events")]
    fn enqueue_work_failed(&mut self, generator_id: GeneratorId) {
        self.enqueue_runtime_event(RuntimeEvent::WorkFailed(generator_id));
    }

    #[cfg(feature = "runtime-events")]
    fn enqueue_generator_lagging(&mut self, generator_id: GeneratorId) {
        self.enqueue_runtime_event(RuntimeEvent::GeneratorLagging(generator_id));
    }

    #[cfg(feature = "runtime-events")]
    fn enqueue_queue_pressure(&mut self) {
        self.enqueue_runtime_event(RuntimeEvent::EventQueuePressure);
    }

    fn take_next_ready_work(&mut self, now: Instant) -> Option<ScheduledWork<E, Err>> {
        if self.global_in_flight >= self.config.max_in_flight() {
            return None;
        }

        let target_ids = self.targets.keys().cloned().collect::<Vec<_>>();
        let target_count = target_ids.len();

        for offset in 0..target_count {
            let target_index = (self.target_cursor + offset) % target_count;
            let target_id = &target_ids[target_index];
            let Some(target) = self.targets.get_mut(target_id) else {
                continue;
            };

            if target.paused {
                continue;
            }

            if target.in_flight >= target.limits.max_in_flight() {
                target.throttled_count += 1;
                continue;
            }

            let Some(work) = target.take_next_ready_work(now) else {
                continue;
            };

            #[cfg(feature = "runtime-events")]
            {
                if target
                    .generators
                    .get(work.meta().generator_id())
                    .is_some_and(|generator| generator.lag.is_some())
                {
                    self.enqueue_generator_lagging(work.meta().generator_id().clone());
                }
            }

            self.target_cursor = (target_index + 1) % target_count;
            return Some(work);
        }

        None
    }

    fn report_completion(&mut self, meta: WorkMeta, outcome: CompletionOutcome, stats: WorkStats) {
        self.global_in_flight = self.global_in_flight.saturating_sub(1);

        if let Some(target) = self.targets.get_mut(meta.target_id()) {
            target.in_flight = target.in_flight.saturating_sub(1);

            if let Some(generator) = target.generators.get_mut(meta.generator_id()) {
                let completion = WorkCompletion::new(meta, outcome, stats);
                generator.generator.on_complete(&completion);
            }
        }
    }

    fn target_stats(&self) -> Vec<TargetStats> {
        self.targets
            .iter()
            .map(|(target_id, target)| {
                TargetStats::new(
                    Some(target_id.clone()),
                    target.in_flight,
                    target.throttled_count,
                )
            })
            .collect::<Vec<_>>()
    }

    fn class_stats(&self) -> Vec<ClassStats> {
        let mut dispatched_by_class = BTreeMap::<Option<ClassId>, u64>::new();

        for target in self.targets.values() {
            for generator in target.generators.values() {
                let dispatched = dispatched_by_class
                    .entry(generator.class_id.clone())
                    .or_insert(0);
                *dispatched += generator.dispatched_count;
            }
        }

        dispatched_by_class
            .into_iter()
            .map(|(class_id, dispatched_count)| ClassStats::new(class_id, dispatched_count, 0))
            .collect::<Vec<_>>()
    }

    fn generator_stats(&self) -> Vec<GeneratorStats> {
        self.targets
            .values()
            .flat_map(|target| {
                target.generators.iter().map(|(generator_id, generator)| {
                    GeneratorStats::new(
                        Some(generator_id.clone()),
                        generator.lag,
                        generator.missed_intervals,
                        generator.actual_interval,
                    )
                })
            })
            .collect::<Vec<_>>()
    }
}

impl<E, Err> TargetState<E, Err> {
    fn take_next_ready_work(&mut self, now: Instant) -> Option<ScheduledWork<E, Err>> {
        let generator_ids = self.generators.keys().cloned().collect::<Vec<_>>();
        let generator_count = generator_ids.len();
        let mut candidates = Vec::new();

        for offset in 0..generator_count {
            let generator_index = (self.generator_cursor + offset) % generator_count;
            let generator_id = &generator_ids[generator_index];
            let Some(generator) = self.generators.get_mut(generator_id) else {
                continue;
            };

            if generator.paused || !generator.config.enabled() {
                continue;
            }

            if !generator.is_interval_ready(now) {
                continue;
            }

            let readiness = generator.generator.update_ready(now);
            if !readiness.is_ready() {
                continue;
            }

            candidates.push(ReadyGenerator {
                generator_id: generator_id.clone(),
                generator_index,
                scan_order: offset,
                dispatched_cost: generator.dispatched_cost,
            });
        }

        candidates.sort_by_key(ReadyGenerator::sort_key);

        for candidate in candidates {
            let Some(generator) = self.generators.get_mut(&candidate.generator_id) else {
                continue;
            };

            if let Some(work) = generator.generator.take_next() {
                generator.record_dispatch(
                    now,
                    work.meta().class_id().clone(),
                    work.meta().cost().get(),
                );
                generator.triggered = false;
                self.generator_cursor = (candidate.generator_index + 1) % generator_count;
                return Some(work);
            }
        }

        None
    }
}

struct ReadyGenerator {
    generator_id: GeneratorId,
    generator_index: usize,
    scan_order: usize,
    dispatched_cost: u64,
}

impl ReadyGenerator {
    const fn sort_key(&self) -> (u64, usize) {
        (self.dispatched_cost, self.scan_order)
    }
}

impl<E, Err> GeneratorState<E, Err> {
    fn is_interval_ready(&self, now: Instant) -> bool {
        if self.triggered {
            return true;
        }

        let Some(requested_interval) = self.config.requested_interval() else {
            return true;
        };
        let Some(previous) = self.last_dispatch_at else {
            return true;
        };

        now.checked_duration_since(previous)
            .is_some_and(|elapsed| elapsed >= requested_interval)
    }

    fn record_dispatch(&mut self, now: Instant, class_id: ClassId, cost: u64) {
        let previous = self.last_dispatch_at.unwrap_or(self.registered_at);
        let elapsed = now.checked_duration_since(previous).unwrap_or_default();

        if self.last_dispatch_at.is_some() {
            self.actual_interval = Some(self.reported_actual_interval(elapsed));
        }

        if let Some(requested_interval) = self.config.requested_interval() {
            self.record_periodic_lag(elapsed, requested_interval);
        }

        self.class_id = Some(class_id);
        self.dispatched_count += 1;
        self.dispatched_cost = self.dispatched_cost.saturating_add(cost);
        self.last_dispatch_at = Some(now);
    }

    fn record_periodic_lag(&mut self, elapsed: Duration, requested_interval: Duration) {
        if elapsed <= requested_interval {
            self.lag = None;
            return;
        }

        self.lag = elapsed.checked_sub(requested_interval);

        let requested_nanos = requested_interval.as_nanos();
        if requested_nanos == 0 {
            return;
        }

        let elapsed_intervals = elapsed.as_nanos() / requested_nanos;
        let missed_intervals = elapsed_intervals.saturating_sub(1);
        self.missed_intervals += u64::try_from(missed_intervals).unwrap_or(u64::MAX);
    }

    fn reported_actual_interval(&self, elapsed: Duration) -> Duration {
        let Some(requested_interval) = self.config.requested_interval() else {
            return elapsed;
        };
        let requested_nanos = requested_interval.as_nanos();
        if requested_nanos == 0 {
            return elapsed;
        }

        let elapsed_intervals = elapsed.as_nanos() / requested_nanos;
        if elapsed_intervals == 0 {
            return elapsed;
        }

        duration_from_nanos(requested_nanos * elapsed_intervals)
    }
}

fn duration_from_nanos(nanos: u128) -> Duration {
    Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX))
}
