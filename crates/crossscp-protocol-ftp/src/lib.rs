// SPDX-License-Identifier: AGPL-3.0-or-later

//! FTP/FTPS protocol adapter backed by `suppaftp`.

use std::fmt;
use std::fs::{self, File};
use std::io::{Cursor, Write};
use std::path::Path;
use std::str::FromStr;

use crossscp_core::{ProtocolCapabilities, RemoteFile, SessionProfile, SessionProtocol};
use crossscp_security::{CredentialRef, CredentialSecret, CredentialService, SecurityError};
use suppaftp::list::File as FtpListFile;
use suppaftp::native_tls::TlsConnector;
use suppaftp::{FtpStream, NativeTlsConnector, NativeTlsFtpStream};

pub const DEFAULT_FTP_PORT: u16 = 21;
pub const DEFAULT_IMPLICIT_FTPS_PORT: u16 = 990;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FtpsMode {
    Explicit,
    Implicit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FtpConnectionConfig {
    pub protocol: SessionProtocol,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub passive_mode: bool,
    pub ftps_mode: Option<FtpsMode>,
    pub initial_remote_path: Option<String>,
    pub credential_ref: Option<CredentialRef>,
}

impl FtpConnectionConfig {
    pub fn from_profile(profile: &SessionProfile) -> Result<Self, FtpError> {
        if !matches!(
            profile.protocol,
            SessionProtocol::Ftp | SessionProtocol::Ftps
        ) {
            return Err(FtpError::InvalidProtocol(profile.protocol.clone()));
        }
        if profile.host.trim().is_empty() {
            return Err(FtpError::MissingHost);
        }
        let credential_ref = profile
            .credential_ref
            .as_ref()
            .map(|reference| CredentialRef::new(reference.clone()))
            .transpose()?;
        Ok(Self {
            protocol: profile.protocol.clone(),
            host: profile.host.clone(),
            port: profile.port.unwrap_or(DEFAULT_FTP_PORT),
            username: profile.username.clone(),
            passive_mode: true,
            ftps_mode: (profile.protocol == SessionProtocol::Ftps).then_some(FtpsMode::Explicit),
            initial_remote_path: profile.initial_remote_path.clone(),
            credential_ref,
        })
    }

    #[must_use]
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FtpAuthMaterial {
    pub username: String,
    pub password: String,
}

pub fn resolve_ftp_credentials(
    config: &FtpConnectionConfig,
    credential_service: &dyn CredentialService,
) -> Result<FtpAuthMaterial, FtpError> {
    let reference = config
        .credential_ref
        .as_ref()
        .ok_or(FtpError::MissingCredentialRef)?;
    let secret = credential_service
        .load(reference)?
        .ok_or_else(|| FtpError::CredentialNotFound(reference.as_str().to_string()))?;
    let password = match secret {
        CredentialSecret::Password(secret) | CredentialSecret::Token(secret) => {
            secret.expose().to_string()
        }
        CredentialSecret::PrivateKey { .. } | CredentialSecret::PrivateKeyPassphrase(_) => {
            return Err(FtpError::UnsupportedAuthMethod(
                "FTP/FTPS supports password or token credentials only".to_string(),
            ));
        }
    };
    let username = config.username.clone().ok_or(FtpError::MissingUsername)?;
    Ok(FtpAuthMaterial { username, password })
}

#[must_use]
pub fn capabilities(protocol: SessionProtocol) -> ProtocolCapabilities {
    ProtocolCapabilities::ftp_like(protocol == SessionProtocol::Ftps)
}

enum FtpSession {
    Plain(FtpStream),
    Secure(NativeTlsFtpStream),
}

impl FtpSession {
    fn list(&mut self, path: &str) -> Result<Vec<String>, suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.mlsd(Some(path)),
            Self::Secure(stream) => stream.mlsd(Some(path)),
        }
    }

    fn list_fallback(&mut self, path: &str) -> Result<Vec<String>, suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.list(Some(path)),
            Self::Secure(stream) => stream.list(Some(path)),
        }
    }

    fn put_file(&mut self, remote_path: &str, file: &mut File) -> Result<u64, suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.put_file(remote_path, file),
            Self::Secure(stream) => stream.put_file(remote_path, file),
        }
    }

    fn retr_as_buffer(&mut self, remote_path: &str) -> Result<Cursor<Vec<u8>>, suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.retr_as_buffer(remote_path),
            Self::Secure(stream) => stream.retr_as_buffer(remote_path),
        }
    }

    fn mkdir(&mut self, remote_path: &str) -> Result<(), suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.mkdir(remote_path),
            Self::Secure(stream) => stream.mkdir(remote_path),
        }
    }

    fn rm(&mut self, remote_path: &str) -> Result<(), suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.rm(remote_path),
            Self::Secure(stream) => stream.rm(remote_path),
        }
    }

    fn rmdir(&mut self, remote_path: &str) -> Result<(), suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.rmdir(remote_path),
            Self::Secure(stream) => stream.rmdir(remote_path),
        }
    }

    fn rename(&mut self, from: &str, to: &str) -> Result<(), suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.rename(from, to),
            Self::Secure(stream) => stream.rename(from, to),
        }
    }

    fn quit(&mut self) -> Result<(), suppaftp::FtpError> {
        match self {
            Self::Plain(stream) => stream.quit(),
            Self::Secure(stream) => stream.quit(),
        }
    }
}

pub struct FtpAdapter {
    config: FtpConnectionConfig,
    auth: FtpAuthMaterial,
    session: Option<FtpSession>,
}

impl FtpAdapter {
    #[must_use]
    pub fn new(config: FtpConnectionConfig, auth: FtpAuthMaterial) -> Self {
        Self {
            config,
            auth,
            session: None,
        }
    }

    pub fn connect(&mut self) -> Result<(), FtpError> {
        let address = self.config.address();
        let session = match self.config.protocol {
            SessionProtocol::Ftp => {
                let mut stream = FtpStream::connect(address)?;
                stream.login(&self.auth.username, &self.auth.password)?;
                FtpSession::Plain(stream)
            }
            SessionProtocol::Ftps => {
                if self.config.ftps_mode == Some(FtpsMode::Implicit) {
                    return Err(FtpError::UnsupportedOperation(
                        "implicit FTPS is not supported by the selected backend yet",
                    ));
                }
                let tls = TlsConnector::new()?;
                let stream = NativeTlsFtpStream::connect(address)?;
                let mut stream =
                    stream.into_secure(NativeTlsConnector::from(tls), &self.config.host)?;
                stream.login(&self.auth.username, &self.auth.password)?;
                FtpSession::Secure(stream)
            }
            ref protocol => return Err(FtpError::InvalidProtocol(protocol.clone())),
        };
        self.session = Some(session);
        Ok(())
    }

    pub fn disconnect(&mut self) -> Result<(), FtpError> {
        if let Some(mut session) = self.session.take() {
            session.quit()?;
        }
        Ok(())
    }

    pub fn list_directory(&mut self, path: &str) -> Result<Vec<RemoteFile>, FtpError> {
        let lines = match self.session_mut()?.list(path) {
            Ok(lines) => lines,
            Err(_) => self.session_mut()?.list_fallback(path)?,
        };
        Ok(lines
            .into_iter()
            .filter_map(|line| parse_list_line(path, &line))
            .collect())
    }

    pub fn create_directory(&mut self, remote_path: &str) -> Result<(), FtpError> {
        self.session_mut()?.mkdir(remote_path)?;
        Ok(())
    }

    pub fn delete_path(&mut self, remote_path: &str) -> Result<(), FtpError> {
        match self.session_mut()?.rm(remote_path) {
            Ok(()) => Ok(()),
            Err(file_error) => match self.session_mut()?.rmdir(remote_path) {
                Ok(()) => Ok(()),
                Err(_) => Err(FtpError::Backend(file_error.to_string())),
            },
        }
    }

    pub fn delete_path_recursive(&mut self, remote_path: &str) -> Result<(), FtpError> {
        match self.list_directory(remote_path) {
            Ok(entries) => {
                for entry in entries {
                    if entry.name == "." || entry.name == ".." {
                        continue;
                    }
                    if entry.is_directory {
                        self.delete_path_recursive(&entry.path)?;
                    } else {
                        self.session_mut()?.rm(&entry.path)?;
                    }
                }
                self.session_mut()?.rmdir(remote_path)?;
                Ok(())
            }
            Err(_) => self.delete_path(remote_path),
        }
    }

    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), FtpError> {
        self.session_mut()?.rename(from, to)?;
        Ok(())
    }

    pub fn upload_path(
        &mut self,
        local_path: &str,
        remote_path: &str,
    ) -> Result<FtpTransferSummary, FtpError> {
        let local = Path::new(local_path);
        if local.is_dir() {
            self.upload_directory(local, remote_path)
        } else {
            self.upload_file(local, remote_path)
        }
    }

    pub fn download_path(
        &mut self,
        remote_path: &str,
        local_path: &str,
    ) -> Result<FtpTransferSummary, FtpError> {
        if let Ok(entries) = self.list_directory(remote_path) {
            fs::create_dir_all(local_path)?;
            let mut summary = FtpTransferSummary::new(remote_path, local_path);
            for entry in entries {
                if entry.name == "." || entry.name == ".." {
                    continue;
                }
                let child_local = Path::new(local_path).join(&entry.name);
                let child_summary = if entry.is_directory {
                    self.download_path(&entry.path, &child_local.to_string_lossy())?
                } else {
                    self.download_file(&entry.path, &child_local)?
                };
                summary.bytes_done += child_summary.bytes_done;
                summary.bytes_total = Some(summary.bytes_done);
            }
            Ok(summary)
        } else {
            self.download_file(remote_path, Path::new(local_path))
        }
    }

    fn upload_file(
        &mut self,
        local_path: &Path,
        remote_path: &str,
    ) -> Result<FtpTransferSummary, FtpError> {
        let bytes_total = fs::metadata(local_path)?.len();
        let mut file = File::open(local_path)?;
        let bytes_done = self.session_mut()?.put_file(remote_path, &mut file)?;
        Ok(FtpTransferSummary {
            source: local_path.to_string_lossy().to_string(),
            destination: remote_path.to_string(),
            bytes_done,
            bytes_total: Some(bytes_total),
        })
    }

    fn upload_directory(
        &mut self,
        local_path: &Path,
        remote_path: &str,
    ) -> Result<FtpTransferSummary, FtpError> {
        let _ = self.create_directory(remote_path);
        let mut summary = FtpTransferSummary::new(local_path.to_string_lossy(), remote_path);
        for entry in fs::read_dir(local_path)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let child_remote = join_remote_path(remote_path, &name);
            let child_summary = if entry.file_type()?.is_dir() {
                self.upload_directory(&entry.path(), &child_remote)?
            } else {
                self.upload_file(&entry.path(), &child_remote)?
            };
            summary.bytes_done += child_summary.bytes_done;
            summary.bytes_total = Some(summary.bytes_done);
        }
        Ok(summary)
    }

    fn download_file(
        &mut self,
        remote_path: &str,
        local_path: &Path,
    ) -> Result<FtpTransferSummary, FtpError> {
        if let Some(parent) = local_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let buffer = self.session_mut()?.retr_as_buffer(remote_path)?;
        let bytes = buffer.into_inner();
        let mut file = File::create(local_path)?;
        file.write_all(&bytes)?;
        Ok(FtpTransferSummary {
            source: remote_path.to_string(),
            destination: local_path.to_string_lossy().to_string(),
            bytes_done: bytes.len() as u64,
            bytes_total: Some(bytes.len() as u64),
        })
    }

    fn session_mut(&mut self) -> Result<&mut FtpSession, FtpError> {
        self.session.as_mut().ok_or(FtpError::NotConnected)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FtpTransferSummary {
    pub source: String,
    pub destination: String,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
}

impl FtpTransferSummary {
    #[must_use]
    pub fn new(source: impl Into<String>, destination: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            destination: destination.into(),
            bytes_done: 0,
            bytes_total: Some(0),
        }
    }
}

fn parse_list_line(parent: &str, line: &str) -> Option<RemoteFile> {
    let file = FtpListFile::from_str(line).ok()?;
    let name = file.name().to_string();
    if name == "." || name == ".." {
        return None;
    }
    Some(RemoteFile {
        path: join_remote_path(parent, &name),
        name,
        size: Some(file.size() as u64),
        is_directory: file.is_directory(),
        is_symlink: file.is_symlink(),
        permissions: None,
    })
}

#[must_use]
pub fn join_remote_path(base: &str, name: &str) -> String {
    let base = if base.trim().is_empty() {
        "/"
    } else {
        base.trim()
    };
    if base == "/" {
        format!("/{name}")
    } else if base.ends_with('/') {
        format!("{base}{name}")
    } else {
        format!("{base}/{name}")
    }
}

#[derive(Debug)]
pub enum FtpError {
    InvalidProtocol(SessionProtocol),
    MissingHost,
    MissingUsername,
    MissingCredentialRef,
    CredentialNotFound(String),
    UnsupportedAuthMethod(String),
    AuthenticationFailed,
    NotConnected,
    Security(SecurityError),
    Tls(String),
    Backend(String),
    Io(String),
    UnsupportedOperation(&'static str),
}

impl fmt::Display for FtpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProtocol(protocol) => {
                write!(formatter, "FTP/FTPS adapter cannot use {protocol}")
            }
            Self::MissingHost => formatter.write_str("FTP/FTPS host is required"),
            Self::MissingUsername => formatter.write_str("FTP/FTPS username is required"),
            Self::MissingCredentialRef => {
                formatter.write_str("FTP/FTPS credential reference is required")
            }
            Self::CredentialNotFound(reference) => {
                write!(formatter, "FTP/FTPS credential not found: {reference}")
            }
            Self::UnsupportedAuthMethod(message) => {
                write!(formatter, "unsupported FTP/FTPS auth method: {message}")
            }
            Self::AuthenticationFailed => formatter.write_str("FTP/FTPS authentication failed"),
            Self::NotConnected => formatter.write_str("FTP/FTPS backend is not connected"),
            Self::Security(error) => write!(formatter, "FTP/FTPS credential error: {error}"),
            Self::Tls(message) => write!(formatter, "FTPS TLS error: {message}"),
            Self::Backend(message) => write!(formatter, "FTP/FTPS backend error: {message}"),
            Self::Io(message) => write!(formatter, "FTP/FTPS I/O error: {message}"),
            Self::UnsupportedOperation(operation) => {
                write!(
                    formatter,
                    "FTP/FTPS operation is not supported: {operation}"
                )
            }
        }
    }
}

impl std::error::Error for FtpError {}

impl From<SecurityError> for FtpError {
    fn from(error: SecurityError) -> Self {
        Self::Security(error)
    }
}

impl From<std::io::Error> for FtpError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<suppaftp::FtpError> for FtpError {
    fn from(error: suppaftp::FtpError) -> Self {
        let message = error.to_string();
        if message.to_ascii_lowercase().contains("login") || message.contains("530") {
            Self::AuthenticationFailed
        } else {
            Self::Backend(message)
        }
    }
}

impl From<suppaftp::native_tls::Error> for FtpError {
    fn from(error: suppaftp::native_tls::Error) -> Self {
        Self::Tls(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use crossscp_security::{
        CredentialSecret, CredentialService, InMemoryCredentialService, SecretString,
    };

    use super::*;

    #[test]
    fn ftps_defaults_to_explicit_mode_and_tls_capabilities() {
        let profile = sample_profile(SessionProtocol::Ftps);
        let config = FtpConnectionConfig::from_profile(&profile).expect("config");
        assert_eq!(config.port, DEFAULT_FTP_PORT);
        assert_eq!(config.ftps_mode, Some(FtpsMode::Explicit));
        assert!(capabilities(SessionProtocol::Ftps).supports_tls_policy);
    }

    #[test]
    fn ftp_config_rejects_wrong_protocol() {
        let mut profile = sample_profile(SessionProtocol::Ftp);
        profile.protocol = SessionProtocol::Sftp;
        assert!(matches!(
            FtpConnectionConfig::from_profile(&profile),
            Err(FtpError::InvalidProtocol(SessionProtocol::Sftp))
        ));
    }

    #[test]
    fn credentials_resolve_password_only() {
        let profile = sample_profile(SessionProtocol::Ftp);
        let config = FtpConnectionConfig::from_profile(&profile).expect("config");
        let mut service = InMemoryCredentialService::new();
        service
            .store(
                CredentialRef::new("env://CROSSSCP_REMOTE_PASSWORD").expect("ref"),
                CredentialSecret::Password(SecretString::new("secret").expect("secret")),
            )
            .expect("store");
        let auth = resolve_ftp_credentials(&config, &service).expect("auth");
        assert_eq!(auth.username, "alice");
        assert_eq!(auth.password, "secret");
    }

    #[test]
    fn joins_remote_paths() {
        assert_eq!(join_remote_path("/", "file.txt"), "/file.txt");
        assert_eq!(join_remote_path("/pub", "file.txt"), "/pub/file.txt");
        assert_eq!(join_remote_path("/pub/", "file.txt"), "/pub/file.txt");
    }

    fn sample_profile(protocol: SessionProtocol) -> SessionProfile {
        SessionProfile {
            name: "ftp".to_string(),
            protocol,
            host: "example.com".to_string(),
            port: None,
            username: Some("alice".to_string()),
            initial_remote_path: Some("/".to_string()),
            credential_ref: Some("env://CROSSSCP_REMOTE_PASSWORD".to_string()),
        }
    }
}
