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

//! `snapshot` subcommand body.
//!
//! Runs the same scraper discovery as `discover`, accumulates every emitted
//! [`RedfishResourceEvent`], then derives [`ReconstructionRecord`]s via
//! [`reconstruction_iter`] and writes them as JSONL. The output destination
//! is `stdout` when `--output -` is supplied and a filesystem path
//! otherwise. The command preserves the discover renderer for live progress
//! (mirrored to stderr in the file-output case so stdout stays a clean JSON
//! stream when `--output -` is used).

use crate::cli::ConnectArgs;
use crate::cli::Format;
use crate::cli::OutputArgs;
use crate::cli::SnapshotArgs;
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
use nv_redfish_scraper::adapter::redfish::RedfishResourceEvent;
use nv_redfish_scraper::reconstruction_iter;
use nv_redfish_scraper::GeneratorConfig;
use nv_redfish_scraper::Runtime;
use nv_redfish_scraper::RuntimeConfig;
use nv_redfish_scraper::RuntimeOutput;
use nv_redfish_scraper::TargetId;
use nv_redfish_scraper::TargetLimits;
use nv_redfish_scraper::WorkSuccess;
use std::error::Error as StdError;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::io::Write as _;
use std::path::Path;

/// Execute the `snapshot` subcommand.
///
/// # Errors
///
/// Returns an error if the connection fails, if the runtime cannot accept
/// the seed target, if loading the chassis / system collections fails,
/// or if the output writer cannot be opened / written.
pub async fn run(
    connect_args: &ConnectArgs,
    output_args: &OutputArgs,
    args: SnapshotArgs,
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

    let mut collected: Vec<RedfishResourceEvent> = Vec::new();
    let final_stats = runtime_loop::drive(&mut runtime, |out| {
        snapshot_progress(out, output_args);
        if let RuntimeOutput::Work(Ok(WorkSuccess { events, .. })) = out {
            collected.extend(events.iter().filter_map(|event| match event {
                RedfishEvent::Resource(resource) => Some(resource.clone()),
                _ => None,
            }));
        }
    })
    .await;

    let written = write_records(&collected, &args)?;

    if output_args.stats {
        eprintln!("[stats] {final_stats:#?}");
    }
    eprintln!(
        "[snapshot] wrote {written} reconstruction record(s) from {} resource event(s) to {}",
        collected.len(),
        display_destination(&args.output),
    );

    Ok(())
}

/// Mirror `discover`'s rendering during the snapshot pass so the user can
/// follow progress. When `--output -` selects stdout for the JSONL records,
/// pretty progress is suppressed so the snapshot stream stays clean. JSONL
/// progress to stdout is suppressed unconditionally during snapshot to keep
/// the stdout stream reserved for the reconstruction records emitted at
/// the end of the run.
fn snapshot_progress(
    out: &RuntimeOutput<RedfishEvent, RedfishAdapterError>,
    output_args: &OutputArgs,
) {
    if matches!(output_args.format, Format::Jsonl) {
        return;
    }
    render::render_output(out, output_args);
}

async fn attach_chassis_generators(
    runtime: &Runtime<RedfishEvent, RedfishAdapterError>,
    target: TargetId,
    bmc_id: &BmcId,
    root: &ServiceRoot<Bmc>,
    args: &SnapshotArgs,
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
    args: &SnapshotArgs,
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

/// Open the destination writer, derive `ReconstructionRecord`s from
/// `events`, and write one JSON object per line. Returns the number of
/// records written.
fn write_records(events: &[RedfishResourceEvent], args: &SnapshotArgs) -> Result<usize, Box<dyn StdError>> {
    let records = reconstruction_iter(events.iter());
    let mut writer = open_writer(&args.output, args.append)?;
    let mut written = 0_usize;
    for record in records {
        serde_json::to_writer(&mut writer, &record)?;
        writer.write_all(b"\n")?;
        written += 1;
    }
    writer.flush()?;
    Ok(written)
}

/// Open either stdout (when `path` is `-`) or a file in append / truncate
/// mode and wrap it in a `BufWriter`.
fn open_writer(
    path: &Path,
    append: bool,
) -> Result<BufWriter<Box<dyn std::io::Write>>, Box<dyn StdError>> {
    let inner: Box<dyn std::io::Write> = if path == Path::new("-") {
        Box::new(std::io::stdout().lock())
    } else {
        let mut opts = OpenOptions::new();
        opts.write(true).create(true);
        if append {
            opts.append(true);
        } else {
            opts.truncate(true);
        }
        Box::new(opts.open(path)?)
    };
    Ok(BufWriter::new(inner))
}

fn display_destination(path: &Path) -> String {
    if path == Path::new("-") {
        "<stdout>".to_string()
    } else {
        path.display().to_string()
    }
}
