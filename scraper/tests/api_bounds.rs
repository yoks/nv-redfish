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

mod support;

use nv_redfish_scraper::ClassId;
use nv_redfish_scraper::GeneratorConfig;
use nv_redfish_scraper::GeneratorId;
use nv_redfish_scraper::Runtime;
use nv_redfish_scraper::RuntimeConfig;
use nv_redfish_scraper::RuntimeOutput;
use nv_redfish_scraper::TargetId;
use nv_redfish_scraper::TargetLimits;
use nv_redfish_scraper::WorkError;
use nv_redfish_scraper::WorkStats;
use nv_redfish_scraper::WorkSuccess;
use support::fake_error::NonFormattingError;
use support::fake_event::NonTraitEvent;

#[test]
fn runtime_output_accepts_event_without_clone_debug_eq_or_partial_eq() {
    let event = NonTraitEvent::new("event");
    let output: RuntimeOutput<NonTraitEvent, ()> =
        RuntimeOutput::Work(Ok(WorkSuccess::new(vec![event], WorkStats::default())));

    match output {
        RuntimeOutput::Work(Ok(success)) => {
            assert_eq!(success.events()[0].name(), "event");
        }
        RuntimeOutput::Work(Err(_)) => {
            panic!("expected success output");
        }
        RuntimeOutput::Runtime(_) => {
            panic!("did not expect runtime output");
        }
    }
}

#[test]
fn work_error_accepts_error_without_formatting_traits() {
    let error = NonFormattingError::new("error");
    let output: RuntimeOutput<(), NonFormattingError> =
        RuntimeOutput::Work(Err(WorkError::new(error, WorkStats::default())));

    match output {
        RuntimeOutput::Work(Err(error)) => {
            assert_eq!(error.error().name(), "error");
        }
        RuntimeOutput::Work(Ok(_)) => {
            panic!("expected error output");
        }
        RuntimeOutput::Runtime(_) => {
            panic!("did not expect runtime output");
        }
    }
}

#[test]
fn public_ids_are_opaque_and_intentionally_displayable() {
    let target_id = TargetId::new("target-a");
    let generator_id = GeneratorId::new("generator-a");
    let class_id = ClassId::new("class-a");

    assert_eq!(target_id.as_str(), "target-a");
    assert_eq!(generator_id.to_string(), "generator-a");
    assert_eq!(class_id.as_str(), "class-a");
}

#[test]
fn common_runtime_api_compiles_without_redfish_features() {
    let mut runtime: Runtime<NonTraitEvent, NonFormattingError> =
        Runtime::new(RuntimeConfig::default());
    let target_id = TargetId::new("target");
    let generator_id = GeneratorId::new("generator");

    assert!(runtime
        .add_target(target_id.clone(), TargetLimits::default())
        .is_ok());
    assert!(runtime.trigger_generator(&generator_id).is_err());
    assert_eq!(runtime.stats().target_count(), 1);

    let config = GeneratorConfig::default();
    assert!(config.enabled());
}
