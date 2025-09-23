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

use crate::compiler::QualifiedName;
use crate::generator::rust::Config;
use crate::generator::rust::ModName;
use crate::generator::rust::TypeName;
use proc_macro2::Punct;
use proc_macro2::Spacing;
use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::TokenStreamExt as _;
use quote::quote;

/// Fully quailified type name for generation of the rust code.
///
/// Example:
///
/// `redfish::service_root::ServiceRoot`
pub struct FullTypeName<'a, 'config> {
    type_name: QualifiedName<'a>,
    config: &'config Config,
}

impl<'a, 'config> FullTypeName<'a, 'config> {
    /// Create new fully qualified type name.
    #[must_use]
    pub const fn new(type_name: QualifiedName<'a>, config: &'config Config) -> Self {
        Self { type_name, config }
    }
}

impl ToTokens for FullTypeName<'_, '_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let top = &self.config.top_module_alias;
        tokens.extend(quote! { #top });
        for depth in 0..self.type_name.namespace.len() {
            if let Some(id) = self.type_name.namespace.get_id(depth) {
                let name = ModName::new(id);
                tokens.append(Punct::new(':', Spacing::Joint));
                tokens.append(Punct::new(':', Spacing::Joint));
                tokens.extend(quote! { #name });
            }
        }
        let name = TypeName::new_qualified(self.type_name.name);
        tokens.append(Punct::new(':', Spacing::Joint));
        tokens.append(Punct::new(':', Spacing::Joint));
        tokens.extend(quote! { #name });
    }
}
