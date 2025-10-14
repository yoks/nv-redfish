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

use crate::oem::nvidia::Product;
use crate::oem::AccountServiceUpdate;

#[cfg(feature = "accounts")]
pub fn best_bmaas_password_policy(product: &Product) -> Option<AccountServiceUpdate> {
    match product {
        Product::GBx00 => {
            // We were able to set AccountLockoutThreshold on the initial 3 GB200 trays we received in pdx-lab
            // however, with the recent trays we received, it is not happy with setting a value of 0
            // for AccountLockoutThreshold: "The property 'AccountLockoutThreshold' with the requested value
            // of '0' could not be written because the value does not meet the constraints of the implementation."
            //
            // Never lock
            // ("AccountLockoutThreshold", Number(0.into())),
            // instead, use the same threshold that we picked for vikings: the bmc will lock the account out after 4 attempts
            Some(
                AccountServiceUpdate::builder()
                    .with_account_lockout_threshold(4)
                    // 600 is the smallest value it will accept. 10 minutes, in seconds.
                    .with_account_lockout_duration(600)
                    .build(),
            )
        }
        Product::DPU => {
            // Bluefield 2 and Bluefield 3 say that account properties are
            // read-only.
            None
        }
    }
}
