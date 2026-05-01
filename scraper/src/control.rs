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

//! Runtime control configuration and error types.

use core::fmt;
use std::error::Error as StdError;
use std::time::Duration;

/// Configuration for the generic runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeConfig {
    max_in_flight: usize,
    output_queue_bound: Option<usize>,
}

impl RuntimeConfig {
    /// Creates runtime configuration with a global in-flight work limit.
    #[must_use]
    pub const fn new(max_in_flight: usize) -> Self {
        Self {
            max_in_flight,
            output_queue_bound: None,
        }
    }

    /// Sets a bounded output queue capacity.
    #[must_use]
    pub const fn with_output_queue_bound(mut self, bound: usize) -> Self {
        self.output_queue_bound = Some(bound);
        self
    }

    /// Returns the global in-flight work limit.
    #[must_use]
    pub const fn max_in_flight(&self) -> usize {
        self.max_in_flight
    }

    /// Returns the configured output queue bound.
    #[must_use]
    pub const fn output_queue_bound(&self) -> Option<usize> {
        self.output_queue_bound
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self::new(1)
    }
}

/// Per-target scheduling and executor limits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetLimits {
    max_in_flight: usize,
}

impl TargetLimits {
    /// Creates per-target limits with an in-flight work limit.
    #[must_use]
    pub const fn new(max_in_flight: usize) -> Self {
        Self { max_in_flight }
    }

    /// Returns the per-target in-flight work limit.
    #[must_use]
    pub const fn max_in_flight(&self) -> usize {
        self.max_in_flight
    }
}

impl Default for TargetLimits {
    fn default() -> Self {
        Self::new(1)
    }
}

/// Runtime-owned generator configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratorConfig {
    enabled: bool,
    requested_interval: Option<Duration>,
}

impl GeneratorConfig {
    /// Creates generator configuration.
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self {
            enabled,
            requested_interval: None,
        }
    }

    /// Sets a requested periodic interval.
    #[must_use]
    pub const fn with_requested_interval(mut self, interval: Duration) -> Self {
        self.requested_interval = Some(interval);
        self
    }

    /// Returns whether the generator is enabled.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the requested periodic interval.
    #[must_use]
    pub const fn requested_interval(&self) -> Option<Duration> {
        self.requested_interval
    }
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self::new(true)
    }
}

/// Error returned by runtime control operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlError {
    /// A target id already exists.
    TargetAlreadyExists,
    /// A target id is unknown.
    TargetNotFound,
    /// A generator id already exists.
    GeneratorAlreadyExists,
    /// A generator id is unknown.
    GeneratorNotFound,
}

impl fmt::Display for ControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetAlreadyExists => formatter.write_str("target already exists"),
            Self::TargetNotFound => formatter.write_str("target not found"),
            Self::GeneratorAlreadyExists => formatter.write_str("generator already exists"),
            Self::GeneratorNotFound => formatter.write_str("generator not found"),
        }
    }
}

impl StdError for ControlError {}

/// Error returned by runtime execution operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    /// The requested runtime behavior is not implemented yet.
    NotImplemented,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotImplemented => formatter.write_str("runtime behavior is not implemented"),
        }
    }
}

impl StdError for RuntimeError {}
