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

//! `PCIe` devices
//!

use crate::hardware_id::HardwareIdRef;
use crate::hardware_id::Manufacturer as HardwareIdManufacturer;
use crate::hardware_id::Model as HardwareIdModel;
use crate::hardware_id::PartNumber as HardwareIdPartNumber;
use crate::hardware_id::SerialNumber as HardwareIdSerialNumber;
use crate::schema::redfish::pcie_device::PcieDevice as PcieDeviceSchema;
use crate::schema::redfish::pcie_device_collection::PcieDeviceCollection as PcieDeviceCollectionSchema;
use crate::Error;
use crate::NvBmc;
use crate::Resource;
use crate::ResourceSchema;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;
use std::marker::PhantomData;
use std::sync::Arc;

/// `PCIe` devices collection.
///
/// Provides functions to access collection members.
pub struct PcieDeviceCollection<B: Bmc> {
    bmc: NvBmc<B>,
    collection: Arc<PcieDeviceCollectionSchema>,
}

impl<B: Bmc> PcieDeviceCollection<B> {
    /// Create a new manager collection handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<PcieDeviceCollectionSchema>,
    ) -> Result<Self, Error<B>> {
        let collection = bmc.expand_property(nav).await?;
        Ok(Self {
            bmc: bmc.clone(),
            collection,
        })
    }

    /// List all managers available in this BMC.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching manager data fails.
    pub async fn members(&self) -> Result<Vec<PcieDevice<B>>, Error<B>> {
        let mut members = Vec::new();
        for m in &self.collection.members {
            members.push(PcieDevice::new(&self.bmc, m).await?);
        }
        Ok(members)
    }
}

#[doc(hidden)]
pub enum PcieDeviceTag {}

/// Chassis manufacturer.
pub type Manufacturer<T> = HardwareIdManufacturer<T, PcieDeviceTag>;

/// Chassis model.
pub type Model<T> = HardwareIdModel<T, PcieDeviceTag>;

/// Chassis part number.
pub type PartNumber<T> = HardwareIdPartNumber<T, PcieDeviceTag>;

/// Chassis serial number.
pub type SerialNumber<T> = HardwareIdSerialNumber<T, PcieDeviceTag>;

/// `PCIe` device.
///
/// Provides functions to access `PCIe` device data.
pub struct PcieDevice<B: Bmc> {
    data: Arc<PcieDeviceSchema>,
    _marker: PhantomData<B>,
}

impl<B: Bmc> PcieDevice<B> {
    /// Create a new log service handle.
    pub(crate) async fn new(
        bmc: &NvBmc<B>,
        nav: &NavProperty<PcieDeviceSchema>,
    ) -> Result<Self, Error<B>> {
        nav.get(bmc.as_ref())
            .await
            .map_err(crate::Error::Bmc)
            .map(|data| Self {
                data,
                _marker: PhantomData,
            })
    }

    /// Get the raw schema data for this `PCIe` device.
    #[must_use]
    pub fn raw(&self) -> Arc<PcieDeviceSchema> {
        self.data.clone()
    }

    /// Get hardware identifier of the `PCIe` device.
    #[must_use]
    pub fn hardware_id(&self) -> HardwareIdRef<'_, PcieDeviceTag> {
        HardwareIdRef {
            manufacturer: self
                .data
                .manufacturer
                .as_ref()
                .and_then(Option::as_ref)
                .map(Manufacturer::new),
            model: self
                .data
                .model
                .as_ref()
                .and_then(Option::as_ref)
                .map(Model::new),
            part_number: self
                .data
                .part_number
                .as_ref()
                .and_then(Option::as_ref)
                .map(PartNumber::new),
            serial_number: self
                .data
                .serial_number
                .as_ref()
                .and_then(Option::as_ref)
                .map(SerialNumber::new),
        }
    }
}

impl<B: Bmc> Resource for PcieDevice<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}
