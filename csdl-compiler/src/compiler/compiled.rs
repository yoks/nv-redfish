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

use crate::compiler::CompiledComplexType;
use crate::compiler::CompiledEntityType;
use crate::compiler::CompiledEnumType;
use crate::compiler::CompiledSingleton;
use crate::compiler::CompiledTypeDefinition;
use crate::compiler::QualifiedName;
use crate::compiler::SimpleType;
use crate::compiler::SimpleTypeAttrs;
use std::collections::HashMap;

/// Compiled data frome schema.
#[derive(Default, Debug)]
pub struct Compiled<'a> {
    pub complex_types: HashMap<QualifiedName<'a>, CompiledComplexType<'a>>,
    pub entity_types: HashMap<QualifiedName<'a>, CompiledEntityType<'a>>,
    pub simple_types: HashMap<QualifiedName<'a>, SimpleType<'a>>,
    pub root_singletons: Vec<CompiledSingleton<'a>>,
}

impl<'a> Compiled<'a> {
    #[must_use]
    pub fn new_entity_type(v: CompiledEntityType<'a>) -> Self {
        Self {
            entity_types: vec![(v.name, v)].into_iter().collect(),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn new_complex_type(v: CompiledComplexType<'a>) -> Self {
        Self {
            complex_types: vec![(v.name, v)].into_iter().collect(),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn new_singleton(v: CompiledSingleton<'a>) -> Self {
        Self {
            root_singletons: vec![v],
            ..Default::default()
        }
    }

    #[must_use]
    pub fn new_type_definition(v: CompiledTypeDefinition<'a>) -> Self {
        Self {
            simple_types: vec![(
                v.name,
                SimpleType {
                    name: v.name,
                    attrs: SimpleTypeAttrs::TypeDefinition(v),
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn new_enum_type(v: CompiledEnumType<'a>) -> Self {
        Self {
            simple_types: vec![(
                v.name,
                SimpleType {
                    name: v.name,
                    attrs: SimpleTypeAttrs::EnumType(v),
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        self.complex_types.extend(other.complex_types);
        self.simple_types.extend(other.simple_types);
        self.entity_types.extend(other.entity_types);
        self.root_singletons.extend(other.root_singletons);
        self
    }
}
