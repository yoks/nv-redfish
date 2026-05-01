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

#[test]
#[cfg(not(feature = "redfish-adapter"))]
fn runtime_only_build_does_not_enable_redfish_adapter_api() {
    assert!(!cfg!(feature = "redfish-adapter"));
}

#[test]
#[cfg(not(feature = "redfish-adapter"))]
fn redfish_adapter_module_is_absent_without_feature() {
    let tests = trybuild::TestCases::new();

    tests.compile_fail("tests/trybuild/default_no_redfish_adapter.rs");
}

#[test]
#[cfg(not(feature = "runtime-events"))]
fn concrete_runtime_event_type_is_absent_without_feature() {
    let tests = trybuild::TestCases::new();

    tests.compile_fail("tests/trybuild/default_no_runtime_event.rs");
}

#[test]
#[cfg(feature = "redfish-adapter")]
fn redfish_adapter_feature_exposes_adapter_module() {
    let bmc_id = nv_redfish_scraper::adapter::redfish::BmcId::new("bmc");

    assert_eq!(bmc_id.as_str(), "bmc");
}

#[test]
#[cfg(not(feature = "runtime-events"))]
fn runtime_events_are_disabled_by_default() {
    assert!(!cfg!(feature = "runtime-events"));
}

#[test]
#[cfg(feature = "runtime-events")]
fn runtime_events_feature_exposes_runtime_event_type() {
    let event = nv_redfish_scraper::RuntimeEvent::GlobalThrottled;

    assert!(matches!(
        event,
        nv_redfish_scraper::RuntimeEvent::GlobalThrottled
    ));
}

#[test]
#[cfg(feature = "redfish-adapter")]
fn detached_redfish_command_language_is_not_available() {
    let tests = trybuild::TestCases::new();

    tests.compile_fail("tests/trybuild/no_detached_redfish_command.rs");
}
