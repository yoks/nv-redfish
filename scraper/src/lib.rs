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

//! Generic scraping runtime and optional Redfish adapter.
//!
//! The crate is split into a Redfish-independent runtime and optional adapter
//! APIs. Applications own discovery policy and domain models; this crate owns
//! scheduling contracts, execution contracts, ordered outputs, and adapter event
//! shapes.

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

/// Optional adapter APIs.
pub mod adapter;
/// Runtime control configuration and errors.
pub mod control;
/// Runtime event types.
pub mod event;
/// Generator and scheduled work contracts.
pub mod generator;
/// Opaque runtime identifiers.
pub mod ids;
/// Ordered runtime output types.
pub mod output;
/// Runtime entry point.
pub mod runtime;
/// Scheduler metadata contracts.
pub mod scheduler;
/// Runtime statistics snapshots.
pub mod stats;

#[doc(inline)]
pub use control::ControlError;
#[doc(inline)]
pub use control::GeneratorConfig;
#[doc(inline)]
pub use control::RuntimeConfig;
#[doc(inline)]
pub use control::RuntimeError;
#[doc(inline)]
pub use control::TargetLimits;
#[doc(inline)]
#[cfg(feature = "runtime-events")]
pub use event::RuntimeEvent;
#[doc(inline)]
pub use event::RuntimeEventType;
#[doc(inline)]
pub use generator::CompletionOutcome;
#[doc(inline)]
pub use generator::Generator;
#[doc(inline)]
pub use generator::ScheduledWork;
#[doc(inline)]
pub use generator::ScheduledWorkFuture;
#[doc(inline)]
pub use generator::ScheduledWorkResult;
#[doc(inline)]
pub use generator::WorkCompletion;
#[doc(inline)]
pub use generator::WorkMeta;
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
pub use runtime::RunOutcome;
#[doc(inline)]
pub use runtime::Runtime;
#[doc(inline)]
pub use runtime::RuntimeHandle;
#[doc(inline)]
pub use scheduler::CostUnits;
#[doc(inline)]
pub use scheduler::Readiness;
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
