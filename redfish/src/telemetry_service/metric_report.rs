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

use std::sync::Arc;

use crate::schema::redfish::metric_report::MetricReport as MetricReportSchema;
use crate::Error;
use crate::NvBmc;
use nv_redfish_core::Bmc;
use nv_redfish_core::NavProperty;

/// Metric report entity wrapper.
pub struct MetricReport<B: Bmc> {
    bmc: NvBmc<B>,
    nav: NavProperty<MetricReportSchema>,
}

impl<B: Bmc> MetricReport<B> {
    pub(crate) fn new(bmc: &NvBmc<B>, nav: NavProperty<MetricReportSchema>) -> Self {
        Self {
            bmc: bmc.clone(),
            nav,
        }
    }

    /// Fetch latest data for this metric report.
    ///
    /// # Errors
    ///
    /// Returns an error if fetching the entity fails.
    pub async fn fetch(&self) -> Result<Arc<MetricReportSchema>, Error<B>> {
        self.nav.get(self.bmc.as_ref()).await.map_err(Error::Bmc)
    }

    /// Delete this metric report.
    ///
    /// # Errors
    ///
    /// Returns an error if deleting the entity fails.
    pub async fn delete(&self) -> Result<(), Error<B>> {
        self.bmc
            .as_ref()
            .delete(self.nav.id())
            .await
            .map_err(Error::Bmc)
            .map(|_| ())
    }
}
