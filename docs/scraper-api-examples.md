# Redfish Scraper API Examples

These examples describe the intended user-facing API. Names are provisional.

## Create A Scraper

```rust
use nv_redfish_scraper::{BmcCapacity, Discovery, Scraper};

let scraper = Scraper::builder(bmc)
    .capacity(
        BmcCapacity::adaptive()
            .initial_in_flight(1)
            .max_in_flight(16)
            .max_requests_per_second(30)
            .interactive_share(50)
            .subscription_share(30)
            .discovery_share(15)
            .maintenance_share(5),
    )
    .discover(Discovery::standard())
    .build()
    .await?;

let scraper_task = scraper.spawn();
```

Registering discovery makes strategies available. It does not require a full eager BMC crawl.

## List Temperature Sensors Once

```rust
use nv_redfish_scraper::sensor;
use redfish_std::redfish::sensor::Sensor;

let temps = scraper
    .query::<Sensor>()
    .where_(sensor::reading_type().temperature())
    .list()
    .await?;

for temp in temps {
    println!("{} = {:?}", temp.id, temp.value.reading);
}
```

`list()` creates temporary demand. It can run discovery and fetch candidate resources, but it does not keep polling after it returns.

## Subscribe To Temperature Sensors

```rust
use std::time::Duration;

use futures_util::StreamExt;
use nv_redfish_scraper::{sensor, ResourceEvent};
use redfish_std::redfish::sensor::Sensor;

let mut temps = scraper
    .query::<Sensor>()
    .where_(sensor::reading_type().temperature())
    .freshness(Duration::from_secs(5))
    .discovery_freshness(Duration::from_secs(60))
    .subscribe()
    .await?;

while let Some(event) = temps.next().await {
    match event {
        ResourceEvent::Added(snapshot) => {
            println!("new temp sensor: {}", snapshot.id);
        }
        ResourceEvent::Updated { new, .. } => {
            println!("{} = {:?}", new.id, new.value.reading);
        }
        ResourceEvent::Removed(id) => {
            println!("removed temp sensor: {id}");
        }
        ResourceEvent::Error { id, error } => {
            eprintln!("failed to refresh {id}: {error}");
        }
        ResourceEvent::FreshnessMissed { id, age, desired } => {
            eprintln!("{id} is stale: age={age:?}, desired={desired:?}");
        }
    }
}
```

This means:

- discover temperature sensors
- re-check membership about every 60 seconds
- refresh matching sensors about every 5 seconds when the BMC can handle it
- emit changes and freshness misses

## Query By Relation

```rust
use std::time::Duration;

use nv_redfish_scraper::sensor;
use redfish_std::redfish::drive::Drive;
use redfish_std::redfish::sensor::Sensor;

let mut drive_temps = scraper
    .query::<Sensor>()
    .where_(sensor::reading_type().temperature())
    .where_(sensor::related_to::<Drive>())
    .freshness(Duration::from_secs(10))
    .discovery_freshness(Duration::from_secs(60))
    .subscribe()
    .await?;
```

The relation predicate can guide discovery toward drive paths, but matching is still validated against fetched snapshots and store relations.

## Query With Text Fallbacks

```rust
use nv_redfish_scraper::sensor;
use redfish_std::redfish::sensor::Sensor;

let gpu_temps = scraper
    .query::<Sensor>()
    .where_(sensor::reading_type().temperature())
    .where_(sensor::name().contains("GPU"))
    .list()
    .await?;
```

Semantic predicates should be preferred when available. Text predicates are useful for vendor-specific or inconsistent BMCs.

## Keep A Resource Set Warm Without Reading Events

```rust
use std::time::Duration;

use redfish_std::redfish::drive::Drive;

let drive_watch = scraper
    .query::<Drive>()
    .freshness(Duration::from_secs(30))
    .discovery_freshness(Duration::from_secs(300))
    .watch()
    .await?;

let drives = scraper.resources::<Drive>().list_cached();

drop(drive_watch);
```

`watch()` creates background demand. Dropping the watch removes the demand.

## Direct Access To A Known Resource

```rust
use redfish_std::redfish::sensor::Sensor;

let sensor = scraper
    .resources::<Sensor>()
    .refresh("/redfish/v1/Chassis/1/Sensors/InletTemp")
    .await?;

println!("fresh value: {:?}", sensor.value.reading);
```

`refresh(id)` always attempts BMC revalidation, but still respects scheduler limits.

## Cached-Only Access

```rust
use redfish_std::redfish::sensor::Sensor;

if let Some(sensor) = scraper
    .resources::<Sensor>()
    .cached("/redfish/v1/Chassis/1/Sensors/InletTemp")
{
    println!("cached value: {:?}", sensor.value.reading);
}
```

`cached(id)` never performs BMC I/O.

## Subscribe To The Global Event Stream

```rust
use futures_util::StreamExt;
use nv_redfish_scraper::{ScraperEvent, SchedulerEvent};

let mut events = scraper.subscribe_events();

while let Some(envelope) = events.next().await {
    match envelope.event {
        ScraperEvent::Resource(event) => {
            tracing::debug!(seq = envelope.seq, ?event, "resource event");
        }
        ScraperEvent::Scheduler(SchedulerEvent::LoadChanged { state }) => {
            tracing::info!(?state, "BMC load state changed");
        }
        _ => {}
    }
}
```

Typed query subscriptions are filtered views over this same event stream.

## Custom Discovery

```rust
use nv_redfish_scraper::{
    Discoverer, DiscoveryBatch, DiscoveryContext, DiscoveryHint, ODataId,
};
use redfish_std::redfish::sensor::Sensor;

struct MyVendorSensorDiscovery;

#[async_trait::async_trait]
impl Discoverer<Sensor> for MyVendorSensorDiscovery {
    async fn discover(
        &self,
        cx: &mut DiscoveryContext<'_>,
        hint: DiscoveryHint,
    ) -> Result<DiscoveryBatch, nv_redfish_scraper::Error> {
        let ids: Vec<ODataId> = cx
            .raw_json("/redfish/v1/Oem/MyVendor/Sensors")
            .await?
            .extract_sensor_ids();

        Ok(DiscoveryBatch::candidates(ids))
    }
}

let scraper = Scraper::builder(bmc)
    .discover(Discovery::standard())
    .discover(MyVendorSensorDiscovery)
    .build()
    .await?;
```

Custom discoverers add candidate URIs. The query engine still fetches and filters snapshots.

## Health Service Integration

The health service should run one scraper per BMC endpoint and project scraper events into existing health events.

```rust
use std::time::Duration;

use futures_util::StreamExt;
use nv_redfish_scraper::{sensor, BmcCapacity, Discovery, ResourceEvent, Scraper};
use redfish_std::redfish::sensor::Sensor;

let scraper = Scraper::builder(bmc)
    .capacity(BmcCapacity::adaptive())
    .discover(Discovery::standard())
    .build()
    .await?;

let mut sensors = scraper
    .query::<Sensor>()
    .freshness(sensor_cfg.sensor_fetch_interval)
    .discovery_freshness(sensor_cfg.state_refresh_interval)
    .subscribe()
    .await?;

while let Some(event) = sensors.next().await {
    match event {
        ResourceEvent::Added(snapshot) | ResourceEvent::Updated { new: snapshot, .. } => {
            if let Some(metric) = sensor_projection.to_health_metric(snapshot).await? {
                data_sink.handle_event(&event_context, &metric.into());
            }
        }
        ResourceEvent::FreshnessMissed { id, age, desired } => {
            tracing::warn!(%id, ?age, ?desired, "sensor freshness missed");
        }
        ResourceEvent::Error { id, error } => {
            tracing::warn!(%id, ?error, "sensor refresh failed");
        }
        ResourceEvent::Removed(_) => {}
    }
}
```

The health projection owns health-specific behavior:

- metric labels
- unit sanitization
- threshold attributes
- derived health reports
- sink emission

The scraper owns Redfish discovery, refresh, scheduling, caching, and freshness.

## Firmware Projection

```rust
use std::time::Duration;

use futures_util::StreamExt;
use redfish_std::redfish::software_inventory::SoftwareInventory;

let mut firmware = scraper
    .query::<SoftwareInventory>()
    .freshness(Duration::from_secs(60 * 60 * 2))
    .subscribe()
    .await?;

while let Some(event) = firmware.next().await {
    if let ResourceEvent::Added(snapshot) | ResourceEvent::Updated { new: snapshot, .. } = event {
        if let Some(info) = firmware_projection.to_firmware_info(snapshot) {
            data_sink.handle_event(&event_context, &CollectorEvent::Firmware(info));
        }
    }
}
```

## Log Service Projection

```rust
use std::time::Duration;

use redfish_std::redfish::log_service::LogService;

let log_services = scraper
    .query::<LogService>()
    .freshness(Duration::from_secs(1800))
    .watch()
    .await?;
```

For the first migration, health should keep persistent `last_seen_id` and log file writing policy. The scraper can discover services, schedule reads, and protect the BMC.

## API Semantics Summary

```text
query::<T>()       discover, filter, and manage a resource set
resources::<T>()   direct typed access to known or explicit resources
list()             one-shot discover/fetch
subscribe()        watch changes and keep matching resources fresh
watch()            keep matching resources fresh without returning typed events
refresh(id)        force immediate revalidation through the scheduler
cached(id)         read local store only
subscribe_events() observe the shared event stream
```
