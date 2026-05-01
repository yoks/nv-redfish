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

//! Test-controlled future and matching generator.
//!
//! Useful for scheduling/control tests that need a work item to remain
//! "in flight" without sleeping or relying on a real async runtime. Tests
//! create a [`Trigger`], hand it to a [`ControlledGen`], and call
//! [`Trigger::fire`] when the in-flight work should resolve.

use core::future::Future;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;
use core::task::Waker;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use nv_redfish_scraper::CompletionOutcome;
use nv_redfish_scraper::CostUnits;
use nv_redfish_scraper::Generator;
use nv_redfish_scraper::Readiness;
use nv_redfish_scraper::ScheduledWork;
use nv_redfish_scraper::WorkCompletion;
use nv_redfish_scraper::WorkMeta;

use super::fake_error::FakeError;
use super::fake_event::FakeEvent;
use super::fake_generator::CallCounters;

#[derive(Default)]
struct TriggerState {
    fired: bool,
    waker: Option<Waker>,
}

/// Cloneable handle that controls when an associated future resolves.
#[derive(Clone, Default)]
pub struct Trigger {
    inner: Arc<Mutex<TriggerState>>,
}

impl Trigger {
    /// Construct a fresh trigger in the "pending" state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark the associated future ready. Subsequent polls return `Ready`.
    pub fn fire(&self) {
        let mut s = self.inner.lock().expect("trigger lock poisoned");
        s.fired = true;
        if let Some(w) = s.waker.take() {
            w.wake();
        }
    }

    /// True if [`Trigger::fire`] has been called.
    pub fn fired(&self) -> bool {
        self.inner.lock().expect("trigger lock poisoned").fired
    }

    fn poll(&self, cx: &mut Context<'_>) -> Poll<()> {
        let mut s = self.inner.lock().expect("trigger lock poisoned");
        if s.fired {
            Poll::Ready(())
        } else {
            s.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

struct TriggerFut {
    trigger: Trigger,
}

impl Future for TriggerFut {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        self.trigger.poll(cx)
    }
}

/// Generator that produces exactly one work item which stays pending until
/// its [`Trigger`] is fired, then resolves to the configured `result`.
///
/// After the single work item is taken the generator reports "not ready"
/// forever.
pub struct ControlledGen {
    trigger: Trigger,
    result: Option<Result<Vec<FakeEvent>, FakeError>>,
    cost: CostUnits,
    counters: CallCounters,
}

impl ControlledGen {
    /// Build a controlled generator that will return `result` when `trigger`
    /// fires.
    pub fn new(trigger: Trigger, result: Result<Vec<FakeEvent>, FakeError>) -> Self {
        Self {
            trigger,
            result: Some(result),
            cost: CostUnits::ZERO,
            counters: CallCounters::default(),
        }
    }

    /// Configure the cost reported for the produced work item.
    pub fn with_cost(mut self, cost: CostUnits) -> Self {
        self.cost = cost;
        self
    }

    /// Borrow the call counters; clones share the same backing counters.
    pub fn counters(&self) -> CallCounters {
        self.counters.clone()
    }
}

impl Generator<FakeEvent, FakeError> for ControlledGen {
    fn update_ready(&mut self, _now: Instant) -> Readiness {
        self.counters.tick_update_ready();
        if self.result.is_some() {
            Readiness::ready(Some(self.cost))
        } else {
            Readiness::not_ready(None)
        }
    }

    fn take_next(&mut self) -> Option<ScheduledWork<FakeEvent, FakeError>> {
        self.counters.tick_take_next();
        let result = self.result.take()?;
        let cost = self.cost;
        let trigger = self.trigger.clone();
        let fut = Box::pin(async move {
            TriggerFut { trigger }.await;
            result
        });
        Some(ScheduledWork::new(WorkMeta::with_cost(cost), fut))
    }

    fn on_complete(&mut self, completion: &WorkCompletion) {
        match completion.outcome {
            CompletionOutcome::Succeeded => self.counters.tick_on_complete_success(),
            CompletionOutcome::Failed => self.counters.tick_on_complete_failed(),
        }
    }
}
