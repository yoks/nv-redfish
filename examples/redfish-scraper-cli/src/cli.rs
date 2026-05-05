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

//! Clap argument-parsing types shared by every subcommand.

use clap::Args;
use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use std::path::PathBuf;
use std::time::Duration;
use url::Url;

/// Top-level CLI shape: shared connection / output flags plus a subcommand.
#[derive(Parser, Debug)]
#[command(
    name = "nv-redfish-scraper-cli",
    about = "Comprehensive Redfish scraper CLI",
    version
)]
pub struct Cli {
    /// Connection-flags shared by every subcommand.
    #[command(flatten)]
    pub connect: ConnectArgs,

    /// Output-flags shared by every subcommand.
    #[command(flatten)]
    pub output: OutputArgs,

    /// Selected subcommand.
    #[command(subcommand)]
    pub command: Command,
}

/// Connection flags resolved into an `HttpBmc` + `ServiceRoot` by
/// [`crate::connect`].
///
/// All flags below live on the parent [`Cli`] command via `flatten`, so
/// callers must place them *before* the subcommand on the command line:
///
/// ```text
/// nv-redfish-scraper-cli --bmc URL --insecure discover
/// ```
///
/// `--bmc` is required (clap forbids `global = true` on required args, so
/// this CLI keeps connection flags at the parent rather than promoting
/// them to global).
#[derive(Args, Debug, Clone)]
pub struct ConnectArgs {
    /// Redfish endpoint URL of the BMC, e.g. `https://192.0.2.10/`.
    #[arg(long, value_name = "URL")]
    pub bmc: Url,

    /// Username for HTTP basic authentication. Requires `--password`.
    #[arg(long, requires = "password")]
    pub username: Option<String>,

    /// Password for HTTP basic authentication. Requires `--username`.
    #[arg(long, requires = "username")]
    pub password: Option<String>,

    /// Accept self-signed / invalid TLS certificates.
    #[arg(long, default_value_t = false)]
    pub insecure: bool,

    /// Optional human-readable BMC identifier carried by every resource
    /// event. Defaults to the URL host.
    #[arg(long, value_name = "NAME")]
    pub bmc_id: Option<String>,

    /// HTTP request timeout. Accepts a unit suffix (`s`, `ms`, `m`, `h`).
    #[arg(long, default_value = "30s", value_parser = parse_duration)]
    pub timeout: Duration,
}

/// Output formatting and verbosity flags.
///
/// Like [`ConnectArgs`], these live on the parent and must precede the
/// subcommand.
#[derive(Args, Debug, Clone)]
pub struct OutputArgs {
    /// Output format. `pretty` is human-readable, `jsonl` emits one JSON
    /// object per line for piping into tools like `jq`.
    #[arg(long, value_enum, default_value_t = Format::Pretty)]
    pub format: Format,

    /// Include runtime events (`RuntimeOutput::Runtime`). Off by default.
    #[arg(long, default_value_t = false)]
    pub verbose: bool,

    /// Print final `RuntimeStats` to stderr after the run completes.
    #[arg(long, default_value_t = false)]
    pub stats: bool,
}

/// Selected output format for the renderer.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Human-readable single-line-per-event format.
    Pretty,
    /// One JSON object per line.
    Jsonl,
}

/// Subcommand surface.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Walk service-root → chassis → sensors → computer systems and emit
    /// every observed resource event.
    Discover(DiscoverArgs),
    /// Discover the chassis tree, then poll sensor readings on an interval.
    Sensors(SensorsArgs),
    /// Discover the tree, then write `ReconstructionRecord` JSONL to disk.
    Snapshot(SnapshotArgs),
}

/// Flags accepted by the `discover` subcommand.
#[derive(Args, Debug, Clone)]
pub struct DiscoverArgs {
    /// Skip attaching a chassis generator per discovered chassis.
    #[arg(long, default_value_t = false)]
    pub no_chassis: bool,

    /// Skip attaching a sensors generator per discovered chassis.
    #[arg(long, default_value_t = false)]
    pub no_sensors: bool,

    /// Skip attaching a computer-system generator per discovered system.
    #[arg(long, default_value_t = false)]
    pub no_systems: bool,

    /// Optional global cap on the number of in-flight work items.
    #[arg(long, value_name = "N")]
    pub max_in_flight: Option<u32>,
}

/// Flags accepted by the `sensors` subcommand.
#[derive(Args, Debug, Clone)]
pub struct SensorsArgs {
    /// Polling interval between sensor reads. Accepts a unit suffix.
    #[arg(long, default_value = "5s", value_parser = parse_duration)]
    pub interval: Duration,

    /// Run a single read pass and exit.
    #[arg(long, default_value_t = false)]
    pub once: bool,

    /// Optional global cap on the number of in-flight work items during the
    /// chassis discovery pass.
    #[arg(long, value_name = "N")]
    pub max_in_flight: Option<u32>,
}

/// Flags accepted by the `snapshot` subcommand.
#[derive(Args, Debug, Clone)]
pub struct SnapshotArgs {
    /// Output path. Use `-` for stdout.
    #[arg(long, value_name = "PATH")]
    pub output: PathBuf,

    /// Append to the output file instead of truncating it.
    #[arg(long, default_value_t = false)]
    pub append: bool,

    /// Skip attaching a chassis generator per discovered chassis.
    #[arg(long, default_value_t = false)]
    pub no_chassis: bool,

    /// Skip attaching a sensors generator per discovered chassis.
    #[arg(long, default_value_t = false)]
    pub no_sensors: bool,

    /// Skip attaching a computer-system generator per discovered system.
    #[arg(long, default_value_t = false)]
    pub no_systems: bool,

    /// Optional global cap on the number of in-flight work items.
    #[arg(long, value_name = "N")]
    pub max_in_flight: Option<u32>,
}

/// Parse a duration string of the form `<integer><suffix>`.
///
/// Recognised suffixes: `ms`, `s` (default), `m`, `h`. The empty suffix is
/// treated as seconds. This is a small subset of the `humantime` grammar
/// kept inline so the CLI does not pull a new transitive dependency.
fn parse_duration(s: &str) -> Result<Duration, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err("empty duration".to_string());
    }
    let split = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(trimmed.len());
    let (num, suffix) = trimmed.split_at(split);
    let n: u64 = num
        .parse()
        .map_err(|err| format!("invalid duration {trimmed:?}: {err}"))?;
    let dur = match suffix.trim() {
        "" | "s" | "sec" | "secs" => Duration::from_secs(n),
        "ms" => Duration::from_millis(n),
        "m" | "min" | "mins" => Duration::from_secs(n.saturating_mul(60)),
        "h" | "hr" | "hrs" => Duration::from_secs(n.saturating_mul(3600)),
        other => return Err(format!("unknown duration suffix {other:?}")),
    };
    Ok(dur)
}
