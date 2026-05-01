// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

// Without the `runtime-events` feature, `RuntimeEventType = Infallible` and
// the `Runtime` variant of `RuntimeOutput` cannot be constructed with any
// inhabited payload.

fn main() {
    let _: nv_redfish_scraper::RuntimeOutput<(), ()> =
        nv_redfish_scraper::RuntimeOutput::Runtime(());
}
