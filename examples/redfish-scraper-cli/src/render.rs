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

//! Rendering for [`RuntimeOutput`] values and post-discovery sensor reads.
//!
//! The CLI exposes two formats:
//!
//! - `Pretty`: one human-readable line per resource event, with Runtime
//!   events gated behind `--verbose`.
//! - `Jsonl`: one JSON object per line, suitable for piping into `jq`.
//!
//! `RuntimeOutput`, `WorkSuccess`, and `WorkError` deliberately do not
//! derive `serde::Serialize` (they are generic over user payloads and the
//! scraper crate avoids imposing those bounds). The JSONL renderer therefore
//! constructs a small per-event view struct that re-uses the underlying
//! event types' own `Serialize` impls.

use crate::cli::Format;
use crate::cli::OutputArgs;
use nv_redfish_scraper::adapter::redfish::ChangeKind;
use nv_redfish_scraper::adapter::redfish::EntityPayload;
use nv_redfish_scraper::adapter::redfish::GeneratorEvent;
use nv_redfish_scraper::adapter::redfish::RedfishAdapterError;
use nv_redfish_scraper::adapter::redfish::RedfishEvent;
use nv_redfish_scraper::adapter::redfish::RedfishResourceEvent;
use nv_redfish_scraper::adapter::redfish::ScrapeEvent;
use nv_redfish_scraper::RuntimeOutput;
use nv_redfish_scraper::WorkError;
use nv_redfish_scraper::WorkSuccess;
use serde::Serialize;
use std::io::Write as _;

/// Render a single [`RuntimeOutput`] under the configured output flags.
pub fn render_output(out: &RuntimeOutput<RedfishEvent, RedfishAdapterError>, args: &OutputArgs) {
    match args.format {
        Format::Pretty => render_pretty(out, args.verbose),
        Format::Jsonl => render_jsonl(out, args.verbose),
    }
}

/// Render a sensor-reading observation produced by a post-discovery sensor
/// poll loop.
pub fn render_sensor_reading(reading: &SensorReadingView<'_>, args: &OutputArgs) {
    match args.format {
        Format::Pretty => println!(
            "sensor {}{} = {} {}",
            reading.name.unwrap_or("<unnamed>"),
            reading.odata_id.map_or_else(String::new, |id| format!(" ({id})")),
            reading.reading.unwrap_or("<no value>"),
            reading.reading_units.unwrap_or(""),
        ),
        Format::Jsonl => write_jsonl(reading),
    }
}

/// Render a sensor-read failure observed during a polling pass.
pub fn render_sensor_error(error: &SensorReadingError<'_>, args: &OutputArgs) {
    match args.format {
        Format::Pretty => eprintln!(
            "sensor {} fetch failed: {}",
            error.odata_id, error.message,
        ),
        Format::Jsonl => write_jsonl(error),
    }
}

fn render_pretty(out: &RuntimeOutput<RedfishEvent, RedfishAdapterError>, verbose: bool) {
    match out {
        RuntimeOutput::Work(Ok(success)) => render_pretty_success(success),
        RuntimeOutput::Work(Err(err)) => render_pretty_error(err),
        RuntimeOutput::Runtime(event) => {
            if verbose {
                eprintln!("[runtime] {event:?}");
            }
        }
        RuntimeOutput::Shutdown => {
            eprintln!("[runtime] shutdown");
        }
    }
}

fn render_pretty_success(success: &WorkSuccess<RedfishEvent>) {
    for event in &success.events {
        match event {
            RedfishEvent::Resource(resource) => println!("{}", PrettyResource(resource)),
            RedfishEvent::Generator(gen) => println!("[generator] {}", PrettyGenerator(gen)),
            RedfishEvent::Scrape(scrape) => println!("[scrape] {}", PrettyScrape(scrape)),
            // `RedfishEvent` is `non_exhaustive`; future variants render as Debug.
            other => println!("[event] {other:?}"),
        }
    }
}

fn render_pretty_error(err: &WorkError<RedfishAdapterError>) {
    eprintln!(
        "[work-error] generator={} latency_ms={} error={}",
        err.generator_id,
        err.stats.latency.as_millis(),
        err.error,
    );
}

fn render_jsonl(out: &RuntimeOutput<RedfishEvent, RedfishAdapterError>, verbose: bool) {
    let view = match out {
        RuntimeOutput::Work(Ok(success)) => {
            for event in &success.events {
                let generator = success.generator_id.to_string();
                let latency_ms = success.stats.latency.as_millis();
                let view = match event {
                    RedfishEvent::Resource(resource) => OutputView::Resource {
                        generator,
                        latency_ms,
                        event: resource,
                    },
                    RedfishEvent::Generator(gen) => OutputView::Generator {
                        generator,
                        latency_ms,
                        event: gen,
                    },
                    RedfishEvent::Scrape(scrape) => OutputView::Scrape {
                        generator,
                        latency_ms,
                        event: scrape,
                    },
                    // `RedfishEvent` is `non_exhaustive`; render unknown variants
                    // as a generic `Runtime`-style description so JSONL stays
                    // forward-compatible.
                    other => OutputView::Runtime {
                        description: format!("{other:?}"),
                    },
                };
                write_jsonl(&view);
            }
            return;
        }
        RuntimeOutput::Work(Err(err)) => OutputView::Error {
            generator: err.generator_id.to_string(),
            latency_ms: err.stats.latency.as_millis(),
            message: err.error.to_string(),
        },
        RuntimeOutput::Runtime(event) => {
            if !verbose {
                return;
            }
            OutputView::Runtime {
                description: format!("{event:?}"),
            }
        }
        RuntimeOutput::Shutdown => OutputView::Shutdown,
    };
    write_jsonl(&view);
}

fn write_jsonl<T: Serialize>(value: &T) {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    if let Err(err) = serde_json::to_writer(&mut handle, value) {
        eprintln!("[render] failed to serialize JSON: {err}");
        return;
    }
    if let Err(err) = handle.write_all(b"\n") {
        eprintln!("[render] failed to write newline: {err}");
    }
}

/// JSONL view of a sensor-reading observation.
#[derive(Serialize)]
pub struct SensorReadingView<'a> {
    /// Discriminator field for jq friendliness.
    #[serde(rename = "kind")]
    pub kind: &'static str,
    /// Sensor `@odata.id`.
    pub odata_id: Option<&'a str>,
    /// Sensor name, if any.
    pub name: Option<&'a str>,
    /// Reading value as rendered by `Sensor.Reading` (string-typed because
    /// readings are decimals stringified at the protocol boundary).
    pub reading: Option<&'a str>,
    /// Reading type (e.g. `Temperature`, `Voltage`).
    pub reading_type: Option<&'a str>,
    /// Reading units (e.g. `Cel`, `V`).
    pub reading_units: Option<&'a str>,
}

impl<'a> SensorReadingView<'a> {
    /// Build a sensor-reading view ready for rendering.
    #[must_use]
    pub const fn new(
        odata_id: Option<&'a str>,
        name: Option<&'a str>,
        reading: Option<&'a str>,
        reading_type: Option<&'a str>,
        reading_units: Option<&'a str>,
    ) -> Self {
        Self {
            kind: "sensor_reading",
            odata_id,
            name,
            reading,
            reading_type,
            reading_units,
        }
    }
}

/// JSONL view of a failed sensor read.
#[derive(Serialize)]
pub struct SensorReadingError<'a> {
    #[serde(rename = "kind")]
    kind: &'static str,
    /// Sensor `@odata.id` that failed to fetch.
    pub odata_id: &'a str,
    /// Underlying error message.
    pub message: String,
}

impl<'a> SensorReadingError<'a> {
    /// Build a sensor-read error view.
    #[must_use]
    pub const fn new(odata_id: &'a str, message: String) -> Self {
        Self {
            kind: "sensor_error",
            odata_id,
            message,
        }
    }
}

/// JSONL projection of a [`RuntimeOutput`] event, defined inline because
/// the runtime types intentionally do not derive `Serialize`.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum OutputView<'a> {
    Resource {
        generator: String,
        latency_ms: u128,
        event: &'a RedfishResourceEvent,
    },
    Generator {
        generator: String,
        latency_ms: u128,
        event: &'a GeneratorEvent,
    },
    Scrape {
        generator: String,
        latency_ms: u128,
        event: &'a ScrapeEvent,
    },
    Error {
        generator: String,
        latency_ms: u128,
        message: String,
    },
    Runtime {
        description: String,
    },
    Shutdown,
}

/// `Display` wrapper for a resource event in pretty mode.
struct PrettyResource<'a>(&'a RedfishResourceEvent);

impl std::fmt::Display for PrettyResource<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let r = self.0;
        let kind = r
            .payload
            .as_ref()
            .map_or("<unknown>", |p: &EntityPayload| p.kind.as_str());
        let parent_segment = r
            .parent_odata_id
            .as_ref()
            .map_or_else(String::new, |p| format!(" parent={p}"));
        write!(
            f,
            "[{}] {} {}{}",
            change_label(r.change),
            kind,
            r.odata_id,
            parent_segment,
        )
    }
}

struct PrettyGenerator<'a>(&'a GeneratorEvent);

impl std::fmt::Display for PrettyGenerator<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            GeneratorEvent::Started { bmc_id, kind } => {
                write!(f, "started bmc={bmc_id} kind={kind}")
            }
            GeneratorEvent::Stopped { bmc_id, kind } => {
                write!(f, "stopped bmc={bmc_id} kind={kind}")
            }
            // `GeneratorEvent` is `non_exhaustive`; fall back to Debug for any
            // future variant the CLI has not been updated for.
            other => write!(f, "{other:?}"),
        }
    }
}

struct PrettyScrape<'a>(&'a ScrapeEvent);

impl std::fmt::Display for PrettyScrape<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            ScrapeEvent::Completed { bmc_id, resources } => {
                write!(f, "completed bmc={bmc_id} resources={resources}")
            }
            ScrapeEvent::Failed { bmc_id, error } => {
                write!(f, "failed bmc={bmc_id} error={error}")
            }
            other => write!(f, "{other:?}"),
        }
    }
}

const fn change_label(change: ChangeKind) -> &'static str {
    match change {
        ChangeKind::Inserted => "inserted",
        ChangeKind::Updated => "updated",
        ChangeKind::RefreshedNoChange => "refreshed",
        ChangeKind::FetchFailed => "fetch-failed",
        ChangeKind::Stale => "stale",
        ChangeKind::Removed => "removed",
        // `ChangeKind` is `non_exhaustive` — keep a forward-compatible label.
        _ => "unknown",
    }
}
