// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

use core::fmt::Display;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;
use serde::Deserialize;
use std::marker::PhantomData;

/// Type for `@odata.id` identifier.
#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ActionTarget(String);

impl Display for ActionTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.0.fmt(f)
    }
}

#[derive(Deserialize, Debug)]
pub struct Action<T> {
    #[serde(rename = "target")]
    pub target: ActionTarget,
    // TODO: we can retrieve constrains on attributes here.
    #[serde(skip_deserializing)]
    pub _marker: PhantomData<T>,
}
