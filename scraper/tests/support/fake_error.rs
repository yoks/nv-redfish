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

//! Fake error type used to prove the public API does not impose accidental
//! `Display` or `std::error::Error` bounds on the application work error
//! type `Err`.

/// Zero-bound fake error with an opaque integer id.
///
/// Intentionally derives nothing and does not implement `Display` or
/// `std::error::Error`. Tests that need to assert no accidental trait
/// bounds use this type as the runtime's `Err` parameter.
pub struct FakeError {
    id: u64,
}

impl FakeError {
    /// Construct a new fake error with the given id.
    pub fn new(id: u64) -> Self {
        Self { id }
    }

    /// Return the fake error id.
    pub fn id(&self) -> u64 {
        self.id
    }
}
