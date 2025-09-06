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

//! Deserialization and validation of Annotations

use crate::edmx::TermName;
use serde::Deserialize;

/// 14.3 Element edm:Annotation
#[derive(Debug, Deserialize)]
pub struct Annotation {
    /// 14.3.1 Attribute Term
    #[serde(rename = "@Term")]
    pub term: TermName,
    #[serde(rename = "@String")]
    pub string: Option<String>,
    #[serde(rename = "@Bool")]
    pub bool_value: Option<bool>,
    #[serde(rename = "@Int")]
    pub int_value: Option<i64>,
    #[serde(rename = "@EnumMember")]
    pub enum_member: Option<String>,
    #[serde(rename = "Collection")]
    pub collection: Option<AnnotationCollection>,
    #[serde(rename = "Record")]
    pub record: Option<AnnotationRecord>,
}

#[derive(Debug, Deserialize)]
pub struct AnnotationCollection {
    #[serde(rename = "String", default)]
    pub strings: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct AnnotationRecord {
    #[serde(rename = "PropertyValue")]
    pub property_value: PropertyValue,
    #[serde(rename = "Annotation", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Deserialize)]
pub struct PropertyValue {
    #[serde(rename = "@Property")]
    pub property: String,
    #[serde(rename = "@Bool")]
    pub bool_value: Option<bool>,
    #[serde(rename = "@String")]
    pub string_value: Option<String>,
    #[serde(rename = "@Int")]
    pub int_value: Option<i64>,
}
