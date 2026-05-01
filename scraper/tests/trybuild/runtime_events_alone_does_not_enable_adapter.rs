// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

// Even with `runtime-events` enabled, the `redfish-adapter` module must NOT
// be reachable unless `redfish-adapter` is also enabled. Naming an adapter
// type here must fail to compile.

fn main() {
    let _: nv_redfish_scraper::adapter::redfish::BmcId = ::std::unreachable!();
}
