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

mod item;

#[cfg(feature = "network-adapters")]
mod network_adapter;
#[cfg(feature = "power")]
mod power;
#[cfg(feature = "power-supplies")]
mod power_supply;
#[cfg(feature = "thermal")]
mod thermal;

use nv_redfish_core::Bmc;
use std::sync::Arc;

#[doc(inline)]
pub use item::Chassis;
#[doc(inline)]
pub use item::Manufacturer;
#[doc(inline)]
pub use item::Model;
#[doc(inline)]
pub use item::PartNumber;
#[doc(inline)]
pub use item::SerialNumber;

#[doc(inline)]
#[cfg(feature = "network-adapters")]
pub use network_adapter::Manufacturer as NetworkAdapterManufacturer;
#[doc(inline)]
#[cfg(feature = "network-adapters")]
pub use network_adapter::Model as NetworkAdapterModel;
#[doc(inline)]
#[cfg(feature = "network-adapters")]
pub use network_adapter::NetworkAdapter;
#[cfg(feature = "network-adapters")]
pub use network_adapter::NetworkAdapterCollection;
#[doc(inline)]
#[cfg(feature = "network-adapters")]
pub use network_adapter::PartNumber as NetworkAdapterPartNumber;
#[doc(inline)]
#[cfg(feature = "network-adapters")]
pub use network_adapter::SerialNumber as NetworkAdapterSerialNumber;
#[doc(inline)]
#[cfg(feature = "power")]
pub use power::Power;
#[doc(inline)]
#[cfg(feature = "power-supplies")]
pub use power_supply::PowerSupply;
#[doc(inline)]
#[cfg(feature = "thermal")]
pub use thermal::Thermal;

use crate::patch_support::JsonValue;
use crate::patch_support::ReadPatchFn;
use crate::schema::redfish::chassis_collection::ChassisCollection as ChassisCollectionSchema;
use crate::{Error, NvBmc, ServiceRoot};

/// Chassis collection.
///
/// Provides functions to access collection members.
pub struct ChassisCollection<B: Bmc> {
    bmc: NvBmc<B>,
    collection: Arc<ChassisCollectionSchema>,
    read_patch_fn: Option<ReadPatchFn>,
}

impl<B: Bmc> ChassisCollection<B> {
    pub(crate) async fn new(bmc: &NvBmc<B>, root: &ServiceRoot<B>) -> Result<Self, Error<B>> {
        let collection_ref = root
            .root
            .chassis
            .as_ref()
            .ok_or(Error::ChassisNotSupported)?;

        let mut patches = Vec::new();
        if root.bug_invalid_contained_by_fields() {
            patches.push(remove_invalid_contained_by_fields);
        }
        let read_patch_fn = if patches.is_empty() {
            None
        } else {
            let read_patch_fn: ReadPatchFn =
                Arc::new(move |v| patches.iter().fold(v, |acc, f| f(acc)));
            Some(read_patch_fn)
        };

        let collection = bmc.expand_property(collection_ref).await?;
        Ok(Self {
            bmc: bmc.clone(),
            collection,
            read_patch_fn,
        })
    }

    /// List all chassis avaiable in this BMC
    ///
    /// # Errors
    ///
    /// Returns an error if fetching collection data fails.
    pub async fn members(&self) -> Result<Vec<Chassis<B>>, Error<B>> {
        let mut chassis_members = Vec::new();
        for chassis in &self.collection.members {
            chassis_members
                .push(Chassis::new(&self.bmc, chassis, self.read_patch_fn.as_ref()).await?);
        }

        Ok(chassis_members)
    }
}

fn remove_invalid_contained_by_fields(mut v: JsonValue) -> JsonValue {
    if let JsonValue::Object(ref mut obj) = v {
        if let Some(JsonValue::Object(ref mut links_obj)) = obj.get_mut("Links") {
            if let Some(JsonValue::Object(ref mut contained_by_obj)) =
                links_obj.get_mut("ContainedBy")
            {
                contained_by_obj.retain(|k, _| k == "@odata.id");
            }
        }
    }
    v
}
