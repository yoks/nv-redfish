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

use crate::oem::hpe::schema::redfish::hpei_lo_account_service::HpeiLoAccountServiceUpdate;
use crate::oem::hpe::Product;
use crate::oem::AccountServiceUpdate;
use crate::schema::redfish::resource::OemUpdate;
use crate::schema::redfish::resource::ResourceUpdate;
use serde::Serialize;

#[derive(Serialize)]
struct HpeOemUpdate {
    #[serde(rename = "Hpe")]
    oem_root: HpeiLoAccountServiceUpdate,
}

pub fn best_bmaas_password_policy(_product: &Product) -> AccountServiceUpdate {
    let oem_root = HpeiLoAccountServiceUpdate::builder()
        .with_auth_failure_delay_time_seconds(2)
        .with_auth_failure_logging_threshold(0)
        .with_auth_failures_before_delay(0)
        .with_enforce_password_complexity(false)
        .build();

    AccountServiceUpdate::builder()
        .with_base(
            ResourceUpdate::builder()
                .with_oem(OemUpdate {
                    additional_properties: serde_json::to_value(HpeOemUpdate { oem_root })
                        .expect("HPE schema is serializable"),
                })
                .build(),
        )
        .build()
}
