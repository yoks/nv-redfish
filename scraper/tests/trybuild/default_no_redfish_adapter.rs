// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

// Without the `redfish-adapter` feature the `adapter` module is not compiled,
// so naming any of its public types must fail to compile.

fn main() {
    let _: nv_redfish_scraper::adapter::redfish::RedfishEvent =
        ::std::unreachable!();
}
