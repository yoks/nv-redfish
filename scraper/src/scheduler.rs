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

//! Scheduler metadata contracts used by generators and runtime control.

use std::time::Instant;

/// Abstract weighted cost of a scheduled work item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CostUnits(u64);

impl CostUnits {
    /// Creates a cost value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric cost value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl Default for CostUnits {
    fn default() -> Self {
        Self::new(1)
    }
}

/// Generator readiness metadata observed by schedulers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Readiness {
    ready: bool,
    next_update_at: Option<Instant>,
    next_cost: Option<CostUnits>,
}

impl Readiness {
    /// Creates readiness metadata.
    #[must_use]
    pub const fn new(
        ready: bool,
        next_update_at: Option<Instant>,
        next_cost: Option<CostUnits>,
    ) -> Self {
        Self {
            ready,
            next_update_at,
            next_cost,
        }
    }

    /// Creates ready metadata with an estimated cost.
    #[must_use]
    pub const fn ready(next_cost: CostUnits) -> Self {
        Self::new(true, None, Some(next_cost))
    }

    /// Creates not-ready metadata with an optional next update time.
    #[must_use]
    pub const fn not_ready(next_update_at: Option<Instant>) -> Self {
        Self::new(false, next_update_at, None)
    }

    /// Returns whether the generator is ready to produce work.
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.ready
    }

    /// Returns the next time the runtime should refresh readiness.
    #[must_use]
    pub const fn next_update_at(&self) -> Option<Instant> {
        self.next_update_at
    }

    /// Returns the estimated cost of the next work item.
    #[must_use]
    pub const fn next_cost(&self) -> Option<CostUnits> {
        self.next_cost
    }
}
