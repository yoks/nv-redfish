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

//! Runtime and work statistics snapshots.

use crate::ids::ClassId;
use crate::ids::GeneratorId;
use crate::ids::TargetId;
use std::time::Duration;

/// Runtime-owned statistics attached to completed work.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkStats {
    started_count: u64,
    completed_count: u64,
    latency: Option<Duration>,
}

impl WorkStats {
    /// Creates work statistics.
    #[must_use]
    pub const fn new(started_count: u64, completed_count: u64) -> Self {
        Self {
            started_count,
            completed_count,
            latency: None,
        }
    }

    /// Creates work statistics with latency.
    #[must_use]
    pub const fn with_latency(started_count: u64, completed_count: u64, latency: Duration) -> Self {
        Self {
            started_count,
            completed_count,
            latency: Some(latency),
        }
    }

    /// Returns how many work items were started.
    #[must_use]
    pub const fn started_count(&self) -> u64 {
        self.started_count
    }

    /// Returns how many work items completed.
    #[must_use]
    pub const fn completed_count(&self) -> u64 {
        self.completed_count
    }

    /// Returns observed work latency, when known.
    #[must_use]
    pub const fn latency(&self) -> Option<Duration> {
        self.latency
    }
}

/// Per-target runtime statistics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TargetStats {
    target_id: Option<TargetId>,
    in_flight: usize,
    throttled_count: u64,
}

impl TargetStats {
    /// Creates per-target statistics.
    #[must_use]
    pub const fn new(target_id: Option<TargetId>, in_flight: usize, throttled_count: u64) -> Self {
        Self {
            target_id,
            in_flight,
            throttled_count,
        }
    }

    /// Returns the target id.
    #[must_use]
    pub const fn target_id(&self) -> Option<&TargetId> {
        self.target_id.as_ref()
    }

    /// Returns in-flight work for this target.
    #[must_use]
    pub const fn in_flight(&self) -> usize {
        self.in_flight
    }

    /// Returns how many times this target was throttled.
    #[must_use]
    pub const fn throttled_count(&self) -> u64 {
        self.throttled_count
    }
}

/// Per-class scheduler statistics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClassStats {
    class_id: Option<ClassId>,
    dispatched_count: u64,
    starved_count: u64,
}

impl ClassStats {
    /// Creates per-class statistics.
    #[must_use]
    pub const fn new(class_id: Option<ClassId>, dispatched_count: u64, starved_count: u64) -> Self {
        Self {
            class_id,
            dispatched_count,
            starved_count,
        }
    }

    /// Returns the class id.
    #[must_use]
    pub const fn class_id(&self) -> Option<&ClassId> {
        self.class_id.as_ref()
    }

    /// Returns dispatched work count.
    #[must_use]
    pub const fn dispatched_count(&self) -> u64 {
        self.dispatched_count
    }

    /// Returns starvation count.
    #[must_use]
    pub const fn starved_count(&self) -> u64 {
        self.starved_count
    }
}

/// Per-generator scheduler statistics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GeneratorStats {
    generator_id: Option<GeneratorId>,
    lag: Option<Duration>,
    missed_intervals: u64,
    actual_interval: Option<Duration>,
}

impl GeneratorStats {
    /// Creates per-generator statistics.
    #[must_use]
    pub const fn new(
        generator_id: Option<GeneratorId>,
        lag: Option<Duration>,
        missed_intervals: u64,
        actual_interval: Option<Duration>,
    ) -> Self {
        Self {
            generator_id,
            lag,
            missed_intervals,
            actual_interval,
        }
    }

    /// Returns the generator id.
    #[must_use]
    pub const fn generator_id(&self) -> Option<&GeneratorId> {
        self.generator_id.as_ref()
    }

    /// Returns generator lag.
    #[must_use]
    pub const fn lag(&self) -> Option<Duration> {
        self.lag
    }

    /// Returns missed requested intervals.
    #[must_use]
    pub const fn missed_intervals(&self) -> u64 {
        self.missed_intervals
    }

    /// Returns actual interval between dispatches, when known.
    #[must_use]
    pub const fn actual_interval(&self) -> Option<Duration> {
        self.actual_interval
    }
}

/// Runtime statistics snapshot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeStats {
    target_count: usize,
    generator_count: usize,
    global_in_flight: usize,
    queued_outputs: usize,
    target_stats: Vec<TargetStats>,
    class_stats: Vec<ClassStats>,
    generator_stats: Vec<GeneratorStats>,
}

impl RuntimeStats {
    /// Creates a runtime statistics snapshot.
    #[must_use]
    pub const fn new(
        target_count: usize,
        generator_count: usize,
        global_in_flight: usize,
        queued_outputs: usize,
    ) -> Self {
        Self {
            target_count,
            generator_count,
            global_in_flight,
            queued_outputs,
            target_stats: Vec::new(),
            class_stats: Vec::new(),
            generator_stats: Vec::new(),
        }
    }

    /// Creates a runtime statistics snapshot with detailed scheduler stats.
    #[must_use]
    pub const fn with_details(
        target_count: usize,
        generator_count: usize,
        global_in_flight: usize,
        queued_outputs: usize,
        target_stats: Vec<TargetStats>,
        class_stats: Vec<ClassStats>,
        generator_stats: Vec<GeneratorStats>,
    ) -> Self {
        Self {
            target_count,
            generator_count,
            global_in_flight,
            queued_outputs,
            target_stats,
            class_stats,
            generator_stats,
        }
    }

    /// Returns the number of targets currently registered.
    #[must_use]
    pub const fn target_count(&self) -> usize {
        self.target_count
    }

    /// Returns the number of generators currently registered.
    #[must_use]
    pub const fn generator_count(&self) -> usize {
        self.generator_count
    }

    /// Returns the number of globally in-flight work items.
    #[must_use]
    pub const fn global_in_flight(&self) -> usize {
        self.global_in_flight
    }

    /// Returns the number of queued runtime outputs.
    #[must_use]
    pub const fn queued_outputs(&self) -> usize {
        self.queued_outputs
    }

    /// Returns per-target statistics.
    #[must_use]
    pub fn target_stats(&self) -> &[TargetStats] {
        &self.target_stats
    }

    /// Returns per-class statistics.
    #[must_use]
    pub fn class_stats(&self) -> &[ClassStats] {
        &self.class_stats
    }

    /// Returns per-generator statistics.
    #[must_use]
    pub fn generator_stats(&self) -> &[GeneratorStats] {
        &self.generator_stats
    }
}
