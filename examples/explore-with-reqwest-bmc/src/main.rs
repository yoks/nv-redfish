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

use nv_redfish::bmc::BmcCredentials;
use nv_redfish::http::BmcHttpError;
use nv_redfish::http::ExpandQuery;
use nv_redfish::http::HttpBmc;
use nv_redfish::http::ReqwestClient;
use nv_redfish::http::ReqwestClientParams;
use nv_redfish::Expandable;
use nv_redfish::ODataId;
use url::Url;

#[tokio::main]
async fn main() -> Result<(), BmcHttpError> {
    let client = ReqwestClient::with_params(ReqwestClientParams::new().accept_invalid_certs(true))
        .map_err(|e| BmcHttpError::Generic(e.to_string()))?;

    let creds = BmcCredentials::new("username".into(), "password".into());
    let bmc = HttpBmc::new(client, Url::parse("https://192.168.2.2").unwrap(), creds);

    let service_root =
        nv_redfish::NavProperty::<redfish_std::redfish::service_root::ServiceRoot>::new_reference(
            ODataId::service_root(),
        )
        .get(&bmc)
        .await?;

    let chassis_members = &service_root
        .chassis
        .as_ref()
        .unwrap()
        .get(&bmc)
        .await?
        .members;

    let chassis = chassis_members.iter().next().unwrap().get(&bmc).await?;

    let all_devices = &chassis
        .pc_ie_devices
        .as_ref()
        .unwrap()
        .get(&bmc)
        .await?
        .members;
    for device in all_devices {
        if let Some(nav_prop) = &device.get(&bmc).await?.pc_ie_functions {
            let function_handles = nav_prop.get(&bmc).await?;
            for function_handle in &function_handles.members {
                let _function = function_handle.get(&bmc).await?;
            }

            println!("{function_handles:?}"); // unpolulated members
            let function_handles = function_handles.expand(&bmc, ExpandQuery::default()).await?;
            println!("{function_handles:?}"); // memberrs populated
        }
    }

    let systems = &service_root
        .systems
        .as_ref()
        .expect("no systems")
        .get(&bmc)
        .await?
        .members;
    println!(
        "{:?}",
        systems
            .into_iter()
            .next()
            .expect("at least one system")
            .get(&bmc)
            .await?
    );

    Ok(())
}
