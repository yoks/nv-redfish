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

use crate::compiler::CompiledNavProperty;
use crate::compiler::CompiledProperty;

#[derive(Default, Debug)]
pub struct CompiledProperties<'a> {
    pub properties: Vec<CompiledProperty<'a>>,
    pub nav_properties: Vec<CompiledNavProperty<'a>>,
}

impl CompiledProperties<'_> {
    /// Join properties in reverse order.
    #[must_use]
    pub fn rev_join(src: Vec<Self>) -> Self {
        let (properties, nav_properties): (Vec<_>, Vec<_>) = src
            .into_iter()
            .map(|v| (v.properties, v.nav_properties))
            .unzip();
        Self {
            properties: properties.into_iter().rev().flatten().collect(),
            nav_properties: nav_properties.into_iter().rev().flatten().collect(),
        }
    }

    /// No properties defined.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.properties.is_empty() && self.nav_properties.is_empty()
    }
}
