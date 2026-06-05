// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared HTTP/TLS policy placeholders for WebDAV and S3 protocol adapters.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HttpVersionPolicy {
    #[default]
    Auto,
    Http1Only,
    Http2Preferred,
    Http2Only,
    Http3Preferred,
    Http3Only,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TlsMinimumVersion {
    #[default]
    Default,
    Tls12,
    Tls13,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TlsPolicy {
    pub minimum_version: TlsMinimumVersion,
    pub require_valid_certificate: bool,
}

impl Default for TlsPolicy {
    fn default() -> Self {
        Self {
            minimum_version: TlsMinimumVersion::Default,
            require_valid_certificate: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tls_policy_requires_valid_certificates_by_default() {
        assert!(TlsPolicy::default().require_valid_certificate);
    }
}
