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

//! Generator and scheduled work contracts.

use crate::ids::ClassId;
use crate::ids::GeneratorId;
use crate::ids::TargetId;
use crate::scheduler::CostUnits;
use crate::scheduler::Readiness;
use crate::stats::WorkStats;
use std::future::Future;
use std::pin::Pin;
use std::time::Instant;

/// Result produced by one scheduled work item.
pub type ScheduledWorkResult<E, Err> = Result<Vec<E>, Err>;

/// Boxed executable future owned by scheduled work.
pub type ScheduledWorkFuture<E, Err> =
    Pin<Box<dyn Future<Output = ScheduledWorkResult<E, Err>> + Send + 'static>>;

/// Runtime metadata attached to scheduled work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkMeta {
    target_id: TargetId,
    generator_id: GeneratorId,
    class_id: ClassId,
    cost: CostUnits,
}

impl WorkMeta {
    /// Creates work metadata.
    #[must_use]
    pub const fn new(
        target_id: TargetId,
        generator_id: GeneratorId,
        class_id: ClassId,
        cost: CostUnits,
    ) -> Self {
        Self {
            target_id,
            generator_id,
            class_id,
            cost,
        }
    }

    /// Returns the target id associated with the work.
    #[must_use]
    pub const fn target_id(&self) -> &TargetId {
        &self.target_id
    }

    /// Returns the generator id associated with the work.
    #[must_use]
    pub const fn generator_id(&self) -> &GeneratorId {
        &self.generator_id
    }

    /// Returns the scheduler class id associated with the work.
    #[must_use]
    pub const fn class_id(&self) -> &ClassId {
        &self.class_id
    }

    /// Returns the estimated work cost.
    #[must_use]
    pub const fn cost(&self) -> CostUnits {
        self.cost
    }
}

/// Executable work selected by a scheduler.
pub struct ScheduledWork<E, Err> {
    meta: WorkMeta,
    future: ScheduledWorkFuture<E, Err>,
}

impl<E, Err> ScheduledWork<E, Err> {
    /// Creates scheduled work from runtime metadata and an executable future.
    #[must_use]
    pub fn new(
        meta: WorkMeta,
        future: impl Future<Output = ScheduledWorkResult<E, Err>> + Send + 'static,
    ) -> Self {
        Self {
            meta,
            future: Box::pin(future),
        }
    }

    /// Returns runtime metadata for the work item.
    #[must_use]
    pub const fn meta(&self) -> &WorkMeta {
        &self.meta
    }

    /// Splits scheduled work into metadata and executable future.
    #[must_use]
    pub fn into_parts(self) -> (WorkMeta, ScheduledWorkFuture<E, Err>) {
        (self.meta, self.future)
    }
}

/// Outcome category reported to a generator after execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletionOutcome {
    /// Work completed successfully.
    Success,
    /// Work failed with the adapter or application error.
    Failure,
}

/// Runtime-owned completion report for a dispatched work item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkCompletion {
    meta: WorkMeta,
    outcome: CompletionOutcome,
    stats: WorkStats,
}

impl WorkCompletion {
    /// Creates a completion report.
    #[must_use]
    pub const fn new(meta: WorkMeta, outcome: CompletionOutcome, stats: WorkStats) -> Self {
        Self {
            meta,
            outcome,
            stats,
        }
    }

    /// Returns metadata for the completed work.
    #[must_use]
    pub const fn meta(&self) -> &WorkMeta {
        &self.meta
    }

    /// Returns the completion outcome category.
    #[must_use]
    pub const fn outcome(&self) -> &CompletionOutcome {
        &self.outcome
    }

    /// Returns runtime-owned work statistics.
    #[must_use]
    pub const fn stats(&self) -> &WorkStats {
        &self.stats
    }
}

/// Stateful scheduling leaf that creates executable work after selection.
pub trait Generator<E, Err>: Send {
    /// Updates and returns generator readiness.
    fn update_ready(&mut self, now: Instant) -> Readiness;

    /// Creates the next executable work item after the scheduler selects this generator.
    fn take_next(&mut self) -> Option<ScheduledWork<E, Err>>;

    /// Reports completion for previously dispatched work.
    fn on_complete(&mut self, completion: &WorkCompletion);
}
