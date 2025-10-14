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

use crate::oem::lenovo::schema::redfish::lenovo_account_service::LenovoAccountServicePropertiesUpdate;
use crate::oem::lenovo::Product;
use crate::schema::redfish::account_service::AccountServiceUpdate;
use crate::schema::redfish::resource::OemUpdate;
use crate::schema::redfish::resource::ResourceUpdate;
use serde::Serialize;

#[derive(Serialize)]
struct LenovoOemUpdate {
    #[serde(rename = "Lenovo")]
    oem_root: LenovoAccountServicePropertiesUpdate,
}

pub fn best_bmaas_password_policy(_product: &Product) -> AccountServiceUpdate {
    // Redfish equivalent of `accseccfg -pew 0 -pe 0 -chgnew off -rc 0 -ci 0 -lf 0`
    let oem_root = LenovoAccountServicePropertiesUpdate::builder()
        .with_password_expiration_period_days(0.0) // -pe 0
        .with_password_change_on_first_access(false) // -chgnew off
        .with_minimum_password_change_interval_hours(0.0) // -ci 0
        .with_minimum_password_reuse_cycle(0.0) // -rc 0
        .with_password_expiration_warning_period(0.0) // -pew 0
        .build();

    AccountServiceUpdate::builder()
        .with_account_lockout_threshold(0) // -lf 0
        .with_account_lockout_threshold(60) // 60 secs is the shortest Lenovo allows. The docs say 0 disables it, but tests show that Lenovo rejects 0.
        .with_base(
            ResourceUpdate::builder()
                .with_oem(OemUpdate {
                    additional_properties: serde_json::to_value(LenovoOemUpdate { oem_root })
                        .expect("Lenovo schema is serializable"),
                })
                .build(),
        )
        .build()
}
