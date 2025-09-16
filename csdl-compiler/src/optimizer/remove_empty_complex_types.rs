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
use crate::compiler::CompiledProperty;
use crate::compiler::QualifiedName;
use std::collections::HashMap;

type Replacements<'a> = HashMap<QualifiedName<'a>, QualifiedName<'a>>;

pub fn remove_empty_complex_types(input: Compiled<'_>) -> Compiled<'_> {
    let ct_replacements = collect_ct_replacements(&input);
    Compiled {
        complex_types: input
            .complex_types
            .into_iter()
            .filter_map(|(name, v)| {
                if ct_replacements.contains_key(&name) {
                    None
                } else {
                    Some((
                        name,
                        CompiledComplexType {
                            name: v.name,
                            base: v.base.as_ref().map(|base| replace(base, &ct_replacements)),
                            properties: replace_properties(v.properties, &ct_replacements),
                            nav_properties: v.nav_properties,
                            description: v.description,
                            long_description: v.long_description,
                        },
                    ))
                }
            })
            .collect(),
        entity_types: input
            .entity_types
            .into_iter()
            .map(|(name, v)| {
                (
                    name,
                    CompiledEntityType {
                        name: v.name,
                        base: v.base,
                        properties: replace_properties(v.properties, &ct_replacements),
                        nav_properties: v.nav_properties,
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

const fn ct_is_empty(ct: &CompiledComplexType<'_>) -> bool {
    ct.properties.is_empty() && ct.nav_properties.is_empty()
}

fn replace_properties<'a>(
    properties: Vec<CompiledProperty<'a>>,
    ct_replacements: &Replacements<'a>,
) -> Vec<CompiledProperty<'a>> {
    properties
        .into_iter()
        .map(|p| CompiledProperty {
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

fn collect_ct_replacements<'a>(
    input: &Compiled<'a>,
) -> HashMap<QualifiedName<'a>, QualifiedName<'a>> {
    input
        .complex_types
        .values()
        .filter_map(|v| {
            if ct_is_empty(v) {
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
    while let Some(ct) = input.complex_types.get(&qname) {
        if !ct_is_empty(ct) {
            return Some(qname);
        }
        if let Some(base) = ct.base {
            qname = base;
        } else {
            return None;
        }
    }
    None
}
