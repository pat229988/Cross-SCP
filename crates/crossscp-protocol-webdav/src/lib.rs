// SPDX-License-Identifier: AGPL-3.0-or-later

//! WebDAV protocol adapter scaffold.

use std::fmt;

use crossscp_core::{ProtocolCapabilities, SessionProfile, SessionProtocol};
use crossscp_protocol_http::{HttpVersionPolicy, TlsPolicy};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebDavConnectionConfig {
    pub base_url: String,
    pub username: Option<String>,
    pub root_path: Option<String>,
    pub credential_ref: Option<String>,
    pub tls_policy: TlsPolicy,
    pub http_version_policy: HttpVersionPolicy,
}

impl WebDavConnectionConfig {
    pub fn from_profile(profile: &SessionProfile) -> Result<Self, WebDavError> {
        if profile.protocol != SessionProtocol::WebDav {
            return Err(WebDavError::InvalidProtocol(profile.protocol.clone()));
        }
        if profile.host.trim().is_empty() {
            return Err(WebDavError::MissingBaseUrl);
        }
        Ok(Self {
            base_url: profile.host.clone(),
            username: profile.username.clone(),
            root_path: profile.initial_remote_path.clone(),
            credential_ref: profile.credential_ref.clone(),
            tls_policy: TlsPolicy::default(),
            http_version_policy: HttpVersionPolicy::Auto,
        })
    }
}

#[must_use]
pub fn capabilities() -> ProtocolCapabilities {
    ProtocolCapabilities::webdav()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WebDavError {
    InvalidProtocol(SessionProtocol),
    MissingBaseUrl,
    UnsupportedOperation(&'static str),
}

impl fmt::Display for WebDavError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProtocol(protocol) => {
                write!(formatter, "WebDAV adapter cannot use {protocol}")
            }
            Self::MissingBaseUrl => formatter.write_str("WebDAV base URL is required"),
            Self::UnsupportedOperation(operation) => write!(
                formatter,
                "WebDAV operation is not implemented yet: {operation}"
            ),
        }
    }
}

impl std::error::Error for WebDavError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webdav_config_requires_base_url_and_defaults_tls() {
        let profile = SessionProfile {
            name: "dav".to_string(),
            protocol: SessionProtocol::WebDav,
            host: "https://example.com/dav".to_string(),
            port: None,
            username: Some("alice".to_string()),
            initial_remote_path: Some("/".to_string()),
            credential_ref: Some("env://CROSSSCP_REMOTE_PASSWORD".to_string()),
        };
        let config = WebDavConnectionConfig::from_profile(&profile).expect("config");
        assert!(config.tls_policy.require_valid_certificate);
        assert!(capabilities().supports_http_version_policy);
    }
}
