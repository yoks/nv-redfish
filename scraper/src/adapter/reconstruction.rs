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

//! Reconstruction-record derivation and replay helpers.
//!
//! This module derives a stream of [`ReconstructionRecord`]s from a stream of
//! [`RedfishResourceEvent`]s and provides a thin runtime-coupled replay helper
//! that rebuilds the scheduler tree from previously captured records. The
//! application owns the policy that decides which builder to invoke for each
//! record; this module only dispatches the resulting [`ReplayDecision`]s
//! against the runtime's [`add_target`](crate::Runtime::add_target) and
//! [`add_generator`](crate::Runtime::add_generator) APIs.

use crate::adapter::redfish::ChangeKind;
use crate::adapter::redfish::ReconstructionRecord;
use crate::adapter::redfish::RedfishResourceEvent;
use crate::control::GeneratorConfig;
use crate::control::TargetLimits;
use crate::generator::Generator;
use crate::ids::GeneratorId;
use crate::ids::TargetId;
use crate::runtime::Runtime;

/// Derive a [`ReconstructionRecord`] iterator from a borrow of a
/// [`RedfishResourceEvent`] stream.
///
/// Mapping by [`ChangeKind`]:
///
/// - [`ChangeKind::Inserted`], [`ChangeKind::Updated`],
///   [`ChangeKind::RefreshedNoChange`], [`ChangeKind::Stale`] each yield a
///   full record built via [`ReconstructionRecord::from_resource_event`].
/// - [`ChangeKind::Removed`] yields an identity-preserving record with
///   `payload = None` so consumers can mark the entity as deleted in their
///   persisted store while keeping `bmc_id`, `odata_id`, and
///   `parent_odata_id` intact.
/// - [`ChangeKind::FetchFailed`] is skipped.
///
/// The match is intentionally exhaustive (no catch-all): when a future
/// [`ChangeKind`] variant lands the compiler forces this function to make
/// an explicit emit-or-skip decision rather than silently dropping the
/// new variant.
///
/// The iterator borrows from `events` and produces fresh
/// [`ReconstructionRecord`] values; running it twice over the same slice
/// yields two equal record vectors (idempotency).
#[must_use = "reconstruction_iter is lazy; consume the iterator (e.g. via `collect`) to derive records"]
pub fn reconstruction_iter<'a, I>(events: I) -> impl Iterator<Item = ReconstructionRecord> + 'a
where
    I: IntoIterator<Item = &'a RedfishResourceEvent> + 'a,
{
    events.into_iter().filter_map(|event| match event.change {
        ChangeKind::Inserted
        | ChangeKind::Updated
        | ChangeKind::RefreshedNoChange
        | ChangeKind::Stale => Some(ReconstructionRecord::from_resource_event(event)),
        ChangeKind::Removed => Some(ReconstructionRecord {
            bmc_id: event.bmc_id.clone(),
            odata_id: event.odata_id.clone(),
            parent_odata_id: event.parent_odata_id.clone(),
            payload: None,
        }),
        ChangeKind::FetchFailed => None,
    })
}

/// Per-record decision returned by the policy passed to [`replay_records`].
///
/// The application observes one [`ReconstructionRecord`] at a time and tells
/// the helper what to do with it. The helper is intentionally thin: it does
/// not interpret the record itself, it only dispatches the resulting
/// [`ReplayDecision`].
#[non_exhaustive]
pub enum ReplayDecision<Ev, Err> {
    /// Ignore this record.
    Skip,
    /// Add a new target with the supplied [`TargetLimits`].
    AddTarget {
        /// Limits applied to the new target.
        limits: TargetLimits,
    },
    /// Attach a generator to an existing target.
    AddGenerator {
        /// Target the generator should be attached to.
        target: TargetId,
        /// Generator instance reconstructed by the application.
        generator: Box<dyn Generator<Ev, Err> + Send>,
        /// Configuration applied to the new generator.
        config: GeneratorConfig,
    },
}

/// Outcome summary returned by [`replay_records`].
///
/// Each successfully dispatched [`ReplayDecision::AddTarget`] /
/// [`ReplayDecision::AddGenerator`] pushes the runtime-allocated
/// [`TargetId`] / [`GeneratorId`] into [`added_targets`](Self::added_targets)
/// / [`added_generators`](Self::added_generators) so callers can later
/// pause, resume, update, or remove the entities they just rebuilt without
/// having to rescan [`Runtime::stats`]. Vec ordering matches record
/// iteration order, which makes the helper deterministic.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplayStats {
    /// Target ids allocated by successful [`ReplayDecision::AddTarget`]
    /// dispatches, in record-iteration order.
    pub added_targets: Vec<TargetId>,
    /// Generator ids allocated by successful [`ReplayDecision::AddGenerator`]
    /// dispatches, in record-iteration order.
    pub added_generators: Vec<GeneratorId>,
    /// Number of [`ReplayDecision::Skip`] decisions.
    pub skipped: usize,
    /// Number of decisions whose runtime call returned an error or `None`
    /// (for example, [`Runtime::add_target`] returned `None` because shutdown
    /// has already started, or [`Runtime::add_generator`] returned an
    /// [`AddGeneratorError`](crate::control::AddGeneratorError)).
    pub failed: usize,
}

impl ReplayStats {
    /// Number of targets successfully added by the replay.
    #[must_use]
    pub const fn targets_added(&self) -> usize {
        self.added_targets.len()
    }

    /// Number of generators successfully added by the replay.
    #[must_use]
    pub const fn generators_added(&self) -> usize {
        self.added_generators.len()
    }

    /// Record one [`ReplayDecision::Skip`] decision and return the updated stats.
    #[must_use]
    const fn skip(mut self) -> Self {
        self.skipped += 1;
        self
    }

    /// Record one failed dispatch and return the updated stats.
    #[must_use]
    const fn fail(mut self) -> Self {
        self.failed += 1;
        self
    }

    /// Record a newly-allocated [`TargetId`] and return the updated stats.
    #[must_use]
    fn with_target(mut self, id: TargetId) -> Self {
        self.added_targets.push(id);
        self
    }

    /// Record a newly-allocated [`GeneratorId`] and return the updated stats.
    #[must_use]
    fn with_generator(mut self, id: GeneratorId) -> Self {
        self.added_generators.push(id);
        self
    }
}

/// Replay a stream of [`ReconstructionRecord`]s against a live [`Runtime`].
///
/// For each record the supplied `policy` is invoked once and the returned
/// [`ReplayDecision`] is dispatched:
///
/// - [`ReplayDecision::AddTarget`] -> [`Runtime::add_target`].
/// - [`ReplayDecision::AddGenerator`] -> [`Runtime::add_generator`].
/// - [`ReplayDecision::Skip`] -> no runtime mutation.
///
/// The helper does not interpret records itself — record-to-builder mapping
/// is owned by the application via `policy`. The returned [`ReplayStats`]
/// counts successful dispatches and per-decision failures so the caller can
/// detect partial replays without aborting the iteration.
#[must_use = "ReplayStats reports targets/generators added and failed dispatches; ignoring it hides partial-replay errors"]
pub fn replay_records<Ev, Err, I, F>(
    runtime: &Runtime<Ev, Err>,
    records: I,
    mut policy: F,
) -> ReplayStats
where
    Ev: Send + 'static,
    Err: Send + 'static,
    I: IntoIterator<Item = ReconstructionRecord>,
    F: FnMut(&ReconstructionRecord) -> ReplayDecision<Ev, Err>,
{
    records
        .into_iter()
        .fold(ReplayStats::default(), |stats, record| match policy(&record) {
            ReplayDecision::Skip => stats.skip(),
            ReplayDecision::AddTarget { limits } => match runtime.add_target(limits) {
                Some(id) => stats.with_target(id),
                None => stats.fail(),
            },
            ReplayDecision::AddGenerator {
                target,
                generator,
                config,
            } => match runtime.add_generator(target, generator, config) {
                Ok(id) => stats.with_generator(id),
                Err(_) => stats.fail(),
            },
        })
}
