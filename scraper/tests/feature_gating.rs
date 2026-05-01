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

//! Feature-gating tests.
//!
//! Verifies the dependency direction of feature flags both ways:
//! * Without `redfish-adapter`, the adapter module is unreachable.
//! * With `redfish-adapter`, the adapter module is reachable.
//! * Without `runtime-events`, the `Runtime` variant of `RuntimeOutput`
//!   cannot be constructed with an inhabited payload.
//! * The Redfish adapter never exposes a detached fetch entry point.

#[cfg(feature = "redfish-adapter")]
#[test]
fn adapter_module_is_reachable_with_feature() {
    let _ = nv_redfish_scraper::adapter::redfish::BmcId::new("bmc");
}

#[cfg(all(
    not(feature = "redfish-adapter"),
    not(feature = "runtime-events"),
))]
#[test]
fn trybuild_default_feature_gating() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/default_no_redfish_adapter.rs");
    t.compile_fail("tests/trybuild/default_no_runtime_event.rs");
}

// With the adapter enabled the module exists but the API must not expose any
// detached fetch entry point. This trybuild case is run only in the
// adapter-enabled configuration so the failure cleanly identifies the
// missing function rather than the missing module.
#[cfg(feature = "redfish-adapter")]
#[test]
fn trybuild_no_detached_redfish_command() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/no_detached_redfish_command.rs");
}

// `runtime-events` alone must not pull in the Redfish adapter module.
// Run only when `runtime-events` is the only of the two relevant features
// enabled, so the test failure points at the missing adapter module.
#[cfg(all(
    feature = "runtime-events",
    not(feature = "redfish-adapter"),
))]
#[test]
fn trybuild_runtime_events_alone_does_not_enable_adapter() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/runtime_events_alone_does_not_enable_adapter.rs");
}

// With `redfish-adapter` enabled but no per-capability feature, the
// per-capability builders must be hidden. Gate this driver on having the
// adapter enabled but every adapter-capability feature off.
#[cfg(all(
    feature = "redfish-adapter",
    not(feature = "adapter-service-root"),
    not(feature = "adapter-chassis"),
    not(feature = "adapter-sensors"),
    not(feature = "adapter-computer-systems"),
))]
#[test]
fn trybuild_adapter_without_capability_hides_per_cap_builder() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/adapter_without_capability_hides_per_cap_builder.rs");
}

// With `adapter-service-root` enabled but `adapter-chassis` off, the
// chassis builder must not be reachable.
#[cfg(all(
    feature = "redfish-adapter",
    feature = "adapter-service-root",
    not(feature = "adapter-chassis"),
))]
#[test]
fn trybuild_adapter_with_one_cap_hides_others() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/adapter_with_one_cap_hides_others.rs");
}
