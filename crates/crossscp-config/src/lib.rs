// SPDX-License-Identifier: AGPL-3.0-or-later

//! Session profile persistence for CrossSCP.
//!
//! This phase intentionally uses a small dependency-free line format so config
//! behavior can be tested before choosing a final serde/TOML/JSON strategy.
//! Secret values must not be stored here; profiles may only keep credential
//! references.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crossscp_core::{SessionProfile, SessionProtocol};

/// Session persistence behavior shared by memory/file/platform stores.
pub trait SessionStore {
    fn save(&mut self, profile: SessionProfile) -> Result<(), ConfigError>;
    fn get(&self, name: &str) -> Result<Option<SessionProfile>, ConfigError>;
    fn list(&self) -> Result<Vec<SessionProfile>, ConfigError>;
    fn remove(&mut self, name: &str) -> Result<bool, ConfigError>;
}

/// Config persistence errors.
#[derive(Debug)]
pub enum ConfigError {
    EmptyProfileName,
    InvalidRecord(String),
    InvalidProtocol(String),
    Io(std::io::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProfileName => write!(formatter, "session profile name cannot be empty"),
            Self::InvalidRecord(record) => write!(formatter, "invalid session record: {record}"),
            Self::InvalidProtocol(protocol) => {
                write!(formatter, "invalid session protocol: {protocol}")
            }
            Self::Io(error) => write!(formatter, "config I/O error: {error}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::EmptyProfileName | Self::InvalidRecord(_) | Self::InvalidProtocol(_) => None,
        }
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Deterministic in-memory session store for tests and non-persistent callers.
#[derive(Debug, Default)]
pub struct InMemorySessionStore {
    profiles: BTreeMap<String, SessionProfile>,
}

impl InMemorySessionStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl SessionStore for InMemorySessionStore {
    fn save(&mut self, profile: SessionProfile) -> Result<(), ConfigError> {
        validate_profile(&profile)?;
        self.profiles.insert(profile.name.clone(), profile);
        Ok(())
    }

    fn get(&self, name: &str) -> Result<Option<SessionProfile>, ConfigError> {
        Ok(self.profiles.get(name).cloned())
    }

    fn list(&self) -> Result<Vec<SessionProfile>, ConfigError> {
        Ok(self.profiles.values().cloned().collect())
    }

    fn remove(&mut self, name: &str) -> Result<bool, ConfigError> {
        Ok(self.profiles.remove(name).is_some())
    }
}

/// File-backed session store using the dependency-free phase-9 line format.
#[derive(Debug)]
pub struct FileSessionStore {
    path: PathBuf,
    memory: InMemorySessionStore,
}

impl FileSessionStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, ConfigError> {
        let path = path.into();
        let memory = if path.exists() {
            parse_profiles(&fs::read_to_string(&path)?)?
        } else {
            InMemorySessionStore::new()
        };

        Ok(Self { path, memory })
    }

    fn flush(&self) -> Result<(), ConfigError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, render_profiles(&self.memory.list()?))?;
        Ok(())
    }
}

impl SessionStore for FileSessionStore {
    fn save(&mut self, profile: SessionProfile) -> Result<(), ConfigError> {
        self.memory.save(profile)?;
        self.flush()
    }

    fn get(&self, name: &str) -> Result<Option<SessionProfile>, ConfigError> {
        self.memory.get(name)
    }

    fn list(&self) -> Result<Vec<SessionProfile>, ConfigError> {
        self.memory.list()
    }

    fn remove(&mut self, name: &str) -> Result<bool, ConfigError> {
        let removed = self.memory.remove(name)?;
        if removed {
            self.flush()?;
        }
        Ok(removed)
    }
}

fn validate_profile(profile: &SessionProfile) -> Result<(), ConfigError> {
    if profile.name.trim().is_empty() {
        return Err(ConfigError::EmptyProfileName);
    }
    Ok(())
}

fn render_profiles(profiles: &[SessionProfile]) -> String {
    let mut output = String::from("# CrossSCP sessions v1\n");
    for profile in profiles {
        output.push_str(
            &[
                escape(&profile.name),
                protocol_to_str(&profile.protocol).to_string(),
                escape(&profile.host),
                profile
                    .port
                    .map(|port| port.to_string())
                    .unwrap_or_default(),
                escape_option(&profile.username),
                escape_option(&profile.initial_remote_path),
                escape_option(&profile.credential_ref),
            ]
            .join("\t"),
        );
        output.push('\n');
    }
    output
}

fn parse_profiles(input: &str) -> Result<InMemorySessionStore, ConfigError> {
    let mut store = InMemorySessionStore::new();
    for line in input.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }

        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 7 {
            return Err(ConfigError::InvalidRecord(line.to_string()));
        }

        let profile = SessionProfile {
            name: unescape(fields[0])?,
            protocol: str_to_protocol(fields[1])?,
            host: unescape(fields[2])?,
            port: parse_port(fields[3])?,
            username: unescape_option(fields[4])?,
            initial_remote_path: unescape_option(fields[5])?,
            credential_ref: unescape_option(fields[6])?,
        };
        store.save(profile)?;
    }
    Ok(store)
}

fn parse_port(field: &str) -> Result<Option<u16>, ConfigError> {
    if field.is_empty() {
        Ok(None)
    } else {
        field
            .parse::<u16>()
            .map(Some)
            .map_err(|_| ConfigError::InvalidRecord(format!("invalid port: {field}")))
    }
}

fn escape_option(value: &Option<String>) -> String {
    value
        .as_ref()
        .map_or_else(String::new, |value| escape(value))
}

fn unescape_option(value: &str) -> Result<Option<String>, ConfigError> {
    if value.is_empty() {
        Ok(None)
    } else {
        unescape(value).map(Some)
    }
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape(value: &str) -> Result<String, ConfigError> {
    let mut output = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let escaped = chars
                .next()
                .ok_or_else(|| ConfigError::InvalidRecord(value.to_string()))?;
            match escaped {
                '\\' => output.push('\\'),
                't' => output.push('\t'),
                'n' => output.push('\n'),
                _ => return Err(ConfigError::InvalidRecord(value.to_string())),
            }
        } else {
            output.push(ch);
        }
    }
    Ok(output)
}

fn protocol_to_str(protocol: &SessionProtocol) -> &'static str {
    match protocol {
        SessionProtocol::Sftp => "sftp",
        SessionProtocol::Scp => "scp",
        SessionProtocol::Ftp => "ftp",
        SessionProtocol::Ftps => "ftps",
        SessionProtocol::WebDav => "webdav",
        SessionProtocol::S3 => "s3",
        SessionProtocol::Local => "local",
    }
}

fn str_to_protocol(protocol: &str) -> Result<SessionProtocol, ConfigError> {
    match protocol {
        "sftp" => Ok(SessionProtocol::Sftp),
        "scp" => Ok(SessionProtocol::Scp),
        "ftp" => Ok(SessionProtocol::Ftp),
        "ftps" => Ok(SessionProtocol::Ftps),
        "webdav" => Ok(SessionProtocol::WebDav),
        "s3" => Ok(SessionProtocol::S3),
        "local" => Ok(SessionProtocol::Local),
        _ => Err(ConfigError::InvalidProtocol(protocol.to_string())),
    }
}

#[allow(dead_code)]
fn default_session_path(config_dir: &Path) -> PathBuf {
    config_dir.join("sessions.tsv")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crossscp_core::{SessionProfile, SessionProtocol};

    use super::{FileSessionStore, InMemorySessionStore, SessionStore};

    #[test]
    fn memory_store_saves_lists_gets_and_removes_profiles() {
        let mut store = InMemorySessionStore::new();
        let profile = sample_profile("site-a");

        store.save(profile.clone()).expect("save profile");

        assert_eq!(store.get("site-a").expect("get profile"), Some(profile));
        assert_eq!(store.list().expect("list profiles").len(), 1);
        assert!(store.remove("site-a").expect("remove profile"));
        assert!(store.get("site-a").expect("get removed").is_none());
    }

    #[test]
    fn memory_store_rejects_empty_profile_names() {
        let mut store = InMemorySessionStore::new();
        let mut profile = sample_profile("");
        profile.name = "   ".to_string();

        assert!(store.save(profile).is_err());
    }

    #[test]
    fn file_store_persists_profiles_across_reopen() {
        let root = unique_temp_dir("crossscp-config-file-store");
        let path = root.join("sessions.tsv");
        let profile = sample_profile("prod\tsite");

        {
            let mut store = FileSessionStore::open(&path).expect("open store");
            store.save(profile.clone()).expect("save profile");
        }

        let reopened = FileSessionStore::open(&path).expect("reopen store");

        assert_eq!(
            reopened.get(&profile.name).expect("get profile"),
            Some(profile)
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn file_store_remove_persists() {
        let root = unique_temp_dir("crossscp-config-remove");
        let path = root.join("sessions.tsv");

        {
            let mut store = FileSessionStore::open(&path).expect("open store");
            store
                .save(sample_profile("to-remove"))
                .expect("save profile");
            assert!(store.remove("to-remove").expect("remove profile"));
        }

        let reopened = FileSessionStore::open(&path).expect("reopen store");
        assert!(reopened.get("to-remove").expect("get removed").is_none());

        fs::remove_dir_all(root).expect("cleanup");
    }

    fn sample_profile(name: &str) -> SessionProfile {
        SessionProfile {
            name: name.to_string(),
            protocol: SessionProtocol::Sftp,
            host: "example.com".to_string(),
            port: Some(22),
            username: Some("alice".to_string()),
            initial_remote_path: Some("/home/alice".to_string()),
            credential_ref: Some("keychain://site-a".to_string()),
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
        fs::create_dir(&path).expect("create temp dir");
        path
    }
}
