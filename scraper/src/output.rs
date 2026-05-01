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

//! Ordered runtime output types.

use crate::event::RuntimeEventType;
use crate::stats::WorkStats;

/// Work result carried by a runtime output item.
pub type WorkResult<E, Err> = Result<WorkSuccess<E>, WorkError<Err>>;

/// Successful scheduled work output.
pub struct WorkSuccess<E> {
    events: Vec<E>,
    stats: WorkStats,
}

impl<E> WorkSuccess<E> {
    /// Creates successful work output.
    #[must_use]
    pub const fn new(events: Vec<E>, stats: WorkStats) -> Self {
        Self { events, stats }
    }

    /// Returns events produced by the work item.
    #[must_use]
    pub fn events(&self) -> &[E] {
        &self.events
    }

    /// Consumes the wrapper and returns produced events.
    #[must_use]
    pub fn into_events(self) -> Vec<E> {
        self.events
    }

    /// Returns runtime-owned work statistics.
    #[must_use]
    pub const fn stats(&self) -> &WorkStats {
        &self.stats
    }
}

/// Failed scheduled work output.
pub struct WorkError<Err> {
    error: Err,
    stats: WorkStats,
}

impl<Err> WorkError<Err> {
    /// Creates failed work output.
    #[must_use]
    pub const fn new(error: Err, stats: WorkStats) -> Self {
        Self { error, stats }
    }

    /// Returns the adapter or application error value.
    #[must_use]
    pub const fn error(&self) -> &Err {
        &self.error
    }

    /// Consumes the wrapper and returns the error value.
    #[must_use]
    pub fn into_error(self) -> Err {
        self.error
    }

    /// Returns runtime-owned work statistics.
    #[must_use]
    pub const fn stats(&self) -> &WorkStats {
        &self.stats
    }
}

/// Ordered output item emitted by the runtime.
pub enum RuntimeOutput<E, Err, R = RuntimeEventType> {
    /// Output from scheduled work.
    Work(WorkResult<E, Err>),
    /// Out-of-band runtime event, when compiled in.
    Runtime(R),
}

/// Output queue statistics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutputQueueStats {
    len: usize,
    dropped: u64,
    rejected: u64,
}

impl OutputQueueStats {
    /// Creates output queue statistics.
    #[must_use]
    pub const fn new(len: usize, dropped: u64, rejected: u64) -> Self {
        Self {
            len,
            dropped,
            rejected,
        }
    }

    /// Returns the current queue length.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the queue is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the number of dropped outputs.
    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Returns the number of rejected outputs.
    #[must_use]
    pub const fn rejected(&self) -> u64 {
        self.rejected
    }
}
