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
//! Integration tests for Manager collection behavior.

#![recursion_limit = "256"]

use nv_redfish::ServiceRoot;
use nv_redfish_core::ODataId;
use nv_redfish_tests::ami_viking_service_root;
use nv_redfish_tests::anonymous_1_9_service_root;
use nv_redfish_tests::Bmc;
use nv_redfish_tests::Expect;
use nv_redfish_tests::ODATA_ID;
use nv_redfish_tests::ODATA_TYPE;
use serde_json::json;
use std::error::Error as StdError;
use std::sync::Arc;
use tokio::test;

const MANAGER_COLLECTION_DATA_TYPE: &str = "#ManagerCollection.ManagerCollection";
const MANAGER_DATA_TYPE: &str = "#Manager.v1_16_0.Manager";

#[test]
async fn ami_viking_missing_root_managers_nav_workaround() -> Result<(), Box<dyn StdError>> {
    let bmc = Arc::new(Bmc::default());
    let ids = ids();
    bmc.expect(Expect::get(
        &ids.root_id,
        ami_viking_service_root(&ids.root_id, json!({})),
    ));
    let root = ServiceRoot::new(bmc.clone()).await?;

    bmc.expect(Expect::get(
        &ids.managers_id,
        json!({
            ODATA_ID: &ids.managers_id,
            ODATA_TYPE: MANAGER_COLLECTION_DATA_TYPE,
            "Id": "Managers",
            "Name": "Manager Collection",
            "Members": [manager_payload(&ids)]
        }),
    ));

    let collection = root.managers().await?.unwrap();
    let members = collection.members().await?;
    assert_eq!(members.len(), 1);

    Ok(())
}

#[test]
async fn anonymous_1_9_0_wrong_manager_status_state_workaround() -> Result<(), Box<dyn StdError>> {
    // Platform under test: Liteon powershelf class (anonymous Redfish 1.9.0 root).
    // Quirk under test: invalid Manager.Status.State="Standby".
    let bmc = Arc::new(Bmc::default());
    let ids = ids();
    let root = expect_anonymous_1_9_service_root(
        bmc.clone(),
        &ids,
        json!({
            "Managers": { ODATA_ID: &ids.managers_id }
        }),
    )
    .await?;

    bmc.expect(Expect::get(
        &ids.managers_id,
        json!({
            ODATA_ID: &ids.managers_id,
            ODATA_TYPE: MANAGER_COLLECTION_DATA_TYPE,
            "Id": "Managers",
            "Name": "Manager Collection",
            "Members": [{ ODATA_ID: &ids.manager_id }]
        }),
    ));

    let collection = root.managers().await?.unwrap();
    bmc.expect(Expect::get(
        &ids.manager_id,
        manager_payload_with_state(&ids, "Standby"),
    ));
    let members = collection.members().await?;
    assert_eq!(members.len(), 1);

    Ok(())
}

struct Ids {
    root_id: ODataId,
    managers_id: String,
    manager_id: String,
}

fn ids() -> Ids {
    let root_id = ODataId::service_root();
    let managers_id = format!("{root_id}/Managers");
    let manager_id = format!("{managers_id}/1");
    Ids {
        root_id,
        managers_id,
        manager_id,
    }
}

fn manager_payload(ids: &Ids) -> serde_json::Value {
    manager_payload_with_state(ids, "Enabled")
}

fn manager_payload_with_state(ids: &Ids, state: &str) -> serde_json::Value {
    json!({
        ODATA_ID: &ids.manager_id,
        ODATA_TYPE: MANAGER_DATA_TYPE,
        "Id": "1",
        "Name": "Manager",
        "Status": { "State": state }
    })
}

async fn expect_anonymous_1_9_service_root(
    bmc: Arc<Bmc>,
    ids: &Ids,
    fields: serde_json::Value,
) -> Result<ServiceRoot<Bmc>, Box<dyn StdError>> {
    bmc.expect(Expect::get(
        &ids.root_id,
        anonymous_1_9_service_root(&ids.root_id, fields),
    ));
    ServiceRoot::new(bmc).await.map_err(Into::into)
}
