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

use nv_redfish_scraper::ClassId;
use nv_redfish_scraper::CompletionOutcome;
use nv_redfish_scraper::CostUnits;
use nv_redfish_scraper::Generator;
use nv_redfish_scraper::GeneratorId;
use nv_redfish_scraper::Readiness;
use nv_redfish_scraper::ScheduledWork;
use nv_redfish_scraper::ScheduledWorkResult;
use nv_redfish_scraper::TargetId;
use nv_redfish_scraper::WorkCompletion;
use nv_redfish_scraper::WorkMeta;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

pub struct FakeGenerator<E, Err> {
    state: Arc<Mutex<FakeGeneratorState<E, Err>>>,
}

pub struct FakeGeneratorHandle<E, Err> {
    state: Arc<Mutex<FakeGeneratorState<E, Err>>>,
}

struct FakeGeneratorState<E, Err> {
    target_id: TargetId,
    generator_id: GeneratorId,
    class_id: ClassId,
    cost: CostUnits,
    readiness: VecDeque<Readiness>,
    work: VecDeque<ScheduledWorkResult<E, Err>>,
    update_ready_count: usize,
    take_next_count: usize,
    completion_outcomes: Vec<CompletionOutcome>,
}

impl<E, Err> FakeGenerator<E, Err> {
    pub fn new(
        target_id: TargetId,
        generator_id: GeneratorId,
        class_id: ClassId,
        cost: CostUnits,
        readiness: Vec<Readiness>,
        work: Vec<ScheduledWorkResult<E, Err>>,
    ) -> (Self, FakeGeneratorHandle<E, Err>) {
        let state = Arc::new(Mutex::new(FakeGeneratorState {
            target_id,
            generator_id,
            class_id,
            cost,
            readiness: readiness.into(),
            work: work.into(),
            update_ready_count: 0,
            take_next_count: 0,
            completion_outcomes: Vec::new(),
        }));

        (
            Self {
                state: state.clone(),
            },
            FakeGeneratorHandle { state },
        )
    }
}

impl<E, Err> FakeGeneratorHandle<E, Err> {
    pub fn update_ready_count(&self) -> usize {
        self.state
            .lock()
            .expect("fake generator mutex must not be poisoned")
            .update_ready_count
    }

    pub fn take_next_count(&self) -> usize {
        self.state
            .lock()
            .expect("fake generator mutex must not be poisoned")
            .take_next_count
    }

    pub fn completion_count(&self) -> usize {
        self.state
            .lock()
            .expect("fake generator mutex must not be poisoned")
            .completion_outcomes
            .len()
    }

    pub fn completion_outcomes(&self) -> Vec<CompletionOutcome> {
        self.state
            .lock()
            .expect("fake generator mutex must not be poisoned")
            .completion_outcomes
            .clone()
    }
}

impl<E, Err> Generator<E, Err> for FakeGenerator<E, Err>
where
    E: Send + 'static,
    Err: Send + 'static,
{
    fn update_ready(&mut self, _now: Instant) -> Readiness {
        let mut state = self
            .state
            .lock()
            .expect("fake generator mutex must not be poisoned");
        state.update_ready_count += 1;
        state
            .readiness
            .pop_front()
            .unwrap_or_else(|| Readiness::ready(state.cost))
    }

    fn take_next(&mut self) -> Option<ScheduledWork<E, Err>> {
        let mut state = self
            .state
            .lock()
            .expect("fake generator mutex must not be poisoned");
        state.take_next_count += 1;
        let result = state.work.pop_front()?;
        let meta = WorkMeta::new(
            state.target_id.clone(),
            state.generator_id.clone(),
            state.class_id.clone(),
            state.cost,
        );
        Some(ScheduledWork::new(meta, async move { result }))
    }

    fn on_complete(&mut self, completion: &WorkCompletion) {
        self.state
            .lock()
            .expect("fake generator mutex must not be poisoned")
            .completion_outcomes
            .push(completion.outcome().clone());
    }
}
