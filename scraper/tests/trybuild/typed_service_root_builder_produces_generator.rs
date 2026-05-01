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

use nv_redfish::ServiceRoot;
use nv_redfish_bmc_mock::Bmc;
use nv_redfish_scraper::adapter::redfish::BmcId;
use nv_redfish_scraper::adapter::redfish::RedfishAdapterError;
use nv_redfish_scraper::adapter::redfish::RedfishEvent;
use nv_redfish_scraper::adapter::redfish::ServiceRootGeneratorBuilder;
use nv_redfish_scraper::Generator;

fn assert_generator<G>(generator: G)
where
    G: Generator<RedfishEvent, RedfishAdapterError>,
{
    let _ = generator;
}

fn service_root() -> ServiceRoot<Bmc<std::io::Error>> {
    unimplemented!("trybuild only type-checks this helper")
}

fn assert_service_root_builder_produces_generator() {
    let builder = ServiceRootGeneratorBuilder::new(BmcId::new("bmc-a"), service_root());
    let generator = builder.build();

    assert_generator(generator);
}

fn main() {}
