// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

// With `redfish-adapter` and only `adapter-service-root` enabled, the
// chassis builder must NOT be reachable. Naming `build_chassis_generator`
// must fail to compile.

fn main() {
    let _ = nv_redfish_scraper::adapter::redfish::build_chassis_generator::<()>;
}
