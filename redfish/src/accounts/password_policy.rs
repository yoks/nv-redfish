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

//! This is helper module that supports setup of password policy for
//! differnt vendors.
//!
//! Password policies have many vendor-specific limitation and this
//! module provides policies tested with vendors values.

use crate::schema::redfish::account_service::AccountServiceUpdate;
use crate::schema::redfish::resource::OemUpdate;
use crate::schema::redfish::resource::ResourceUpdate;
#[cfg(feature = "oem-hpe")]
use crate::schema_oem_hpe::redfish::hpei_lo_account_service::HpeiLoAccountServiceUpdate;
#[cfg(feature = "oem-lenovo")]
use crate::schema_oem_lenovo::redfish::lenovo_account_service::LenovoAccountServicePropertiesUpdate;
use crate::Vendor;
use serde::Serialize;

/// Policy with never-expired passwords with minimal restrictions on
/// locking.
#[must_use]
pub fn never_expire_policy(vendor: &Vendor) -> AccountServiceUpdate {
    match vendor {
        #[cfg(feature = "oem-lenovo")]
        Vendor::Lenovo => never_expire_policy_lenovo(),
        #[cfg(feature = "oem-hpe")]
        Vendor::Hpe => never_expire_policy_hpe(),
    }
}

#[cfg(feature = "oem-lenovo")]
#[derive(Serialize)]
struct LenovoOemUpdate {
    #[serde(rename = "Lenovo")]
    oem_root: LenovoAccountServicePropertiesUpdate,
}

#[cfg(feature = "oem-lenovo")]
fn never_expire_policy_lenovo() -> AccountServiceUpdate {
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

#[cfg(feature = "oem-hpe")]
#[derive(Serialize)]
struct HpeOemUpdate {
    #[serde(rename = "Hpe")]
    oem_root: HpeiLoAccountServiceUpdate,
}

#[cfg(feature = "oem-hpe")]
fn never_expire_policy_hpe() -> AccountServiceUpdate {
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
