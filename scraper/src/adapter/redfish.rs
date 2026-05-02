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
#[cfg(feature = "adapter-chassis")]
use core::iter::once;
#[cfg(any(
    feature = "adapter-chassis",
    feature = "adapter-sensors",
    feature = "adapter-computer-systems",
))]
use core::future::Future;
#[cfg(any(
    feature = "adapter-chassis",
    feature = "adapter-sensors",
    feature = "adapter-computer-systems",
))]
use core::pin::Pin;
#[cfg(all(feature = "adapter-service-root", feature = "adapter-chassis"))]
use std::collections::HashMap;
#[cfg(feature = "adapter-sensors")]
use std::collections::VecDeque;
use std::error::Error as StdError;
use std::sync::Arc;
#[cfg(any(feature = "adapter-service-root", feature = "adapter-sensors"))]
use std::sync::Mutex;
#[cfg(any(
    feature = "adapter-service-root",
    feature = "adapter-chassis",
    feature = "adapter-sensors",
    feature = "adapter-computer-systems",
))]
use std::time::Instant;

// `adapter-chassis` needs the trait by name (the `expanded_child_snapshot`
// helper has it as a generic bound). The other two adapter features only
// need it in scope so trait methods like `.odata_id()` resolve, hence
// `as _`. `adapter-sensors` implies `adapter-chassis`, so it picks up the
// named import transitively.
#[cfg(feature = "adapter-chassis")]
use nv_redfish::core::EntityTypeRef;
#[cfg(all(
    not(feature = "adapter-chassis"),
    any(feature = "adapter-sensors", feature = "adapter-computer-systems"),
))]
use nv_redfish::core::EntityTypeRef as _;
#[cfg(feature = "adapter-chassis")]
use nv_redfish::core::NavProperty;
use nv_redfish::core::ODataETag;
use nv_redfish::core::ODataId;
#[cfg(any(
    feature = "adapter-service-root",
    feature = "adapter-chassis",
    feature = "adapter-sensors",
    feature = "adapter-computer-systems",
))]
use nv_redfish::Bmc;
#[cfg(any(feature = "adapter-chassis", feature = "adapter-sensors"))]
use nv_redfish::chassis::Chassis;
#[cfg(feature = "adapter-computer-systems")]
use nv_redfish::computer_system::ComputerSystem;
#[cfg(feature = "adapter-sensors")]
use nv_redfish::sensor::SensorLink;
#[cfg(feature = "adapter-service-root")]
use nv_redfish::Resource as _;
#[cfg(feature = "adapter-service-root")]
use nv_redfish::ServiceRoot;

#[cfg(feature = "serde")]
use serde::Deserialize;
#[cfg(feature = "serde")]
use serde::Serialize;

#[cfg(any(
    feature = "adapter-service-root",
    feature = "adapter-chassis",
    feature = "adapter-sensors",
    feature = "adapter-computer-systems",
))]
use crate::generator::CostUnits;
#[cfg(any(
    feature = "adapter-service-root",
    feature = "adapter-chassis",
    feature = "adapter-sensors",
    feature = "adapter-computer-systems",
))]
use crate::generator::Generator;
#[cfg(any(
    feature = "adapter-service-root",
    feature = "adapter-chassis",
    feature = "adapter-sensors",
    feature = "adapter-computer-systems",
))]
use crate::generator::Readiness;
#[cfg(any(
    feature = "adapter-service-root",
    feature = "adapter-chassis",
    feature = "adapter-sensors",
    feature = "adapter-computer-systems",
))]
use crate::generator::ScheduledWork;
#[cfg(any(
    feature = "adapter-chassis",
    feature = "adapter-sensors",
    feature = "adapter-computer-systems",
))]
use crate::generator::ScheduledWorkResult;
#[cfg(any(
    feature = "adapter-service-root",
    feature = "adapter-chassis",
    feature = "adapter-sensors",
    feature = "adapter-computer-systems",
))]
use crate::generator::WorkCompletion;
#[cfg(any(
    feature = "adapter-chassis",
    feature = "adapter-sensors",
    feature = "adapter-computer-systems",
))]
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

// Gated on the adapter features that actually emit scrape events. The
// `adapter-service-root`-only build never reaches `scrape` (the only
// service-root caller, `fetch_chassis_collection`, is itself gated on
// `adapter-chassis`), so leaving service-root out of this list avoids a
// dead-code lint in that minimal build.
#[cfg(any(
    feature = "adapter-chassis",
    feature = "adapter-sensors",
    feature = "adapter-computer-systems",
))]
impl RedfishResourceEvent {
    /// Build a freshly-scraped resource event with payload and metadata
    /// populated consistently from the same `kind` / `odata_id` / `etag`.
    ///
    /// This is the in-crate builder used by every adapter generator. It
    /// removes the field-by-field struct literal noise where every call
    /// site was repeating the same `EntityPayload { kind, odata_id, etag } +
    /// ResourceMetadata { etag, .. }` pair. `change` defaults to
    /// [`ChangeKind::Inserted`] (overridden via [`Self::with_change`]) and
    /// `parent_odata_id` defaults to `None` (set via [`Self::with_parent`]).
    fn scrape(
        kind: &str,
        bmc_id: BmcId,
        odata_id: ODataId,
        etag: Option<ODataETag>,
    ) -> Self {
        Self {
            bmc_id,
            odata_id: odata_id.clone(),
            parent_odata_id: None,
            change: ChangeKind::Inserted,
            payload: Some(EntityPayload {
                kind: String::from(kind),
                odata_id,
                etag: etag.clone(),
            }),
            metadata: ResourceMetadata {
                etag,
                generation: None,
                fetch_latency_ms: None,
                error: None,
            },
        }
    }

    /// Set the parent `@odata.id` for this event.
    #[cfg(feature = "adapter-chassis")]
    #[must_use]
    fn with_parent(mut self, parent: ODataId) -> Self {
        self.parent_odata_id = Some(parent);
        self
    }

    /// Override the [`ChangeKind`] for this event (defaults to
    /// [`ChangeKind::Inserted`]). Used by the service-root generator to
    /// emit a re-observed collection's classified change kind.
    #[cfg(all(feature = "adapter-service-root", feature = "adapter-chassis"))]
    #[must_use]
    const fn with_change(mut self, change: ChangeKind) -> Self {
        self.change = change;
        self
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

/// Boxed work future returned by every adapter generator.
///
/// Centralised so the runtime sees a single concrete future shape regardless
/// of the underlying capability that produced it.
#[cfg(any(
    feature = "adapter-chassis",
    feature = "adapter-sensors",
    feature = "adapter-computer-systems",
))]
type RedfishWorkFuture = Pin<
    Box<dyn Future<Output = ScheduledWorkResult<RedfishEvent, RedfishAdapterError>> + Send + 'static>,
>;

/// Discoverable child collection kinds the service-root generator can scrape.
///
/// Phase 6 wires only `Chassis`. Later phases (Systems, EventService, ...)
/// extend this enum with additional kinds and matching fetch arms in
/// [`fetch_child`].
#[cfg(feature = "adapter-service-root")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ChildKind {
    /// `ChassisCollection` rooted at `service_root.root.chassis`.
    #[cfg(feature = "adapter-chassis")]
    Chassis,
}

/// Per-`@odata.id` change-detection state retained across scrapes.
///
/// Equality of two `ObservedState` snapshots from successive fetches is the
/// signal the scraper uses to assign [`ChangeKind`] to the emitted resource
/// event:
///
/// - `etag.is_some()` on both sides and equal -> [`ChangeKind::RefreshedNoChange`]
/// - `etag.is_none()` on both sides and `arc_addr` equal -> same: cached Arc
///   was returned by the underlying `Bmc`. Production BMC implementations
///   that cache by URL exhibit this behaviour.
/// - any other combination -> [`ChangeKind::Updated`]
#[cfg(all(feature = "adapter-service-root", feature = "adapter-chassis"))]
#[derive(Debug, Clone)]
struct ObservedState {
    etag: Option<ODataETag>,
    /// `Arc::as_ptr` of the previously observed payload, cast to `usize` so
    /// the value is `'static` and can be compared without rehydrating the
    /// previous schema type.
    arc_addr: Option<usize>,
}

#[cfg(all(feature = "adapter-service-root", feature = "adapter-chassis"))]
fn classify_change(
    prev: Option<&ObservedState>,
    new_etag: Option<&ODataETag>,
    new_arc_addr: usize,
) -> ChangeKind {
    let Some(prev) = prev else {
        return ChangeKind::Inserted;
    };
    if let (Some(p), Some(n)) = (prev.etag.as_ref(), new_etag) {
        return if p == n {
            ChangeKind::RefreshedNoChange
        } else {
            ChangeKind::Updated
        };
    }
    if prev.etag.is_none()
        && new_etag.is_none()
        && prev.arc_addr == Some(new_arc_addr)
    {
        return ChangeKind::RefreshedNoChange;
    }
    ChangeKind::Updated
}

/// Shared cache between the service-root generator and its in-flight work
/// future.
///
/// The generator owns this behind an `Arc<Mutex<...>>` and clones the handle
/// into each work future so the future can update the cache after a
/// successful fetch. The lock is only held synchronously between the fetch
/// `await` returning and the future yielding its result, never across an
/// `await` point.
#[cfg(feature = "adapter-service-root")]
#[derive(Default)]
struct ServiceRootShared {
    /// Per-`@odata.id` snapshot of the most-recently-fetched child collection.
    /// Empty in Phase 6 builds without `adapter-chassis` because no kind
    /// produces an entry until at least one child fetch lands.
    #[cfg(feature = "adapter-chassis")]
    seen: HashMap<ODataId, ObservedState>,
}

/// Service-root scrape generator.
///
/// On each `take_next` the generator advances a cursor over discoverable
/// child collection kinds (Phase 6: only `Chassis` when `adapter-chassis` is
/// enabled). The returned work future fetches that child via the typed
/// `nv-redfish` accessor (`service_root.chassis().await`) and emits at most
/// one [`RedfishResourceEvent`].
///
/// The cursor wraps after the last kind so subsequent scrape passes
/// re-observe the same children with [`ChangeKind::RefreshedNoChange`] (or
/// [`ChangeKind::Updated`]) according to the cached [`ObservedState`].
#[cfg(feature = "adapter-service-root")]
#[cfg_attr(not(feature = "adapter-chassis"), allow(dead_code))]
struct ServiceRootGenerator<B: Bmc> {
    bmc_id: BmcId,
    service_root: ServiceRoot<B>,
    parent_odata_id: ODataId,
    cursor: Vec<ChildKind>,
    cursor_idx: usize,
    pending: bool,
    shared: Arc<Mutex<ServiceRootShared>>,
}

#[cfg(feature = "adapter-service-root")]
impl<B> ServiceRootGenerator<B>
where
    B: Bmc + Send + Sync + 'static,
{
    #[cfg_attr(not(feature = "adapter-chassis"), allow(unused_mut))]
    fn new(bmc_id: BmcId, service_root: ServiceRoot<B>) -> Self {
        let parent_odata_id = service_root.odata_id().clone();
        let mut cursor: Vec<ChildKind> = Vec::new();
        #[cfg(feature = "adapter-chassis")]
        if service_root.root.chassis.is_some() {
            cursor.push(ChildKind::Chassis);
        }
        Self {
            bmc_id,
            service_root,
            parent_odata_id,
            cursor,
            cursor_idx: 0,
            pending: true,
            shared: Arc::new(Mutex::new(ServiceRootShared::default())),
        }
    }
}

#[cfg(feature = "adapter-service-root")]
impl<B> Generator<RedfishEvent, RedfishAdapterError> for ServiceRootGenerator<B>
where
    B: Bmc + Send + Sync + 'static,
{
    fn update_ready(&mut self, _now: Instant) -> Readiness {
        if self.pending && !self.cursor.is_empty() {
            Readiness::ready(Some(CostUnits::new(1)))
        } else {
            Readiness::not_ready(None)
        }
    }

    #[allow(clippy::needless_return)]
    fn take_next(&mut self) -> Option<ScheduledWork<RedfishEvent, RedfishAdapterError>> {
        if !self.pending || self.cursor.is_empty() {
            return None;
        }
        // Phase 6: only `ChildKind::Chassis` is wired. Builds without
        // `adapter-chassis` keep `cursor` empty, so the early-return above
        // already covers that branch and the body below is never compiled
        // into them.
        #[cfg(feature = "adapter-chassis")]
        {
            let kind = self.cursor[self.cursor_idx];
            self.pending = false;
            let bmc_id = self.bmc_id.clone();
            let parent_odata_id = self.parent_odata_id.clone();
            let service_root = self.service_root.clone();
            let shared = self.shared.clone();
            let future: RedfishWorkFuture =
                Box::pin(fetch_child(kind, bmc_id, parent_odata_id, service_root, shared));
            return Some(ScheduledWork::new(
                WorkMeta::with_cost(CostUnits::new(1)),
                future,
            ));
        }
        #[cfg(not(feature = "adapter-chassis"))]
        {
            None
        }
    }

    fn on_complete(&mut self, _completion: &WorkCompletion) {
        if !self.cursor.is_empty() {
            self.cursor_idx = (self.cursor_idx + 1) % self.cursor.len();
        }
        self.pending = true;
    }
}

/// Resolve the next discoverable child collection and emit one resource
/// event for it.
///
/// Returns `Ok(vec![])` when the BMC has no link for the requested kind
/// (the cursor still advances on completion). Transport-level failures are
/// mapped to [`RedfishAdapterError::Transport`].
#[cfg(all(feature = "adapter-service-root", feature = "adapter-chassis"))]
async fn fetch_child<B>(
    kind: ChildKind,
    bmc_id: BmcId,
    parent_odata_id: ODataId,
    service_root: ServiceRoot<B>,
    shared: Arc<Mutex<ServiceRootShared>>,
) -> ScheduledWorkResult<RedfishEvent, RedfishAdapterError>
where
    B: Bmc + Send + Sync + 'static,
{
    match kind {
        ChildKind::Chassis => {
            fetch_chassis_collection(bmc_id, parent_odata_id, &service_root, &shared).await
        }
    }
}

#[cfg(all(feature = "adapter-service-root", feature = "adapter-chassis"))]
async fn fetch_chassis_collection<B>(
    bmc_id: BmcId,
    parent_odata_id: ODataId,
    service_root: &ServiceRoot<B>,
    shared: &Arc<Mutex<ServiceRootShared>>,
) -> ScheduledWorkResult<RedfishEvent, RedfishAdapterError>
where
    B: Bmc + Send + Sync + 'static,
{
    let Some(coll) = service_root
        .chassis()
        .await
        .map_err(|err| RedfishAdapterError::Transport(format!("{err}")))?
    else {
        return Ok(Vec::new());
    };
    let raw = coll.raw();
    let odata_id = raw.odata_id().clone();
    let etag = raw.etag().cloned();
    let arc_addr = Arc::as_ptr(&raw).cast::<()>() as usize;
    let change = shared
        .lock()
        .map_err(|e| RedfishAdapterError::Transport(format!("scraper state poisoned: {e}")))
        .map(|mut guard| {
            let change = classify_change(guard.seen.get(&odata_id), etag.as_ref(), arc_addr);
            guard.seen.insert(
                odata_id.clone(),
                ObservedState {
                    etag: etag.clone(),
                    arc_addr: Some(arc_addr),
                },
            );
            change
        })?;
    let event = RedfishResourceEvent::scrape("ChassisCollection", bmc_id, odata_id, etag)
        .with_parent(parent_odata_id)
        .with_change(change);
    Ok(vec![RedfishEvent::Resource(event)])
}

/// Snapshot of one inlined chassis sub-resource emitted alongside the
/// parent chassis event when the BMC returned the navigation property in
/// `NavProperty::Expanded` form.
#[cfg(feature = "adapter-chassis")]
struct ChildSnapshot {
    kind: String,
    odata_id: ODataId,
    etag: Option<ODataETag>,
}

/// Project an inlined `NavProperty<T>` into a [`ChildSnapshot`], or `None`
/// if the BMC returned the property as a [`NavProperty::Reference`]
/// (children that arrive as references have no inlined payload to emit
/// alongside the parent in this work item).
#[cfg(feature = "adapter-chassis")]
fn expanded_child_snapshot<T>(kind: &str, nav: &NavProperty<T>) -> Option<ChildSnapshot>
where
    T: EntityTypeRef,
{
    matches!(nav, NavProperty::Expanded(_)).then(|| ChildSnapshot {
        kind: String::from(kind),
        odata_id: nav.odata_id().clone(),
        etag: nav.etag().cloned(),
    })
}

/// Chassis scrape generator.
///
/// Phase 7 walks the chassis schema's expandable navigation properties
/// (`Thermal`, `Power`, `Sensors`) and, for each one whose [`NavProperty`]
/// arrived in [`NavProperty::Expanded`] form, emits an additional child
/// [`RedfishResourceEvent`] alongside the parent [`Chassis`] event in a
/// single work item. The parent's `parent_odata_id` is `None` (its real
/// parent — the chassis collection — is owned by the service-root
/// generator); each child's `parent_odata_id` is the parent chassis
/// `@odata.id`.
///
/// Whether children are emitted depends entirely on what the BMC returned
/// at fetch time: when the service root advertises `$expand` support,
/// `nv-redfish` requests an expanded payload so the inlined sub-resources
/// are visible here. When `$expand` is unsupported, the same generator
/// degrades to the Phase-6 behaviour (single parent event, no children).
///
/// Per-chassis re-fetch (`Updated` / `RefreshedNoChange`) remains deferred
/// to Phase 8.
#[cfg(feature = "adapter-chassis")]
struct ChassisGenerator<B: Bmc> {
    bmc_id: BmcId,
    chassis: Chassis<B>,
    emitted: bool,
}

#[cfg(feature = "adapter-chassis")]
impl<B> Generator<RedfishEvent, RedfishAdapterError> for ChassisGenerator<B>
where
    B: Bmc + Send + Sync + 'static,
{
    fn update_ready(&mut self, _now: Instant) -> Readiness {
        if self.emitted {
            Readiness::not_ready(None)
        } else {
            Readiness::ready(Some(CostUnits::new(1)))
        }
    }

    fn take_next(&mut self) -> Option<ScheduledWork<RedfishEvent, RedfishAdapterError>> {
        if self.emitted {
            return None;
        }
        self.emitted = true;
        let bmc_id = self.bmc_id.clone();
        let raw = self.chassis.raw();
        let chassis_id = raw.odata_id().clone();
        let chassis_etag = raw.etag().cloned();

        // Snapshot the inlined sub-resources eagerly so the work future is
        // self-contained and does not borrow back into `Chassis<B>`. Only
        // navigation properties that arrived in `NavProperty::Expanded` form
        // contribute a child event; references are skipped. Each entry is
        // resolved by `expanded_child_snapshot` against its concrete
        // `NavProperty<T>` type. The three Options are chained — using an
        // array would force `.iter()` semantics under edition-2018 and
        // borrow the snapshots instead of moving them.
        let children: Vec<ChildSnapshot> = raw
            .thermal
            .as_ref()
            .and_then(|nav| expanded_child_snapshot("Thermal", nav))
            .into_iter()
            .chain(
                raw.power
                    .as_ref()
                    .and_then(|nav| expanded_child_snapshot("Power", nav)),
            )
            .chain(
                raw.sensors
                    .as_ref()
                    .and_then(|nav| expanded_child_snapshot("SensorCollection", nav)),
            )
            .collect();

        let future: RedfishWorkFuture = Box::pin(async move {
            let parent = RedfishResourceEvent::scrape(
                "Chassis",
                bmc_id.clone(),
                chassis_id.clone(),
                chassis_etag,
            );
            let children = children.into_iter().map(|child| {
                RedfishResourceEvent::scrape(&child.kind, bmc_id.clone(), child.odata_id, child.etag)
                    .with_parent(chassis_id.clone())
            });
            Ok(once(parent)
                .chain(children)
                .map(RedfishEvent::Resource)
                .collect())
        });
        Some(ScheduledWork::new(
            WorkMeta::with_cost(CostUnits::new(1)),
            future,
        ))
    }

    fn on_complete(&mut self, _completion: &WorkCompletion) {}
}

/// Sensors scrape generator.
///
/// On its first `take_next` the generator yields a discovery work future
/// that walks the chassis's sensor sub-tree via `Chassis::sensor_links`.
/// The future emits one [`RedfishResourceEvent`] for the first sensor link
/// returned by the BMC and stores any remaining links into a shared queue.
/// Each subsequent `take_next` pops one queued link and yields a
/// trivially-resolved work future that emits a single sensor event. After
/// the queue drains the generator becomes idle.
///
/// Each emitted sensor event sets `parent_odata_id` to the chassis
/// `@odata.id` so downstream consumers can reconstruct the chassis tree
/// from the event stream alone.
#[cfg(feature = "adapter-sensors")]
struct SensorsGenerator<B: Bmc> {
    bmc_id: BmcId,
    parent_odata_id: ODataId,
    chassis: Arc<Chassis<B>>,
    in_flight: bool,
    discovery_complete: bool,
    shared: Arc<Mutex<SensorsShared<B>>>,
}

/// Per-`SensorsGenerator` queue shared with its in-flight work futures.
///
/// The discovery future fills the queue with the tail of the sensor links
/// returned by `Chassis::sensor_links`; each drain `take_next` pops one
/// link synchronously before constructing its work future. The mutex is
/// only ever held synchronously and never across an `await` point.
#[cfg(feature = "adapter-sensors")]
struct SensorsShared<B: Bmc> {
    queue: VecDeque<SensorLink<B>>,
}

#[cfg(feature = "adapter-sensors")]
impl<B> SensorsGenerator<B>
where
    B: Bmc + Send + Sync + 'static,
{
    fn new(bmc_id: BmcId, chassis: Chassis<B>) -> Self {
        let parent_odata_id = chassis.raw().odata_id().clone();
        Self {
            bmc_id,
            parent_odata_id,
            chassis: Arc::new(chassis),
            in_flight: false,
            discovery_complete: false,
            shared: Arc::new(Mutex::new(SensorsShared {
                queue: VecDeque::new(),
            })),
        }
    }
}

#[cfg(feature = "adapter-sensors")]
fn sensor_event<B>(link: &SensorLink<B>, bmc_id: BmcId, parent: ODataId) -> RedfishEvent
where
    B: Bmc,
{
    RedfishEvent::Resource(
        RedfishResourceEvent::scrape("Sensor", bmc_id, link.odata_id().clone(), None)
            .with_parent(parent),
    )
}

#[cfg(feature = "adapter-sensors")]
impl<B> Generator<RedfishEvent, RedfishAdapterError> for SensorsGenerator<B>
where
    B: Bmc + Send + Sync + 'static,
{
    fn update_ready(&mut self, _now: Instant) -> Readiness {
        if self.in_flight {
            return Readiness::not_ready(None);
        }
        if !self.discovery_complete {
            return Readiness::ready(Some(CostUnits::new(1)));
        }
        let queue_empty = match self.shared.lock() {
            Ok(g) => g.queue.is_empty(),
            Err(_) => return Readiness::not_ready(None),
        };
        if queue_empty {
            Readiness::not_ready(None)
        } else {
            Readiness::ready(Some(CostUnits::new(1)))
        }
    }

    fn take_next(&mut self) -> Option<ScheduledWork<RedfishEvent, RedfishAdapterError>> {
        if self.in_flight {
            return None;
        }
        if !self.discovery_complete {
            self.in_flight = true;
            self.discovery_complete = true;
            let bmc_id = self.bmc_id.clone();
            let parent = self.parent_odata_id.clone();
            let chassis = self.chassis.clone();
            let shared = self.shared.clone();
            let future: RedfishWorkFuture = Box::pin(async move {
                let Some(links) = chassis
                    .sensor_links()
                    .await
                    .map_err(|err| RedfishAdapterError::Transport(format!("{err}")))?
                else {
                    return Ok(Vec::new());
                };
                let mut iter = links.into_iter();
                let first = iter.next();
                shared
                    .lock()
                    .map_err(|e| {
                        RedfishAdapterError::Transport(format!("scraper state poisoned: {e}"))
                    })?
                    .queue
                    .extend(iter);
                Ok(first
                    .map(|link| sensor_event(&link, bmc_id, parent))
                    .into_iter()
                    .collect())
            });
            return Some(ScheduledWork::new(
                WorkMeta::with_cost(CostUnits::new(1)),
                future,
            ));
        }
        let link = {
            let mut guard = self.shared.lock().ok()?;
            guard.queue.pop_front()
        };
        let link = link?;
        self.in_flight = true;
        let bmc_id = self.bmc_id.clone();
        let parent = self.parent_odata_id.clone();
        let future: RedfishWorkFuture = Box::pin(async move {
            Ok(vec![sensor_event(&link, bmc_id, parent)])
        });
        Some(ScheduledWork::new(
            WorkMeta::with_cost(CostUnits::new(1)),
            future,
        ))
    }

    fn on_complete(&mut self, _completion: &WorkCompletion) {
        self.in_flight = false;
    }
}

/// Computer-system scrape generator.
///
/// Phase 7 mirrors the Phase-6 [`ChassisGenerator`] semantics: emit a
/// single [`ChangeKind::Inserted`] event derived from the supplied
/// `ComputerSystem<B>` payload, then become idle. Per-system re-fetch is
/// deferred alongside the analogous chassis re-fetch in Phase 8.
#[cfg(feature = "adapter-computer-systems")]
struct ComputerSystemGenerator<B: Bmc> {
    bmc_id: BmcId,
    system: ComputerSystem<B>,
    emitted: bool,
}

#[cfg(feature = "adapter-computer-systems")]
impl<B> Generator<RedfishEvent, RedfishAdapterError> for ComputerSystemGenerator<B>
where
    B: Bmc + Send + Sync + 'static,
{
    fn update_ready(&mut self, _now: Instant) -> Readiness {
        if self.emitted {
            Readiness::not_ready(None)
        } else {
            Readiness::ready(Some(CostUnits::new(1)))
        }
    }

    fn take_next(&mut self) -> Option<ScheduledWork<RedfishEvent, RedfishAdapterError>> {
        if self.emitted {
            return None;
        }
        self.emitted = true;
        let bmc_id = self.bmc_id.clone();
        let raw = self.system.raw();
        let odata_id = raw.odata_id().clone();
        let etag = raw.etag().cloned();
        let future: RedfishWorkFuture = Box::pin(async move {
            Ok(vec![RedfishEvent::Resource(
                RedfishResourceEvent::scrape("ComputerSystem", bmc_id, odata_id, etag),
            )])
        });
        Some(ScheduledWork::new(
            WorkMeta::with_cost(CostUnits::new(1)),
            future,
        ))
    }

    fn on_complete(&mut self, _completion: &WorkCompletion) {}
}

/// Builder for the service-root scrape generator.
///
/// The returned generator advances a cursor over child collections that the
/// supplied `ServiceRoot<B>` advertises (Phase 6 wires only Chassis under
/// `adapter-chassis`). Each `take_next` fetches one child via the typed
/// `nv-redfish` accessor and emits at most one [`RedfishResourceEvent`].
///
/// The first observation of an `@odata.id` produces [`ChangeKind::Inserted`].
/// Subsequent observations classify by ETag equality, falling back to
/// `Arc::ptr_eq` of the cached schema when both ETags are absent, to
/// distinguish [`ChangeKind::Updated`] from
/// [`ChangeKind::RefreshedNoChange`].
///
/// # Errors
///
/// The returned generator's work futures resolve to
/// [`RedfishAdapterError::Transport`] when the underlying `Bmc` returns an
/// error while fetching a discoverable child collection. Capabilities that
/// are present in the API but not yet wired in this build still resolve to
/// [`RedfishAdapterError::NotImplemented`].
#[cfg(feature = "adapter-service-root")]
#[must_use]
pub fn build_service_root_generator<B>(
    bmc_id: BmcId,
    service_root: ServiceRoot<B>,
) -> Box<dyn Generator<RedfishEvent, RedfishAdapterError> + Send>
where
    B: Bmc + Send + Sync + 'static,
{
    Box::new(ServiceRootGenerator::new(bmc_id, service_root))
}

/// Builder for the chassis-item scrape generator.
///
/// The returned generator emits a single [`ChangeKind::Inserted`] event
/// for the supplied [`Chassis<B>`]. When the chassis schema arrived with
/// expanded sub-resources (`Thermal`, `Power`, `Sensors`) — typically
/// because the BMC advertises `$expand` support and `nv-redfish` requested
/// expansion — the generator emits one additional child event per
/// inlined navigation property in the same work item, with each child's
/// `parent_odata_id` set to the chassis `@odata.id`. After the single
/// emission the generator becomes idle; per-chassis re-fetch is deferred
/// to Phase 8.
///
/// # Errors
///
/// The returned generator's work futures only emit successful resource
/// events. Transport / parse failures during initial chassis construction
/// surface through `nv-redfish::ServiceRoot` / `ChassisCollection::members`
/// before this builder is reached.
#[cfg(feature = "adapter-chassis")]
#[must_use]
pub fn build_chassis_generator<B>(
    bmc_id: BmcId,
    chassis: Chassis<B>,
) -> Box<dyn Generator<RedfishEvent, RedfishAdapterError> + Send>
where
    B: Bmc + Send + Sync + 'static,
{
    Box::new(ChassisGenerator {
        bmc_id,
        chassis,
        emitted: false,
    })
}

/// Builder for the sensor scrape generator.
///
/// The returned generator walks the chassis's sensor sub-tree via
/// `Chassis::sensor_links` on its first scrape pass and emits one
/// [`RedfishResourceEvent`] per sensor link, one work item at a time.
/// Each event's `parent_odata_id` is the chassis `@odata.id`. After all
/// sensors have been emitted the generator becomes idle.
///
/// # Errors
///
/// The first work future resolves to
/// [`RedfishAdapterError::Transport`] if the underlying `Bmc` fails when
/// resolving the sensor links. Subsequent drain futures only emit
/// pre-discovered links and do not perform additional I/O, so they
/// always succeed.
#[cfg(feature = "adapter-sensors")]
#[must_use]
pub fn build_sensors_generator<B>(
    bmc_id: BmcId,
    chassis: Chassis<B>,
) -> Box<dyn Generator<RedfishEvent, RedfishAdapterError> + Send>
where
    B: Bmc + Send + Sync + 'static,
{
    Box::new(SensorsGenerator::new(bmc_id, chassis))
}

/// Builder for the computer-systems scrape generator.
///
/// The returned generator emits a single [`ChangeKind::Inserted`] event
/// for the supplied [`ComputerSystem<B>`], mirroring the Phase-6
/// chassis-item generator. After the emission it becomes idle;
/// per-system re-fetch is deferred to Phase 8.
///
/// # Errors
///
/// The returned generator's single work future emits a successful
/// resource event; any transport / parse failure originates earlier
/// during `SystemCollection::members` and is surfaced before this
/// builder is reached.
#[cfg(feature = "adapter-computer-systems")]
#[must_use]
pub fn build_computer_system_generator<B>(
    bmc_id: BmcId,
    system: ComputerSystem<B>,
) -> Box<dyn Generator<RedfishEvent, RedfishAdapterError> + Send>
where
    B: Bmc + Send + Sync + 'static,
{
    Box::new(ComputerSystemGenerator {
        bmc_id,
        system,
        emitted: false,
    })
}
