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

//! `sensors` subcommand body.
//!
//! Two phases:
//!
//! 1. Discovery — call `ServiceRoot::chassis_links` directly and `upgrade`
//!    each link into a `Chassis<B>`. Direct access is simpler than running
//!    a full scraper pass because we already have typed handles to the
//!    chassis tree at this point.
//! 2. Streaming — collect every chassis's `SensorLink<B>` once, then enter
//!    a tokio-interval loop that fetches each link in parallel via
//!    `EntityLink::fetch` and renders a small `{name, reading, units}`
//!    projection.
//!
//! The post-discovery loop intentionally bypasses the scraper runtime: the
//! scraper-built sensors generator emits identity events only (no readings).
//! Building a value-aware sensor generator requires a `Telemetry` variant
//! on `RedfishEvent` and lives on the v2 backlog (see README.md).

use crate::cli::ConnectArgs;
use crate::cli::OutputArgs;
use crate::cli::SensorsArgs;
use crate::connect;
use crate::connect::Bmc;
use crate::render;
use crate::render::SensorReadingError;
use crate::render::SensorReadingView;
use futures_util::future::join_all;
use nv_redfish::chassis::Chassis;
use nv_redfish::sensor::SensorLink;
use nv_redfish::ServiceRoot;
use serde_json::Value as JsonValue;
use std::error::Error as StdError;
use tokio::signal;
use tokio::time::interval;

/// Execute the `sensors` subcommand.
///
/// # Errors
///
/// Returns an error if the connection or chassis discovery fails. Per-tick
/// sensor-fetch failures are rendered inline and do not abort the stream.
pub async fn run(
    connect_args: &ConnectArgs,
    output_args: &OutputArgs,
    args: SensorsArgs,
) -> Result<(), Box<dyn StdError>> {
    let conn = connect::connect(connect_args).await?;

    let chassis = load_chassis(&conn.root).await?;
    let links = collect_sensor_links(&chassis).await?;

    if links.is_empty() {
        eprintln!("[sensors] no sensor links discovered; exiting");
        return Ok(());
    }

    eprintln!(
        "[sensors] discovered {} sensor link(s) across {} chassis",
        links.len(),
        chassis.len()
    );

    poll_sensors(&links, &args, output_args).await;
    Ok(())
}

async fn load_chassis(
    root: &ServiceRoot<Bmc>,
) -> Result<Vec<Chassis<Bmc>>, Box<dyn StdError>> {
    let Some(links) = root.chassis_links().await? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(links.len());
    for link in links {
        out.push(link.upgrade::<Chassis<Bmc>>().await?);
    }
    Ok(out)
}

async fn collect_sensor_links(
    chassis: &[Chassis<Bmc>],
) -> Result<Vec<SensorLink<Bmc>>, Box<dyn StdError>> {
    let mut all = Vec::new();
    for c in chassis {
        if let Some(links) = c.sensor_links().await? {
            all.extend(links);
        }
    }
    Ok(all)
}

async fn poll_sensors(
    links: &[SensorLink<Bmc>],
    args: &SensorsArgs,
    output_args: &OutputArgs,
) {
    let mut ticker = interval(args.interval);
    loop {
        tokio::select! {
            biased;
            sig = signal::ctrl_c() => {
                if let Err(err) = sig {
                    eprintln!("[sensors] failed to install Ctrl-C handler: {err}");
                }
                break;
            }
            _ = ticker.tick() => {
                run_one_pass(links, output_args).await;
                if args.once {
                    break;
                }
            }
        }
    }
}

async fn run_one_pass(links: &[SensorLink<Bmc>], output_args: &OutputArgs) {
    let fetched = join_all(links.iter().map(fetch_sensor_value)).await;
    for (link, result) in links.iter().zip(fetched) {
        let odata_id = link.odata_id().to_string();
        match result {
            Ok(value) => render_one(&odata_id, &value, output_args),
            Err(err) => render::render_sensor_error(
                &SensorReadingError::new(&odata_id, err),
                output_args,
            ),
        }
    }
}

/// Fetch one sensor through its link and project it into a `serde_json`
/// value. Serialising the entire schema gives us a transport-form JSON
/// object whose property names match the original Redfish (`Reading`,
/// `ReadingType`, `ReadingUnits`, `Name`, …) regardless of the generated
/// Rust field naming. The renderer pulls the small subset it cares about
/// by string key, which keeps this command independent from the exact
/// shape of the generated `Sensor` schema.
async fn fetch_sensor_value(link: &SensorLink<Bmc>) -> Result<JsonValue, String> {
    let arc = link.fetch().await.map_err(|err| format!("{err}"))?;
    serde_json::to_value(&*arc).map_err(|err| format!("serialize sensor failed: {err}"))
}

fn render_one(odata_id: &str, value: &JsonValue, output_args: &OutputArgs) {
    let name = json_str(value, "Name");
    let reading = json_scalar(value, "Reading");
    let reading_type = json_str(value, "ReadingType");
    let reading_units = json_str(value, "ReadingUnits");
    let view = SensorReadingView::new(
        Some(odata_id),
        name,
        reading.as_deref(),
        reading_type,
        reading_units,
    );
    render::render_sensor_reading(&view, output_args);
}

fn json_str<'a>(value: &'a JsonValue, key: &str) -> Option<&'a str> {
    match value.get(key) {
        Some(JsonValue::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// Render a Redfish "scalar" reading field (`Reading`) as a string. The
/// generated schema decodes `Reading` as a JSON number / decimal, so the
/// rendered form preserves whatever the BMC reported (integer, float, or
/// stringified decimal) without lossy float conversions.
fn json_scalar(value: &JsonValue, key: &str) -> Option<String> {
    match value.get(key)? {
        JsonValue::Null => None,
        JsonValue::String(s) => Some(s.clone()),
        JsonValue::Number(n) => Some(n.to_string()),
        JsonValue::Bool(b) => Some(b.to_string()),
        other => Some(other.to_string()),
    }
}
