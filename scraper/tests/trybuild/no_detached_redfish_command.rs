// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

// The Redfish adapter must not expose a detached, ad-hoc fetch entry point.
// Calling such a (non-existent) function must fail to compile, proving the
// adapter API only routes through typed `nv-redfish` objects + scraper
// generators.

fn main() {
    let _ = nv_redfish_scraper::adapter::redfish::fetch_arbitrary_resource;
}
