// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

// With `redfish-adapter` enabled but no per-capability feature on, the
// per-capability builders must NOT be in scope. Naming
// `build_service_root_generator` must fail to compile.

fn main() {
    let _ = nv_redfish_scraper::adapter::redfish::build_service_root_generator::<()>;
}
