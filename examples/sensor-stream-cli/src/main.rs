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

use clap::Parser;
use futures_util::stream;
use futures_util::StreamExt as _;
use nv_redfish::bmc_http::reqwest::BmcError;
use nv_redfish::bmc_http::reqwest::Client;
use nv_redfish::bmc_http::reqwest::ClientParams;
use nv_redfish::bmc_http::BmcCredentials;
use nv_redfish::bmc_http::CacheSettings;
use nv_redfish::bmc_http::HttpBmc;
use nv_redfish::core::EntityTypeRef as _;
use nv_redfish::core::ODataId;
use nv_redfish::schema::sensor::Sensor;
use nv_redfish::sensor::SensorLink;
use nv_redfish::Bmc;
use nv_redfish::ServiceRoot;
use nv_redfish_scraper::ClassId;
use nv_redfish_scraper::CostUnits;
use nv_redfish_scraper::Generator;
use nv_redfish_scraper::GeneratorConfig;
use nv_redfish_scraper::GeneratorId;
use nv_redfish_scraper::Readiness;
use nv_redfish_scraper::RunOutcome;
use nv_redfish_scraper::Runtime;
use nv_redfish_scraper::RuntimeConfig;
use nv_redfish_scraper::RuntimeOutput;
use nv_redfish_scraper::ScheduledWork;
use nv_redfish_scraper::TargetId;
use nv_redfish_scraper::TargetLimits;
use nv_redfish_scraper::WorkCompletion;
use nv_redfish_scraper::WorkMeta;
use serde::Serialize;
use std::collections::BTreeSet;
use std::error::Error;
use std::error::Error as StdError;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::time;
use url::Url;

type ReqwestBmc = HttpBmc<Client>;

#[derive(Debug, Parser)]
#[command(about = "Poll Redfish sensors from a real BMC and print newline JSON")]
struct Args {
    /// BMC base URL, for example https://192.168.1.100.
    #[arg(long)]
    url: Url,

    /// BMC username.
    #[arg(long)]
    username: Option<String>,

    /// BMC password.
    #[arg(long)]
    password: Option<String>,

    /// Poll interval in seconds.
    #[arg(long, default_value_t = 30)]
    interval_secs: u64,

    /// Number of poll iterations. Omit to run until Ctrl-C.
    #[arg(long)]
    iterations: Option<usize>,

    /// Maximum concurrent sensor fetches.
    #[arg(long, default_value_t = 16)]
    concurrency: usize,

    /// Accept invalid TLS certificates.
    #[arg(long, alias = "accept-invalid-certs")]
    insecure: bool,

    /// Include component-linked sensors from systems and power supplies.
    #[arg(long)]
    include_component_sensors: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum SensorStreamEvent {
    Sensor(Box<SensorRecord>),
    SensorError(SensorErrorRecord),
}

#[derive(Debug, Serialize)]
struct SensorRecord {
    sampled_at_unix_ms: u128,
    odata_id: String,
    name: Option<String>,
    reading: Option<f64>,
    reading_type: Option<String>,
    reading_units: Option<String>,
    health: Option<String>,
    state: Option<String>,
    physical_context: Option<String>,
    upper_critical: Option<f64>,
    lower_critical: Option<f64>,
    upper_fatal: Option<f64>,
    lower_fatal: Option<f64>,
}

#[derive(Debug, Serialize)]
struct SensorErrorRecord {
    sampled_at_unix_ms: u128,
    odata_id: String,
    error: String,
}

#[derive(Debug)]
struct SensorStreamError {
    message: String,
}

impl fmt::Display for SensorStreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl Error for SensorStreamError {}

fn unix_ms(now: SystemTime) -> u128 {
    now.duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

fn sensor_record(sensor: &Sensor, sampled_at: SystemTime) -> SensorRecord {
    let status = sensor.status.as_ref();
    let thresholds = sensor.thresholds.as_ref();

    SensorRecord {
        sampled_at_unix_ms: unix_ms(sampled_at),
        odata_id: sensor.odata_id().to_string(),
        name: Some(sensor.base.name.clone()),
        reading: sensor.reading.flatten(),
        reading_type: sensor
            .reading_type
            .flatten()
            .map(|value| format!("{value:?}")),
        reading_units: sensor.reading_units.clone().flatten(),
        health: status
            .and_then(|status| status.health.flatten())
            .map(|value| format!("{value:?}")),
        state: status
            .and_then(|status| status.state.flatten())
            .map(|value| format!("{value:?}")),
        physical_context: sensor
            .physical_context
            .flatten()
            .map(|value| format!("{value:?}")),
        upper_critical: thresholds.and_then(|value| {
            value
                .upper_critical
                .as_ref()
                .and_then(|threshold| threshold.reading.flatten())
        }),
        lower_critical: thresholds.and_then(|value| {
            value
                .lower_critical
                .as_ref()
                .and_then(|threshold| threshold.reading.flatten())
        }),
        upper_fatal: thresholds.and_then(|value| {
            value
                .upper_fatal
                .as_ref()
                .and_then(|threshold| threshold.reading.flatten())
        }),
        lower_fatal: thresholds.and_then(|value| {
            value
                .lower_fatal
                .as_ref()
                .and_then(|threshold| threshold.reading.flatten())
        }),
    }
}

async fn collect_chassis_sensor_links<B>(
    service_root: &ServiceRoot<B>,
) -> Result<Vec<SensorLink<B>>, nv_redfish::Error<B>>
where
    B: Bmc + 'static,
{
    let mut links = Vec::new();
    if let Some(chassis_collection) = service_root.chassis().await? {
        for chassis in chassis_collection.members().await? {
            if let Some(sensors) = chassis.sensor_links().await? {
                links.extend(sensors);
            }
            links.extend(chassis.environment_sensor_links().await?);
        }
    }
    Ok(links)
}

async fn collect_component_sensor_links<B>(
    service_root: &ServiceRoot<B>,
) -> Result<Vec<SensorLink<B>>, nv_redfish::Error<B>>
where
    B: Bmc + 'static,
{
    let mut links = Vec::new();

    if let Some(system_collection) = service_root.systems().await? {
        for system in system_collection.members().await? {
            for processor in system.processors().await?.unwrap_or_default() {
                links.extend(processor.environment_sensor_links().await?);
                links.extend(processor.metrics_sensor_links().await?);
            }

            for memory in system.memory_modules().await?.unwrap_or_default() {
                links.extend(memory.environment_sensor_links().await?);
            }

            for storage in system.storage_controllers().await?.unwrap_or_default() {
                for drive in storage.drives().await?.unwrap_or_default() {
                    links.extend(drive.environment_sensor_links().await?);
                }
            }
        }
    }

    if let Some(chassis_collection) = service_root.chassis().await? {
        for chassis in chassis_collection.members().await? {
            for power_supply in chassis.power_supplies().await? {
                links.extend(power_supply.metrics_sensor_links().await?);
            }
        }
    }

    Ok(links)
}

async fn discover_sensor_links(
    bmc: Arc<ReqwestBmc>,
    include_component_sensors: bool,
) -> Result<Vec<SensorLink<ReqwestBmc>>, nv_redfish::Error<ReqwestBmc>> {
    let service_root = ServiceRoot::new(bmc).await?;
    let mut links = collect_chassis_sensor_links(&service_root).await?;

    if include_component_sensors {
        links.extend(collect_component_sensor_links(&service_root).await?);
    }

    let mut seen = BTreeSet::<ODataId>::new();
    Ok(links
        .into_iter()
        .filter(|link| seen.insert(link.odata_id().clone()))
        .collect())
}

async fn fetch_sensor_events(
    bmc: Arc<ReqwestBmc>,
    sensor_ids: Arc<Vec<ODataId>>,
    concurrency: usize,
) -> Vec<SensorStreamEvent> {
    let sampled_at = SystemTime::now();
    let concurrency = concurrency.max(1);
    stream::iter(sensor_ids.iter().cloned())
        .map(|odata_id| {
            let bmc = bmc.clone();
            async move {
                let id_text = odata_id.to_string();
                match bmc.get::<Sensor>(&odata_id).await {
                    Ok(sensor) => SensorStreamEvent::Sensor(Box::new(sensor_record(
                        sensor.as_ref(),
                        sampled_at,
                    ))),
                    Err(error) => SensorStreamEvent::SensorError(SensorErrorRecord {
                        sampled_at_unix_ms: unix_ms(sampled_at),
                        odata_id: id_text,
                        error: error.to_string(),
                    }),
                }
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await
}

fn print_event(event: &SensorStreamEvent) {
    match serde_json::to_string(event) {
        Ok(line) => println!("{line}"),
        Err(error) => eprintln!("failed to encode sensor record: {error}"),
    }
}

struct SensorPollGenerator {
    target_id: TargetId,
    generator_id: GeneratorId,
    class_id: ClassId,
    bmc: Arc<ReqwestBmc>,
    sensor_ids: Arc<Vec<ODataId>>,
    concurrency: usize,
}

impl SensorPollGenerator {
    fn new(
        target_id: TargetId,
        generator_id: GeneratorId,
        class_id: ClassId,
        bmc: Arc<ReqwestBmc>,
        sensor_ids: Vec<ODataId>,
        concurrency: usize,
    ) -> Self {
        Self {
            target_id,
            generator_id,
            class_id,
            bmc,
            sensor_ids: Arc::new(sensor_ids),
            concurrency,
        }
    }
}

impl Generator<SensorStreamEvent, SensorStreamError> for SensorPollGenerator {
    fn update_ready(&mut self, _now: Instant) -> Readiness {
        Readiness::ready(CostUnits::new(self.sensor_ids.len().max(1) as u64))
    }

    fn take_next(&mut self) -> Option<ScheduledWork<SensorStreamEvent, SensorStreamError>> {
        let meta = WorkMeta::new(
            self.target_id.clone(),
            self.generator_id.clone(),
            self.class_id.clone(),
            CostUnits::new(self.sensor_ids.len().max(1) as u64),
        );
        let bmc = self.bmc.clone();
        let sensor_ids = self.sensor_ids.clone();
        let concurrency = self.concurrency;

        Some(ScheduledWork::new(meta, async move {
            Ok(fetch_sensor_events(bmc, sensor_ids, concurrency).await)
        }))
    }

    fn on_complete(&mut self, _completion: &WorkCompletion) {}
}

fn credentials(args: &Args) -> Result<BmcCredentials, Box<dyn StdError>> {
    match (&args.username, &args.password) {
        (Some(username), password) => Ok(BmcCredentials::username_password(
            username.clone(),
            password.clone(),
        )),
        (None, Some(_)) => Err("--password requires --username".into()),
        (None, None) => Ok(BmcCredentials::none()),
    }
}

fn create_bmc(args: &Args) -> Result<Arc<ReqwestBmc>, Box<dyn StdError>> {
    let client = Client::with_params(ClientParams::new().accept_invalid_certs(args.insecure))
        .map_err(BmcError::ReqwestError)?;
    let credentials = credentials(args)?;

    Ok(Arc::new(HttpBmc::new(
        client,
        args.url.clone(),
        credentials,
        CacheSettings::default(),
    )))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let args = Args::parse();
    let bmc = create_bmc(&args)?;
    let links = discover_sensor_links(bmc.clone(), args.include_component_sensors).await?;
    let sensor_ids = links
        .iter()
        .map(|link| link.odata_id().clone())
        .collect::<Vec<_>>();

    eprintln!("discovered {} sensor links", sensor_ids.len());

    let target_id = TargetId::new("bmc");
    let generator_id = GeneratorId::new("redfish.sensor-poll");
    let class_id = ClassId::new("redfish.sensors");
    let mut runtime = Runtime::<SensorStreamEvent, SensorStreamError>::new(RuntimeConfig::new(1));
    runtime.add_target(target_id.clone(), TargetLimits::new(1))?;
    runtime.add_generator(
        &target_id,
        generator_id.clone(),
        GeneratorConfig::new(true)
            .with_requested_interval(Duration::from_secs(args.interval_secs.max(1))),
        SensorPollGenerator::new(
            target_id.clone(),
            generator_id,
            class_id,
            bmc,
            sensor_ids,
            args.concurrency,
        ),
    )?;

    match args.iterations {
        Some(iterations) => {
            let mut completed_iterations = 0usize;
            while completed_iterations < iterations {
                if run_scraper_once(&mut runtime).await? {
                    completed_iterations += 1;
                }
                time::sleep(Duration::from_millis(100)).await;
            }
        }
        None => loop {
            let _ = run_scraper_once(&mut runtime).await?;
            time::sleep(Duration::from_millis(100)).await;
        },
    }

    Ok(())
}

async fn run_scraper_once(
    runtime: &mut Runtime<SensorStreamEvent, SensorStreamError>,
) -> Result<bool, Box<dyn StdError>> {
    let outcome = runtime.run_once(Instant::now()).await?;
    for output in runtime.drain_outputs() {
        match output {
            RuntimeOutput::Work(Ok(success)) => {
                for event in success.events() {
                    print_event(event);
                }
            }
            RuntimeOutput::Work(Err(error)) => {
                eprintln!("sensor poll failed: {}", error.error());
            }
            RuntimeOutput::Runtime(_) => {}
        }
    }

    Ok(matches!(outcome, RunOutcome::Dispatched))
}
