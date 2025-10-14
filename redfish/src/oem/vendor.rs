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

//! Supported vendors.

#[cfg(feature = "oem-ami")]
pub use crate::oem::ami::Product as AmiProduct;
#[cfg(feature = "oem-dell")]
pub use crate::oem::dell::Product as DellProduct;
#[cfg(feature = "oem-hpe")]
pub use crate::oem::hpe::Product as HpeProduct;
#[cfg(feature = "oem-lenovo")]
pub use crate::oem::lenovo::Product as LenovoProduct;
#[cfg(feature = "oem-nvidia")]
pub use crate::oem::nvidia::Product as NvidiaProduct;
#[cfg(feature = "oem-supermicro")]
pub use crate::oem::supermicro::Product as SupermicroProduct;

/// All supported vendors defined by features.
pub enum Vendor {
    /// Lenovo systems.
    #[cfg(feature = "oem-lenovo")]
    Lenovo,
    /// HPE systems.
    #[cfg(feature = "oem-hpe")]
    Hpe,
    /// Supermicro systems.
    #[cfg(feature = "oem-supermicro")]
    Supermicro,
    /// Dell systems.
    #[cfg(feature = "oem-dell")]
    Dell,
    /// AMI.
    #[cfg(feature = "oem-ami")]
    Ami,
    /// NVIDIA.
    #[cfg(feature = "oem-nvidia")]
    Nvidia,
}

/// All supported products.
pub enum Product {
    /// Lenovo systems.
    #[cfg(feature = "oem-lenovo")]
    Lenovo(LenovoProduct),
    /// HPE systems.
    #[cfg(feature = "oem-hpe")]
    Hpe(HpeProduct),
    /// Supermicro systems.
    #[cfg(feature = "oem-supermicro")]
    Supermicro(SupermicroProduct),
    /// Dell systems.
    #[cfg(feature = "oem-dell")]
    Dell(DellProduct),
    /// AMI.
    #[cfg(feature = "oem-ami")]
    Ami(AmiProduct),
    /// NVIDIA.
    #[cfg(feature = "oem-nvidia")]
    Nvidia(NvidiaProduct),
}
