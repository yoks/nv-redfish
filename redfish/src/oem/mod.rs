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

//! OEM-specific support.

/// Vendors.
pub mod vendor;

/// Support of AMI.
#[cfg(feature = "oem-ami")]
pub mod ami;
/// Support of Dell.
#[cfg(feature = "oem-dell")]
pub mod dell;
/// Support of HPE.
#[cfg(feature = "oem-hpe")]
pub mod hpe;
/// Support of Lenovo.
#[cfg(feature = "oem-lenovo")]
pub mod lenovo;
/// Support of NVIDIA.
#[cfg(feature = "oem-nvidia")]
pub mod nvidia;
/// Support of Supermicro.
#[cfg(feature = "oem-supermicro")]
pub mod supermicro;

#[doc(inline)]
pub use vendor::Product;
#[doc(inline)]
pub use vendor::Vendor;

#[cfg(feature = "accounts")]
use crate::schema::redfish::account_service::AccountServiceUpdate;

/// Account password policy support.
#[cfg(feature = "accounts")]
#[must_use]
pub fn best_bmaas_password_policy(product: &Product) -> Option<AccountServiceUpdate> {
    match product {
        #[cfg(feature = "oem-ami")]
        Product::Ami(v) => Some(ami::best_bmaas_password_policy(v)),
        #[cfg(feature = "oem-dell")]
        Product::Dell(v) => dell::best_bmaas_password_policy(v),
        #[cfg(feature = "oem-hpe")]
        Product::Hpe(v) => Some(hpe::best_bmaas_password_policy(v)),
        #[cfg(feature = "oem-lenovo")]
        Product::Lenovo(v) => Some(lenovo::best_bmaas_password_policy(v)),
        #[cfg(feature = "oem-nvidia")]
        Product::Nvidia(v) => nvidia::best_bmaas_password_policy(v),
        #[cfg(feature = "oem-supermicro")]
        Product::Supermicro(v) => Some(supermicro::best_bmaas_password_policy(v)),
    }
}
