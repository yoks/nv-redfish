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

//! Runtime control surface.
//!
//! The control API is synchronous. Mutating operations may briefly lock
//! runtime state, but they do not wait for work futures.

use core::fmt::Debug;
use core::fmt::Display;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;
use std::error::Error as StdError;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use crate::generator::CostUnits;
use crate::ids::ClassId;
use crate::ids::GeneratorId;
use crate::ids::TargetId;
use crate::runtime::RuntimeState;
use crate::runtime::WakerSlot;

/// Runtime-wide configuration set when the runtime is constructed.
#[derive(Debug, Clone, Default)]
pub struct RuntimeConfig {
    /// Optional global maximum number of in-flight work items.
    pub global_max_in_flight: Option<u32>,
    /// Optional bound on the output queue. When `None` the queue is unbounded.
    pub output_queue_capacity: Option<usize>,
}

/// Per-target limits set when a target is added or updated.
#[derive(Debug, Clone, Copy, Default)]
pub struct TargetLimits {
    /// Maximum number of in-flight work items for this target.
    pub max_in_flight: Option<u32>,
    /// Maximum cost budget per scheduling round for this target.
    pub max_cost_per_round: Option<CostUnits>,
}

/// Per-generator configuration set when a generator is added or updated.
#[derive(Debug, Clone, Default)]
pub struct GeneratorConfig {
    /// Optional class identifier for class-based scheduling.
    pub class: Option<ClassId>,
    /// Optional service weight for weighted scheduling.
    pub weight: Option<u32>,
}

/// Errors returned when adding a generator fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AddGeneratorError {
    /// The target id does not exist (never added or already removed).
    TargetNotFound,
    /// Graceful shutdown has started; no new generators may be added.
    ShutdownStarted,
}

impl Display for AddGeneratorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::TargetNotFound => f.write_str("target not found"),
            Self::ShutdownStarted => f.write_str("graceful shutdown already started"),
        }
    }
}

impl StdError for AddGeneratorError {}

/// Cloneable handle to a running [`crate::Runtime`].
///
/// `RuntimeHandle` exposes the synchronous control surface. It can be cloned
/// and shared across tasks; mutating operations may briefly lock internal
/// state but never wait on work futures.
///
/// The runtime itself is *not* `Clone` — only one consumer drives the output
/// stream via `Runtime::next`.
pub struct RuntimeHandle<Ev, Err> {
    pub(crate) shared: Arc<Shared<Ev, Err>>,
}

impl<Ev, Err> Clone for RuntimeHandle<Ev, Err> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

/// Internal shared state behind the handle and the runtime.
pub(crate) struct Shared<Ev, Err> {
    pub(crate) state: Mutex<RuntimeState<Ev, Err>>,
    pub(crate) waker: WakerSlot,
}

impl<Ev, Err> Shared<Ev, Err> {
    /// Acquire the runtime state lock.
    ///
    /// # Panics
    ///
    /// Panics if the runtime state lock is poisoned, which only happens if a
    /// panic occurred while another caller held the lock. This is treated as
    /// an unrecoverable invariant violation.
    pub(crate) fn lock_state(&self) -> MutexGuard<'_, RuntimeState<Ev, Err>> {
        self.state.lock().expect("runtime state lock poisoned")
    }
}

/// Marker trait used by [`RuntimeHandle::mutate`] to decide whether the
/// shared waker should be woken after a mutation.
///
/// `bool::is_success` returns `true` when the boolean is `true`, mirroring
/// the established `if updated { wake() }` discipline. `Option::is_success`
/// returns `true` when the option is `Some(_)`, matching `add_target`'s
/// "wake on successful allocation" behaviour. `Result::is_success` returns
/// `true` when the result is `Ok(_)`, matching `add_generator`'s "wake on
/// successful registration" behaviour.
trait IsSuccess {
    fn is_success(&self) -> bool;
}

impl IsSuccess for bool {
    fn is_success(&self) -> bool {
        *self
    }
}

impl<T> IsSuccess for Option<T> {
    fn is_success(&self) -> bool {
        self.is_some()
    }
}

impl<T, E> IsSuccess for Result<T, E> {
    fn is_success(&self) -> bool {
        self.is_ok()
    }
}

// Every method below acquires `Shared::lock_state` and may panic only on
// mutex poisoning; the panic discipline is documented on `Shared::lock_state`.
#[allow(clippy::missing_panics_doc)]
impl<Ev, Err> RuntimeHandle<Ev, Err> {
    /// Run `f` against the locked runtime state and return its result.
    ///
    /// The lock is dropped before the result is returned. Use this for
    /// read-only or wake-irrelevant mutations (`pause_*` on a paused entity
    /// is a no-op as far as the parked `next()` future is concerned).
    fn with_state<R>(&self, f: impl FnOnce(&mut RuntimeState<Ev, Err>) -> R) -> R {
        f(&mut self.shared.lock_state())
    }

    /// Run a state mutation and wake the parked runtime when the result
    /// reports success per [`IsSuccess`].
    ///
    /// This collapses the lock-mutate-maybe-wake-return pattern that every
    /// state-changing handle method otherwise repeats verbatim.
    fn mutate<R: IsSuccess>(&self, f: impl FnOnce(&mut RuntimeState<Ev, Err>) -> R) -> R {
        let result = self.with_state(f);
        if result.is_success() {
            self.shared.waker.wake();
        }
        result
    }

    /// Add a target to the runtime and return its newly-allocated id.
    ///
    /// If graceful shutdown has already started the call returns `None`.
    #[must_use]
    pub fn add_target(&self, limits: TargetLimits) -> Option<TargetId> {
        self.mutate(|g| g.add_target(limits))
    }

    /// Remove the target with the given id.
    ///
    /// Returns `true` if the target existed. All attached generators are
    /// removed as part of this call.
    #[must_use]
    pub fn remove_target(&self, id: TargetId) -> bool {
        self.mutate(|g| g.remove_target(id))
    }

    /// Update the limits of an existing target. Returns `true` on success.
    #[must_use]
    pub fn update_target_limits(&self, id: TargetId, limits: TargetLimits) -> bool {
        self.mutate(|g| g.update_target_limits(id, limits))
    }

    /// Pause an existing target. Returns `true` on success.
    ///
    /// Pausing never wakes the parked runtime: a paused target only excludes
    /// itself from future selections, it does not produce any new ready
    /// work to consume.
    #[must_use]
    pub fn pause_target(&self, id: TargetId) -> bool {
        self.with_state(|g| g.pause_target(id))
    }

    /// Resume a paused target. Returns `true` on success.
    #[must_use]
    pub fn resume_target(&self, id: TargetId) -> bool {
        self.mutate(|g| g.resume_target(id))
    }

    /// Add a generator under the specified target.
    ///
    /// # Errors
    ///
    /// Returns [`AddGeneratorError::TargetNotFound`] if `target` is not registered.
    /// Returns [`AddGeneratorError::ShutdownStarted`] if graceful shutdown
    /// has begun.
    #[allow(clippy::unwrap_in_result)] // expect path is in `lock_state`, not on a Result-returning op
    pub fn add_generator(
        &self,
        target: TargetId,
        generator: Box<dyn crate::Generator<Ev, Err> + Send>,
        config: GeneratorConfig,
    ) -> Result<GeneratorId, AddGeneratorError> {
        self.mutate(|g| g.add_generator(target, generator, config))
    }

    /// Remove a generator. Returns `true` if it existed.
    ///
    /// In-flight work for the removed generator continues to completion; only
    /// future selections are prevented.
    #[must_use]
    pub fn remove_generator(&self, id: GeneratorId) -> bool {
        self.mutate(|g| g.remove_generator(id))
    }

    /// Update generator configuration. Returns `true` on success.
    #[must_use]
    pub fn update_generator(&self, id: GeneratorId, config: GeneratorConfig) -> bool {
        self.mutate(|g| g.update_generator(id, config))
    }

    /// Pause a generator. Returns `true` on success.
    ///
    /// Pausing never wakes the parked runtime; see [`Self::pause_target`].
    #[must_use]
    pub fn pause_generator(&self, id: GeneratorId) -> bool {
        self.with_state(|g| g.pause_generator(id))
    }

    /// Resume a paused generator. Returns `true` on success.
    #[must_use]
    pub fn resume_generator(&self, id: GeneratorId) -> bool {
        self.mutate(|g| g.resume_generator(id))
    }

    /// Hint to the scheduler that a generator should be considered ready now.
    ///
    /// Returns `true` if the generator exists.
    #[must_use]
    pub fn trigger_generator(&self, id: GeneratorId) -> bool {
        self.mutate(|g| g.trigger_generator(id))
    }

    /// Begin graceful shutdown. Idempotent: subsequent calls do nothing.
    ///
    /// After shutdown starts, mutating control operations reject new target
    /// and generator changes; in-flight work is allowed to complete; queued
    /// outputs are still delivered, and finally the sticky shutdown output is
    /// emitted by [`crate::Runtime::next`].
    pub fn graceful_shutdown(&self) {
        let _ = self.mutate(RuntimeState::start_shutdown);
    }

    /// Snapshot of runtime statistics.
    #[must_use]
    pub fn stats(&self) -> crate::RuntimeStats {
        self.with_state(|g| g.stats_snapshot())
    }
}
