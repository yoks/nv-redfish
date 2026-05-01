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

//! Public-API boundary tests.
//!
//! These tests assert that the public scraper API does not impose accidental
//! `Clone`, `Debug`, `Eq`, `PartialEq`, `Send`, `Sync`, `Display`, or `Error`
//! bounds on the user-supplied event type `Ev` or error type `Err`, and that
//! identifier types remain opaque.

mod support;

use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use nv_redfish_scraper::ClassId;
use nv_redfish_scraper::GeneratorId;
use nv_redfish_scraper::Runtime;
use nv_redfish_scraper::RuntimeConfig;
use nv_redfish_scraper::RuntimeOutput;
use nv_redfish_scraper::TargetId;
use nv_redfish_scraper::TargetLimits;
use nv_redfish_scraper::WorkSuccess;

use support::fake_error::FakeError;
use support::fake_event::FakeEvent;

#[test]
fn runtime_accepts_event_and_error_types_without_extra_bounds() {
    // Compiles iff Runtime<FakeEvent, FakeError> does not require Clone,
    // Debug, Eq, PartialEq, Display, or Error on the type parameters.
    let _runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
}

#[test]
fn runtime_handle_is_cloneable() {
    let runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let h1 = runtime.handle();
    let h2 = h1.clone();
    drop(h1);
    drop(h2);
}

#[test]
fn runtime_output_does_not_require_clone_on_event_or_error() {
    // Construct an output value without using Clone or Debug bounds.
    let runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let t = runtime
        .add_target(TargetLimits::default())
        .expect("add target");
    let g = runtime
        .add_generator(
            t,
            Box::new(support::fake_generator::FakeGenerator::new([])),
            nv_redfish_scraper::GeneratorConfig::default(),
        )
        .expect("add generator");
    let success: WorkSuccess<FakeEvent> = WorkSuccess {
        events: vec![FakeEvent::new(7)],
        stats: nv_redfish_scraper::WorkStats::default(),
        generator_id: g,
    };
    let _: RuntimeOutput<FakeEvent, FakeError> = RuntimeOutput::Work(Ok(success));
}

#[test]
fn ids_are_hash_eq_ord_clone_copy() {
    let runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let t1 = runtime
        .add_target(TargetLimits::default())
        .expect("add target");
    let t2 = runtime
        .add_target(TargetLimits::default())
        .expect("add target");
    assert_ne!(t1, t2, "newly allocated target ids are unique");
    assert!(t1 < t2, "target ids preserve allocation order");

    // Hash trait works on TargetId.
    let mut h = DefaultHasher::new();
    t1.hash(&mut h);
    let _ = h.finish();

    // Copy works (no compile error implies Copy).
    let copied: TargetId = t1;
    assert_eq!(copied, t1);

    // GeneratorId carries its parent TargetId.
    let g = runtime
        .add_generator(
            t1,
            Box::new(support::fake_generator::FakeGenerator::new([])),
            nv_redfish_scraper::GeneratorConfig::default(),
        )
        .expect("add generator");
    assert_eq!(g.target_id(), t1);

    let _ = std::mem::size_of::<GeneratorId>();
}

#[test]
fn class_id_is_opaque_but_constructible_from_string() {
    let a = ClassId::new("sensors");
    let b = ClassId::new(String::from("sensors"));
    assert_eq!(a, b);
    assert_eq!(a.as_str(), "sensors");
    let mut h = DefaultHasher::new();
    a.hash(&mut h);
    let _ = h.finish();
}

#[test]
fn ids_have_intentional_display_format() {
    // TargetId Display uses a short prefix; the exact format is intentional
    // (not derive-default) and proves the type is not transparent.
    let runtime: Runtime<FakeEvent, FakeError> = Runtime::new(RuntimeConfig::default());
    let t = runtime
        .add_target(TargetLimits::default())
        .expect("add target");
    let s = format!("{}", t);
    assert!(s.starts_with("target:"), "TargetId Display = {}", s);

    let d = format!("{:?}", t);
    assert!(d.starts_with("TargetId("), "TargetId Debug = {}", d);

    let cls = ClassId::new("a");
    assert_eq!(format!("{}", cls), "class:a");
    assert_eq!(format!("{:?}", cls), "ClassId(\"a\")");
}

