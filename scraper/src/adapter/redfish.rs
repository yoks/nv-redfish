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

//! Redfish adapter API boundary.

use crate::generator::Generator;
use crate::generator::ScheduledWork;
use crate::generator::WorkCompletion;
use crate::generator::WorkMeta;
use crate::ids::ClassId;
use crate::ids::GeneratorId;
use crate::ids::TargetId;
use crate::scheduler::CostUnits;
use crate::scheduler::Readiness;
use core::fmt;
use nv_redfish::core::ODataETag;
use nv_redfish::core::ODataId;
use nv_redfish::Bmc;
use nv_redfish::ServiceRoot;
use std::error::Error as StdError;
use std::marker::PhantomData;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

/// Opaque application identity for a BMC.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BmcId(String);

impl BmcId {
    /// Creates a BMC id.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the BMC id text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BmcId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Change classification for a Redfish resource event.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    /// Resource was observed for the first time.
    Inserted,
    /// Resource was observed with changed data.
    Updated,
    /// Resource was refreshed without a known data change.
    Refreshed,
    /// Resource fetch failed.
    FetchFailed,
    /// Resource became stale.
    Stale,
    /// Resource was removed.
    Removed,
}

/// Narrow boundary expected from generated `EntityPayload` support.
pub trait EntityPayload {
    /// Returns the generated entity kind.
    fn entity_kind(&self) -> &str;

    /// Returns the resource `@odata.id`, when present.
    fn odata_id(&self) -> Option<&ODataId>;

    /// Returns the resource `@odata.etag`, when present.
    fn etag(&self) -> Option<&ODataETag>;
}

/// Uninhabited placeholder used when no generated entity payload is attached.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum NoEntityPayload {}

/// Scrape metadata attached to a Redfish resource event.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceMetadata {
    scraped_at: SystemTime,
    latency: Duration,
    generation: u64,
    error: Option<String>,
}

impl ResourceMetadata {
    /// Creates resource metadata.
    #[must_use]
    pub const fn new(
        scraped_at: SystemTime,
        latency: Duration,
        generation: u64,
        error: Option<String>,
    ) -> Self {
        Self {
            scraped_at,
            latency,
            generation,
            error,
        }
    }

    /// Returns the scrape timestamp.
    #[must_use]
    pub const fn scraped_at(&self) -> SystemTime {
        self.scraped_at
    }

    /// Returns observed scrape latency.
    #[must_use]
    pub const fn latency(&self) -> Duration {
        self.latency
    }

    /// Returns adapter/application generation metadata.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the fetch error text, when represented as metadata.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

/// Redfish resource work event.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RedfishResourceEvent<P = NoEntityPayload> {
    bmc_id: BmcId,
    odata_id: ODataId,
    parent_odata_id: Option<ODataId>,
    change: ChangeKind,
    payload: Option<P>,
    metadata: ResourceMetadata,
}

impl<P> RedfishResourceEvent<P> {
    /// Creates a Redfish resource event.
    #[must_use]
    pub const fn new(
        bmc_id: BmcId,
        odata_id: ODataId,
        parent_odata_id: Option<ODataId>,
        change: ChangeKind,
        payload: Option<P>,
        metadata: ResourceMetadata,
    ) -> Self {
        Self {
            bmc_id,
            odata_id,
            parent_odata_id,
            change,
            payload,
            metadata,
        }
    }

    /// Returns the source BMC id.
    #[must_use]
    pub const fn bmc_id(&self) -> &BmcId {
        &self.bmc_id
    }

    /// Returns the resource `@odata.id`.
    #[must_use]
    pub const fn odata_id(&self) -> &ODataId {
        &self.odata_id
    }

    /// Returns the parent resource `@odata.id`, when known.
    #[must_use]
    pub const fn parent_odata_id(&self) -> Option<&ODataId> {
        self.parent_odata_id.as_ref()
    }

    /// Returns the event change kind.
    #[must_use]
    pub const fn change(&self) -> &ChangeKind {
        &self.change
    }

    /// Returns the generated entity payload, when present.
    #[must_use]
    pub const fn payload(&self) -> Option<&P> {
        self.payload.as_ref()
    }

    /// Returns scrape metadata.
    #[must_use]
    pub const fn metadata(&self) -> &ResourceMetadata {
        &self.metadata
    }

    /// Consumes the event and returns its payload.
    #[must_use]
    pub fn into_payload(self) -> Option<P> {
        self.payload
    }
}

/// Adapter-level generator event.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratorEvent {
    /// A generator was created by the adapter.
    Created,
    /// A generator observed lag or missed interval metadata.
    LagObserved,
}

/// Adapter-level scrape event.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScrapeEvent {
    /// A scrape operation began.
    Started,
    /// A scrape operation completed.
    Completed,
    /// A scrape operation failed.
    Failed,
}

/// Redfish adapter work event.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum RedfishEvent<P = NoEntityPayload> {
    /// Resource data or resource fetch result.
    Resource(RedfishResourceEvent<P>),
    /// Adapter generator fact.
    Generator(GeneratorEvent),
    /// Adapter scrape fact.
    Scrape(ScrapeEvent),
}

/// Redfish adapter error.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RedfishAdapterError {
    /// Real adapter fetching is intentionally not implemented in Phase 0.
    NotImplemented,
}

impl fmt::Display for RedfishAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotImplemented => {
                formatter.write_str("redfish adapter behavior is not implemented")
            }
        }
    }
}

impl StdError for RedfishAdapterError {}

impl EntityPayload for nv_redfish::EntityPayload {
    fn entity_kind(&self) -> &str {
        self.kind()
    }

    fn odata_id(&self) -> Option<&ODataId> {
        self.resource_odata_id()
    }

    fn etag(&self) -> Option<&ODataETag> {
        self.resource_etag()
    }
}

/// Stored Redfish resource identity used by optional reconstruction helpers.
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ReconstructionRecord<P = NoEntityPayload> {
    bmc_id: BmcId,
    odata_id: ODataId,
    parent_odata_id: Option<ODataId>,
    payload: Option<P>,
}

impl<P> ReconstructionRecord<P> {
    /// Creates a reconstruction record from resource identity and payload data.
    #[must_use]
    pub const fn new(
        bmc_id: BmcId,
        odata_id: ODataId,
        parent_odata_id: Option<ODataId>,
        payload: Option<P>,
    ) -> Self {
        Self {
            bmc_id,
            odata_id,
            parent_odata_id,
            payload,
        }
    }

    /// Creates a reconstruction record from a public resource event.
    #[must_use]
    pub fn from_resource_event(event: RedfishResourceEvent<P>) -> Self {
        let RedfishResourceEvent {
            bmc_id,
            odata_id,
            parent_odata_id,
            payload,
            ..
        } = event;

        Self::new(bmc_id, odata_id, parent_odata_id, payload)
    }

    /// Returns the source BMC id.
    #[must_use]
    pub const fn bmc_id(&self) -> &BmcId {
        &self.bmc_id
    }

    /// Returns the resource id.
    #[must_use]
    pub const fn odata_id(&self) -> &ODataId {
        &self.odata_id
    }

    /// Returns the parent resource id, when known.
    #[must_use]
    pub const fn parent_odata_id(&self) -> Option<&ODataId> {
        self.parent_odata_id.as_ref()
    }

    /// Returns the stored payload, when present.
    #[must_use]
    pub const fn payload(&self) -> Option<&P> {
        self.payload.as_ref()
    }
}

/// Builder for a service-root Redfish generator.
pub struct ServiceRootGeneratorBuilder<B: Bmc> {
    bmc_id: BmcId,
    service_root: ServiceRoot<B>,
}

impl<B: Bmc> ServiceRootGeneratorBuilder<B> {
    /// Creates a builder that closes over a typed service root wrapper.
    #[must_use]
    pub const fn new(bmc_id: BmcId, service_root: ServiceRoot<B>) -> Self {
        Self {
            bmc_id,
            service_root,
        }
    }

    /// Returns the source BMC id.
    #[must_use]
    pub const fn bmc_id(&self) -> &BmcId {
        &self.bmc_id
    }

    /// Returns the typed service root wrapper.
    #[must_use]
    pub const fn service_root(&self) -> &ServiceRoot<B> {
        &self.service_root
    }

    /// Builds a runtime generator over the captured service root.
    #[must_use]
    pub fn build(self) -> impl Generator<RedfishEvent, RedfishAdapterError> {
        ServiceRootGenerator::new(self.bmc_id, self.service_root)
    }
}

struct ServiceRootGenerator<B: Bmc> {
    bmc_id: BmcId,
    service_root: ServiceRoot<B>,
    dispatched: bool,
}

impl<B: Bmc> ServiceRootGenerator<B> {
    const fn new(bmc_id: BmcId, service_root: ServiceRoot<B>) -> Self {
        Self {
            bmc_id,
            service_root,
            dispatched: false,
        }
    }
}

impl<B: Bmc> Generator<RedfishEvent, RedfishAdapterError> for ServiceRootGenerator<B> {
    fn update_ready(&mut self, _now: Instant) -> Readiness {
        if self.dispatched {
            Readiness::not_ready(None)
        } else {
            Readiness::ready(CostUnits::new(1))
        }
    }

    fn take_next(&mut self) -> Option<ScheduledWork<RedfishEvent, RedfishAdapterError>> {
        if self.dispatched {
            return None;
        }

        self.dispatched = true;
        let _service_root = self.service_root.clone();
        let meta = WorkMeta::new(
            TargetId::new(self.bmc_id.as_str().to_owned()),
            GeneratorId::new("redfish.service-root"),
            ClassId::new("redfish.discovery"),
            CostUnits::new(1),
        );

        Some(ScheduledWork::new(meta, async {
            Err(RedfishAdapterError::NotImplemented)
        }))
    }

    fn on_complete(&mut self, _completion: &WorkCompletion) {}
}

/// Typed marker used by compile-time tests for object-bound builders.
pub struct TypedRedfishBuilder<B: Bmc, O> {
    bmc_id: BmcId,
    object: O,
    _bmc: PhantomData<B>,
}

impl<B: Bmc, O> TypedRedfishBuilder<B, O> {
    /// Creates a typed Redfish builder from an object that determines valid operations.
    #[must_use]
    pub const fn new(bmc_id: BmcId, object: O) -> Self {
        Self {
            bmc_id,
            object,
            _bmc: PhantomData,
        }
    }

    /// Returns the source BMC id.
    #[must_use]
    pub const fn bmc_id(&self) -> &BmcId {
        &self.bmc_id
    }

    /// Returns the typed object captured by the builder.
    #[must_use]
    pub const fn object(&self) -> &O {
        &self.object
    }
}
