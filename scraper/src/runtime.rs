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

//! The generic scraper runtime.
//!
//! [`Runtime::next`] is the single ordered output and execution interface.
//! Each call advances the runtime by at most one selected work item, drains
//! at most one in-flight completion to the output queue, and returns the
//! oldest queued output. When nothing can make progress, the future parks
//! until a control-plane mutation or an in-flight task completes.

use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;
use core::task::Waker;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use futures_core::Stream as _;
use futures_util::stream::FuturesUnordered;

#[cfg(feature = "runtime-events")]
use crate::event::RuntimeEvent;

use crate::control::AddGeneratorError;
use crate::control::GeneratorConfig;
use crate::control::RuntimeConfig;
use crate::control::RuntimeHandle;
use crate::control::Shared;
use crate::control::TargetLimits;
use crate::generator::CompletionOutcome;
use crate::generator::CostUnits;
use crate::generator::Generator;
use crate::generator::Readiness;
use crate::generator::ScheduledWork;
use crate::generator::ScheduledWorkResult;
use crate::generator::WorkCompletion;
use crate::ids::ClassId;
use crate::ids::GeneratorId;
use crate::ids::TargetId;
use crate::output::OutputQueueStats;
use crate::output::RuntimeOutput;
use crate::output::WorkError;
use crate::output::WorkResult;
use crate::output::WorkSuccess;
use crate::stats::ClassStats;
use crate::stats::GeneratorStats;
use crate::stats::RuntimeStats;
use crate::stats::TargetStats;
use crate::stats::WorkStats;

/// In-crate waker slot used to park `Runtime::next` when no work can be made.
///
/// Control-plane mutations and completed in-flight tasks call
/// [`WakerSlot::wake`] to resume the parked task.
pub struct WakerSlot {
    inner: Mutex<Option<Waker>>,
}

impl WakerSlot {
    pub(crate) const fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub(crate) fn register(&self, waker: &Waker) {
        let mut g = self.inner.lock().expect("waker slot poisoned");
        *g = Some(waker.clone());
    }

    pub(crate) fn wake(&self) {
        let waker = {
            let mut g = self.inner.lock().expect("waker slot poisoned");
            g.take()
        };
        if let Some(w) = waker {
            w.wake();
        }
    }
}

/// Virtual-time stride base for the per-class weighted DRR.
///
/// Each successful dispatch advances the chosen class's `pass` by
/// `STRIDE_BASE / weight`. Because comparisons are relative, the absolute
/// scale only needs to be large enough that integer truncation does not
/// distort the ratio between practical weights (1..1024). `1 << 32` is
/// comfortably larger than any reasonable weight while leaving
/// `u64::MAX / STRIDE_BASE = 2^32` safe dispatches before saturation kicks
/// in.
const STRIDE_BASE: u64 = 1 << 32;

/// Per-class scheduling record kept inside each `TargetEntry`.
///
/// Phase 2 implements weighted deficit-round-robin across classes that share
/// a target: each class accumulates virtual time in `pass` proportional to
/// `STRIDE_BASE / weight`, and the scheduler picks the class with the
/// smallest `pass` (ties broken by class_order index). Within a class,
/// `members` are visited in plain round-robin order via `cursor`.
pub(crate) struct ClassSchedState {
    /// Effective class weight, derived from `max(member.config.weight)`.
    /// Always ≥ 1 so `STRIDE_BASE / weight` is finite.
    pub(crate) weight: u32,
    /// Virtual-time accumulator. Smaller is "owed more dispatches".
    pub(crate) pass: u64,
    /// Class members in insertion order; stable across additions.
    pub(crate) members: Vec<GeneratorId>,
    /// Round-robin position within `members`.
    pub(crate) cursor: usize,
}

pub(crate) struct GeneratorEntry<Ev, Err> {
    pub(crate) generator: Box<dyn Generator<Ev, Err> + Send>,
    pub(crate) paused: bool,
    pub(crate) config: GeneratorConfig,
    pub(crate) stats: GeneratorStats,
    pub(crate) trigger_pending: bool,
    /// Number of consecutive scheduling rounds in which this generator's work
    /// was admission-blocked by the per-target round budget. Reset to zero on
    /// successful admission. Phase 1 anti-starvation uses this counter to
    /// grant a one-shot exception once the deficit reaches the configured
    /// `weight` threshold.
    pub(crate) deficit: u32,
    /// True once the user has called `remove_generator` while work was still
    /// in flight. The entry is kept alive (tombstoned) so the runtime can
    /// still call `on_complete` on the original generator object; it is
    /// dropped from the map once `stats.in_flight` reaches zero.
    pub(crate) removed: bool,
    /// Phase 3 cached `Readiness` from the most recent `update_ready` call.
    /// Re-used by subsequent `select_candidate` scans until either the
    /// generator's `next_update_at` deadline is reached, a control-plane
    /// mutation invalidates it, or `take_next` is called and clears it.
    pub(crate) cached_readiness: Option<Readiness>,
    /// Phase 4 timestamp of the most recently observed `WorkCompletion`.
    /// `finalize_completion` computes `GeneratorStats::actual_interval` as
    /// the wall-clock delta between successive completions, seeding the
    /// first one from the dispatch latency so single-completion observers
    /// already see `Some(_)`.
    pub(crate) last_completion_at: Option<Instant>,
}

pub(crate) struct TargetEntry<Ev, Err> {
    pub(crate) limits: TargetLimits,
    pub(crate) paused: bool,
    pub(crate) generators: Vec<GeneratorId>,
    pub(crate) in_flight: u64,
    pub(crate) dispatched: u64,
    pub(crate) next_gen_seq: u64,
    /// Cumulative cost charged to the current scheduling round. Reset to
    /// `CostUnits::ZERO` whenever `in_flight` returns to zero (the natural
    /// round boundary for both immediate and pending in-flight work).
    pub(crate) round_cost: CostUnits,
    /// True once the user has called `remove_target` while a child work item
    /// was still in flight. The entry stays alive until `in_flight` drops to
    /// zero so completion bookkeeping can still find the target.
    pub(crate) removed: bool,
    /// Class insertion order, used as the deterministic tie-breaker when
    /// multiple classes have an equal `pass` value during selection.
    pub(crate) class_order: Vec<Option<ClassId>>,
    /// Per-class weighted-DRR state, keyed by `Option<ClassId>` (the
    /// implicit `<unclassified>` bucket is the `None` key).
    pub(crate) class_state: HashMap<Option<ClassId>, ClassSchedState>,
    pub(crate) _marker: PhantomData<fn() -> (Ev, Err)>,
}

/// Mutable runtime state guarded by a single mutex.
///
/// All control-plane mutations and `next()` runtime bookkeeping happen under
/// this lock, but the lock is *never* held while polling user-supplied
/// futures. `Runtime::next` always drops the lock before awaiting work.
pub struct RuntimeState<Ev, Err> {
    pub(crate) config: RuntimeConfig,
    pub(crate) targets: HashMap<TargetId, TargetEntry<Ev, Err>>,
    pub(crate) generators: HashMap<GeneratorId, GeneratorEntry<Ev, Err>>,
    pub(crate) target_order: Vec<TargetId>,
    pub(crate) generator_order: Vec<GeneratorId>,
    pub(crate) output_queue: VecDeque<RuntimeOutput<Ev, Err>>,
    pub(crate) output_dropped: u64,
    /// Whether an `EventQueuePressure` event has been emitted for the
    /// current "high-water" cycle. Cleared in `pop_output` once the queue
    /// depth drops back below the watermark, so the event stream itself
    /// does not amplify pressure.
    #[cfg(feature = "runtime-events")]
    pub(crate) queue_pressure_active: bool,
    pub(crate) shutdown_started: bool,
    pub(crate) total_in_flight: u64,
    pub(crate) total_dispatched: u64,
    next_target_seq: u64,
}

impl<Ev, Err> RuntimeState<Ev, Err> {
    fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            targets: HashMap::new(),
            generators: HashMap::new(),
            target_order: Vec::new(),
            generator_order: Vec::new(),
            output_queue: VecDeque::new(),
            output_dropped: 0,
            #[cfg(feature = "runtime-events")]
            queue_pressure_active: false,
            shutdown_started: false,
            total_in_flight: 0,
            total_dispatched: 0,
            next_target_seq: 0,
        }
    }

    /// Drop every generator's cached `Readiness`.
    ///
    /// Phase 3 caches the result of `update_ready` in
    /// `GeneratorEntry::cached_readiness` so that repeated `select_candidate`
    /// scans do not re-poll a generator that already reported "not ready
    /// until deadline". Any control-plane mutation that materially changes
    /// what the user expects the next pick to look like (target/generator
    /// add/remove, pause/resume, limit/config updates, manual triggers) must
    /// invalidate the cache so the next dispatch round consults the
    /// generator afresh.
    fn invalidate_all_readiness(&mut self) {
        for gen in self.generators.values_mut() {
            gen.cached_readiness = None;
        }
    }

    pub(crate) fn add_target(&mut self, limits: TargetLimits) -> Option<TargetId> {
        if self.shutdown_started {
            return None;
        }
        let id = TargetId::from_seq(self.next_target_seq);
        self.next_target_seq += 1;
        self.targets.insert(
            id,
            TargetEntry {
                limits,
                paused: false,
                generators: Vec::new(),
                in_flight: 0,
                dispatched: 0,
                next_gen_seq: 0,
                round_cost: CostUnits::ZERO,
                removed: false,
                class_order: Vec::new(),
                class_state: HashMap::new(),
                _marker: PhantomData,
            },
        );
        self.target_order.push(id);
        self.invalidate_all_readiness();
        Some(id)
    }

    pub(crate) fn remove_target(&mut self, id: TargetId) -> bool {
        if !self.targets.contains_key(&id) {
            return false;
        }
        // Always detach the target from the public-facing order so it is no
        // longer eligible for new dispatches and disappears from stats.
        self.target_order.retain(|t| *t != id);

        // Snapshot the attached generator ids before mutating the maps.
        let attached: Vec<GeneratorId> = self
            .targets
            .get(&id)
            .map(|t| t.generators.clone())
            .unwrap_or_default();

        for gen_id in attached {
            // Detach from public scheduling order regardless of in-flight
            // status; tombstoned generators must not be picked again.
            self.generator_order.retain(|g| *g != gen_id);
            // Mark the (now-detached) target's local list as empty for this
            // generator so subsequent stats snapshots drop it from view.
            if let Some(t) = self.targets.get_mut(&id) {
                t.generators.retain(|g| *g != gen_id);
            }
            // If no work is in flight for this generator we can drop it
            // outright. Otherwise tombstone it so the in-flight completion
            // can still reach `on_complete`.
            let still_in_flight = self
                .generators
                .get(&gen_id)
                .is_some_and(|g| g.stats.in_flight > 0);
            if still_in_flight {
                if let Some(g) = self.generators.get_mut(&gen_id) {
                    g.removed = true;
                }
            } else {
                self.generators.remove(&gen_id);
            }
        }

        // Decide the target's fate by the post-detach in-flight count. If a
        // child completion is still pending the target entry must remain in
        // the map so finalize_completion can decrement and reset round_cost.
        let target_still_in_flight = self
            .targets
            .get(&id)
            .is_some_and(|t| t.in_flight > 0);
        if target_still_in_flight {
            if let Some(t) = self.targets.get_mut(&id) {
                t.removed = true;
            }
        } else {
            self.targets.remove(&id);
        }

        self.invalidate_all_readiness();
        true
    }

    pub(crate) fn update_target_limits(&mut self, id: TargetId, limits: TargetLimits) -> bool {
        let Some(entry) = self.targets.get_mut(&id) else {
            return false;
        };
        entry.limits = limits;
        self.invalidate_all_readiness();
        true
    }

    pub(crate) fn pause_target(&mut self, id: TargetId) -> bool {
        let Some(entry) = self.targets.get_mut(&id) else {
            return false;
        };
        entry.paused = true;
        self.invalidate_all_readiness();
        true
    }

    pub(crate) fn resume_target(&mut self, id: TargetId) -> bool {
        let Some(entry) = self.targets.get_mut(&id) else {
            return false;
        };
        entry.paused = false;
        self.invalidate_all_readiness();
        true
    }

    pub(crate) fn add_generator(
        &mut self,
        target: TargetId,
        generator: Box<dyn Generator<Ev, Err> + Send>,
        config: GeneratorConfig,
    ) -> Result<GeneratorId, AddGeneratorError> {
        if self.shutdown_started {
            return Err(AddGeneratorError::ShutdownStarted);
        }
        let Some(target_entry) = self.targets.get_mut(&target) else {
            return Err(AddGeneratorError::TargetNotFound);
        };
        let seq = target_entry.next_gen_seq;
        target_entry.next_gen_seq += 1;
        let id = GeneratorId::new(target, seq);
        target_entry.generators.push(id);

        // Register the generator into its class scheduling bucket. The
        // bucket is created on first use and `class_order` records the
        // insertion order to give a deterministic tie-break later in
        // `select_candidate`.
        let class_key = config.class.clone();
        let member_weight = config.weight.unwrap_or(1).max(1);
        if let Some(cls) = target_entry.class_state.get_mut(&class_key) {
            cls.members.push(id);
            cls.weight = cls.weight.max(member_weight);
        } else {
            target_entry.class_state.insert(
                class_key.clone(),
                ClassSchedState {
                    weight: member_weight,
                    pass: 0,
                    members: vec![id],
                    cursor: 0,
                },
            );
            target_entry.class_order.push(class_key);
        }

        self.generators.insert(
            id,
            GeneratorEntry {
                generator,
                paused: false,
                config,
                stats: GeneratorStats::default(),
                trigger_pending: false,
                deficit: 0,
                removed: false,
                cached_readiness: None,
                last_completion_at: None,
            },
        );
        self.generator_order.push(id);
        self.invalidate_all_readiness();
        Ok(id)
    }

    pub(crate) fn remove_generator(&mut self, id: GeneratorId) -> bool {
        // Resolve the generator's class key while we still have a borrow on
        // its entry. The class key is needed to detach it from the per-class
        // scheduling state.
        let class_key = match self.generators.get_mut(&id) {
            Some(entry) => {
                let key = entry.config.class.clone();
                if entry.stats.in_flight == 0 {
                    self.generators.remove(&id);
                } else {
                    entry.removed = true;
                }
                key
            }
            None => return false,
        };
        self.generator_order.retain(|g| *g != id);
        let target_id = id.target_id();
        if let Some(t) = self.targets.get_mut(&target_id) {
            t.generators.retain(|g| *g != id);
            detach_member_from_class(t, class_key.as_ref(), id, &self.generators);
        }
        self.invalidate_all_readiness();
        true
    }

    pub(crate) fn update_generator(&mut self, id: GeneratorId, config: GeneratorConfig) -> bool {
        let Some(entry) = self.generators.get_mut(&id) else {
            return false;
        };
        let old_class = entry.config.class.clone();
        let new_class = config.class.clone();
        entry.config = config;
        let target_id = id.target_id();
        if old_class == new_class {
            // Same class; just refresh the class weight from current members
            // in case the user lowered/raised this generator's weight.
            if let Some(t) = self.targets.get_mut(&target_id) {
                recompute_class_weight(t, new_class.as_ref(), &self.generators);
            }
            self.invalidate_all_readiness();
            return true;
        }
        // Class changed: detach from the old class, attach to the new class.
        if let Some(t) = self.targets.get_mut(&target_id) {
            detach_member_from_class(t, old_class.as_ref(), id, &self.generators);
            attach_member_to_class(t, new_class.as_ref(), id, &self.generators);
        }
        self.invalidate_all_readiness();
        true
    }

    pub(crate) fn pause_generator(&mut self, id: GeneratorId) -> bool {
        let Some(entry) = self.generators.get_mut(&id) else {
            return false;
        };
        entry.paused = true;
        self.invalidate_all_readiness();
        true
    }

    pub(crate) fn resume_generator(&mut self, id: GeneratorId) -> bool {
        let Some(entry) = self.generators.get_mut(&id) else {
            return false;
        };
        entry.paused = false;
        self.invalidate_all_readiness();
        true
    }

    pub(crate) fn trigger_generator(&mut self, id: GeneratorId) -> bool {
        let Some(entry) = self.generators.get_mut(&id) else {
            return false;
        };
        entry.trigger_pending = true;
        self.invalidate_all_readiness();
        true
    }

    // Without `runtime-events` this method body is trivially `const`-able,
    // but with the feature enabled it borrows generators mutably and
    // enqueues events. Allow the lint so a single signature serves both.
    #[allow(clippy::missing_const_for_fn)]
    pub(crate) fn start_shutdown(&mut self) -> bool {
        if self.shutdown_started {
            return false;
        }
        self.shutdown_started = true;

        // Phase 5: any generator that still reports ready at shutdown time
        // is by definition lagging — its ready work will either drain on the
        // way out or never run at all. Emit one `GeneratorLagging` per such
        // generator. The two-step collect-then-emit avoids overlapping
        // mutable borrows of `self.generators` and `self.output_queue`.
        #[cfg(feature = "runtime-events")]
        {
            let now = Instant::now();
            let lagging: Vec<GeneratorId> = self
                .generators
                .iter_mut()
                .filter_map(|(id, gen)| {
                    let r = gen.generator.update_ready(now);
                    gen.cached_readiness = Some(r);
                    if r.ready { Some(*id) } else { None }
                })
                .collect();
            for id in lagging {
                self.enqueue_output(RuntimeOutput::Runtime(
                    RuntimeEvent::GeneratorLagging { generator_id: id },
                ));
            }
        }

        true
    }

    pub(crate) fn enqueue_output(&mut self, output: RuntimeOutput<Ev, Err>) {
        if let Some(cap) = self.config.output_queue_capacity {
            if self.output_queue.len() >= cap {
                self.output_dropped += 1;
                return;
            }
        }
        self.output_queue.push_back(output);

        // Phase 5: when the bounded queue rises to the high-water mark
        // (`max(cap/2, 1)`) emit a single `EventQueuePressure` event and
        // latch `queue_pressure_active` so we don't re-emit on every
        // subsequent enqueue while pressure persists. Unbounded queues
        // never trigger pressure.
        #[cfg(feature = "runtime-events")]
        if !self.queue_pressure_active {
            if let Some(cap) = self.config.output_queue_capacity {
                let watermark = (cap / 2).max(1);
                if self.output_queue.len() >= watermark {
                    self.queue_pressure_active = true;
                    let queued = self.output_queue.len();
                    let event = RuntimeOutput::Runtime(
                        RuntimeEvent::EventQueuePressure { queued },
                    );
                    if self.output_queue.len() < cap {
                        self.output_queue.push_back(event);
                    } else {
                        self.output_dropped += 1;
                    }
                }
            }
        }
    }

    pub(crate) fn pop_output(&mut self) -> Option<RuntimeOutput<Ev, Err>> {
        let out = self.output_queue.pop_front();
        #[cfg(feature = "runtime-events")]
        if self.queue_pressure_active {
            if let Some(cap) = self.config.output_queue_capacity {
                let watermark = (cap / 2).max(1);
                if self.output_queue.len() < watermark {
                    self.queue_pressure_active = false;
                }
            }
        }
        out
    }

    pub(crate) fn output_queue_stats(&self) -> OutputQueueStats {
        OutputQueueStats {
            queued: self.output_queue.len(),
            capacity: self.config.output_queue_capacity,
            dropped: self.output_dropped,
        }
    }

    pub(crate) fn stats_snapshot(&self) -> RuntimeStats {
        let per_target = self
            .target_order
            .iter()
            .filter_map(|tid| {
                self.targets.get(tid).map(|t| {
                    let per_generator = t
                        .generators
                        .iter()
                        .filter_map(|gid| {
                            self.generators.get(gid).map(|g| (*gid, g.stats))
                        })
                        .collect::<Vec<_>>();
                    TargetStats {
                        target: Some(*tid),
                        generators: t.generators.len() as u64,
                        in_flight: t.in_flight,
                        dispatched: t.dispatched,
                        per_generator,
                    }
                })
            })
            .collect::<Vec<_>>();
        RuntimeStats {
            // Use the public-facing order vectors so tombstoned (removed-
            // but-still-completing) entries do not leak into user-visible
            // counts.
            targets: self.target_order.len() as u64,
            generators: self.generator_order.len() as u64,
            in_flight: self.total_in_flight,
            dispatched: self.total_dispatched,
            output_queue: self.output_queue_stats(),
            per_target,
        }
    }

    pub(crate) fn class_stats_snapshot(&self) -> Vec<ClassStats> {
        let mut by_class: HashMap<Option<ClassId>, ClassStats> = HashMap::new();
        for gen in self.generators.values() {
            // Skip tombstoned generators so per-class numbers reflect only
            // the visible runtime.
            if gen.removed {
                continue;
            }
            let key = gen.config.class.clone();
            let entry = by_class.entry(key.clone()).or_insert_with(|| ClassStats {
                class: key,
                ..ClassStats::default()
            });
            entry.dispatched += gen.stats.dispatched;
            entry.in_flight += gen.stats.in_flight;
        }
        by_class.into_values().collect()
    }
}

/// Detach a generator id from its class scheduling state.
///
/// On detachment the class's `members` vector loses the id, the cursor is
/// clamped, and `class.weight` is recomputed as the maximum of the remaining
/// members' configured weights. If the class becomes empty it is removed
/// from both `class_state` and `class_order` so it does not occupy a slot
/// in subsequent selection scans.
fn detach_member_from_class<Ev, Err>(
    target: &mut TargetEntry<Ev, Err>,
    class_key: Option<&ClassId>,
    gid: GeneratorId,
    all_generators: &HashMap<GeneratorId, GeneratorEntry<Ev, Err>>,
) {
    let Some(class) = target.class_state.get_mut(&class_key.cloned()) else {
        return;
    };
    class.members.retain(|m| *m != gid);
    if class.members.is_empty() {
        target.class_state.remove(&class_key.cloned());
        target.class_order.retain(|k| k.as_ref() != class_key);
        return;
    }
    if class.cursor >= class.members.len() {
        class.cursor = 0;
    }
    class.weight = max_weight_for_members(&class.members, all_generators);
}

/// Attach a generator id to its (possibly new) class scheduling state.
///
/// Mirrors the registration logic in `add_generator`: creates the class on
/// first use (appending to `class_order`), otherwise appends the id to the
/// existing members vector and refreshes `class.weight` against the new
/// member set.
fn attach_member_to_class<Ev, Err>(
    target: &mut TargetEntry<Ev, Err>,
    class_key: Option<&ClassId>,
    gid: GeneratorId,
    all_generators: &HashMap<GeneratorId, GeneratorEntry<Ev, Err>>,
) {
    let owned_key = class_key.cloned();
    if let Some(class) = target.class_state.get_mut(&owned_key) {
        class.members.push(gid);
        class.weight = max_weight_for_members(&class.members, all_generators);
    } else {
        let weight = all_generators
            .get(&gid)
            .map_or(1, |g| g.config.weight.unwrap_or(1).max(1));
        target.class_state.insert(
            owned_key.clone(),
            ClassSchedState {
                weight,
                pass: 0,
                members: vec![gid],
                cursor: 0,
            },
        );
        target.class_order.push(owned_key);
    }
}

/// Refresh `class.weight` from the current member set without changing the
/// membership list itself. Useful after `update_generator` toggles a
/// generator's `weight` while keeping it in the same class.
fn recompute_class_weight<Ev, Err>(
    target: &mut TargetEntry<Ev, Err>,
    class_key: Option<&ClassId>,
    all_generators: &HashMap<GeneratorId, GeneratorEntry<Ev, Err>>,
) {
    if let Some(class) = target.class_state.get_mut(&class_key.cloned()) {
        class.weight = max_weight_for_members(&class.members, all_generators);
    }
}

fn max_weight_for_members<Ev, Err>(
    members: &[GeneratorId],
    all_generators: &HashMap<GeneratorId, GeneratorEntry<Ev, Err>>,
) -> u32 {
    let mut max = 1u32;
    for gid in members {
        if let Some(g) = all_generators.get(gid) {
            let w = g.config.weight.unwrap_or(1).max(1);
            if w > max {
                max = w;
            }
        }
    }
    max
}

/// In-flight work item polled inside [`Runtime::next`].
struct InFlight<Ev, Err> {
    generator_id: GeneratorId,
    cost: CostUnits,
    started_at: Instant,
    future: Pin<Box<dyn Future<Output = ScheduledWorkResult<Ev, Err>> + Send + 'static>>,
}

impl<Ev, Err> Future for InFlight<Ev, Err> {
    type Output = (GeneratorId, CostUnits, Instant, ScheduledWorkResult<Ev, Err>);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let s = self.get_mut();
        match s.future.as_mut().poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(r) => Poll::Ready((s.generator_id, s.cost, s.started_at, r)),
        }
    }
}

/// Generic scraper runtime parameterized by application work event type `Ev`
/// and work error type `Err`.
///
/// The runtime is *not* `Clone`. Only one consumer drives the output stream
/// via [`Runtime::next`]. Use [`Runtime::handle`] to obtain cloneable control
/// handles for cross-task control operations.
pub struct Runtime<Ev, Err> {
    shared: Arc<Shared<Ev, Err>>,
    in_flight: FuturesUnordered<InFlight<Ev, Err>>,
    shutdown_emitted: bool,
}

impl<Ev, Err> Runtime<Ev, Err>
where
    Ev: Send + 'static,
    Err: Send + 'static,
{
    /// Build a new runtime with the given configuration.
    #[must_use]
    pub fn new(config: RuntimeConfig) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(RuntimeState::new(config)),
            waker: WakerSlot::new(),
        });
        Self {
            shared,
            in_flight: FuturesUnordered::new(),
            shutdown_emitted: false,
        }
    }

    /// Return a cloneable handle that exposes the synchronous control surface.
    #[must_use]
    pub fn handle(&self) -> RuntimeHandle<Ev, Err> {
        RuntimeHandle {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Add a target to the runtime and return its newly-allocated id.
    ///
    /// Forwarded convenience for [`RuntimeHandle::add_target`].
    #[must_use]
    pub fn add_target(&self, limits: TargetLimits) -> Option<TargetId> {
        self.handle().add_target(limits)
    }

    /// Remove a target. Forwarded convenience for [`RuntimeHandle::remove_target`].
    #[must_use]
    pub fn remove_target(&self, id: TargetId) -> bool {
        self.handle().remove_target(id)
    }

    /// Update target limits. Forwarded convenience.
    #[must_use]
    pub fn update_target_limits(&self, id: TargetId, limits: TargetLimits) -> bool {
        self.handle().update_target_limits(id, limits)
    }

    /// Pause a target. Forwarded convenience.
    #[must_use]
    pub fn pause_target(&self, id: TargetId) -> bool {
        self.handle().pause_target(id)
    }

    /// Resume a target. Forwarded convenience.
    #[must_use]
    pub fn resume_target(&self, id: TargetId) -> bool {
        self.handle().resume_target(id)
    }

    /// Add a generator. Forwarded convenience for [`RuntimeHandle::add_generator`].
    ///
    /// # Errors
    ///
    /// Forwarded from [`RuntimeHandle::add_generator`].
    pub fn add_generator(
        &self,
        target: TargetId,
        generator: Box<dyn Generator<Ev, Err> + Send>,
        config: GeneratorConfig,
    ) -> Result<GeneratorId, AddGeneratorError> {
        self.handle().add_generator(target, generator, config)
    }

    /// Remove a generator. Forwarded convenience.
    #[must_use]
    pub fn remove_generator(&self, id: GeneratorId) -> bool {
        self.handle().remove_generator(id)
    }

    /// Update generator config. Forwarded convenience.
    #[must_use]
    pub fn update_generator(&self, id: GeneratorId, config: GeneratorConfig) -> bool {
        self.handle().update_generator(id, config)
    }

    /// Pause a generator. Forwarded convenience.
    #[must_use]
    pub fn pause_generator(&self, id: GeneratorId) -> bool {
        self.handle().pause_generator(id)
    }

    /// Resume a generator. Forwarded convenience.
    #[must_use]
    pub fn resume_generator(&self, id: GeneratorId) -> bool {
        self.handle().resume_generator(id)
    }

    /// Trigger a generator. Forwarded convenience.
    #[must_use]
    pub fn trigger_generator(&self, id: GeneratorId) -> bool {
        self.handle().trigger_generator(id)
    }

    /// Begin graceful shutdown. Forwarded convenience.
    pub fn graceful_shutdown(&self) {
        self.handle().graceful_shutdown();
    }

    /// Snapshot of runtime statistics.
    #[must_use]
    pub fn stats(&self) -> RuntimeStats {
        self.handle().stats()
    }

    /// Snapshot of per-class statistics.
    #[must_use]
    pub fn class_stats(&self) -> Vec<ClassStats> {
        let g = self.shared.lock_state();
        g.class_stats_snapshot()
    }

    /// Drive the runtime by one step and return the next ordered output.
    ///
    /// Behavior summary:
    ///
    /// 1. If already-emitted shutdown is sticky, return it again.
    /// 2. Drain at most one already-queued output and return it.
    /// 3. Poll in-flight work; on completion enqueue the corresponding
    ///    [`RuntimeOutput::Work`] and call [`Generator::on_complete`] exactly
    ///    once.
    /// 4. If shutdown has started and there is no further in-flight work or
    ///    queued output, emit the sticky shutdown output.
    /// 5. Otherwise scan generators for the first ready one, call
    ///    `take_next`, and admit the future to the in-flight set.
    /// 6. If nothing can make progress, register the current waker and park.
    #[allow(clippy::should_implement_trait)] // public API name fixed by docs/scraper/phase-0.md
    pub const fn next(&mut self) -> NextFuture<'_, Ev, Err> {
        NextFuture { runtime: self }
    }
}

/// Future returned by [`Runtime::next`]. See its docs for behavior.
pub struct NextFuture<'r, Ev, Err> {
    runtime: &'r mut Runtime<Ev, Err>,
}

impl<Ev, Err> Future for NextFuture<'_, Ev, Err>
where
    Ev: Send + 'static,
    Err: Send + 'static,
{
    type Output = RuntimeOutput<Ev, Err>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        loop {
            if this.runtime.shutdown_emitted {
                return Poll::Ready(RuntimeOutput::Shutdown);
            }

            if let Some(out) = pop_output(&*this.runtime) {
                return Poll::Ready(out);
            }

            if poll_in_flight(&mut *this.runtime, cx) {
                continue;
            }

            // Phase 4: when the user opted into a bounded output queue,
            // dispatch up to `cap + 1` ready items per poll so overflow is
            // observable through `OutputQueueStats::dropped`. For unbounded
            // queues the budget is `1`, preserving Phase 0's single-dispatch
            // cadence and all fairness/cost-admission tests that assume it.
            //
            // Phase 5: try_dispatch_one must run *before* try_emit_shutdown so
            // that `graceful_shutdown` drains any still-ready work before the
            // sticky `Shutdown` output is emitted.
            let budget: usize = {
                let g = this.runtime.shared.lock_state();
                g.config
                    .output_queue_capacity
                    .map_or(1, |c| c.saturating_add(1))
            };
            let mut dispatched_any = false;
            for _ in 0..budget {
                if try_dispatch_one(&*this.runtime) {
                    dispatched_any = true;
                } else {
                    break;
                }
            }
            if dispatched_any {
                continue;
            }

            if try_emit_shutdown(&mut *this.runtime) {
                continue;
            }

            this.runtime.shared.waker.register(cx.waker());

            if poll_in_flight(&mut *this.runtime, cx) {
                continue;
            }
            if has_queued(&*this.runtime) {
                continue;
            }
            return Poll::Pending;
        }
    }
}

fn pop_output<Ev, Err>(runtime: &Runtime<Ev, Err>) -> Option<RuntimeOutput<Ev, Err>> {
    let mut g = runtime.shared.lock_state();
    g.pop_output()
}

fn has_queued<Ev, Err>(runtime: &Runtime<Ev, Err>) -> bool {
    let g = runtime.shared.lock_state();
    !g.output_queue.is_empty()
}

/// Poll the in-flight set, draining every completion that is immediately
/// ready in this poll cycle.
///
/// Phase 4 changes this from "drain at most one per call" to "drain all
/// ready" so that bounded output queues can observe back-pressure: when
/// several immediate-ready futures complete together, the queue's
/// `output_dropped` counter increments for each enqueue past capacity.
/// For the common single-in-flight case the behaviour is unchanged.
fn poll_in_flight<Ev, Err>(runtime: &mut Runtime<Ev, Err>, cx: &mut Context<'_>) -> bool
where
    Ev: Send + 'static,
    Err: Send + 'static,
{
    let mut any = false;
    loop {
        let polled = Pin::new(&mut runtime.in_flight).poll_next(cx);
        match polled {
            Poll::Ready(Some((generator_id, cost, started_at, result))) => {
                finalize_completion(runtime, generator_id, cost, started_at, result);
                any = true;
            }
            Poll::Ready(None) | Poll::Pending => return any,
        }
    }
}

fn finalize_completion<Ev, Err>(
    runtime: &Runtime<Ev, Err>,
    generator_id: GeneratorId,
    cost: CostUnits,
    started_at: Instant,
    result: ScheduledWorkResult<Ev, Err>,
) {
    let latency = Instant::now().saturating_duration_since(started_at);
    let stats = WorkStats { latency };
    let outcome = if result.is_ok() {
        CompletionOutcome::Succeeded
    } else {
        CompletionOutcome::Failed
    };
    let work_result: WorkResult<Ev, Err> = match result {
        Ok(events) => Ok(WorkSuccess {
            events,
            stats,
            generator_id,
        }),
        Err(error) => Err(WorkError {
            error,
            stats,
            generator_id,
        }),
    };
    let output = RuntimeOutput::Work(work_result);

    let mut g = runtime.shared.lock_state();

    g.enqueue_output(output);

    // Phase 5: emit the matching `WorkCompleted` / `WorkFailed` event
    // immediately after the work output to preserve the bracket order.
    #[cfg(feature = "runtime-events")]
    {
        let event = match outcome {
            CompletionOutcome::Succeeded => {
                RuntimeEvent::WorkCompleted { generator_id }
            }
            CompletionOutcome::Failed => {
                RuntimeEvent::WorkFailed { generator_id }
            }
        };
        g.enqueue_output(RuntimeOutput::Runtime(event));
    }

    let target_id = generator_id.target_id();
    let mut drop_target = false;
    if let Some(t) = g.targets.get_mut(&target_id) {
        if t.in_flight > 0 {
            t.in_flight -= 1;
        }
        // Reset the per-target round budget when the target's in-flight
        // count returns to zero. This is the natural round boundary used by
        // Phase 1 cost-aware admission.
        if t.in_flight == 0 {
            t.round_cost = CostUnits::ZERO;
        }
        // A tombstoned target is held alive only until its child completion
        // is bookkept; once in_flight is back to zero, drop it for real.
        if t.removed && t.in_flight == 0 {
            drop_target = true;
        }
    }
    if g.total_in_flight > 0 {
        g.total_in_flight -= 1;
    }

    let mut drop_gen = false;
    if let Some(gen) = g.generators.get_mut(&generator_id) {
        match outcome {
            CompletionOutcome::Succeeded => gen.stats.succeeded += 1,
            CompletionOutcome::Failed => gen.stats.failed += 1,
        }
        if gen.stats.in_flight > 0 {
            gen.stats.in_flight -= 1;
        }
        // Phase 4: track wall-clock interval between consecutive
        // completions. The first completion seeds `actual_interval` from
        // the dispatch latency so single-completion observers already see
        // a populated value; subsequent completions report the delta from
        // the previous completion timestamp.
        let now = Instant::now();
        let prev = gen.last_completion_at.replace(now);
        let interval = prev.map_or(latency, |p| now.saturating_duration_since(p));
        gen.stats.actual_interval = Some(interval);
        let completion = WorkCompletion {
            generator_id,
            outcome,
            cost,
            latency,
        };
        gen.generator.on_complete(&completion);
        if gen.removed && gen.stats.in_flight == 0 {
            drop_gen = true;
        }
    }

    // Drop tombstoned entries only after their `on_complete` has fired and
    // their in-flight counters have returned to zero. The order matches the
    // generator-then-target lifetime relationship.
    if drop_gen {
        g.generators.remove(&generator_id);
    }
    if drop_target {
        g.targets.remove(&target_id);
    }
}

fn try_emit_shutdown<Ev, Err>(runtime: &mut Runtime<Ev, Err>) -> bool {
    let should = {
        let g = runtime.shared.lock_state();
        let s = g.shutdown_started && g.output_queue.is_empty() && runtime.in_flight.is_empty();
        drop(g);
        s
    };
    if !should {
        return false;
    }
    runtime.shutdown_emitted = true;
    let mut g = runtime.shared.lock_state();
    g.enqueue_output(RuntimeOutput::Shutdown);
    true
}

/// Scan generators in insertion order for the first ready one and dispatch it.
fn try_dispatch_one<Ev, Err>(runtime: &Runtime<Ev, Err>) -> bool
where
    Ev: Send + 'static,
    Err: Send + 'static,
{
    let now = Instant::now();
    let candidate = select_candidate(runtime, now);

    let Some((generator_id, work)) = candidate else {
        return false;
    };

    let started_at = Instant::now();
    let cost = work.meta.cost;

    {
        let mut g = runtime.shared.lock_state();
        g.total_dispatched += 1;
        g.total_in_flight += 1;
        if let Some(t) = g.targets.get_mut(&generator_id.target_id()) {
            t.in_flight += 1;
            t.dispatched += 1;
            // Charge the actual `WorkMeta.cost` against the per-target round
            // budget. Saturates so a misbehaving generator that reports an
            // enormous cost cannot wrap the counter.
            t.round_cost =
                CostUnits::new(t.round_cost.get().saturating_add(cost.get()));
        }
        if let Some(gen) = g.generators.get_mut(&generator_id) {
            gen.stats.dispatched += 1;
            gen.stats.in_flight += 1;
        }

        // Phase 5: emit `WorkStarted` before pushing the future into the
        // in-flight set so the bracket `WorkStarted -> Work(Ok|Err) ->
        // WorkCompleted|WorkFailed` is preserved per work item.
        #[cfg(feature = "runtime-events")]
        g.enqueue_output(RuntimeOutput::Runtime(
            RuntimeEvent::WorkStarted { generator_id },
        ));

        drop(g);
    }

    runtime.in_flight.push(InFlight {
        generator_id,
        cost,
        started_at,
        future: work.future,
    });
    true
}

fn select_candidate<Ev, Err>(
    runtime: &Runtime<Ev, Err>,
    now: Instant,
) -> Option<(GeneratorId, ScheduledWork<Ev, Err>)>
where
    Ev: Send + 'static,
    Err: Send + 'static,
{
    let chosen;
    {
        let mut g = runtime.shared.lock_state();

        // Phase 5: graceful_shutdown drains existing ready work; we no
        // longer short-circuit dispatch when shutdown_started. Control-plane
        // mutations (add_target/add_generator/...) still reject after
        // shutdown, and `try_emit_shutdown` waits for queue+in_flight to
        // reach zero before emitting `Shutdown`.

        // Level 1: order targets by ascending `dispatched` (uniform-weight
        // DRR across targets), ties broken by `target_order` position.
        let mut target_candidates: Vec<(u64, usize, TargetId)> = g
            .target_order
            .iter()
            .enumerate()
            .filter_map(|(i, tid)| g.targets.get(tid).map(|t| (t.dispatched, i, *tid)))
            .collect();
        if target_candidates.is_empty() {
            drop(g);
            return None;
        }
        target_candidates.sort_by_key(|(disp, ord_idx, _)| (*disp, *ord_idx));

        let mut found: Option<(GeneratorId, ScheduledWork<Ev, Err>)> = None;
        'targets: for (_disp, _ord_idx, tid) in target_candidates {
            if !target_is_eligible(&g, tid) {
                continue;
            }
            let Some(t) = g.targets.get(&tid) else { continue };
            let max_cost_per_round = t.limits.max_cost_per_round;
            let round_cost = t.round_cost;
            // Level 2: order classes within this target by ascending `pass`
            // (weighted DRR across classes), ties by class_order position.
            let mut class_candidates: Vec<(u64, usize, Option<ClassId>)> = t
                .class_order
                .iter()
                .enumerate()
                .filter_map(|(i, k)| t.class_state.get(k).map(|c| (c.pass, i, k.clone())))
                .collect();
            class_candidates.sort_by_key(|(pass, ord_idx, _)| (*pass, *ord_idx));

            for (_pass, _cls_idx, class_key) in class_candidates {
                if let Some(work) = dispatch_from_class(
                    &mut g,
                    tid,
                    class_key.as_ref(),
                    max_cost_per_round,
                    round_cost,
                    now,
                ) {
                    found = Some(work);
                    break 'targets;
                }
            }
        }
        chosen = found;
        drop(g);
    }
    chosen
}

/// Return the `Readiness` to use for this generator on this scheduling pass.
///
/// Phase 3 readiness is cached in `GeneratorEntry::cached_readiness` and
/// reused until either:
/// - the cached value reports `ready=true` (in which case `take_next` will
///   eventually clear the cache), or
/// - the cached value's `next_update_at` deadline has elapsed (`now >= d`),
///   or
/// - the cache was invalidated by a control-plane mutation
///   (`invalidate_all_readiness`).
///
/// When a cached deadline is crossed, `missed_intervals` is bumped exactly
/// once before re-polling the generator, matching the doc rule "missed
/// deadline increments missed_intervals".
fn compute_readiness<Ev, Err>(
    gen_entry: &mut GeneratorEntry<Ev, Err>,
    now: Instant,
) -> Readiness {
    let cached = gen_entry.cached_readiness;
    let reuse = cached.is_some_and(|r| {
        r.ready || r.next_update_at.is_none_or(|d| now < d)
    });
    if let Some(r) = cached {
        if reuse {
            return r;
        }
        if !r.ready && r.next_update_at.is_some_and(|d| now >= d) {
            gen_entry.stats.missed_intervals =
                gen_entry.stats.missed_intervals.saturating_add(1);
        }
    }
    let r = gen_entry.generator.update_ready(now);
    gen_entry.cached_readiness = Some(r);
    r
}

/// Try to dispatch one work item from the given class on the given target.
///
/// Implements Phase 2's level-3 round-robin across class members combined
/// with Phase 1's cost-aware admission and deficit-based exception. On a
/// successful dispatch the class's `cursor` advances past the chosen member
/// and `pass` accumulates `STRIDE_BASE / weight` so that higher-weighted
/// classes get picked more often.
fn dispatch_from_class<Ev, Err>(
    state: &mut RuntimeState<Ev, Err>,
    tid: TargetId,
    class_key: Option<&ClassId>,
    max_cost_per_round: Option<CostUnits>,
    round_cost: CostUnits,
    now: Instant,
) -> Option<(GeneratorId, ScheduledWork<Ev, Err>)>
where
    Ev: Send + 'static,
    Err: Send + 'static,
{
    let owned_key = class_key.cloned();
    let (members, cur, n) = state
        .targets
        .get(&tid)
        .and_then(|t| t.class_state.get(&owned_key))
        .filter(|c| !c.members.is_empty())
        .map(|c| {
            let n = c.members.len();
            (c.members.clone(), c.cursor.min(n.saturating_sub(1)), n)
        })?;

    for offset in 0..n {
        let idx = (cur + offset) % n;
        let gid = members[idx];
        let Some(gen_entry) = state.generators.get_mut(&gid) else {
            continue;
        };
        // Defense in depth: tombstoned/paused generators are also detached
        // from class.members so these checks are normally unreachable.
        if gen_entry.paused || gen_entry.removed {
            continue;
        }
        let readiness = compute_readiness(gen_entry, now);
        if !(readiness.ready || gen_entry.trigger_pending) {
            continue;
        }

        if let Some(max) = max_cost_per_round {
            let cost_hint = readiness.next_cost.unwrap_or(CostUnits::ZERO);
            let admit_normal =
                round_cost.get().saturating_add(cost_hint.get()) <= max.get();
            let weight = gen_entry.config.weight.unwrap_or(1).max(1);
            let admit_exception = gen_entry.deficit >= weight;
            if !admit_normal && !admit_exception {
                gen_entry.deficit = gen_entry.deficit.saturating_add(1);
                continue;
            }
            gen_entry.deficit = 0;
        }

        let work = gen_entry.generator.take_next();
        gen_entry.trigger_pending = false;
        gen_entry.cached_readiness = None;
        if let Some(work) = work {
            if let Some(c) = state
                .targets
                .get_mut(&tid)
                .and_then(|t| t.class_state.get_mut(&owned_key))
            {
                c.cursor = (idx + 1) % n;
                let stride = STRIDE_BASE / u64::from(c.weight.max(1));
                c.pass = c.pass.saturating_add(stride);
            }
            return Some((gid, work));
        }
    }
    None
}

fn target_is_eligible<Ev, Err>(state: &RuntimeState<Ev, Err>, target: TargetId) -> bool {
    let Some(t) = state.targets.get(&target) else {
        return false;
    };
    if t.paused {
        return false;
    }
    if let Some(limit) = t.limits.max_in_flight {
        if t.in_flight >= u64::from(limit) {
            return false;
        }
    }
    if let Some(global_limit) = state.config.global_max_in_flight {
        if state.total_in_flight >= u64::from(global_limit) {
            return false;
        }
    }
    true
}
