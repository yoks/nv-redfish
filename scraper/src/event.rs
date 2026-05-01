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

//! Optional runtime event types.

#[cfg(feature = "runtime-events")]
use crate::ids::GeneratorId;
#[cfg(feature = "runtime-events")]
use crate::ids::TargetId;
#[cfg(feature = "runtime-events")]
use crate::stats::RuntimeStats;
#[cfg(not(feature = "runtime-events"))]
use core::convert::Infallible;

/// Runtime event payload type selected by Cargo features.
#[cfg(feature = "runtime-events")]
pub type RuntimeEventType = RuntimeEvent;

/// Runtime event payload type selected by Cargo features.
#[cfg(not(feature = "runtime-events"))]
pub type RuntimeEventType = Infallible;

/// Runtime-owned out-of-band event.
#[cfg(feature = "runtime-events")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeEvent {
    /// A generator is lagging behind its requested interval.
    GeneratorLagging(GeneratorId),
    /// A generator recovered from lag.
    GeneratorRecovered(GeneratorId),
    /// A generator was ready but starved by scheduling constraints.
    GeneratorStarved(GeneratorId),
    /// A target was throttled by local limits.
    TargetThrottled(TargetId),
    /// Dispatch was throttled by global limits.
    GlobalThrottled,
    /// The output queue reached pressure thresholds.
    EventQueuePressure,
    /// Work started.
    WorkStarted(GeneratorId),
    /// Work completed successfully.
    WorkCompleted(GeneratorId),
    /// Work failed.
    WorkFailed(GeneratorId),
    /// Scheduler statistics snapshot.
    SchedulerStatisticsSnapshot(RuntimeStats),
}
