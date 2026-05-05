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

//! Comprehensive Redfish scraper CLI built on top of `nv-redfish-scraper`.
//!
//! Three subcommands cover the common scrape-then-stream workflow against a
//! live BMC:
//!
//! - `discover`: walk the service-root tree (chassis, sensors, computer
//!   systems) and print every observed resource event.
//! - `sensors`: discover the chassis tree, then poll sensor readings on a
//!   tokio interval.
//! - `snapshot`: discover the tree and write `ReconstructionRecord` JSONL to
//!   disk (or stdout) for offline analysis.

// Library-style print lints are deliberately relaxed for this binary: the
// renderers emit user-facing output via println!/eprintln! by design.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use clap::Parser as _;
use std::error::Error as StdError;

mod cli;
mod cmd_discover;
mod cmd_sensors;
mod cmd_snapshot;
mod connect;
mod render;
mod runtime_loop;

#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let cli = cli::Cli::parse();
    match cli.command {
        cli::Command::Discover(args) => cmd_discover::run(&cli.connect, &cli.output, args).await,
        cli::Command::Sensors(args) => cmd_sensors::run(&cli.connect, &cli.output, args).await,
        cli::Command::Snapshot(args) => cmd_snapshot::run(&cli.connect, &cli.output, args).await,
    }
}
