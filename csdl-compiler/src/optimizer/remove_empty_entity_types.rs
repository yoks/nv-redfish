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

//! Remove empty complex types optimization

use crate::compiler::Compiled;
use crate::compiler::CompiledComplexType;
use crate::compiler::CompiledEntityType;
use crate::compiler::CompiledNavProperty;
use crate::compiler::QualifiedName;
use std::collections::HashMap;

type Replacements<'a> = HashMap<QualifiedName<'a>, QualifiedName<'a>>;

pub fn remove_empty_entity_types(input: Compiled<'_>) -> Compiled<'_> {
    let et_replacements = collect_et_replacements(&input);
    Compiled {
        entity_types: input
            .entity_types
            .into_iter()
            .filter_map(|(name, v)| {
                if et_replacements.contains_key(&name) {
                    None
                } else {
                    Some((
                        name,
                        CompiledEntityType {
                            name: v.name,
                            base: v.base.as_ref().map(|base| replace(base, &et_replacements)),
                            properties: v.properties,
                            nav_properties: replace_properties(v.nav_properties, &et_replacements),
                            description: v.description,
                            long_description: v.long_description,
                        },
                    ))
                }
            })
            .collect(),
        complex_types: input
            .complex_types
            .into_iter()
            .map(|(name, v)| {
                (
                    name,
                    CompiledComplexType {
                        name: v.name,
                        base: v.base,
                        properties: v.properties,
                        nav_properties: replace_properties(v.nav_properties, &et_replacements),
                        description: v.description,
                        long_description: v.long_description,
                    },
                )
            })
            .collect(),
        root_singletons: input.root_singletons,
        simple_types: input.simple_types,
    }
}

const fn et_is_empty(et: &CompiledEntityType<'_>) -> bool {
    et.properties.is_empty() && et.nav_properties.is_empty()
}

fn replace_properties<'a>(
    properties: Vec<CompiledNavProperty<'a>>,
    ct_replacements: &Replacements<'a>,
) -> Vec<CompiledNavProperty<'a>> {
    properties
        .into_iter()
        .map(|p| CompiledNavProperty {
            name: p.name,
            ptype: p.ptype.map(|t| replace(&t, ct_replacements)),
            description: p.description,
            long_description: p.long_description,
        })
        .collect()
}

fn replace<'a>(
    target: &QualifiedName<'a>,
    replacements: &HashMap<QualifiedName<'a>, QualifiedName<'a>>,
) -> QualifiedName<'a> {
    *replacements.get(target).unwrap_or(target)
}

fn collect_et_replacements<'a>(
    input: &Compiled<'a>,
) -> HashMap<QualifiedName<'a>, QualifiedName<'a>> {
    input
        .entity_types
        .values()
        .filter_map(|v| {
            if et_is_empty(v) {
                find_non_empty_parent(input, v.name).map(|parent| (v.name, parent))
            } else {
                None
            }
        })
        .collect()
}

fn find_non_empty_parent<'a>(
    input: &Compiled<'a>,
    mut qname: QualifiedName<'a>,
) -> Option<QualifiedName<'a>> {
    while let Some(et) = input.entity_types.get(&qname) {
        if !et_is_empty(et) {
            return Some(qname);
        }
        if let Some(base) = et.base {
            qname = base;
        } else {
            return None;
        }
    }
    None
}
