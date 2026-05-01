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

//! Scriptable fake generator with call counters.

use std::collections::VecDeque;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use nv_redfish_scraper::CompletionOutcome;
use nv_redfish_scraper::CostUnits;
use nv_redfish_scraper::Generator;
use nv_redfish_scraper::Readiness;
use nv_redfish_scraper::ScheduledWork;
use nv_redfish_scraper::WorkCompletion;
use nv_redfish_scraper::WorkMeta;

use super::fake_error::FakeError;
use super::fake_event::FakeEvent;

/// One scripted step the [`FakeGenerator`] performs in response to scheduler
/// queries.
pub enum Step {
    /// `update_ready` returns `not_ready`. `take_next` is not expected to be
    /// called for this step.
    NotReady,
    /// `update_ready` returns `ready` but `take_next` returns `None`.
    ReadyNoWork,
    /// `update_ready` returns `ready` and `take_next` returns work that
    /// resolves immediately to the supplied events.
    Success(Vec<FakeEvent>),
    /// `update_ready` returns `ready` and `take_next` returns work that
    /// resolves immediately to the supplied error.
    Failure(FakeError),
}

/// Counters tracking calls into a [`FakeGenerator`].
#[derive(Clone, Default)]
pub struct CallCounters {
    update_ready: Arc<AtomicU64>,
    take_next: Arc<AtomicU64>,
    on_complete_success: Arc<AtomicU64>,
    on_complete_failed: Arc<AtomicU64>,
}

impl CallCounters {
    /// Number of times `update_ready` was called.
    pub fn update_ready(&self) -> u64 {
        self.update_ready.load(Ordering::SeqCst)
    }
    /// Number of times `take_next` was called.
    pub fn take_next(&self) -> u64 {
        self.take_next.load(Ordering::SeqCst)
    }
    /// Number of completion callbacks observed with success outcome.
    pub fn on_complete_success(&self) -> u64 {
        self.on_complete_success.load(Ordering::SeqCst)
    }
    /// Number of completion callbacks observed with failure outcome.
    pub fn on_complete_failed(&self) -> u64 {
        self.on_complete_failed.load(Ordering::SeqCst)
    }
    /// Total completion callbacks (success + failure).
    pub fn on_complete_total(&self) -> u64 {
        self.on_complete_success() + self.on_complete_failed()
    }
    /// Increment the `update_ready` counter.
    pub fn tick_update_ready(&self) {
        self.update_ready.fetch_add(1, Ordering::SeqCst);
    }
    /// Increment the `take_next` counter.
    pub fn tick_take_next(&self) {
        self.take_next.fetch_add(1, Ordering::SeqCst);
    }
    /// Increment the success-completion counter.
    pub fn tick_on_complete_success(&self) {
        self.on_complete_success.fetch_add(1, Ordering::SeqCst);
    }
    /// Increment the failure-completion counter.
    pub fn tick_on_complete_failed(&self) {
        self.on_complete_failed.fetch_add(1, Ordering::SeqCst);
    }
}

/// Scripted [`Generator`] producing `FakeEvent` / `FakeError` results.
///
/// The script is processed in order. Once the script is exhausted the
/// generator reports `not_ready` forever (and `take_next` returns `None`).
pub struct FakeGenerator {
    steps: VecDeque<Step>,
    /// Pending work item produced by `take_next` for the most recent ready step.
    counters: CallCounters,
    /// Cost reported by every produced work item.
    cost: CostUnits,
}

impl FakeGenerator {
    /// Build a new fake generator from a script.
    pub fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            counters: CallCounters::default(),
            cost: CostUnits::ZERO,
        }
    }

    /// Configure the cost reported by every produced work item.
    pub fn with_cost(mut self, cost: CostUnits) -> Self {
        self.cost = cost;
        self
    }

    /// Borrow the call counters; clones share the same backing counters.
    pub fn counters(&self) -> CallCounters {
        self.counters.clone()
    }
}

impl Generator<FakeEvent, FakeError> for FakeGenerator {
    fn update_ready(&mut self, _now: Instant) -> Readiness {
        self.counters.update_ready.fetch_add(1, Ordering::SeqCst);
        match self.steps.front() {
            None => Readiness::not_ready(None),
            Some(Step::NotReady) => Readiness::not_ready(None),
            Some(Step::ReadyNoWork | Step::Success(_) | Step::Failure(_)) => {
                Readiness::ready(Some(self.cost))
            }
        }
    }

    fn take_next(&mut self) -> Option<ScheduledWork<FakeEvent, FakeError>> {
        self.counters.take_next.fetch_add(1, Ordering::SeqCst);
        let step = self.steps.pop_front()?;
        match step {
            Step::NotReady => {
                // putting it back and returning None preserves "not ready"
                self.steps.push_front(Step::NotReady);
                None
            }
            Step::ReadyNoWork => None,
            Step::Success(events) => {
                let cost = self.cost;
                let fut = Box::pin(async move { Ok::<_, FakeError>(events) });
                Some(ScheduledWork::new(WorkMeta::with_cost(cost), fut))
            }
            Step::Failure(err) => {
                let cost = self.cost;
                let fut = Box::pin(async move { Err::<Vec<FakeEvent>, _>(err) });
                Some(ScheduledWork::new(WorkMeta::with_cost(cost), fut))
            }
        }
    }

    fn on_complete(&mut self, completion: &WorkCompletion) {
        match completion.outcome {
            CompletionOutcome::Succeeded => {
                self.counters
                    .on_complete_success
                    .fetch_add(1, Ordering::SeqCst);
            }
            CompletionOutcome::Failed => {
                self.counters
                    .on_complete_failed
                    .fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}
