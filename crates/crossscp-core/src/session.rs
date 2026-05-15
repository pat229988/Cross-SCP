// SPDX-License-Identifier: AGPL-3.0-or-later

//! Session profile models shared by protocol adapters and frontends.

/// Supported protocol choices exposed by the clean-room core model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionProtocol {
    Sftp,
    Scp,
    Ftp,
    Ftps,
    WebDav,
    S3,
    Local,
}

/// A connection profile without secret material.
///
/// Passwords, tokens, and key passphrases must be resolved through platform
/// credential services rather than stored directly in this model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionProfile {
    pub name: String,
    pub protocol: SessionProtocol,
    pub host: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub initial_remote_path: Option<String>,
    pub credential_ref: Option<String>,
}

impl SessionProfile {
    /// Create a local-only profile for local-to-local workflows.
    #[must_use]
    pub fn local(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            protocol: SessionProtocol::Local,
            host: String::new(),
            port: None,
            username: None,
            initial_remote_path: None,
            credential_ref: None,
        }
    }
}
