// SPDX-License-Identifier: AGPL-3.0-or-later

//! SCP transfer adapter backed by `ssh2`/libssh2.

use std::fmt;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

use crossscp_core::{ProtocolCapabilities, SessionProfile, SessionProtocol};
use crossscp_security::{CredentialRef, CredentialSecret, CredentialService, SecurityError};

pub const DEFAULT_SCP_PORT: u16 = 22;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScpConnectionConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub initial_remote_path: Option<String>,
    pub credential_ref: Option<CredentialRef>,
}

impl ScpConnectionConfig {
    pub fn from_profile(profile: &SessionProfile) -> Result<Self, ScpError> {
        if profile.protocol != SessionProtocol::Scp {
            return Err(ScpError::InvalidProtocol(profile.protocol.clone()));
        }
        if profile.host.trim().is_empty() {
            return Err(ScpError::MissingHost);
        }
        let credential_ref = profile
            .credential_ref
            .as_ref()
            .map(|reference| CredentialRef::new(reference.clone()))
            .transpose()?;
        Ok(Self {
            host: profile.host.clone(),
            port: profile.port.unwrap_or(DEFAULT_SCP_PORT),
            username: profile.username.clone(),
            initial_remote_path: profile.initial_remote_path.clone(),
            credential_ref,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScpAuthMaterial {
    pub username: Option<String>,
    pub secret: CredentialSecret,
}

pub fn resolve_scp_credentials(
    config: &ScpConnectionConfig,
    credential_service: &dyn CredentialService,
) -> Result<ScpAuthMaterial, ScpError> {
    let reference = config
        .credential_ref
        .as_ref()
        .ok_or(ScpError::MissingCredentialRef)?;
    let secret = credential_service
        .load(reference)?
        .ok_or_else(|| ScpError::CredentialNotFound(reference.as_str().to_string()))?;
    Ok(ScpAuthMaterial {
        username: config.username.clone(),
        secret,
    })
}

#[must_use]
pub fn capabilities() -> ProtocolCapabilities {
    ProtocolCapabilities::scp_transfer_only()
}

pub struct ScpAdapter {
    config: ScpConnectionConfig,
    auth: ScpAuthMaterial,
    timeout: Duration,
    session: Option<ssh2::Session>,
}

impl ScpAdapter {
    #[must_use]
    pub fn new(config: ScpConnectionConfig, auth: ScpAuthMaterial) -> Self {
        Self {
            config,
            auth,
            timeout: Duration::from_secs(300),
            session: None,
        }
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn connect(&mut self) -> Result<(), ScpError> {
        let address = (self.config.host.as_str(), self.config.port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| {
                ScpError::Backend(format!(
                    "no socket address for {}:{}",
                    self.config.host, self.config.port
                ))
            })?;
        let tcp = TcpStream::connect_timeout(&address, self.timeout)?;
        tcp.set_read_timeout(Some(self.timeout))?;
        tcp.set_write_timeout(Some(self.timeout))?;

        let mut session = ssh2::Session::new()?;
        session.set_tcp_stream(tcp);
        session.handshake()?;

        let username = self
            .auth
            .username
            .as_deref()
            .or(self.config.username.as_deref())
            .ok_or(ScpError::MissingUsername)?;

        match &self.auth.secret {
            CredentialSecret::Password(password) => {
                session.userauth_password(username, password.expose())?;
            }
            CredentialSecret::PrivateKey {
                private_key_path,
                passphrase,
            } => {
                session.userauth_pubkey_file(
                    username,
                    None,
                    Path::new(private_key_path),
                    passphrase.as_ref().map(|secret| secret.expose()),
                )?;
            }
            CredentialSecret::PrivateKeyPassphrase(_) | CredentialSecret::Token(_) => {
                return Err(ScpError::UnsupportedAuthMethod(
                    "SCP supports password auth or private-key auth with an optional passphrase"
                        .to_string(),
                ));
            }
        }

        if !session.authenticated() {
            return Err(ScpError::AuthenticationFailed);
        }
        self.session = Some(session);
        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<(), ScpError> {
        if let Some(session) = self.session.take() {
            session.disconnect(None, "CrossSCP disconnect", None)?;
        }
        Ok(())
    }

    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(ssh2::Session::authenticated)
    }

    pub fn upload_file_with_progress<F>(
        &mut self,
        local_path: &str,
        remote_path: &str,
        mut report_progress: F,
    ) -> Result<ScpTransferSummary, ScpError>
    where
        F: FnMut(u64, Option<u64>),
    {
        let session = self.session.as_ref().ok_or(ScpError::NotConnected)?;
        let metadata = fs::metadata(local_path)?;
        if metadata.is_dir() {
            return Err(ScpError::UnsupportedOperation(
                "recursive SCP upload is not implemented yet; use SFTP for folders",
            ));
        }
        let bytes_total = metadata.len();
        let mut remote = session.scp_send(Path::new(remote_path), 0o644, bytes_total, None)?;
        let mut local = File::open(local_path)?;
        let bytes_done = copy_with_progress(
            &mut local,
            &mut remote,
            Some(bytes_total),
            &mut report_progress,
        )?;
        remote.send_eof()?;
        remote.wait_eof()?;
        remote.close()?;
        remote.wait_close()?;
        Ok(ScpTransferSummary {
            source: local_path.to_string(),
            destination: remote_path.to_string(),
            bytes_done,
            bytes_total: Some(bytes_total),
        })
    }

    pub fn download_file_with_progress<F>(
        &mut self,
        remote_path: &str,
        local_path: &str,
        mut report_progress: F,
    ) -> Result<ScpTransferSummary, ScpError>
    where
        F: FnMut(u64, Option<u64>),
    {
        let session = self.session.as_ref().ok_or(ScpError::NotConnected)?;
        if let Some(parent) = Path::new(local_path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let (mut remote, stat) = session.scp_recv(Path::new(remote_path))?;
        let bytes_total = Some(stat.size());
        let mut local = File::create(local_path)?;
        let bytes_done =
            copy_with_progress(&mut remote, &mut local, bytes_total, &mut report_progress)?;
        remote.send_eof()?;
        remote.wait_eof()?;
        remote.close()?;
        remote.wait_close()?;
        Ok(ScpTransferSummary {
            source: remote_path.to_string(),
            destination: local_path.to_string(),
            bytes_done,
            bytes_total,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScpTransferSummary {
    pub source: String,
    pub destination: String,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
}

fn copy_with_progress<R, W, F>(
    reader: &mut R,
    writer: &mut W,
    bytes_total: Option<u64>,
    report_progress: &mut F,
) -> Result<u64, ScpError>
where
    R: Read,
    W: Write,
    F: FnMut(u64, Option<u64>),
{
    let mut buffer = [0_u8; 64 * 1024];
    let mut bytes_done = 0;
    report_progress(0, bytes_total);
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
        bytes_done += read as u64;
        report_progress(bytes_done, bytes_total);
    }
    Ok(bytes_done)
}

#[derive(Debug)]
pub enum ScpError {
    InvalidProtocol(SessionProtocol),
    MissingHost,
    MissingUsername,
    MissingCredentialRef,
    CredentialNotFound(String),
    UnsupportedAuthMethod(String),
    AuthenticationFailed,
    NotConnected,
    Security(SecurityError),
    Backend(String),
    Io(String),
    UnsupportedOperation(&'static str),
}

impl fmt::Display for ScpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProtocol(protocol) => {
                write!(formatter, "SCP adapter cannot use {protocol}")
            }
            Self::MissingHost => formatter.write_str("SCP host is required"),
            Self::MissingUsername => formatter.write_str("SCP username is required"),
            Self::MissingCredentialRef => {
                formatter.write_str("SCP credential reference is required")
            }
            Self::CredentialNotFound(reference) => {
                write!(formatter, "SCP credential not found: {reference}")
            }
            Self::UnsupportedAuthMethod(message) => {
                write!(formatter, "unsupported SCP auth method: {message}")
            }
            Self::AuthenticationFailed => formatter.write_str("SCP authentication failed"),
            Self::NotConnected => formatter.write_str("SCP backend is not connected"),
            Self::Security(error) => write!(formatter, "SCP credential error: {error}"),
            Self::Backend(message) => write!(formatter, "SCP backend error: {message}"),
            Self::Io(message) => write!(formatter, "SCP I/O error: {message}"),
            Self::UnsupportedOperation(operation) => {
                write!(formatter, "SCP operation is not supported: {operation}")
            }
        }
    }
}

impl std::error::Error for ScpError {}

impl From<SecurityError> for ScpError {
    fn from(error: SecurityError) -> Self {
        Self::Security(error)
    }
}

impl From<std::io::Error> for ScpError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<ssh2::Error> for ScpError {
    fn from(error: ssh2::Error) -> Self {
        let message = error.to_string();
        if message.to_ascii_lowercase().contains("auth") {
            Self::AuthenticationFailed
        } else {
            Self::Backend(message)
        }
    }
}

#[cfg(test)]
mod tests {
    use crossscp_security::{
        CredentialSecret, CredentialService, InMemoryCredentialService, SecretString,
    };

    use super::*;

    #[test]
    fn scp_config_defaults_port() {
        let profile = sample_profile();
        assert_eq!(
            ScpConnectionConfig::from_profile(&profile)
                .expect("config")
                .port,
            22
        );
    }

    #[test]
    fn scp_capabilities_are_transfer_only() {
        let caps = capabilities();
        assert!(!caps.can_list);
        assert!(caps.can_upload);
        assert!(caps.can_download);
    }

    #[test]
    fn credentials_resolve_password() {
        let profile = sample_profile();
        let config = ScpConnectionConfig::from_profile(&profile).expect("config");
        let mut service = InMemoryCredentialService::new();
        service
            .store(
                CredentialRef::new("env://CROSSSCP_REMOTE_PASSWORD").expect("ref"),
                CredentialSecret::Password(SecretString::new("secret").expect("secret")),
            )
            .expect("store");
        let auth = resolve_scp_credentials(&config, &service).expect("auth");
        assert_eq!(auth.username, Some("alice".to_string()));
        assert!(matches!(auth.secret, CredentialSecret::Password(_)));
    }

    fn sample_profile() -> SessionProfile {
        SessionProfile {
            name: "site".to_string(),
            protocol: SessionProtocol::Scp,
            host: "example.com".to_string(),
            port: None,
            username: Some("alice".to_string()),
            initial_remote_path: Some("/tmp".to_string()),
            credential_ref: Some("env://CROSSSCP_REMOTE_PASSWORD".to_string()),
        }
    }
}
