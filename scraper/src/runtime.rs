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
use crate::generator::Generator;
use crate::ids::GeneratorId;
use crate::ids::TargetId;
use crate::output::OutputQueueStats;
use crate::output::RuntimeOutput;
use crate::stats::RuntimeStats;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::future::ready;
use std::marker::PhantomData;
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
    #[allow(dead_code)]
    generator: Box<dyn Generator<E, Err>>,
    config: GeneratorConfig,
    paused: bool,
    triggered: bool,
}

struct TargetState<E, Err> {
    limits: TargetLimits,
    paused: bool,
    generators: HashMap<GeneratorId, GeneratorState<E, Err>>,
}

impl<E, Err> TargetState<E, Err> {
    fn new(limits: TargetLimits) -> Self {
        Self {
            limits,
            paused: false,
            generators: HashMap::new(),
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
    targets: HashMap<TargetId, TargetState<E, Err>>,
    outputs: VecDeque<RuntimeOutput<E, Err>>,
    global_in_flight: usize,
}

impl<E, Err> Runtime<E, Err> {
    /// Creates a runtime from configuration.
    #[must_use]
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            targets: HashMap::new(),
            outputs: VecDeque::new(),
            global_in_flight: 0,
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
    pub async fn run_once(&mut self, _now: Instant) -> Result<RunOutcome, RuntimeError> {
        ready(()).await;
        Ok(RunOutcome::Idle)
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
        OutputQueueStats::new(self.outputs.len(), 0, 0)
    }

    /// Returns a runtime statistics snapshot.
    #[must_use]
    pub fn stats(&self) -> RuntimeStats {
        RuntimeStats::new(
            self.targets.len(),
            self.generator_count(),
            self.global_in_flight,
            self.outputs.len(),
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
}
