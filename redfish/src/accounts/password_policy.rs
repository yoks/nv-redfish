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
/// locking. Returns `None` if password policy setup is not supported
/// by vendor.
#[must_use]
pub fn never_expire_policy(vendor: &Vendor) -> Option<AccountServiceUpdate> {
    match vendor {
        #[cfg(feature = "oem-lenovo")]
        Vendor::Lenovo => Some(never_expire_policy_lenovo()),
        #[cfg(feature = "oem-hpe")]
        Vendor::Hpe => Some(never_expire_policy_hpe()),
        #[cfg(feature = "oem-supermicro")]
        Vendor::Supermicro => Some(never_expire_policy_supermicro()),
        // iDRAC does not suport changing password policy. They
        // support IP blocking instead.
        // https://github.com/dell/iDRAC-Redfish-Scripting/issues/295
        #[cfg(feature = "oem-dell")]
        Vendor::Dell => None,
        #[cfg(feature = "oem-ami")]
        Vendor::AMI => Some(never_expire_policy_ami()),
        #[cfg(feature = "oem-nvidia-gbx00")]
        Vendor::NvidiaGbx00 => Some(never_expire_policy_nvidia_gbx00()),
        // Bluefield 2 and Bluefield 3 say that account properties are
        // read-only.
        #[cfg(feature = "oem-nvidia-dpu")]
        Vendor::NvidiaDPU => None,
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

#[cfg(feature = "oem-supermicro")]
fn never_expire_policy_supermicro() -> AccountServiceUpdate {
    AccountServiceUpdate::builder()
        .with_account_lockout_threshold(0)
        .with_account_lockout_duration(0)
        .with_account_lockout_counter_reset_after(0)
        .build()
}

#[cfg(feature = "oem-ami")]
fn never_expire_policy_ami() -> AccountServiceUpdate {
    // Setting to (0,0,0,false,0) causes account lockout. So set them
    // to less harmful values
    AccountServiceUpdate::builder()
        .with_account_lockout_threshold(4)
        .with_account_lockout_duration(20)
        .with_account_lockout_counter_reset_after(20)
        .with_account_lockout_counter_reset_enabled(true)
        .with_auth_failure_logging_threshold(2)
        .build()
}

#[cfg(feature = "oem-nvidia-gbx00")]
fn never_expire_policy_nvidia_gbx00() -> AccountServiceUpdate {
    // We were able to set AccountLockoutThreshold on the initial 3 GB200 trays we received in pdx-lab
    // however, with the recent trays we received, it is not happy with setting a value of 0
    // for AccountLockoutThreshold: "The property 'AccountLockoutThreshold' with the requested value
    // of '0' could not be written because the value does not meet the constraints of the implementation."
    //
    // Never lock
    // ("AccountLockoutThreshold", Number(0.into())),
    // instead, use the same threshold that we picked for vikings: the bmc will lock the account out after 4 attempts
    AccountServiceUpdate::builder()
        .with_account_lockout_threshold(4)
        // 600 is the smallest value it will accept. 10 minutes, in seconds.
        .with_account_lockout_duration(600)
        .build()
}
