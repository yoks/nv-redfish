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

//! Shared integration-test helpers for the scraper crate.
//!
//! Each integration test file pulls these in with `mod support;`.

#![allow(dead_code)] // helpers are shared across many integration test files

pub mod controlled;
pub mod fake_error;
pub mod fake_event;
pub mod fake_generator;
pub mod fake_payload;
pub mod harness;
pub mod lcg;
