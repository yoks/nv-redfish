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

//! Thin driver around [`Runtime::next`].
//!
//! Each subcommand calls [`drive`] with a closure that decides what to do
//! with each [`RuntimeOutput`]. The driver itself owns the Ctrl-C wiring:
//! it issues a single `graceful_shutdown` on the first SIGINT and lets the
//! runtime drain naturally to `Shutdown`. A second SIGINT short-circuits
//! the loop so a hung BMC fetch cannot keep the CLI alive.
//!
//! The driver does not hold any per-subcommand state; it returns the final
//! `RuntimeStats` snapshot so callers can render `--stats` consistently.

use nv_redfish_scraper::adapter::redfish::RedfishAdapterError;
use nv_redfish_scraper::adapter::redfish::RedfishEvent;
use nv_redfish_scraper::Runtime;
use nv_redfish_scraper::RuntimeOutput;
use nv_redfish_scraper::RuntimeStats;
use tokio::signal;

/// Drive the runtime to completion.
///
/// Runs `runtime.next()` in a loop, hands every output to `on_output`, and
/// stops when the runtime emits `Shutdown` or the user presses Ctrl-C
/// twice. The first Ctrl-C asks the runtime to drain gracefully; the
/// second forces an immediate exit even if generators are still in flight.
pub async fn drive<F>(
    runtime: &mut Runtime<RedfishEvent, RedfishAdapterError>,
    mut on_output: F,
) -> RuntimeStats
where
    F: FnMut(&RuntimeOutput<RedfishEvent, RedfishAdapterError>),
{
    let mut shutdown_requested = false;
    loop {
        tokio::select! {
            biased;
            sig = signal::ctrl_c() => {
                match sig {
                    Ok(()) => {
                        if shutdown_requested {
                            eprintln!("[runtime] second Ctrl-C — forcing exit");
                            break;
                        }
                        eprintln!("[runtime] Ctrl-C — requesting graceful shutdown");
                        runtime.graceful_shutdown();
                        shutdown_requested = true;
                    }
                    Err(err) => {
                        eprintln!("[runtime] failed to install Ctrl-C handler: {err}");
                        break;
                    }
                }
            }
            out = runtime.next() => {
                on_output(&out);
                if matches!(out, RuntimeOutput::Shutdown) {
                    break;
                }
            }
        }
    }
    runtime.stats()
}
