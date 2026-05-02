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

//! Single-threaded poll harness for executor-free tests.
//!
//! Tests that want to step the runtime synchronously construct a `Harness`
//! and call `harness.poll(future)`. The harness exposes a `wakes()` counter
//! so tests can assert that pending futures are only woken via real wake
//! sources (control-plane mutations, in-flight completions).

use core::future::Future;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;
use core::task::Waker;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::Wake;

struct CountingWaker {
    count: AtomicU64,
}

impl Wake for CountingWaker {
    fn wake(self: Arc<Self>) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}

/// Polling harness used by integration tests.
pub struct Harness {
    waker_inner: Arc<CountingWaker>,
    waker: Waker,
}

impl Default for Harness {
    fn default() -> Self {
        Self::new()
    }
}

impl Harness {
    /// Build a new harness.
    pub fn new() -> Self {
        let inner = Arc::new(CountingWaker {
            count: AtomicU64::new(0),
        });
        let waker = Waker::from(inner.clone());
        Self {
            waker_inner: inner,
            waker,
        }
    }

    /// Number of times any cloned waker has been woken.
    pub fn wakes(&self) -> u64 {
        self.waker_inner.count.load(Ordering::SeqCst)
    }

    /// Poll the supplied future once.
    ///
    /// The future must be `Unpin` (the runtime's `NextFuture` is, since it
    /// is structurally just a mutable borrow of the runtime).
    pub fn poll<F: Future + Unpin>(&self, fut: &mut F) -> Poll<F::Output> {
        let mut cx = Context::from_waker(&self.waker);
        Pin::new(fut).poll(&mut cx)
    }

    /// Drive a future to completion using the harness waker.
    ///
    /// Used by Phase 6 adapter tests to evaluate `nv-redfish` constructors
    /// (for example, `ServiceRoot::new(mock).await`) synchronously, since
    /// the underlying mock futures are always immediately ready.
    ///
    /// Polls up to 1024 times before panicking; this is a sanity bound and
    /// not a meaningful retry budget.
    ///
    /// # Panics
    ///
    /// Panics if the future does not resolve within 1024 polls.
    pub fn block_on<F: Future>(&self, fut: F) -> F::Output {
        let mut fut = Box::pin(fut);
        let mut cx = Context::from_waker(&self.waker);
        for _ in 0..1024 {
            if let Poll::Ready(value) = fut.as_mut().poll(&mut cx) {
                return value;
            }
        }
        panic!("Harness::block_on: future did not resolve within 1024 polls");
    }
}
