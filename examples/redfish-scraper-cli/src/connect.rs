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

//! Connection helper used by every subcommand.
//!
//! Resolves the connection-flags into a fully-built `(HttpBmc<Client>,
//! ServiceRoot<HttpBmc<Client>>, BmcId)` triple. The `Arc<HttpBmc<Client>>`
//! is the live client cloned into every scraper generator and post-discovery
//! tokio task; the `ServiceRoot` is the typed entry point used to seed
//! generators and to walk chassis/system collections directly when a
//! subcommand prefers a synchronous pre-step over a generator.

use crate::cli::ConnectArgs;
use nv_redfish::bmc_http::reqwest::Client;
use nv_redfish::bmc_http::reqwest::ClientParams;
use nv_redfish::bmc_http::BmcCredentials;
use nv_redfish::bmc_http::CacheSettings;
use nv_redfish::bmc_http::HttpBmc;
use nv_redfish::ServiceRoot;
use nv_redfish_scraper::adapter::redfish::BmcId;
use std::error::Error as StdError;
use std::sync::Arc;

/// Concrete BMC type used by the CLI.
///
/// Aliased so the subcommand modules only have to mention one type and so
/// any future swap to a different transport (e.g. a mock) only touches this
/// file.
pub type Bmc = HttpBmc<Client>;

/// Result of [`connect`].
///
/// `Connection` returns the canonical triple from the plan: the live BMC
/// client, the typed service-root, and the resolved application-level
/// identifier. `root` already holds its own clone of the underlying
/// `Arc<Bmc>` (via `NvBmc<B>`), so the explicit `bmc` field is currently
/// unused by every subcommand. It is kept on the public surface so future
/// commands (e.g. a `replay` subcommand that schedules generators against
/// the same BMC after rehydrating snapshot records) can reach for the live
/// client without re-running [`connect`].
#[allow(dead_code)] // `bmc` reserved for v2 commands.
pub struct Connection {
    /// Live BMC client cloned into each generator and post-discovery task.
    pub bmc: Arc<Bmc>,
    /// Typed Redfish service-root rooted at this BMC.
    pub root: ServiceRoot<Bmc>,
    /// Application-level identifier carried by every emitted event.
    pub bmc_id: BmcId,
}

/// Build a [`Connection`] from the parsed connection flags.
///
/// # Errors
///
/// Returns an error if the HTTP client cannot be constructed, if the BMC
/// URL has no host component to derive a `bmc_id` from, or if the
/// `ServiceRoot` GET fails.
pub async fn connect(args: &ConnectArgs) -> Result<Connection, Box<dyn StdError>> {
    let client_params = ClientParams::new()
        .accept_invalid_certs(args.insecure)
        .timeout(args.timeout);
    let client = Client::with_params(client_params)?;

    let credentials = match (args.username.as_deref(), args.password.as_deref()) {
        (Some(user), Some(pass)) => BmcCredentials::new(user.to_string(), pass.to_string()),
        _ => BmcCredentials::none(),
    };

    let bmc = Arc::new(HttpBmc::new(
        client,
        args.bmc.clone(),
        credentials,
        CacheSettings::default(),
    ));

    let root = ServiceRoot::new(Arc::clone(&bmc)).await?;

    let bmc_id_name = args.bmc_id.clone().unwrap_or_else(|| {
        args.bmc
            .host_str()
            .map_or_else(|| args.bmc.to_string(), str::to_string)
    });
    let bmc_id = BmcId::new(bmc_id_name);

    Ok(Connection { bmc, root, bmc_id })
}
