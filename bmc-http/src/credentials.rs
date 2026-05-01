// SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
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

//! HTTP credentials type.

use std::fmt;

/// Credentials used to access the BMC.
///
/// Security notes:
/// - `Debug`/`Display` redact secrets by design.
/// - Prefer short-lived instances and avoid logging credentials.
#[derive(Clone)]
pub enum BmcCredentials {
    /// Do not send authentication headers.
    None,
    /// Use HTTP Basic authentication with username and password.
    UsernamePassword {
        /// Username to access BMC.
        username: String,
        /// Password to access BMC.
        password: Option<String>,
    },
    /// Use Redfish session token authentication.
    Token {
        /// Token value.
        token: String,
    },
}

impl BmcCredentials {
    /// Create empty credentials for unauthenticated Redfish endpoints.
    #[must_use]
    pub const fn none() -> Self {
        Self::None
    }

    /// Create username/password credentials.
    #[must_use]
    pub const fn username_password(username: String, password: Option<String>) -> Self {
        Self::UsernamePassword { username, password }
    }

    /// Create token credentials.
    #[must_use]
    pub const fn token(token: String) -> Self {
        Self::Token { token }
    }

    /// Create new username/password credentials.
    #[must_use]
    pub const fn new(username: String, password: String) -> Self {
        Self::username_password(username, Some(password))
    }
}

impl fmt::Debug for BmcCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.debug_struct("BmcCredentials::None").finish(),
            Self::UsernamePassword { username, .. } => f
                .debug_struct("BmcCredentials::UsernamePassword")
                .field("username", username)
                .field("password", &"[REDACTED]")
                .finish(),
            Self::Token { .. } => f
                .debug_struct("BmcCredentials::Token")
                .field("token", &"[REDACTED]")
                .finish(),
        }
    }
}

impl fmt::Display for BmcCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "BmcCredentials::None"),
            Self::UsernamePassword { username, .. } => {
                write!(
                    f,
                    "BmcCredentials::UsernamePassword(username: {username}, password: [REDACTED])"
                )
            }
            Self::Token { .. } => write!(f, "BmcCredentials::Token(token: [REDACTED])"),
        }
    }
}
