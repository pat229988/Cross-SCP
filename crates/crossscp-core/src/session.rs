// SPDX-License-Identifier: AGPL-3.0-or-later

//! Session profile models shared by protocol adapters and frontends.

use std::fmt;
use std::str::FromStr;

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

impl SessionProtocol {
    /// Stable lowercase identifier used by config, CLI, and GUI bridges.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Sftp => "sftp",
            Self::Scp => "scp",
            Self::Ftp => "ftp",
            Self::Ftps => "ftps",
            Self::WebDav => "webdav",
            Self::S3 => "s3",
            Self::Local => "local",
        }
    }

    /// Human-oriented protocol label.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Sftp => "SFTP",
            Self::Scp => "SCP",
            Self::Ftp => "FTP",
            Self::Ftps => "FTPS",
            Self::WebDav => "WebDAV",
            Self::S3 => "S3",
            Self::Local => "Local",
        }
    }

    /// Default TCP port where a protocol has one.
    #[must_use]
    pub const fn default_port(&self) -> Option<u16> {
        match self {
            Self::Sftp | Self::Scp => Some(22),
            Self::Ftp | Self::Ftps => Some(21),
            Self::WebDav | Self::S3 => Some(443),
            Self::Local => None,
        }
    }

    /// All protocols exposed by the domain model.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Sftp,
            Self::Scp,
            Self::Ftp,
            Self::Ftps,
            Self::WebDav,
            Self::S3,
            Self::Local,
        ]
    }
}

impl fmt::Display for SessionProtocol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for SessionProtocol {
    type Err = ParseSessionProtocolError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "sftp" => Ok(Self::Sftp),
            "scp" => Ok(Self::Scp),
            "ftp" => Ok(Self::Ftp),
            "ftps" => Ok(Self::Ftps),
            "webdav" | "web-dav" => Ok(Self::WebDav),
            "s3" => Ok(Self::S3),
            "local" => Ok(Self::Local),
            _ => Err(ParseSessionProtocolError(value.to_string())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseSessionProtocolError(String);

impl fmt::Display for ParseSessionProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unsupported session protocol: {}", self.0)
    }
}

impl std::error::Error for ParseSessionProtocolError {}

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
