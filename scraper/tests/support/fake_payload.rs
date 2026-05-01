// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

//! Fake [`EntityPayload`] boundary for adapter tests.
//!
//! Until the CSDL compiler exposes a generated `EntityPayload` enum, the
//! scraper crate models it as an identity-preserving struct (kind,
//! `@odata.id`, optional `@odata.etag`). Tests sometimes need a payload
//! constructor that does not require typing out every field; this helper
//! centralizes that.

#![cfg(feature = "redfish-adapter")]

use nv_redfish::core::ODataETag;
use nv_redfish::core::ODataId;
use nv_redfish_scraper::adapter::redfish::EntityPayload;

/// Build an [`EntityPayload`] with the supplied kind and a synthetic
/// `@odata.id` of the form `"/redfish/v1/<kind>/<seq>"`.
pub fn payload(kind: &str, seq: u64) -> EntityPayload {
    let path = format!("/redfish/v1/{kind}/{seq}");
    EntityPayload {
        kind: String::from(kind),
        odata_id: ODataId::from(path),
        etag: None,
    }
}

/// Build an [`EntityPayload`] with the supplied kind, sequence, and ETag.
pub fn payload_with_etag(kind: &str, seq: u64, etag: &str) -> EntityPayload {
    EntityPayload {
        kind: String::from(kind),
        odata_id: ODataId::from(format!("/redfish/v1/{kind}/{seq}")),
        etag: Some(ODataETag::from(String::from(etag))),
    }
}
