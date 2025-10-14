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

pub mod product;

#[doc(inline)]
pub use product::Product;

#[cfg(feature = "accounts")]
use crate::oem::AccountServiceUpdate;

#[cfg(feature = "accounts")]
pub(crate) fn best_bmaas_password_policy(_product: &Product) -> AccountServiceUpdate {
    AccountServiceUpdate::builder()
        .with_account_lockout_threshold(0)
        .with_account_lockout_duration(0)
        .with_account_lockout_counter_reset_after(0)
        .build()
}
