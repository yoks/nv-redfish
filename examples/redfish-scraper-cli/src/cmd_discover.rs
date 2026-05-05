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

//! `discover` subcommand body.
//!
//! Walks `service-root → chassis (× N) → sensors / computer systems` and
//! prints every observed resource event. Generator fan-out is conditional
//! on the `--no-*` flags.

use crate::cli::ConnectArgs;
use crate::cli::DiscoverArgs;
use crate::cli::OutputArgs;
use crate::connect;
use crate::connect::Bmc;
use crate::render;
use crate::runtime_loop;
use nv_redfish::chassis::Chassis;
use nv_redfish::chassis::ChassisLink;
use nv_redfish::computer_system::ComputerSystem;
use nv_redfish::ServiceRoot;
use nv_redfish_scraper::adapter::redfish::build_chassis_generator;
use nv_redfish_scraper::adapter::redfish::build_computer_system_generator;
use nv_redfish_scraper::adapter::redfish::build_sensors_generator;
use nv_redfish_scraper::adapter::redfish::build_service_root_generator;
use nv_redfish_scraper::adapter::redfish::BmcId;
use nv_redfish_scraper::adapter::redfish::RedfishAdapterError;
use nv_redfish_scraper::adapter::redfish::RedfishEvent;
use nv_redfish_scraper::GeneratorConfig;
use nv_redfish_scraper::Runtime;
use nv_redfish_scraper::RuntimeConfig;
use nv_redfish_scraper::TargetId;
use nv_redfish_scraper::TargetLimits;
use std::error::Error as StdError;

/// Execute the `discover` subcommand.
///
/// # Errors
///
/// Returns an error if the connection fails, if the runtime cannot accept
/// the seed target, or if loading the chassis / system collections fails.
pub async fn run(
    connect_args: &ConnectArgs,
    output_args: &OutputArgs,
    args: DiscoverArgs,
) -> Result<(), Box<dyn StdError>> {
    let conn = connect::connect(connect_args).await?;

    let mut runtime = Runtime::<RedfishEvent, RedfishAdapterError>::new(RuntimeConfig {
        global_max_in_flight: args.max_in_flight,
        ..RuntimeConfig::default()
    });

    let target = runtime
        .add_target(TargetLimits::default())
        .ok_or("failed to add target — runtime already shutting down")?;

    runtime
        .add_generator(
            target,
            build_service_root_generator(conn.bmc_id.clone(), conn.root.clone()),
            GeneratorConfig::default(),
        )
        .map_err(|err| format!("failed to add service-root generator: {err}"))?;

    if !args.no_chassis || !args.no_sensors {
        attach_chassis_generators(&runtime, target, &conn.bmc_id, &conn.root, &args).await?;
    }

    if !args.no_systems {
        attach_system_generators(&runtime, target, &conn.bmc_id, &conn.root).await?;
    }

    let final_stats = runtime_loop::drive(&mut runtime, |out| {
        render::render_output(out, output_args);
    })
    .await;

    if output_args.stats {
        eprintln!("[stats] {final_stats:#?}");
    }

    Ok(())
}

/// Walk the chassis collection and attach the requested per-chassis
/// generators.
///
/// `Chassis<B>` does not derive `Clone`, so each generator that takes a
/// `Chassis<B>` by value needs its own materialised handle. The CLI gets
/// them via `ChassisLink::upgrade`, which the BMC's response cache
/// short-circuits on the second call so we pay at most one network fetch
/// per chassis.
async fn attach_chassis_generators(
    runtime: &Runtime<RedfishEvent, RedfishAdapterError>,
    target: TargetId,
    bmc_id: &BmcId,
    root: &ServiceRoot<Bmc>,
    args: &DiscoverArgs,
) -> Result<(), Box<dyn StdError>> {
    let Some(links) = root.chassis_links().await? else {
        return Ok(());
    };
    for link in links {
        attach_chassis_pair(runtime, target, bmc_id, &link, args).await?;
    }
    Ok(())
}

async fn attach_chassis_pair(
    runtime: &Runtime<RedfishEvent, RedfishAdapterError>,
    target: TargetId,
    bmc_id: &BmcId,
    link: &ChassisLink<Bmc>,
    args: &DiscoverArgs,
) -> Result<(), Box<dyn StdError>> {
    if !args.no_chassis {
        let chassis: Chassis<Bmc> = link.upgrade().await?;
        runtime
            .add_generator(
                target,
                build_chassis_generator(bmc_id.clone(), chassis),
                GeneratorConfig::default(),
            )
            .map_err(|err| format!("failed to add chassis generator: {err}"))?;
    }
    if !args.no_sensors {
        let chassis: Chassis<Bmc> = link.upgrade().await?;
        runtime
            .add_generator(
                target,
                build_sensors_generator(bmc_id.clone(), chassis),
                GeneratorConfig::default(),
            )
            .map_err(|err| format!("failed to add sensors generator: {err}"))?;
    }
    Ok(())
}

async fn attach_system_generators(
    runtime: &Runtime<RedfishEvent, RedfishAdapterError>,
    target: TargetId,
    bmc_id: &BmcId,
    root: &ServiceRoot<Bmc>,
) -> Result<(), Box<dyn StdError>> {
    let Some(coll) = root.systems().await? else {
        return Ok(());
    };
    let systems: Vec<ComputerSystem<Bmc>> = coll.members().await?;
    for system in systems {
        runtime
            .add_generator(
                target,
                build_computer_system_generator(bmc_id.clone(), system),
                GeneratorConfig::default(),
            )
            .map_err(|err| format!("failed to add computer-system generator: {err}"))?;
    }
    Ok(())
}
