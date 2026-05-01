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

use nv_redfish::core::ODataId;
use nv_redfish_scraper::adapter::redfish::BmcId;
use nv_redfish_scraper::adapter::redfish::ChangeKind;
use nv_redfish_scraper::adapter::redfish::ReconstructionRecord;
use nv_redfish_scraper::adapter::redfish::RedfishResourceEvent;
use nv_redfish_scraper::adapter::redfish::ResourceMetadata;
use std::time::Duration;
use std::time::SystemTime;

fn main() {
    let event = RedfishResourceEvent::<()>::new(
        BmcId::new("bmc-a"),
        ODataId::from("/redfish/v1/Chassis/1".to_owned()),
        Some(ODataId::from("/redfish/v1/Chassis".to_owned())),
        ChangeKind::Refreshed,
        None,
        ResourceMetadata::new(SystemTime::UNIX_EPOCH, Duration::ZERO, 1, None),
    );

    let record = ReconstructionRecord::from_resource_event(event);

    assert_eq!(record.bmc_id().as_str(), "bmc-a");
}
