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

use nv_redfish::core::ODataETag;
use nv_redfish::core::ODataId;
use nv_redfish::EntityPayload;
use nv_redfish_scraper::adapter::redfish::EntityPayload as ScraperEntityPayload;
use serde::Serialize;

fn assert_scraper_payload<T>(payload: &T)
where
    T: ScraperEntityPayload + Serialize,
{
    let _ = payload.entity_kind();
    let _ = payload.odata_id();
    let _ = payload.etag();
}

fn assert_generated_payload_shape(payload: &EntityPayload) {
    assert_scraper_payload(payload);
}

fn generated_payload() -> EntityPayload {
    unimplemented!("trybuild only type-checks this helper")
}

fn assert_generated_payload_contract() {
    let _odata_id = ODataId::from("/redfish/v1/Chassis/1".to_owned());
    let _etag = ODataETag::from("etag-1".to_owned());

    assert_generated_payload_shape(&generated_payload());
}

fn main() {}
