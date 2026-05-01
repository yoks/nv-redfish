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

//! Tiny deterministic linear-congruential PRNG used by property-style tests.
//!
//! Not cryptographic. Two invocations of [`Lcg::new`] with the same seed
//! produce the same sequence, which makes failing test runs reproducible by
//! seed alone.

/// Deterministic 64-bit linear congruential generator.
pub struct Lcg {
    state: u64,
}

impl Lcg {
    /// Construct an LCG with the given seed. A zero seed is rewritten to 1
    /// so the generator always advances.
    pub fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    /// Return the next pseudo-random `u64`.
    pub fn next_u64(&mut self) -> u64 {
        // Knuth's MMIX multiplier; perfectly fine for test sequencing.
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.state
    }

    /// Return a uniform value in `0..n`. Returns 0 when `n == 0`.
    pub fn pick(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % n as u64) as usize
    }

    /// Return a `bool` with the supplied probability, expressed as a fraction
    /// `numer / denom`. `denom` must be nonzero.
    pub fn coin(&mut self, numer: u64, denom: u64) -> bool {
        debug_assert!(denom > 0, "coin denominator must be > 0");
        self.next_u64() % denom < numer
    }
}
