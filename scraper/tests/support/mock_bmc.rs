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

//! Test-only Redfish BMC fixture used by Phase 6 end-to-end adapter tests.
//!
//! The fixture wraps the workspace-internal [`nv_redfish_bmc_mock::Bmc`] and
//! supplies a small ergonomic surface (`expect_get_ok`, `expect_get_err`,
//! `service_root_json_with_chassis`, `chassis_collection_json`) that the
//! adapter tests use to drive [`nv_redfish::ServiceRoot`] construction and
//! generator scrape passes deterministically and synchronously.
//!
//! Gated on `feature = "redfish-adapter"` because the entire fixture only
//! makes sense once the adapter module is compiled.

use std::error::Error as StdError;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::sync::Arc;

use nv_redfish_bmc_mock::Bmc as BmcMock;
use nv_redfish_bmc_mock::Expect as ExpectMock;
use nv_redfish_bmc_mock::ExpectedRequest;

/// Synthetic transport-level error raised by [`MockBmc`] when a queued
/// expectation declares a failed response.
#[derive(Debug, Clone)]
pub enum MockTransportError {
    /// Synthetic transport failure with a free-form message used to
    /// fingerprint the path that produced it.
    Synthetic(String),
}

impl Default for MockTransportError {
    fn default() -> Self {
        Self::Synthetic(String::new())
    }
}

impl Display for MockTransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Synthetic(msg) => write!(f, "synthetic transport: {msg}"),
        }
    }
}

impl StdError for MockTransportError {}

/// `nv-redfish` `Bmc` mock specialised for the scraper tests.
pub type MockBmc = BmcMock<MockTransportError>;

/// Single queued expectation against [`MockBmc`].
pub type MockExpect = ExpectMock<MockTransportError>;

/// Build a fresh empty [`MockBmc`] wrapped in `Arc` so it can be passed to
/// [`nv_redfish::ServiceRoot::new`] directly.
#[must_use]
pub fn mock_bmc() -> Arc<MockBmc> {
    Arc::new(MockBmc::default())
}

/// Build a `MockExpect` that successfully answers a `Bmc::get` for `uri`
/// with `body`.
pub fn expect_get_ok(uri: &str, body: serde_json::Value) -> MockExpect {
    MockExpect::get(uri, body.to_string())
}

/// Build a `MockExpect` that fails a `Bmc::get` for `uri` with a
/// [`MockTransportError::Synthetic`] error tagged with `msg`.
pub fn expect_get_err(uri: &str, msg: &str) -> MockExpect {
    MockExpect {
        request: ExpectedRequest::Get {
            id: uri.to_string().into(),
        },
        response: Err(MockTransportError::Synthetic(msg.to_string())),
    }
}

/// Minimal `ServiceRoot` JSON payload that advertises a chassis collection.
///
/// `root` is the `@odata.id` of the service root (typically `"/redfish/v1/"`).
/// The chassis link is generated as `<root_no_trailing_slash>/Chassis`.
/// `ProtocolFeaturesSupported` is intentionally omitted so that
/// `ServiceRoot::new` defaults to `expand_all = false` and `no_links =
/// false`, which means subsequent reads use plain `Bmc::get` rather than
/// `Bmc::expand`.
#[must_use]
pub fn service_root_json_with_chassis(root: &str) -> serde_json::Value {
    let chassis_id = format!("{}/Chassis", trim_one_trailing_slash(root));
    serde_json::json!({
        "@odata.id": root,
        "@odata.type": "#ServiceRoot.v1_13_0.ServiceRoot",
        "Id": "RootService",
        "Name": "RootService",
        "Links": {},
        "Chassis": { "@odata.id": chassis_id },
    })
}

/// `ServiceRoot` JSON payload that advertises both a chassis collection
/// and a systems collection.
///
/// Used by Phase 7 tests that drive the [`build_computer_system_generator`]
/// path. ETag and `ProtocolFeaturesSupported` remain omitted so default
/// (non-expanding) `Bmc::get` is used.
#[must_use]
#[allow(dead_code)] // wired in Phase 7+ adapter tests
pub fn service_root_json_with_chassis_and_systems(root: &str) -> serde_json::Value {
    let trimmed = trim_one_trailing_slash(root);
    let chassis_id = format!("{trimmed}/Chassis");
    let systems_id = format!("{trimmed}/Systems");
    serde_json::json!({
        "@odata.id": root,
        "@odata.type": "#ServiceRoot.v1_13_0.ServiceRoot",
        "Id": "RootService",
        "Name": "RootService",
        "Links": {},
        "Chassis": { "@odata.id": chassis_id },
        "Systems": { "@odata.id": systems_id },
    })
}

/// `ServiceRoot` JSON payload that advertises a chassis collection and
/// declares `ProtocolFeaturesSupported.ExpandQuery.{ExpandAll, NoLinks}`
/// so `nv-redfish::NvBmc::expand_property` requests an expanded fetch.
///
/// This is used by Phase 7 tests that drive the chassis-`$expand`
/// path. Whether the `BMC` actually returns an expanded payload depends
/// on what the test mock answers for the chassis URL; this helper only
/// turns on the client-side advertisement.
#[must_use]
#[allow(dead_code)] // wired in Phase 7+ adapter tests
pub fn service_root_json_advertising_expand(root: &str) -> serde_json::Value {
    let chassis_id = format!("{}/Chassis", trim_one_trailing_slash(root));
    serde_json::json!({
        "@odata.id": root,
        "@odata.type": "#ServiceRoot.v1_13_0.ServiceRoot",
        "Id": "RootService",
        "Name": "RootService",
        "Links": {},
        "Chassis": { "@odata.id": chassis_id },
        "ProtocolFeaturesSupported": {
            "ExpandQuery": {
                "ExpandAll": true,
                "NoLinks": true,
                "Levels": true,
                "MaxLevels": 6,
                "Links": true,
            },
        },
    })
}

/// Minimal `ChassisCollection` JSON payload with optional `@odata.etag`.
///
/// `odata_id` is the collection root (typically `"/redfish/v1/Chassis"`).
/// `etag` may be `None` for "no etag" or `Some("\"v1\"")` for a quoted ETag.
#[must_use]
pub fn chassis_collection_json(odata_id: &str, etag: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "@odata.id": odata_id,
        "@odata.type": "#ChassisCollection.ChassisCollection",
        "Name": "Chassis Collection",
        "Members": [],
    });
    if let Some(etag_value) = etag {
        if let Some(map) = payload.as_object_mut() {
            map.insert(
                String::from("@odata.etag"),
                serde_json::Value::String(etag_value.to_string()),
            );
        }
    }
    payload
}

/// Build a `ChassisCollection` JSON payload that advertises a single
/// chassis member with the supplied `member_id`.
#[must_use]
#[allow(dead_code)] // wired in Phase 7+ adapter tests
pub fn chassis_collection_json_with_member(
    odata_id: &str,
    etag: Option<&str>,
    member_id: &str,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "@odata.id": odata_id,
        "@odata.type": "#ChassisCollection.ChassisCollection",
        "Name": "Chassis Collection",
        "Members": [ { "@odata.id": member_id } ],
    });
    if let Some(etag_value) = etag {
        if let Some(map) = payload.as_object_mut() {
            map.insert(
                String::from("@odata.etag"),
                serde_json::Value::String(etag_value.to_string()),
            );
        }
    }
    payload
}

/// Per-child inlined-payload toggle for [`chassis_item_json_expanded`].
#[derive(Debug, Default, Clone)]
#[allow(dead_code)] // wired in Phase 7+ adapter tests
pub struct ExpandedChildSpec {
    /// When `Some(odata_id)` the chassis JSON inlines a `Thermal` payload
    /// rooted at `odata_id` so the deserialised
    /// `NavProperty<Thermal>` lands as `NavProperty::Expanded`.
    pub thermal: Option<String>,
    /// When `Some(odata_id)` the chassis JSON inlines a `Power` payload.
    pub power: Option<String>,
    /// When `Some(odata_id)` the chassis JSON inlines a `SensorCollection`
    /// payload (no members).
    pub sensors_collection: Option<String>,
}

/// Build a `Chassis` item JSON payload with optional inlined sub-resources.
///
/// `odata_id` is the chassis `@odata.id` (for example,
/// `"/redfish/v1/Chassis/1"`). The optional `etag` populates
/// `@odata.etag`. The `spec` toggles which sub-navigation properties are
/// emitted as fully expanded objects (so `nv-redfish` deserialises them
/// into `NavProperty::Expanded(_)`) versus left absent.
#[must_use]
#[allow(dead_code)] // wired in Phase 7+ adapter tests
pub fn chassis_item_json_expanded(
    odata_id: &str,
    etag: Option<&str>,
    spec: &ExpandedChildSpec,
) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert(
        String::from("@odata.id"),
        serde_json::Value::String(odata_id.to_string()),
    );
    obj.insert(
        String::from("@odata.type"),
        serde_json::Value::String(String::from("#Chassis.v1_22_0.Chassis")),
    );
    obj.insert(
        String::from("Id"),
        serde_json::Value::String(String::from("1")),
    );
    obj.insert(
        String::from("Name"),
        serde_json::Value::String(String::from("Chassis")),
    );
    obj.insert(
        String::from("ChassisType"),
        serde_json::Value::String(String::from("RackMount")),
    );
    if let Some(etag_value) = etag {
        obj.insert(
            String::from("@odata.etag"),
            serde_json::Value::String(etag_value.to_string()),
        );
    }
    if let Some(thermal_id) = &spec.thermal {
        obj.insert(
            String::from("Thermal"),
            serde_json::json!({
                "@odata.id": thermal_id,
                "@odata.type": "#Thermal.v1_7_0.Thermal",
                "Id": "Thermal",
                "Name": "Thermal",
            }),
        );
    }
    if let Some(power_id) = &spec.power {
        obj.insert(
            String::from("Power"),
            serde_json::json!({
                "@odata.id": power_id,
                "@odata.type": "#Power.v1_7_0.Power",
                "Id": "Power",
                "Name": "Power",
            }),
        );
    }
    if let Some(sensors_id) = &spec.sensors_collection {
        obj.insert(
            String::from("Sensors"),
            serde_json::json!({
                "@odata.id": sensors_id,
                "@odata.type": "#SensorCollection.SensorCollection",
                "Name": "Sensors",
                "Members": [],
            }),
        );
    }
    serde_json::Value::Object(obj)
}

/// Build a `Chassis` item JSON payload whose nav properties stay as
/// references (only `@odata.id`), so the deserialised
/// `NavProperty<...>` lands as `NavProperty::Reference`. The `sensors_id`
/// argument controls the `Sensors` link the [`build_sensors_generator`]
/// follows.
#[must_use]
#[allow(dead_code)] // wired in Phase 7+ adapter tests
pub fn chassis_item_json_reference(
    odata_id: &str,
    etag: Option<&str>,
    sensors_id: Option<&str>,
) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert(
        String::from("@odata.id"),
        serde_json::Value::String(odata_id.to_string()),
    );
    obj.insert(
        String::from("@odata.type"),
        serde_json::Value::String(String::from("#Chassis.v1_22_0.Chassis")),
    );
    obj.insert(
        String::from("Id"),
        serde_json::Value::String(String::from("1")),
    );
    obj.insert(
        String::from("Name"),
        serde_json::Value::String(String::from("Chassis")),
    );
    obj.insert(
        String::from("ChassisType"),
        serde_json::Value::String(String::from("RackMount")),
    );
    if let Some(etag_value) = etag {
        obj.insert(
            String::from("@odata.etag"),
            serde_json::Value::String(etag_value.to_string()),
        );
    }
    if let Some(sensors_id) = sensors_id {
        obj.insert(
            String::from("Sensors"),
            serde_json::json!({ "@odata.id": sensors_id }),
        );
    }
    serde_json::Value::Object(obj)
}

/// Build a `SensorCollection` JSON payload with the supplied member
/// references.
#[must_use]
#[allow(dead_code)] // wired in Phase 7+ adapter tests
pub fn sensor_collection_json(odata_id: &str, sensor_ids: &[&str]) -> serde_json::Value {
    let members: Vec<serde_json::Value> = sensor_ids
        .iter()
        .map(|id| serde_json::json!({ "@odata.id": id }))
        .collect();
    serde_json::json!({
        "@odata.id": odata_id,
        "@odata.type": "#SensorCollection.SensorCollection",
        "Name": "Sensors",
        "Members": members,
    })
}

/// Build a `ComputerSystemCollection` JSON payload that advertises a
/// single member with the supplied `member_id`.
#[must_use]
#[allow(dead_code)] // wired in Phase 7+ adapter tests
pub fn system_collection_json_with_member(
    odata_id: &str,
    etag: Option<&str>,
    member_id: &str,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "@odata.id": odata_id,
        "@odata.type": "#ComputerSystemCollection.ComputerSystemCollection",
        "Name": "Computer System Collection",
        "Members": [ { "@odata.id": member_id } ],
    });
    if let Some(etag_value) = etag {
        if let Some(map) = payload.as_object_mut() {
            map.insert(
                String::from("@odata.etag"),
                serde_json::Value::String(etag_value.to_string()),
            );
        }
    }
    payload
}

/// Build a minimal `ComputerSystem` item JSON payload with optional
/// `@odata.etag`.
#[must_use]
#[allow(dead_code)] // wired in Phase 7+ adapter tests
pub fn computer_system_json(odata_id: &str, etag: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "@odata.id": odata_id,
        "@odata.type": "#ComputerSystem.v1_22_0.ComputerSystem",
        "Id": "1",
        "Name": "System",
        "SystemType": "Physical",
    });
    if let Some(etag_value) = etag {
        if let Some(map) = payload.as_object_mut() {
            map.insert(
                String::from("@odata.etag"),
                serde_json::Value::String(etag_value.to_string()),
            );
        }
    }
    payload
}

fn trim_one_trailing_slash(s: &str) -> &str {
    s.strip_suffix('/').unwrap_or(s)
}
