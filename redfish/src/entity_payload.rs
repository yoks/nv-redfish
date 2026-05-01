// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

//! Generated Redfish entity payload boundary.

use crate::core::EntityTypeRef as _;
use crate::core::ODataETag;
use crate::core::ODataId;
use crate::schema;
use serde::Serialize;

macro_rules! entity_payload {
    (
        $(
            $(#[$meta:meta])*
            $variant:ident($type:path) => $kind:literal,
        )+
    ) => {
        /// Read-side payload preserving generated Redfish schema data.
        #[derive(Debug, Serialize)]
        #[serde(tag = "entity_kind", content = "payload")]
        pub enum EntityPayload {
            $(
                $(#[$meta])*
                #[doc = concat!("Generated `", $kind, "` entity payload.")]
                $variant(Box<$type>),
            )+
        }

        impl EntityPayload {
            /// Returns the generated entity kind.
            #[must_use]
            pub const fn entity_kind(&self) -> &'static str {
                match self {
                    $(
                        $(#[$meta])*
                        Self::$variant(_) => $kind,
                    )+
                }
            }

            /// Returns the generated entity kind.
            #[must_use]
            pub const fn kind(&self) -> &'static str {
                self.entity_kind()
            }

            /// Returns the resource `@odata.id`.
            #[must_use]
            pub fn odata_id(&self) -> Option<&ODataId> {
                match self {
                    $(
                        $(#[$meta])*
                        Self::$variant(payload) => Some(payload.odata_id()),
                    )+
                }
            }

            /// Returns the resource `@odata.id`.
            #[must_use]
            pub fn resource_odata_id(&self) -> Option<&ODataId> {
                self.odata_id()
            }

            /// Returns the resource `@odata.etag`, when present.
            #[must_use]
            pub fn etag(&self) -> Option<&ODataETag> {
                match self {
                    $(
                        $(#[$meta])*
                        Self::$variant(payload) => payload.etag(),
                    )+
                }
            }

            /// Returns the resource `@odata.etag`, when present.
            #[must_use]
            pub fn resource_etag(&self) -> Option<&ODataETag> {
                self.etag()
            }
        }
    };
}

entity_payload! {
    ServiceRoot(schema::service_root::ServiceRoot) => "ServiceRoot",

    #[cfg(feature = "accounts")]
    AccountService(schema::account_service::AccountService) => "AccountService",
    #[cfg(feature = "accounts")]
    ManagerAccount(schema::manager_account::ManagerAccount) => "ManagerAccount",
    #[cfg(feature = "accounts")]
    ManagerAccountCollection(schema::manager_account_collection::ManagerAccountCollection) => "ManagerAccountCollection",

    #[cfg(feature = "assembly")]
    Assembly(schema::assembly::Assembly) => "Assembly",

    #[cfg(feature = "chassis")]
    Chassis(schema::chassis::Chassis) => "Chassis",
    #[cfg(feature = "chassis")]
    ChassisCollection(schema::chassis_collection::ChassisCollection) => "ChassisCollection",

    #[cfg(feature = "computer-systems")]
    ComputerSystem(schema::computer_system::ComputerSystem) => "ComputerSystem",
    #[cfg(feature = "computer-systems")]
    ComputerSystemCollection(schema::computer_system_collection::ComputerSystemCollection) => "ComputerSystemCollection",

    #[cfg(feature = "managers")]
    Manager(schema::manager::Manager) => "Manager",
    #[cfg(feature = "managers")]
    ManagerCollection(schema::manager_collection::ManagerCollection) => "ManagerCollection",

    #[cfg(feature = "sensors")]
    Sensor(schema::sensor::Sensor) => "Sensor",

    #[cfg(feature = "update-service")]
    UpdateService(schema::update_service::UpdateService) => "UpdateService",
}
