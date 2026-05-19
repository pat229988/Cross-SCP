// SPDX-License-Identifier: AGPL-3.0-or-later

//! SFTP proof-of-concept foundation.
//!
//! This phase intentionally avoids linking an SSH implementation. It defines
//! the clean-room boundaries that a future `ssh2`/libssh2, `russh`, or libssh
//! adapter must satisfy.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use crossscp_core::{
    HostKeyPrompt, PromptBroker, PromptError, PromptRequest, PromptResponse, RemoteFile,
    SessionProfile, SessionProtocol,
};
use crossscp_security::{CredentialRef, CredentialSecret, CredentialService, SecurityError};

#[cfg(feature = "ssh2-backend")]
pub mod ssh2_backend;

pub const DEFAULT_SFTP_PORT: u16 = 22;

/// Dependency direction selected for the first live SFTP POC.
pub const INITIAL_SFTP_BACKEND_CANDIDATE: &str =
    "ssh2/libssh2 first POC; keep russh/libssh alternatives open";

/// Connection settings derived from a `SessionProfile`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SftpConnectionConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub initial_remote_path: Option<String>,
    pub credential_ref: Option<CredentialRef>,
}

impl SftpConnectionConfig {
    pub fn from_profile(profile: &SessionProfile) -> Result<Self, SftpError> {
        if profile.protocol != SessionProtocol::Sftp {
            return Err(SftpError::InvalidProtocol(profile.protocol.clone()));
        }
        if profile.host.trim().is_empty() {
            return Err(SftpError::MissingHost);
        }

        let credential_ref = profile
            .credential_ref
            .as_ref()
            .map(|reference| CredentialRef::new(reference.clone()))
            .transpose()?;

        Ok(Self {
            host: profile.host.clone(),
            port: profile.port.unwrap_or(DEFAULT_SFTP_PORT),
            username: profile.username.clone(),
            initial_remote_path: profile.initial_remote_path.clone(),
            credential_ref,
        })
    }
}

/// Authentication material resolved for an SFTP connection attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SftpAuthMaterial {
    pub username: Option<String>,
    pub secret: CredentialSecret,
}

/// Resolve credential material using a profile credential reference.
pub fn resolve_sftp_credentials(
    config: &SftpConnectionConfig,
    credential_service: &dyn CredentialService,
) -> Result<SftpAuthMaterial, SftpError> {
    let reference = config
        .credential_ref
        .as_ref()
        .ok_or(SftpError::MissingCredentialRef)?;
    let secret = credential_service
        .load(reference)?
        .ok_or_else(|| SftpError::CredentialNotFound(reference.as_str().to_string()))?;

    Ok(SftpAuthMaterial {
        username: config.username.clone(),
        secret,
    })
}

/// Ask the caller to verify a host key through the prompt broker boundary.
pub fn verify_host_key(
    broker: &dyn PromptBroker,
    host: impl Into<String>,
    algorithm: impl Into<String>,
    fingerprint: impl Into<String>,
) -> Result<HostKeyDecision, SftpError> {
    let response = broker.prompt(PromptRequest::HostKey(HostKeyPrompt {
        host: host.into(),
        algorithm: algorithm.into(),
        fingerprint: fingerprint.into(),
        expected_fingerprint: None,
        expected_algorithm: None,
    }))?;

    match response {
        PromptResponse::Accept | PromptResponse::AcceptAll => Ok(HostKeyDecision::AcceptOnce),
        PromptResponse::RememberAccepted => Ok(HostKeyDecision::AcceptAndRemember),
        PromptResponse::Reject | PromptResponse::RejectAll => Ok(HostKeyDecision::Reject),
        PromptResponse::Cancel => Err(SftpError::Prompt(PromptError::Cancelled)),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostKeyDecision {
    AcceptOnce,
    AcceptAndRemember,
    Reject,
}

/// Host-key material observed during SSH handshake.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostKeyRecord {
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    pub fingerprint: String,
}

impl HostKeyRecord {
    #[must_use]
    pub fn new(
        host: impl Into<String>,
        port: u16,
        algorithm: impl Into<String>,
        fingerprint: impl Into<String>,
    ) -> Self {
        Self {
            host: host.into(),
            port,
            algorithm: algorithm.into(),
            fingerprint: fingerprint.into(),
        }
    }

    #[must_use]
    pub fn key(&self) -> HostKeyStoreKey {
        HostKeyStoreKey {
            host: self.host.clone(),
            port: self.port,
        }
    }
}

/// Store key for known host entries.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostKeyStoreKey {
    pub host: String,
    pub port: u16,
}

/// Result of comparing an observed host key with known host state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostKeyTrust {
    Trusted,
    Unknown,
    Changed {
        expected: HostKeyRecord,
        observed: HostKeyRecord,
    },
}

/// Host-key persistence boundary.
pub trait HostKeyStore {
    fn check(&self, observed: &HostKeyRecord) -> Result<HostKeyTrust, SftpError>;
    fn trust(&mut self, record: HostKeyRecord) -> Result<(), SftpError>;
    fn forget(&mut self, key: &HostKeyStoreKey) -> Result<bool, SftpError>;
}

/// Deterministic in-memory known-host store.
#[derive(Debug, Default)]
pub struct InMemoryHostKeyStore {
    records: BTreeMap<HostKeyStoreKey, HostKeyRecord>,
}

impl InMemoryHostKeyStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// File-backed known-host store using a dependency-free TSV format.
#[derive(Debug)]
pub struct FileHostKeyStore {
    path: PathBuf,
    memory: InMemoryHostKeyStore,
}

impl FileHostKeyStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, SftpError> {
        let path = path.into();
        let memory = if path.exists() {
            parse_host_keys(&fs::read_to_string(&path)?)?
        } else {
            InMemoryHostKeyStore::new()
        };
        Ok(Self { path, memory })
    }

    fn flush(&self) -> Result<(), SftpError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, render_host_keys(self.memory.records.values()))?;
        Ok(())
    }
}

impl HostKeyStore for FileHostKeyStore {
    fn check(&self, observed: &HostKeyRecord) -> Result<HostKeyTrust, SftpError> {
        self.memory.check(observed)
    }

    fn trust(&mut self, record: HostKeyRecord) -> Result<(), SftpError> {
        self.memory.trust(record)?;
        self.flush()
    }

    fn forget(&mut self, key: &HostKeyStoreKey) -> Result<bool, SftpError> {
        let removed = self.memory.forget(key)?;
        if removed {
            self.flush()?;
        }
        Ok(removed)
    }
}

fn render_host_keys<'a>(records: impl Iterator<Item = &'a HostKeyRecord>) -> String {
    let mut output = String::from("# CrossSCP known hosts v1\n");
    for record in records {
        output.push_str(
            &[
                escape_field(&record.host),
                record.port.to_string(),
                escape_field(&record.algorithm),
                escape_field(&record.fingerprint),
            ]
            .join("\t"),
        );
        output.push('\n');
    }
    output
}

fn parse_host_keys(input: &str) -> Result<InMemoryHostKeyStore, SftpError> {
    let mut store = InMemoryHostKeyStore::new();
    for line in input.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 4 {
            return Err(SftpError::InvalidKnownHostRecord(line.to_string()));
        }
        let port = fields[1]
            .parse::<u16>()
            .map_err(|_| SftpError::InvalidKnownHostRecord(line.to_string()))?;
        store.trust(HostKeyRecord::new(
            unescape_field(fields[0])?,
            port,
            unescape_field(fields[2])?,
            unescape_field(fields[3])?,
        ))?;
    }
    Ok(store)
}

fn escape_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape_field(value: &str) -> Result<String, SftpError> {
    let mut output = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let escaped = chars
                .next()
                .ok_or_else(|| SftpError::InvalidKnownHostRecord(value.to_string()))?;
            match escaped {
                '\\' => output.push('\\'),
                't' => output.push('\t'),
                'n' => output.push('\n'),
                _ => return Err(SftpError::InvalidKnownHostRecord(value.to_string())),
            }
        } else {
            output.push(ch);
        }
    }
    Ok(output)
}

impl HostKeyStore for InMemoryHostKeyStore {
    fn check(&self, observed: &HostKeyRecord) -> Result<HostKeyTrust, SftpError> {
        let Some(expected) = self.records.get(&observed.key()) else {
            return Ok(HostKeyTrust::Unknown);
        };

        if expected.algorithm == observed.algorithm && expected.fingerprint == observed.fingerprint
        {
            Ok(HostKeyTrust::Trusted)
        } else {
            Ok(HostKeyTrust::Changed {
                expected: expected.clone(),
                observed: observed.clone(),
            })
        }
    }

    fn trust(&mut self, record: HostKeyRecord) -> Result<(), SftpError> {
        self.records.insert(record.key(), record);
        Ok(())
    }

    fn forget(&mut self, key: &HostKeyStoreKey) -> Result<bool, SftpError> {
        Ok(self.records.remove(key).is_some())
    }
}

/// Verify an observed host key using known-host state and a prompt broker.
pub fn verify_host_key_with_store(
    store: &mut dyn HostKeyStore,
    broker: &dyn PromptBroker,
    observed: HostKeyRecord,
) -> Result<HostKeyDecision, SftpError> {
    match store.check(&observed)? {
        HostKeyTrust::Trusted => Ok(HostKeyDecision::AcceptOnce),
        HostKeyTrust::Unknown => prompt_and_maybe_store_host_key(store, broker, observed),
        HostKeyTrust::Changed { expected, observed } => {
            prompt_changed_and_maybe_store_host_key(store, broker, expected, observed)
        }
    }
}

fn prompt_and_maybe_store_host_key(
    store: &mut dyn HostKeyStore,
    broker: &dyn PromptBroker,
    observed: HostKeyRecord,
) -> Result<HostKeyDecision, SftpError> {
    let decision = verify_host_key(
        broker,
        observed.host.clone(),
        observed.algorithm.clone(),
        observed.fingerprint.clone(),
    )?;
    if decision == HostKeyDecision::AcceptAndRemember {
        store.trust(observed)?;
    }
    Ok(decision)
}

fn prompt_changed_and_maybe_store_host_key(
    store: &mut dyn HostKeyStore,
    broker: &dyn PromptBroker,
    expected: HostKeyRecord,
    observed: HostKeyRecord,
) -> Result<HostKeyDecision, SftpError> {
    let response = broker.prompt(PromptRequest::HostKey(HostKeyPrompt {
        host: observed.host.clone(),
        algorithm: observed.algorithm.clone(),
        fingerprint: observed.fingerprint.clone(),
        expected_fingerprint: Some(expected.fingerprint),
        expected_algorithm: Some(expected.algorithm),
    }))?;

    let decision = match response {
        PromptResponse::Accept | PromptResponse::AcceptAll => HostKeyDecision::AcceptOnce,
        PromptResponse::RememberAccepted => HostKeyDecision::AcceptAndRemember,
        PromptResponse::Reject | PromptResponse::RejectAll => HostKeyDecision::Reject,
        PromptResponse::Cancel => return Err(SftpError::Prompt(PromptError::Cancelled)),
    };
    if decision == HostKeyDecision::AcceptAndRemember {
        store.trust(observed)?;
    }
    Ok(decision)
}

/// Backend interface that a concrete SSH/SFTP implementation must satisfy.
pub trait SftpBackend {
    fn connect(&mut self, config: &SftpConnectionConfig) -> Result<(), SftpError>;
    fn disconnect(&mut self) -> Result<(), SftpError>;
    fn is_connected(&self) -> bool;
    fn list_directory(&mut self, path: &str) -> Result<Vec<SftpRemoteFile>, SftpError>;
    fn upload_file(
        &mut self,
        local_path: &str,
        remote_path: &str,
    ) -> Result<SftpFileProgress, SftpError>;
    fn download_file(
        &mut self,
        remote_path: &str,
        local_path: &str,
    ) -> Result<SftpFileProgress, SftpError>;
    fn create_directory(&mut self, remote_path: &str) -> Result<(), SftpError>;
    fn delete_path(&mut self, remote_path: &str) -> Result<(), SftpError>;
}

/// SFTP remote file metadata returned by backend list operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SftpRemoteFile {
    pub name: String,
    pub path: String,
    pub size: Option<u64>,
    pub is_directory: bool,
    pub is_symlink: bool,
    pub permissions: Option<u32>,
}

impl SftpRemoteFile {
    #[must_use]
    pub fn into_remote_file(self) -> RemoteFile {
        RemoteFile {
            path: self.path,
            name: self.name,
            size: self.size,
            is_directory: self.is_directory,
            is_symlink: self.is_symlink,
            permissions: self
                .permissions
                .map(|permissions| format!("{permissions:o}")),
        }
    }
}

/// Progress summary for a completed SFTP file operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SftpFileProgress {
    pub source: String,
    pub destination: String,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
}

/// SFTP adapter that owns validated config and delegates transport to a backend.
#[derive(Debug)]
pub struct SftpAdapter<B> {
    config: SftpConnectionConfig,
    backend: B,
}

impl<B> SftpAdapter<B>
where
    B: SftpBackend,
{
    #[must_use]
    pub fn new(config: SftpConnectionConfig, backend: B) -> Self {
        Self { config, backend }
    }

    #[must_use]
    pub fn config(&self) -> &SftpConnectionConfig {
        &self.config
    }

    #[must_use]
    pub fn backend(&self) -> &B {
        &self.backend
    }

    #[must_use]
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn connect(&mut self) -> Result<(), SftpError> {
        self.backend.connect(&self.config)
    }

    pub fn disconnect(&mut self) -> Result<(), SftpError> {
        self.backend.disconnect()
    }

    pub fn list_directory(&mut self, path: &str) -> Result<Vec<SftpRemoteFile>, SftpError> {
        self.backend.list_directory(path)
    }

    pub fn upload_file(
        &mut self,
        local_path: &str,
        remote_path: &str,
    ) -> Result<SftpFileProgress, SftpError> {
        self.backend.upload_file(local_path, remote_path)
    }

    pub fn download_file(
        &mut self,
        remote_path: &str,
        local_path: &str,
    ) -> Result<SftpFileProgress, SftpError> {
        self.backend.download_file(remote_path, local_path)
    }

    pub fn create_directory(&mut self, remote_path: &str) -> Result<(), SftpError> {
        self.backend.create_directory(remote_path)
    }

    pub fn delete_path(&mut self, remote_path: &str) -> Result<(), SftpError> {
        self.backend.delete_path(remote_path)
    }

    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.backend.is_connected()
    }
}

/// Backend placeholder used until a live SSH implementation is linked.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BackendNotLinked;

impl SftpBackend for BackendNotLinked {
    fn connect(&mut self, _config: &SftpConnectionConfig) -> Result<(), SftpError> {
        Err(SftpError::BackendNotLinked)
    }

    fn disconnect(&mut self) -> Result<(), SftpError> {
        Ok(())
    }

    fn is_connected(&self) -> bool {
        false
    }

    fn list_directory(&mut self, _path: &str) -> Result<Vec<SftpRemoteFile>, SftpError> {
        Err(SftpError::BackendNotLinked)
    }

    fn upload_file(
        &mut self,
        _local_path: &str,
        _remote_path: &str,
    ) -> Result<SftpFileProgress, SftpError> {
        Err(SftpError::BackendNotLinked)
    }

    fn download_file(
        &mut self,
        _remote_path: &str,
        _local_path: &str,
    ) -> Result<SftpFileProgress, SftpError> {
        Err(SftpError::BackendNotLinked)
    }

    fn create_directory(&mut self, _remote_path: &str) -> Result<(), SftpError> {
        Err(SftpError::BackendNotLinked)
    }

    fn delete_path(&mut self, _remote_path: &str) -> Result<(), SftpError> {
        Err(SftpError::BackendNotLinked)
    }
}

/// Environment-derived configuration for gated live SFTP tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveSftpTestConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub credential_ref: String,
    pub timeout: Duration,
}

impl LiveSftpTestConfig {
    pub fn from_env() -> Result<Option<Self>, SftpError> {
        let Some(host) = env_value("CROSSSCP_SFTP_TEST_HOST") else {
            return Ok(None);
        };
        let Some(username) = env_value("CROSSSCP_SFTP_TEST_USERNAME") else {
            return Ok(None);
        };
        let Some(credential_ref) = env_value("CROSSSCP_SFTP_TEST_CREDENTIAL_REF") else {
            return Ok(None);
        };

        let port = match env_value("CROSSSCP_SFTP_TEST_PORT") {
            Some(port) => port
                .parse::<u16>()
                .map_err(|_| SftpError::InvalidLiveTestConfig("invalid test port".to_string()))?,
            None => DEFAULT_SFTP_PORT,
        };
        let timeout = match env_value("CROSSSCP_SFTP_TEST_TIMEOUT_SECS") {
            Some(timeout) => Duration::from_secs(timeout.parse::<u64>().map_err(|_| {
                SftpError::InvalidLiveTestConfig("invalid test timeout".to_string())
            })?),
            None => Duration::from_secs(10),
        };

        Ok(Some(Self {
            host,
            port,
            username,
            credential_ref,
            timeout,
        }))
    }

    #[must_use]
    pub fn to_session_profile(&self) -> SessionProfile {
        SessionProfile {
            name: "live-sftp-test".to_string(),
            protocol: SessionProtocol::Sftp,
            host: self.host.clone(),
            port: Some(self.port),
            username: Some(self.username.clone()),
            initial_remote_path: None,
            credential_ref: Some(self.credential_ref.clone()),
        }
    }

    #[must_use]
    pub fn initial_list_path(&self) -> String {
        env_value("CROSSSCP_SFTP_TEST_LIST_PATH").unwrap_or_else(|| ".".to_string())
    }

    #[must_use]
    pub fn transfer_paths(&self) -> Option<(String, String)> {
        Some((
            env_value("CROSSSCP_SFTP_TEST_LOCAL_FILE")?,
            env_value("CROSSSCP_SFTP_TEST_REMOTE_FILE")?,
        ))
    }
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

#[derive(Debug)]
pub enum SftpError {
    InvalidProtocol(SessionProtocol),
    MissingHost,
    MissingUsername,
    MissingCredentialRef,
    CredentialNotFound(String),
    UnsupportedAuthMethod(String),
    AuthenticationFailed,
    NotConnected,
    Security(SecurityError),
    Prompt(PromptError),
    BackendNotLinked,
    Backend(String),
    Io(String),
    InvalidLiveTestConfig(String),
    InvalidKnownHostRecord(String),
}

impl fmt::Display for SftpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProtocol(protocol) => {
                write!(formatter, "SFTP adapter cannot use {protocol:?}")
            }
            Self::MissingHost => write!(formatter, "SFTP host is required"),
            Self::MissingUsername => write!(formatter, "SFTP username is required"),
            Self::MissingCredentialRef => {
                write!(formatter, "SFTP credential reference is required")
            }
            Self::CredentialNotFound(reference) => {
                write!(formatter, "SFTP credential not found: {reference}")
            }
            Self::UnsupportedAuthMethod(message) => {
                write!(formatter, "unsupported SFTP auth method: {message}")
            }
            Self::AuthenticationFailed => write!(formatter, "SFTP authentication failed"),
            Self::NotConnected => write!(formatter, "SFTP backend is not connected"),
            Self::Security(error) => write!(formatter, "SFTP credential error: {error}"),
            Self::Prompt(error) => write!(formatter, "SFTP prompt error: {error}"),
            Self::BackendNotLinked => write!(formatter, "SFTP backend is not linked yet"),
            Self::Backend(message) => write!(formatter, "SFTP backend error: {message}"),
            Self::Io(message) => write!(formatter, "SFTP I/O error: {message}"),
            Self::InvalidLiveTestConfig(message) => {
                write!(formatter, "invalid live SFTP test configuration: {message}")
            }
            Self::InvalidKnownHostRecord(record) => {
                write!(formatter, "invalid known-host record: {record}")
            }
        }
    }
}

impl std::error::Error for SftpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Security(error) => Some(error),
            Self::Prompt(error) => Some(error),
            Self::InvalidProtocol(_)
            | Self::MissingHost
            | Self::MissingUsername
            | Self::MissingCredentialRef
            | Self::CredentialNotFound(_)
            | Self::UnsupportedAuthMethod(_)
            | Self::AuthenticationFailed
            | Self::NotConnected
            | Self::BackendNotLinked
            | Self::Backend(_)
            | Self::Io(_)
            | Self::InvalidLiveTestConfig(_)
            | Self::InvalidKnownHostRecord(_) => None,
        }
    }
}

impl From<std::io::Error> for SftpError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<SecurityError> for SftpError {
    fn from(error: SecurityError) -> Self {
        Self::Security(error)
    }
}

impl From<PromptError> for SftpError {
    fn from(error: PromptError) -> Self {
        Self::Prompt(error)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use crossscp_core::{
        FixedPromptBroker, PromptBroker, PromptError, PromptRequest, PromptResponse,
    };
    use crossscp_security::{
        CredentialRef, CredentialSecret, CredentialService, InMemoryCredentialService, SecretString,
    };

    use super::{
        resolve_sftp_credentials, verify_host_key, verify_host_key_with_store, BackendNotLinked,
        FileHostKeyStore, HostKeyDecision, HostKeyRecord, HostKeyStore, HostKeyTrust,
        InMemoryHostKeyStore, LiveSftpTestConfig, SftpAdapter, SftpBackend, SftpConnectionConfig,
        SftpError, SftpRemoteFile, DEFAULT_SFTP_PORT, INITIAL_SFTP_BACKEND_CANDIDATE,
    };
    use crossscp_core::{SessionProfile, SessionProtocol};

    #[test]
    fn connection_config_rejects_non_sftp_profiles() {
        let profile = SessionProfile::local("local");

        assert!(matches!(
            SftpConnectionConfig::from_profile(&profile),
            Err(SftpError::InvalidProtocol(SessionProtocol::Local))
        ));
    }

    #[test]
    fn connection_config_applies_default_port_and_credential_ref() {
        let profile = sample_sftp_profile(None, Some("vault://site-a"));

        let config = SftpConnectionConfig::from_profile(&profile).expect("valid config");

        assert_eq!(config.host, "sftp.example.com");
        assert_eq!(config.port, DEFAULT_SFTP_PORT);
        assert_eq!(config.username, Some("alice".to_string()));
        assert_eq!(
            config.credential_ref.expect("credential ref").as_str(),
            "vault://site-a"
        );
    }

    #[test]
    fn credential_resolution_uses_credential_service_boundary() {
        let profile = sample_sftp_profile(Some(2222), Some("vault://site-a"));
        let config = SftpConnectionConfig::from_profile(&profile).expect("valid config");
        let mut credentials = InMemoryCredentialService::new();
        let reference = CredentialRef::new("vault://site-a").expect("valid ref");
        credentials
            .store(
                reference,
                CredentialSecret::Password(SecretString::new("secret").expect("valid secret")),
            )
            .expect("store credential");

        let auth = resolve_sftp_credentials(&config, &credentials).expect("resolve auth");

        assert_eq!(auth.username, Some("alice".to_string()));
        assert!(matches!(auth.secret, CredentialSecret::Password(_)));
    }

    #[test]
    fn credential_resolution_requires_reference() {
        let profile = sample_sftp_profile(None, None);
        let config = SftpConnectionConfig::from_profile(&profile).expect("valid config");
        let credentials = InMemoryCredentialService::new();

        assert!(matches!(
            resolve_sftp_credentials(&config, &credentials),
            Err(SftpError::MissingCredentialRef)
        ));
    }

    #[test]
    fn host_key_prompt_accepts_and_remembers() {
        let broker = RememberBroker;

        assert_eq!(
            verify_host_key(&broker, "sftp.example.com", "ssh-ed25519", "SHA256:abc")
                .expect("host key decision"),
            HostKeyDecision::AcceptAndRemember
        );
    }

    #[test]
    fn host_key_prompt_rejects() {
        let broker = FixedPromptBroker::reject();

        assert_eq!(
            verify_host_key(&broker, "sftp.example.com", "ssh-ed25519", "SHA256:abc")
                .expect("host key decision"),
            HostKeyDecision::Reject
        );
    }

    #[test]
    fn host_key_store_trusts_matching_record() {
        let mut store = InMemoryHostKeyStore::new();
        let record = sample_host_key("SHA256:abc");

        store.trust(record.clone()).expect("trust record");

        assert_eq!(
            store.check(&record).expect("check record"),
            HostKeyTrust::Trusted
        );
    }

    #[test]
    fn host_key_store_detects_changed_key() {
        let mut store = InMemoryHostKeyStore::new();
        let expected = sample_host_key("SHA256:old");
        let observed = sample_host_key("SHA256:new");

        store.trust(expected.clone()).expect("trust record");

        assert_eq!(
            store.check(&observed).expect("check changed"),
            HostKeyTrust::Changed { expected, observed }
        );
    }

    #[test]
    fn host_key_verify_remembers_unknown_key_when_prompt_requests_remember() {
        let mut store = InMemoryHostKeyStore::new();
        let broker = RememberBroker;
        let record = sample_host_key("SHA256:abc");

        assert_eq!(
            verify_host_key_with_store(&mut store, &broker, record.clone())
                .expect("verify host key"),
            HostKeyDecision::AcceptAndRemember
        );
        assert_eq!(
            store.check(&record).expect("check remembered"),
            HostKeyTrust::Trusted
        );
    }

    #[test]
    fn host_key_verify_rejects_unknown_key_without_storing() {
        let mut store = InMemoryHostKeyStore::new();
        let broker = FixedPromptBroker::reject();
        let record = sample_host_key("SHA256:abc");

        assert_eq!(
            verify_host_key_with_store(&mut store, &broker, record.clone())
                .expect("verify host key"),
            HostKeyDecision::Reject
        );
        assert_eq!(
            store.check(&record).expect("check rejected"),
            HostKeyTrust::Unknown
        );
    }

    #[test]
    fn changed_host_key_prompt_includes_expected_key_details() {
        let mut store = InMemoryHostKeyStore::new();
        let expected = sample_host_key("SHA256:old");
        let observed = sample_host_key("SHA256:new");
        store.trust(expected).expect("trust old key");
        let broker = ChangedKeyAssertingBroker;

        assert_eq!(
            verify_host_key_with_store(&mut store, &broker, observed).expect("verify changed"),
            HostKeyDecision::Reject
        );
    }

    #[test]
    fn adapter_connect_reports_backend_not_linked() {
        let config = SftpConnectionConfig::from_profile(&sample_sftp_profile(None, None))
            .expect("valid config");
        let mut adapter = SftpAdapter::new(config, BackendNotLinked);

        assert!(matches!(
            adapter.connect(),
            Err(SftpError::BackendNotLinked)
        ));
        assert!(INITIAL_SFTP_BACKEND_CANDIDATE.contains("ssh2"));
    }

    #[test]
    fn adapter_delegates_lifecycle_to_backend() {
        let config = SftpConnectionConfig::from_profile(&sample_sftp_profile(None, None))
            .expect("valid config");
        let mut adapter = SftpAdapter::new(config, FakeSftpBackend::default());

        assert!(!adapter.is_connected());
        adapter.connect().expect("fake connect");
        assert!(adapter.is_connected());
        adapter.disconnect().expect("fake disconnect");
        assert!(!adapter.is_connected());
    }

    #[test]
    fn adapter_delegates_directory_listing_to_backend() {
        let config = SftpConnectionConfig::from_profile(&sample_sftp_profile(None, None))
            .expect("valid config");
        let mut adapter = SftpAdapter::new(config, FakeSftpBackend::default());

        let entries = adapter
            .list_directory("/home/alice")
            .expect("list directory");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "readme.txt");
    }

    #[test]
    fn adapter_delegates_upload_and_download_to_backend() {
        let config = SftpConnectionConfig::from_profile(&sample_sftp_profile(None, None))
            .expect("valid config");
        let mut adapter = SftpAdapter::new(config, FakeSftpBackend::default());

        let upload = adapter
            .upload_file("/tmp/local.txt", "/remote/local.txt")
            .expect("upload file");
        let download = adapter
            .download_file("/remote/local.txt", "/tmp/downloaded.txt")
            .expect("download file");

        assert_eq!(upload.bytes_done, 10);
        assert_eq!(upload.bytes_total, Some(10));
        assert_eq!(download.bytes_done, 11);
        assert_eq!(download.bytes_total, Some(11));
    }

    #[test]
    fn sftp_remote_file_converts_to_core_remote_file() {
        let remote = SftpRemoteFile {
            name: "readme.txt".to_string(),
            path: "/home/alice/readme.txt".to_string(),
            size: Some(12),
            is_directory: false,
            is_symlink: false,
            permissions: Some(0o644),
        }
        .into_remote_file();

        assert_eq!(remote.name, "readme.txt");
        assert_eq!(remote.path, "/home/alice/readme.txt");
        assert_eq!(remote.permissions.as_deref(), Some("644"));
    }

    #[test]
    fn file_host_key_store_persists_trusted_records() {
        let root = unique_temp_dir("crossscp-known-hosts");
        let path = root.join("known_hosts.tsv");
        let record = HostKeyRecord::new("sftp\texample.com", 2222, "ssh-ed25519", "SHA256:abc");

        {
            let mut store = FileHostKeyStore::open(&path).expect("open store");
            store.trust(record.clone()).expect("trust record");
        }

        let reopened = FileHostKeyStore::open(&path).expect("reopen store");
        assert_eq!(
            reopened.check(&record).expect("check record"),
            HostKeyTrust::Trusted
        );

        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn live_test_config_is_none_when_required_env_is_absent() {
        let _guard = env_lock().lock().expect("env lock");
        remove_live_env();

        assert_eq!(LiveSftpTestConfig::from_env().expect("parse env"), None);
    }

    #[test]
    fn live_test_config_parses_when_env_is_complete() {
        let _guard = env_lock().lock().expect("env lock");
        remove_live_env();
        std::env::set_var("CROSSSCP_SFTP_TEST_HOST", "127.0.0.1");
        std::env::set_var("CROSSSCP_SFTP_TEST_USERNAME", "alice");
        std::env::set_var("CROSSSCP_SFTP_TEST_CREDENTIAL_REF", "vault://live");
        std::env::set_var("CROSSSCP_SFTP_TEST_PORT", "2222");
        std::env::set_var("CROSSSCP_SFTP_TEST_TIMEOUT_SECS", "3");
        std::env::set_var("CROSSSCP_SFTP_TEST_LIST_PATH", "/upload");
        std::env::set_var("CROSSSCP_SFTP_TEST_LOCAL_FILE", "/tmp/crossscp-local.txt");
        std::env::set_var(
            "CROSSSCP_SFTP_TEST_REMOTE_FILE",
            "/upload/crossscp-remote.txt",
        );

        let config = LiveSftpTestConfig::from_env()
            .expect("parse env")
            .expect("config present");

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 2222);
        assert_eq!(config.username, "alice");
        assert_eq!(config.credential_ref, "vault://live");
        assert_eq!(config.timeout, std::time::Duration::from_secs(3));
        assert_eq!(config.initial_list_path(), "/upload");
        assert_eq!(
            config.transfer_paths(),
            Some((
                "/tmp/crossscp-local.txt".to_string(),
                "/upload/crossscp-remote.txt".to_string()
            ))
        );
        assert_eq!(config.to_session_profile().protocol, SessionProtocol::Sftp);

        remove_live_env();
    }

    fn sample_sftp_profile(port: Option<u16>, credential_ref: Option<&str>) -> SessionProfile {
        SessionProfile {
            name: "site-a".to_string(),
            protocol: SessionProtocol::Sftp,
            host: "sftp.example.com".to_string(),
            port,
            username: Some("alice".to_string()),
            initial_remote_path: Some("/home/alice".to_string()),
            credential_ref: credential_ref.map(str::to_string),
        }
    }

    fn sample_host_key(fingerprint: &str) -> HostKeyRecord {
        HostKeyRecord::new("sftp.example.com", 22, "ssh-ed25519", fingerprint)
    }

    struct RememberBroker;

    impl PromptBroker for RememberBroker {
        fn prompt(&self, request: PromptRequest) -> Result<PromptResponse, PromptError> {
            assert!(matches!(request, PromptRequest::HostKey(_)));
            Ok(PromptResponse::RememberAccepted)
        }
    }

    struct ChangedKeyAssertingBroker;

    impl PromptBroker for ChangedKeyAssertingBroker {
        fn prompt(&self, request: PromptRequest) -> Result<PromptResponse, PromptError> {
            let PromptRequest::HostKey(prompt) = request else {
                panic!("expected host-key prompt");
            };
            assert_eq!(prompt.fingerprint, "SHA256:new");
            assert_eq!(prompt.expected_fingerprint.as_deref(), Some("SHA256:old"));
            assert_eq!(prompt.expected_algorithm.as_deref(), Some("ssh-ed25519"));
            Ok(PromptResponse::Reject)
        }
    }

    #[derive(Default)]
    struct FakeSftpBackend {
        connected: bool,
    }

    impl SftpBackend for FakeSftpBackend {
        fn connect(&mut self, _config: &SftpConnectionConfig) -> Result<(), SftpError> {
            self.connected = true;
            Ok(())
        }

        fn disconnect(&mut self) -> Result<(), SftpError> {
            self.connected = false;
            Ok(())
        }

        fn is_connected(&self) -> bool {
            self.connected
        }

        fn list_directory(&mut self, path: &str) -> Result<Vec<SftpRemoteFile>, SftpError> {
            assert_eq!(path, "/home/alice");
            Ok(vec![SftpRemoteFile {
                name: "readme.txt".to_string(),
                path: "/home/alice/readme.txt".to_string(),
                size: Some(12),
                is_directory: false,
                is_symlink: false,
                permissions: Some(0o644),
            }])
        }

        fn upload_file(
            &mut self,
            local_path: &str,
            remote_path: &str,
        ) -> Result<super::SftpFileProgress, SftpError> {
            assert_eq!(local_path, "/tmp/local.txt");
            assert_eq!(remote_path, "/remote/local.txt");
            Ok(super::SftpFileProgress {
                source: local_path.to_string(),
                destination: remote_path.to_string(),
                bytes_done: 10,
                bytes_total: Some(10),
            })
        }

        fn download_file(
            &mut self,
            remote_path: &str,
            local_path: &str,
        ) -> Result<super::SftpFileProgress, SftpError> {
            assert_eq!(remote_path, "/remote/local.txt");
            assert_eq!(local_path, "/tmp/downloaded.txt");
            Ok(super::SftpFileProgress {
                source: remote_path.to_string(),
                destination: local_path.to_string(),
                bytes_done: 11,
                bytes_total: Some(11),
            })
        }

        fn create_directory(&mut self, _remote_path: &str) -> Result<(), SftpError> {
            Ok(())
        }

        fn delete_path(&mut self, _remote_path: &str) -> Result<(), SftpError> {
            Ok(())
        }
    }

    fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ));
        std::fs::create_dir(&path).expect("create temp dir");
        path
    }

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn remove_live_env() {
        std::env::remove_var("CROSSSCP_SFTP_TEST_HOST");
        std::env::remove_var("CROSSSCP_SFTP_TEST_USERNAME");
        std::env::remove_var("CROSSSCP_SFTP_TEST_CREDENTIAL_REF");
        std::env::remove_var("CROSSSCP_SFTP_TEST_PORT");
        std::env::remove_var("CROSSSCP_SFTP_TEST_TIMEOUT_SECS");
        std::env::remove_var("CROSSSCP_SFTP_TEST_LIST_PATH");
        std::env::remove_var("CROSSSCP_SFTP_TEST_LOCAL_FILE");
        std::env::remove_var("CROSSSCP_SFTP_TEST_REMOTE_FILE");
    }
}
