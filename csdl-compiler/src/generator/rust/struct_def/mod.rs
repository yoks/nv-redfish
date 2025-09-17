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

use crate::generator::rust::StructName;
use crate::odata::annotations::DescriptionRef;
use crate::odata::annotations::LongDescriptionRef;
use proc_macro2::Delimiter;
use proc_macro2::Group;
use proc_macro2::Ident;
use proc_macro2::Literal;
use proc_macro2::Punct;
use proc_macro2::Spacing;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use proc_macro2::TokenTree;
use quote::quote;

/// Definition of Rust struct.
#[derive(Debug)]
pub struct StructDef<'a> {
    pub name: StructName<'a>,
    pub description: Option<DescriptionRef<'a>>,
    pub long_description: Option<LongDescriptionRef<'a>>,
}

impl<'a> StructDef<'a> {
    #[must_use]
    pub const fn new(name: StructName<'a>) -> Self {
        Self {
            name,
            description: None,
            long_description: None,
        }
    }

    pub fn generate(self, tokens: &mut TokenStream) {
        let content = TokenStream::new();
        let name = self.name;
        tokens.extend([
            self.format_description()
                .map(|lines| {
                    let mut ts = TokenStream::new();
                    for l in lines {
                        let mut attr_inner = TokenStream::new();
                        attr_inner.extend([
                            TokenTree::Ident(Ident::new("doc", Span::call_site())),
                            TokenTree::Punct(Punct::new('=', Spacing::Alone)),
                            TokenTree::Literal(Literal::string(&l)),
                        ]);
                        ts.extend([
                            TokenTree::Punct(Punct::new('#', Spacing::Alone)),
                            TokenTree::Group(Group::new(Delimiter::Bracket, attr_inner)),
                        ]);
                    }
                    ts
                })
                .unwrap_or_default(),
            quote! {
                pub struct #name
            },
            TokenTree::Group(Group::new(Delimiter::Brace, content)).into(),
        ]);
    }

    #[must_use]
    pub fn format_description(&self) -> Option<Vec<String>> {
        let maybe_descr = self.description.as_ref().map(ToString::to_string);
        let maybe_long_descr = self.long_description.as_ref().map(ToString::to_string);
        match (maybe_descr, maybe_long_descr) {
            (None, None) => None,
            (Some(d), None) => Some(vec![format!(" {d}")]),
            (None, Some(ld)) => Some(vec![
                format!(" {}", self.name),
                String::new(),
                format!(" {ld}"),
            ]),
            (Some(d), Some(ld)) => Some(vec![format!(" {d}"), String::new(), format!(" {ld}")]),
        }
    }
}
