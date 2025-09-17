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

pub mod name;

use crate::compiler::CompiledComplexType;
use crate::compiler::CompiledEntityType;
use crate::generator::rust::Error;
use crate::generator::rust::ModName;
use crate::generator::rust::StructDef;
use crate::generator::rust::StructName;
use proc_macro2::Delimiter;
use proc_macro2::Group;
use proc_macro2::TokenStream;
use proc_macro2::TokenTree;
use quote::quote;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

#[derive(Default, Debug)]
pub struct ModDef<'a> {
    name: Option<ModName<'a>>,
    structs: HashMap<StructName<'a>, StructDef<'a>>,
    sub_mods: HashMap<ModName<'a>, ModDef<'a>>,
}

impl<'a> ModDef<'a> {
    #[must_use]
    pub fn new(name: ModName<'a>) -> Self {
        Self {
            name: Some(name),
            structs: HashMap::new(),
            sub_mods: HashMap::new(),
        }
    }

    /// Add complex type to the module.
    ///
    /// # Errors
    ///
    /// Returns `CreateStruct` error if failed to add new struct to the
    /// module.  it may only happen in case of name conflicts because
    /// of case conversion.
    pub fn add_complex_type(self, ct: CompiledComplexType<'a>) -> Result<Self, Error<'a>> {
        self.inner_add_complex_type(ct, 0)
    }

    fn inner_add_complex_type(
        mut self,
        ct: CompiledComplexType<'a>,
        depth: usize,
    ) -> Result<Self, Error<'a>> {
        let short_name = ct.name.name;
        if let Some(id) = ct.name.namespace.get_id(depth) {
            let mod_name = ModName::new(id);
            self.sub_mods
                .remove(&mod_name)
                .unwrap_or_else(|| ModDef::new(mod_name))
                .inner_add_complex_type(ct, depth + 1)
                .map(|submod| {
                    self.sub_mods.insert(mod_name, submod);
                    self
                })
        } else {
            let struct_name = StructName::new(short_name);
            match self.structs.entry(struct_name) {
                Entry::Occupied(_) => Err(Error::NameConflict(ct.name)),
                Entry::Vacant(v) => {
                    v.insert(StructDef {
                        name: struct_name,
                        odata: ct.odata,
                    });
                    Ok(self)
                }
            }
            .map_err(Box::new)
            .map_err(|e| Error::CreateStruct(short_name, e))
        }
    }

    /// Add complex type to the module.
    ///
    /// # Errors
    ///
    /// Returns `CreateStruct` error if failed to add new struct to the
    /// module.  it may only happen in case of name conflicts because
    /// of case conversion.
    pub fn add_entity_type(self, ct: CompiledEntityType<'a>) -> Result<Self, Error<'a>> {
        self.inner_add_entity_type(ct, 0)
    }

    fn inner_add_entity_type(
        mut self,
        et: CompiledEntityType<'a>,
        depth: usize,
    ) -> Result<Self, Error<'a>> {
        let short_name = et.name.name;
        if let Some(id) = et.name.namespace.get_id(depth) {
            let mod_name = ModName::new(id);
            self.sub_mods
                .remove(&mod_name)
                .unwrap_or_else(|| ModDef::new(mod_name))
                .inner_add_entity_type(et, depth + 1)
                .map(|submod| {
                    self.sub_mods.insert(mod_name, submod);
                    self
                })
        } else {
            let struct_name = StructName::new(short_name);
            match self.structs.entry(struct_name) {
                Entry::Occupied(_) => Err(Error::NameConflict(et.name)),
                Entry::Vacant(v) => {
                    v.insert(StructDef {
                        name: struct_name,
                        odata: et.odata,
                    });
                    Ok(self)
                }
            }
            .map_err(Box::new)
            .map_err(|e| Error::CreateStruct(short_name, e))
        }
    }

    pub fn generate(self, tokens: &mut TokenStream) {
        let mut sub_mods = self.sub_mods.into_values().collect::<Vec<_>>();
        sub_mods.sort_by_key(|v| v.name);

        let mut structs = self.structs.into_values().collect::<Vec<_>>();
        structs.sort_by_key(|v| v.name);

        let generate = |ts: &mut TokenStream| {
            for s in structs {
                s.generate(ts);
            }

            for m in sub_mods {
                m.generate(ts);
            }
        };

        if let Some(name) = self.name {
            let mut content = TokenStream::new();
            generate(&mut content);
            tokens.extend([
                quote! {
                    pub mod #name
                },
                TokenTree::Group(Group::new(Delimiter::Brace, content)).into(),
            ]);
        } else {
            generate(tokens);
        }
    }
}
