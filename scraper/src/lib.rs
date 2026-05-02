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

//! Generic scraper runtime and Redfish adapter.
//!
//! The crate is organized as three layers:
//!
//! 1. user application — owns policy, models, persistence (outside this crate),
//! 2. generic runtime — Redfish-free, parameterized by application work event
//!    type `Ev` and work error type `Err`,
//! 3. Redfish adapter — feature-gated binding from the runtime to `nv-redfish`.
//!
//! See `docs/scraper/` for the full architecture, scheduling, runtime, and
//! Redfish adapter specifications. Phase 0 establishes the public API and the
//! frozen TDD test suite. Later phases turn behavior tests green by editing
//! only production code.
//!
//! The runtime modules (`ids`, `generator`, `scheduler`, `output`, `event`,
//! `stats`, `control`, `runtime`) compile without `nv-redfish`. The
//! `adapter::redfish` module is only compiled when the `redfish-adapter`
//! feature is enabled.

#![deny(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::suspicious,
    clippy::complexity,
    clippy::perf
)]
#![deny(
    clippy::absolute_paths,
    clippy::todo,
    clippy::unimplemented,
    clippy::tests_outside_test_module,
    clippy::panic,
    clippy::unwrap_used,
    clippy::unwrap_in_result,
    clippy::unused_trait_names,
    clippy::print_stdout,
    clippy::print_stderr
)]
#![deny(missing_docs)]
#![allow(clippy::doc_markdown)]
// Module-name repetition is intentional for this crate's public types
// (RuntimeOutput, RuntimeEvent, RuntimeStats, etc.) which are re-exported.
#![allow(clippy::module_name_repetitions)]

pub mod control;
pub mod event;
pub mod generator;
pub mod ids;
pub mod output;
pub mod runtime;
pub mod scheduler;
pub mod stats;

#[cfg(feature = "redfish-adapter")]
pub mod adapter;

#[doc(inline)]
pub use control::AddGeneratorError;
#[doc(inline)]
pub use control::GeneratorConfig;
#[doc(inline)]
pub use control::RuntimeConfig;
#[doc(inline)]
pub use control::RuntimeHandle;
#[doc(inline)]
pub use control::TargetLimits;
#[doc(inline)]
pub use event::RuntimeEventType;
#[cfg(feature = "runtime-events")]
#[doc(inline)]
pub use event::RuntimeEvent;
#[doc(inline)]
pub use generator::CostUnits;
#[doc(inline)]
pub use generator::Generator;
#[doc(inline)]
pub use generator::Readiness;
#[doc(inline)]
pub use generator::ScheduledWork;
#[doc(inline)]
pub use generator::ScheduledWorkResult;
#[doc(inline)]
pub use generator::WorkCompletion;
#[doc(inline)]
pub use generator::WorkMeta;
#[doc(inline)]
pub use generator::CompletionOutcome;
#[doc(inline)]
pub use ids::ClassId;
#[doc(inline)]
pub use ids::GeneratorId;
#[doc(inline)]
pub use ids::TargetId;
#[doc(inline)]
pub use output::OutputQueueStats;
#[doc(inline)]
pub use output::RuntimeOutput;
#[doc(inline)]
pub use output::WorkError;
#[doc(inline)]
pub use output::WorkResult;
#[doc(inline)]
pub use output::WorkSuccess;
#[doc(inline)]
pub use runtime::Runtime;
#[doc(inline)]
pub use stats::ClassStats;
#[doc(inline)]
pub use stats::GeneratorStats;
#[doc(inline)]
pub use stats::RuntimeStats;
#[doc(inline)]
pub use stats::TargetStats;
#[doc(inline)]
pub use stats::WorkStats;

// Redfish adapter re-exports. Grouped at the end so the alphabetically
// ordered runtime re-exports above stay contiguous.
#[cfg(feature = "redfish-adapter")]
#[doc(inline)]
pub use adapter::reconstruction::reconstruction_iter;
#[cfg(feature = "redfish-adapter")]
#[doc(inline)]
pub use adapter::reconstruction::replay_records;
#[cfg(feature = "redfish-adapter")]
#[doc(inline)]
pub use adapter::reconstruction::ReplayDecision;
#[cfg(feature = "redfish-adapter")]
#[doc(inline)]
pub use adapter::reconstruction::ReplayStats;
