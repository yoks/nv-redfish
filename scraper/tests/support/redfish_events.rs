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

//! Shared constructors for adapter-side Redfish events used by integration
//! tests.
//!
//! These helpers are deliberately lightweight: they let tests express
//! "what" they want to assert about identity, parent linkage, and change
//! kind without re-typing every field of [`RedfishResourceEvent`].

#![cfg(feature = "redfish-adapter")]

use std::time::Instant;

use nv_redfish::core::ODataId;

use nv_redfish_scraper::adapter::redfish::BmcId;
use nv_redfish_scraper::adapter::redfish::ChangeKind;
use nv_redfish_scraper::adapter::redfish::EntityPayload;
use nv_redfish_scraper::adapter::redfish::RedfishAdapterError;
use nv_redfish_scraper::adapter::redfish::RedfishEvent;
use nv_redfish_scraper::adapter::redfish::RedfishResourceEvent;
use nv_redfish_scraper::adapter::redfish::ResourceMetadata;
use nv_redfish_scraper::Generator;
use nv_redfish_scraper::Readiness;
use nv_redfish_scraper::ScheduledWork;
use nv_redfish_scraper::WorkCompletion;

/// Build an [`ODataId`] from any string-like input. Used by tests to keep
/// `@odata.id` literals compact.
pub fn ode<S: Into<String>>(s: S) -> ODataId {
    ODataId::from(s.into())
}

/// Build an [`EntityPayload`] from an explicit kind and `@odata.id`.
///
/// This is intentionally separate from `support::fake_payload::payload`,
/// which builds a synthetic `@odata.id` from a numeric sequence; tests that
/// need real-looking BMC paths use `payload_at` instead.
pub fn payload_at(kind: &str, odata: &str) -> EntityPayload {
    EntityPayload {
        kind: String::from(kind),
        odata_id: ode(odata),
        etag: None,
    }
}

/// Chainable builder for [`RedfishResourceEvent`] used by integration
/// tests.
///
/// Tests build events as
///
/// ```ignore
/// ResourceEvent::at("bmc-1", "/redfish/v1/Chassis/A")
///     .parent("/redfish/v1/Chassis")
///     .change(ChangeKind::Inserted)
///     .payload_kind("Chassis")
///     .build();
/// ```
///
/// The defaults match the most common shape: no parent, [`ChangeKind::Inserted`],
/// no payload, [`ResourceMetadata::default`]. Each `.with_*` method returns
/// `Self` so calls chain in expression position; [`Self::build`] consumes
/// the builder and emits the [`RedfishResourceEvent`].
pub struct ResourceEvent {
    bmc: BmcId,
    odata: ODataId,
    parent: Option<ODataId>,
    change: ChangeKind,
    payload: Option<EntityPayload>,
    metadata: ResourceMetadata,
}

impl ResourceEvent {
    /// Start a builder with the given BMC id and `@odata.id` literal.
    pub fn at(bmc: &str, odata: &str) -> Self {
        Self {
            bmc: BmcId::new(bmc),
            odata: ode(odata),
            parent: None,
            change: ChangeKind::Inserted,
            payload: None,
            metadata: ResourceMetadata::default(),
        }
    }

    /// Set the parent `@odata.id` from a string literal.
    #[must_use]
    pub fn parent(mut self, parent: &str) -> Self {
        self.parent = Some(ode(parent));
        self
    }

    /// Override the [`ChangeKind`] (defaults to [`ChangeKind::Inserted`]).
    #[must_use]
    pub const fn change(mut self, change: ChangeKind) -> Self {
        self.change = change;
        self
    }

    /// Attach an explicit payload to the event.
    #[must_use]
    pub fn payload(mut self, payload: EntityPayload) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Attach a payload whose `@odata.id` matches the event's own
    /// `@odata.id` and whose kind is the supplied entity-kind label.
    #[must_use]
    pub fn payload_kind(mut self, kind: &str) -> Self {
        self.payload = Some(EntityPayload {
            kind: String::from(kind),
            odata_id: self.odata.clone(),
            etag: None,
        });
        self
    }

    /// Mark the event as a fetch failure: clears the payload and sets
    /// `change` to [`ChangeKind::FetchFailed`].
    #[must_use]
    pub fn fetch_failed(mut self) -> Self {
        self.payload = None;
        self.change = ChangeKind::FetchFailed;
        self
    }

    /// Consume the builder and produce the [`RedfishResourceEvent`].
    #[must_use]
    pub fn build(self) -> RedfishResourceEvent {
        RedfishResourceEvent {
            bmc_id: self.bmc,
            odata_id: self.odata,
            parent_odata_id: self.parent,
            change: self.change,
            payload: self.payload,
            metadata: self.metadata,
        }
    }
}

/// Always-idle generator over [`RedfishEvent`] / [`RedfishAdapterError`].
///
/// Used by replay/reconstruction tests that need to attach a generator to a
/// runtime target without producing any work.
#[derive(Default)]
pub struct NoopRedfishGenerator;

impl Generator<RedfishEvent, RedfishAdapterError> for NoopRedfishGenerator {
    fn update_ready(&mut self, _now: Instant) -> Readiness {
        Readiness::not_ready(None)
    }

    fn take_next(&mut self) -> Option<ScheduledWork<RedfishEvent, RedfishAdapterError>> {
        None
    }

    fn on_complete(&mut self, _completion: &WorkCompletion) {}
}
