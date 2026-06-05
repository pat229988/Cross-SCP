// SPDX-License-Identifier: AGPL-3.0-or-later

//! S3 protocol adapter scaffold.

use std::fmt;

use crossscp_core::{ProtocolCapabilities, SessionProfile, SessionProtocol};
use crossscp_protocol_http::{HttpVersionPolicy, TlsPolicy};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct S3ConnectionConfig {
    pub endpoint_url: String,
    pub region: Option<String>,
    pub bucket: String,
    pub prefix: Option<String>,
    pub credential_ref: Option<String>,
    pub path_style: bool,
    pub tls_policy: TlsPolicy,
    pub http_version_policy: HttpVersionPolicy,
}

impl S3ConnectionConfig {
    pub fn from_profile(profile: &SessionProfile) -> Result<Self, S3Error> {
        if profile.protocol != SessionProtocol::S3 {
            return Err(S3Error::InvalidProtocol(profile.protocol.clone()));
        }
        let (endpoint_url, bucket) = split_endpoint_bucket(&profile.host)?;
        Ok(Self {
            endpoint_url,
            region: None,
            bucket,
            prefix: profile.initial_remote_path.clone(),
            credential_ref: profile.credential_ref.clone(),
            path_style: false,
            tls_policy: TlsPolicy::default(),
            http_version_policy: HttpVersionPolicy::Auto,
        })
    }
}

pub fn split_endpoint_bucket(value: &str) -> Result<(String, String), S3Error> {
    let trimmed = value.trim().trim_end_matches('/');
    let Some((endpoint, bucket)) = trimmed.rsplit_once('/') else {
        return Err(S3Error::MissingBucket);
    };
    if endpoint.is_empty() || bucket.is_empty() {
        return Err(S3Error::MissingBucket);
    }
    Ok((endpoint.to_string(), bucket.to_string()))
}

#[must_use]
pub fn capabilities() -> ProtocolCapabilities {
    ProtocolCapabilities::s3()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum S3Error {
    InvalidProtocol(SessionProtocol),
    MissingBucket,
    UnsupportedOperation(&'static str),
}

impl fmt::Display for S3Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProtocol(protocol) => {
                write!(formatter, "S3 adapter cannot use {protocol}")
            }
            Self::MissingBucket => formatter.write_str(
                "S3 profile requires endpoint/bucket in host field until rich profiles land",
            ),
            Self::UnsupportedOperation(operation) => write!(
                formatter,
                "S3 operation is not implemented yet: {operation}"
            ),
        }
    }
}

impl std::error::Error for S3Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s3_splits_endpoint_and_bucket() {
        let (endpoint, bucket) =
            split_endpoint_bucket("https://s3.example.com/my-bucket").expect("split");
        assert_eq!(endpoint, "https://s3.example.com");
        assert_eq!(bucket, "my-bucket");
        assert!(capabilities().uses_object_prefixes);
    }
}
