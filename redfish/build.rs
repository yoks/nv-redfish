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

use csdl_compiler::Error;
use csdl_compiler::commands::Commands;
use csdl_compiler::commands::DEFAULT_ROOT;
use csdl_compiler::commands::process_command;
use std::env::var;
use std::path::PathBuf;

fn main() -> Result<(), Error> {
    let out_dir = PathBuf::from(var("OUT_DIR").unwrap());
    let output = out_dir.join("redfish.rs");
    let schema_path = "../schemas/redfish-csdl";
    let service_root = [
        "Resource_v1.xml",
        "ResolutionStep_v1.xml",
        "ServiceRoot_v1.xml",
    ];
    let service_root_pattens = ["ServiceRoot.*.*"];
    let (accounts, accounts_patterns) = if var("CARGO_FEATURE_ACCOUNTS").is_ok() {
        (
            vec![
                "AccountService_v1.xml",
                "ManagerAccountCollection_v1.xml",
                "ManagerAccount_v1.xml",
                "Privileges_v1.xml",
            ],
            vec![
                "AccountService.*",
                "ManagerAccountCollection.*",
                "ManagerAccount.*",
            ],
        )
    } else {
        (vec![], vec![])
    };
    let (events, events_patterns) = if var("CARGO_FEATURE_EVENTS").is_ok() {
        (
            vec![
                "EventService_v1.xml",
                "Event_v1.xml",
                "EventDestination_v1.xml",
            ],
            vec!["EventService.*"],
        )
    } else {
        (vec![], vec![])
    };
    let csdls = service_root
        .iter()
        .chain(accounts.iter())
        .chain(events.iter())
        .map(|f| format!("{schema_path}/{f}"))
        .collect::<Vec<_>>();

    for f in &csdls {
        println!("cargo:rerun-if-changed={f}");
    }

    process_command(&Commands::Compile {
        root: DEFAULT_ROOT.into(),
        output,
        csdls,
        entity_type_patterns: service_root_pattens
            .iter()
            .chain(accounts_patterns.iter())
            .chain(events_patterns.iter())
            .map(|v| v.parse())
            .collect::<Result<Vec<_>, _>>()
            .expect("must be successfuly parsed"),
    })?;
    Ok(())
}
