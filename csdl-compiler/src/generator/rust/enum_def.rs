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

use crate::compiler::EnumType;
use crate::edmx::attribute_values::SimpleIdentifier;
use crate::generator::casemungler;
use crate::generator::rust::doc::format_and_generate as doc_format_and_generate;
use crate::generator::rust::ident;
use crate::generator::rust::Config;
use crate::generator::rust::TypeName;
use proc_macro2::Delimiter;
use proc_macro2::Group;
use proc_macro2::Literal;
use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use quote::TokenStreamExt as _;

/// Type definition that maps to simple type.
#[derive(Debug)]
pub struct EnumDef<'a> {
    pub name: TypeName<'a>,
    pub compiled: EnumType<'a>,
}

impl EnumDef<'_> {
    /// Generate rust code for types derived from enums.
    ///
    /// Enums are open: an unrecognized wire value decodes into
    /// `UnsupportedValue` carrying the raw token, and serializes back as
    /// that token verbatim. `Serialize`/`Deserialize` are hand-emitted
    /// because derived serde has no way to capture the raw string
    /// (`#[serde(other)]` is unit-variant-only), and the payload costs
    /// `Copy`.
    pub fn generate(self, tokens: &mut TokenStream, config: &Config) {
        let name = self.name;
        let top = &config.top_module_alias;
        let mut members_content = TokenStream::new();
        let mut snake_case_match_arms = TokenStream::new();
        let mut serialize_match_arms = TokenStream::new();
        let mut deserialize_match_arms = TokenStream::new();

        for m in self.compiled.members {
            let rename = Literal::string(m.name.inner().inner());
            let member_name = EnumMemberName::new(m.name.inner());

            let snake_case_str = casemungler::to_snake(m.name.inner().inner());
            let snake_case_literal = Literal::string(&snake_case_str);

            members_content.extend([
                doc_format_and_generate(m.name, &m.odata),
                quote! {
                    #member_name,
                },
            ]);

            snake_case_match_arms.extend(quote! {
                Self::#member_name => #snake_case_literal,
            });
            serialize_match_arms.extend(quote! {
                Self::#member_name => #rename,
            });
            deserialize_match_arms.extend(quote! {
                #rename => #name::#member_name,
            });
        }
        members_content.extend(quote! {
            #[doc = " Fallback for values not in the current Redfish schema; carries the raw token."]
            #[doc = " Serialization emits that token verbatim, so unknown values round-trip."]
            UnsupportedValue(Box<str>),
        });
        snake_case_match_arms.extend(quote! {
            Self::UnsupportedValue(_) => "unsupported_value",
        });
        tokens.extend([
            doc_format_and_generate(self.name, &self.compiled.odata),
            quote! {
                #[derive(Debug, PartialEq, Eq, Clone)]
                #[allow(clippy::enum_variant_names)]
                pub enum #name
            },
        ]);
        tokens.append(Group::new(Delimiter::Brace, members_content));

        let expecting = Literal::string(&format!("a `{name}` string"));
        tokens.extend(quote! {
            impl ::serde::Serialize for #name {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: ::serde::Serializer,
                {
                    serializer.serialize_str(match self {
                        #serialize_match_arms
                        Self::UnsupportedValue(raw) => raw,
                    })
                }
            }

            impl<'de> ::serde::Deserialize<'de> for #name {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: ::serde::Deserializer<'de>,
                {
                    struct EnumVisitor;

                    impl ::serde::de::Visitor<'_> for EnumVisitor {
                        type Value = #name;

                        fn expecting(
                            &self,
                            formatter: &mut ::core::fmt::Formatter<'_>,
                        ) -> ::core::fmt::Result {
                            formatter.write_str(#expecting)
                        }

                        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                        where
                            E: ::serde::de::Error,
                        {
                            Ok(match value {
                                #deserialize_match_arms
                                other => #name::UnsupportedValue(other.into()),
                            })
                        }
                    }

                    deserializer.deserialize_str(EnumVisitor)
                }
            }

            impl #top::ToSnakeCase for #name {
                fn to_snake_case(&self) -> &'static str {
                    match self {
                        #snake_case_match_arms
                    }
                }
            }
        });
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
struct EnumMemberName<'a>(&'a SimpleIdentifier);

impl<'a> EnumMemberName<'a> {
    #[must_use]
    const fn new(v: &'a SimpleIdentifier) -> Self {
        Self(v)
    }
}

impl ToTokens for EnumMemberName<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append(ident::escaped(&casemungler::to_camel(self.0)));
    }
}
