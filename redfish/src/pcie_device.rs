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

/// `PCIe` device.
///
/// Provides functions to access PCIe device data.
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
}

impl<B: Bmc> Resource for PcieDevice<B> {
    fn resource_ref(&self) -> &ResourceSchema {
        &self.data.as_ref().base
    }
}
