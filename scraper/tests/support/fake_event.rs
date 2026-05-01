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

//! Fake event type used to prove the public API does not impose accidental
//! `Clone`, `Debug`, `Eq`, `PartialEq`, `Display`, or `Error` bounds on the
//! application work event type `Ev`.

/// Zero-bound fake event with an opaque integer id.
///
/// Intentionally derives nothing. Tests that need to assert no accidental
/// trait bounds use this type as the runtime's `Ev` parameter.
pub struct FakeEvent {
    id: u64,
}

impl FakeEvent {
    /// Construct a new fake event with the given id.
    pub fn new(id: u64) -> Self {
        Self { id }
    }

    /// Return the fake event id.
    pub fn id(&self) -> u64 {
        self.id
    }
}
