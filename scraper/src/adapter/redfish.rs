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

//! Redfish adapter boundary.
//!
//! Phase 0 establishes the public types and per-capability builder
//! signatures. Builder bodies return generators whose work futures resolve to
//! [`RedfishAdapterError::NotImplemented`]; later phases replace the bodies
//! with real fetch logic without changing the public API.
//!
//! All public Redfish events expose only read-side data and metadata. They
//! never carry execution handles such as `Bmc` clients, `ServiceRoot<B>`,
//! `Chassis<B>`, or `ComputerSystem<B>`.

use core::fmt::Debug;
use core::fmt::Display;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;
use core::future::Future;
use core::pin::Pin;
use std::error::Error as StdError;
use std::sync::Arc;
use std::time::Instant;

use nv_redfish::core::ODataETag;
use nv_redfish::core::ODataId;
#[cfg(any(
    feature = "adapter-service-root",
    feature = "adapter-chassis",
    feature = "adapter-sensors",
    feature = "adapter-computer-systems",
))]
use nv_redfish::Bmc;
#[cfg(feature = "adapter-chassis")]
use nv_redfish::chassis::Chassis;
#[cfg(feature = "adapter-computer-systems")]
use nv_redfish::computer_system::ComputerSystem;
#[cfg(feature = "adapter-service-root")]
use nv_redfish::ServiceRoot;

#[cfg(feature = "serde")]
use serde::Deserialize;
#[cfg(feature = "serde")]
use serde::Serialize;

use crate::generator::CostUnits;
use crate::generator::Generator;
use crate::generator::Readiness;
use crate::generator::ScheduledWork;
use crate::generator::ScheduledWorkResult;
use crate::generator::WorkCompletion;
use crate::generator::WorkMeta;

/// Opaque BMC identifier supplied by the application.
///
/// `BmcId` is an application-level identifier used to tag resource events
/// with their source BMC. It is opaque to the runtime.
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BmcId {
    inner: Arc<str>,
}

impl BmcId {
    /// Construct a [`BmcId`] from any string-like value.
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self { inner: name.into() }
    }

    /// Borrow the BMC identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.inner
    }
}

impl Debug for BmcId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "BmcId({:?})", &*self.inner)
    }
}

impl Display for BmcId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "bmc:{}", &*self.inner)
    }
}

/// Distinguishes the kinds of resource changes carried by a Redfish resource
/// event.
///
/// Phase 0 reserves the four required variants from the requirements
/// document. Later phases may extend with `Stale` and `Removed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum ChangeKind {
    /// First time the resource was observed by the scraper.
    Inserted,
    /// Resource changed since last observation.
    Updated,
    /// Resource was refreshed but did not change.
    RefreshedNoChange,
    /// Fetch failed for the resource.
    FetchFailed,
    /// Resource is known stale and a re-fetch is pending.
    Stale,
    /// Resource is known removed.
    Removed,
}

/// Scrape metadata attached to a Redfish resource event.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ResourceMetadata {
    /// Optional `@odata.etag` of the resource at scrape time.
    pub etag: Option<ODataETag>,
    /// Optional generation counter as understood by the application.
    pub generation: Option<u64>,
    /// Optional fetch latency in milliseconds.
    pub fetch_latency_ms: Option<u64>,
    /// Optional human-readable error message attached to a failed fetch.
    pub error: Option<String>,
}

/// Generated `EntityPayload` boundary used by adapter resource events.
///
/// Phase 0 represents this as an opaque struct that preserves the
/// schema-payload identity (entity kind, `@odata.id`, optional `@odata.etag`).
/// Once the CSDL compiler exposes a generated `EntityPayload` enum, this type
/// becomes a thin wrapper or alias around it without changing the adapter
/// public API.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EntityPayload {
    /// Application-supplied entity kind name (for example, "Chassis").
    pub kind: String,
    /// `@odata.id` of the entity carried by this payload.
    pub odata_id: ODataId,
    /// Optional `@odata.etag` of the entity carried by this payload.
    pub etag: Option<ODataETag>,
}

/// Resource-level Redfish event carried by the runtime work-event stream.
///
/// Carries only read-side data. Execution handles such as
/// `Bmc`/`ServiceRoot<B>`/`Chassis<B>` are intentionally absent.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RedfishResourceEvent {
    /// Source BMC of the event.
    pub bmc_id: BmcId,
    /// `@odata.id` of the resource.
    pub odata_id: ODataId,
    /// Optional parent `@odata.id` if the parent is known.
    pub parent_odata_id: Option<ODataId>,
    /// Kind of change captured by this event.
    pub change: ChangeKind,
    /// Optional preserved schema payload, including any expanded data.
    pub payload: Option<EntityPayload>,
    /// Scrape metadata.
    pub metadata: ResourceMetadata,
}

/// Generator-lifecycle event reported by adapter generators.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum GeneratorEvent {
    /// Adapter generator was started for a target.
    Started {
        /// Source BMC of the generator.
        bmc_id: BmcId,
        /// Human-readable name of the generator (for example, `service-root`).
        kind: String,
    },
    /// Adapter generator was stopped for a target.
    Stopped {
        /// Source BMC of the generator.
        bmc_id: BmcId,
        /// Human-readable name of the generator.
        kind: String,
    },
}

/// Scrape-lifecycle event reported by adapter generators.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum ScrapeEvent {
    /// A scrape pass completed.
    Completed {
        /// Source BMC of the scrape.
        bmc_id: BmcId,
        /// Number of resources observed during the pass.
        resources: u64,
    },
    /// A scrape pass failed.
    Failed {
        /// Source BMC of the scrape.
        bmc_id: BmcId,
        /// Human-readable error message.
        error: String,
    },
}

/// Top-level Redfish work event delivered as `Ev` to the runtime.
///
/// Public Redfish events do not carry `B`, `ServiceRoot<B>`, `Chassis<B>`,
/// `ComputerSystem<B>`, or other execution handles.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum RedfishEvent {
    /// Resource-level event.
    Resource(RedfishResourceEvent),
    /// Generator-lifecycle event.
    Generator(GeneratorEvent),
    /// Scrape-lifecycle event.
    Scrape(ScrapeEvent),
}

/// Reconstruction record persisted alongside the event stream.
///
/// Reconstruction is optional: applications that want to restore scraper
/// state without full rediscovery may persist these records and rebuild the
/// scheduler tree from them. Records preserve identity but not execution
/// handles.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ReconstructionRecord {
    /// Source BMC.
    pub bmc_id: BmcId,
    /// `@odata.id` of the resource.
    pub odata_id: ODataId,
    /// Optional parent `@odata.id`.
    pub parent_odata_id: Option<ODataId>,
    /// Optional preserved schema payload.
    pub payload: Option<EntityPayload>,
}

impl ReconstructionRecord {
    /// Build a reconstruction record from a [`RedfishResourceEvent`].
    #[must_use]
    pub fn from_resource_event(event: &RedfishResourceEvent) -> Self {
        Self {
            bmc_id: event.bmc_id.clone(),
            odata_id: event.odata_id.clone(),
            parent_odata_id: event.parent_odata_id.clone(),
            payload: event.payload.clone(),
        }
    }
}

/// Adapter-level error type.
///
/// The runtime wraps `Err = RedfishAdapterError` into [`crate::WorkError`].
/// Phase 0 only exposes [`RedfishAdapterError::NotImplemented`]; later phases
/// add transport, parsing, and validation variants without breaking the
/// public API.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum RedfishAdapterError {
    /// The adapter capability is wired but not yet implemented in this build.
    ///
    /// Phase 0 builders return this variant from their work futures so the
    /// API contract is exercised while the fetch implementation is deferred.
    NotImplemented,
    /// Transport-layer failure surfaced by the underlying [`nv_redfish::Bmc`].
    ///
    /// Phase 6 introduces this variant. The wrapped string is the
    /// `Display` rendering of the underlying error so it remains
    /// `Send + Sync + 'static` and stays serde-friendly.
    Transport(String),
    /// Schema parse / decode failure surfaced when a `Bmc` response cannot be
    /// decoded into the expected typed entity.
    ///
    /// Phase 6 declares this variant per the Phase 6 design; current adapter
    /// fetch paths route schema-decode errors that bubble through `Bmc::Error`
    /// into [`Self::Transport`]. Future phases that introduce explicit parse
    /// shims (for example, expand fan-out) will use this variant directly.
    Parse(String),
}

impl Display for RedfishAdapterError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::NotImplemented => f.write_str("redfish adapter capability not yet implemented"),
            Self::Transport(msg) => write!(f, "redfish transport error: {msg}"),
            Self::Parse(msg) => write!(f, "redfish parse error: {msg}"),
        }
    }
}

impl StdError for RedfishAdapterError {}

/// Phase 0 stub generator that produces a single not-implemented work item per
/// scheduling round. Used by every per-capability builder until later phases
/// land real fetch bodies.
struct NotImplementedGenerator {
    bmc_id: BmcId,
    kind: &'static str,
    pending: bool,
}

type StubFuture = Pin<
    Box<dyn Future<Output = ScheduledWorkResult<RedfishEvent, RedfishAdapterError>> + Send + 'static>,
>;

impl Generator<RedfishEvent, RedfishAdapterError> for NotImplementedGenerator {
    fn update_ready(&mut self, _now: Instant) -> Readiness {
        Readiness::ready(Some(CostUnits::ZERO))
    }

    fn take_next(&mut self) -> Option<ScheduledWork<RedfishEvent, RedfishAdapterError>> {
        if !self.pending {
            return None;
        }
        self.pending = false;
        let bmc_id = self.bmc_id.clone();
        let kind = self.kind;
        let future: StubFuture = Box::pin(async move {
            let _ = bmc_id;
            let _ = kind;
            Err::<Vec<RedfishEvent>, RedfishAdapterError>(RedfishAdapterError::NotImplemented)
        });
        Some(ScheduledWork::new(
            WorkMeta::with_cost(CostUnits::ZERO),
            future,
        ))
    }

    fn on_complete(&mut self, _completion: &WorkCompletion) {
        self.pending = true;
    }
}

/// Builder for the service-root scrape generator.
///
/// In Phase 0 the builder accepts a typed `nv-redfish` `ServiceRoot<B>` only
/// to demonstrate the typed-binding contract; the value is currently dropped
/// and the returned generator emits [`RedfishAdapterError::NotImplemented`].
///
/// # Errors
///
/// The returned generator's work futures resolve to
/// [`RedfishAdapterError::NotImplemented`] until later phases implement the
/// fetch.
#[cfg(feature = "adapter-service-root")]
#[must_use]
pub fn build_service_root_generator<B>(
    bmc_id: BmcId,
    _service_root: ServiceRoot<B>,
) -> Box<dyn Generator<RedfishEvent, RedfishAdapterError> + Send>
where
    B: Bmc + Send + Sync + 'static,
{
    Box::new(NotImplementedGenerator {
        bmc_id,
        kind: "service-root",
        pending: true,
    })
}

/// Builder for the chassis-collection scrape generator.
///
/// # Errors
///
/// The returned generator's work futures resolve to
/// [`RedfishAdapterError::NotImplemented`] until later phases implement the
/// fetch.
#[cfg(feature = "adapter-chassis")]
#[must_use]
pub fn build_chassis_generator<B>(
    bmc_id: BmcId,
    _chassis: Chassis<B>,
) -> Box<dyn Generator<RedfishEvent, RedfishAdapterError> + Send>
where
    B: Bmc + Send + Sync + 'static,
{
    Box::new(NotImplementedGenerator {
        bmc_id,
        kind: "chassis",
        pending: true,
    })
}

/// Builder for the sensor scrape generator.
///
/// # Errors
///
/// The returned generator's work futures resolve to
/// [`RedfishAdapterError::NotImplemented`] until later phases implement the
/// fetch.
#[cfg(feature = "adapter-sensors")]
#[must_use]
pub fn build_sensors_generator<B>(
    bmc_id: BmcId,
    _chassis: Chassis<B>,
) -> Box<dyn Generator<RedfishEvent, RedfishAdapterError> + Send>
where
    B: Bmc + Send + Sync + 'static,
{
    Box::new(NotImplementedGenerator {
        bmc_id,
        kind: "sensors",
        pending: true,
    })
}

/// Builder for the computer-systems scrape generator.
///
/// # Errors
///
/// The returned generator's work futures resolve to
/// [`RedfishAdapterError::NotImplemented`] until later phases implement the
/// fetch.
#[cfg(feature = "adapter-computer-systems")]
#[must_use]
pub fn build_computer_system_generator<B>(
    bmc_id: BmcId,
    _system: ComputerSystem<B>,
) -> Box<dyn Generator<RedfishEvent, RedfishAdapterError> + Send>
where
    B: Bmc + Send + Sync + 'static,
{
    Box::new(NotImplementedGenerator {
        bmc_id,
        kind: "computer-system",
        pending: true,
    })
}
