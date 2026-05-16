// SPDX-License-Identifier: AGPL-3.0-or-later

//! Security primitives for secret redaction and credential lookup.
//!
//! This crate intentionally avoids platform APIs. Windows Credential Manager,
//! DPAPI, and macOS Keychain integrations should live in platform-specific
//! crates behind the traits defined here.

use std::collections::BTreeMap;
use std::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop};

/// A stable reference to secret material stored outside session profiles.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CredentialRef(String);

impl CredentialRef {
    pub fn new(value: impl Into<String>) -> Result<Self, SecurityError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(SecurityError::InvalidCredentialRef);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Secret string wrapper that redacts debug/display output.
#[derive(Clone, Eq, PartialEq, Zeroize, ZeroizeOnDrop)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Result<Self, SecurityError> {
        let value = value.into();
        if value.is_empty() {
            return Err(SecurityError::EmptySecret);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretString(REDACTED)")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("REDACTED")
    }
}

/// Credential material returned by credential services.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CredentialSecret {
    Password(SecretString),
    PrivateKey {
        private_key_path: String,
        passphrase: Option<SecretString>,
    },
    PrivateKeyPassphrase(SecretString),
    Token(SecretString),
}

/// Minimal master-password vault scaffold.
///
/// This is not persistent encryption yet. It models lock/unlock behavior and
/// keeps the master secret redacted/zeroized while future KDF/encryption design
/// is documented.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MasterPasswordVault {
    master: Option<SecretString>,
}

impl MasterPasswordVault {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn unlock(&mut self, master: SecretString) {
        self.master = Some(master);
    }

    pub fn lock(&mut self) {
        self.master = None;
    }

    #[must_use]
    pub fn is_unlocked(&self) -> bool {
        self.master.is_some()
    }

    pub fn require_unlocked(&self) -> Result<(), SecurityError> {
        if self.is_unlocked() {
            Ok(())
        } else {
            Err(SecurityError::VaultLocked)
        }
    }
}

/// Credential service interface.
pub trait CredentialService {
    fn store(
        &mut self,
        reference: CredentialRef,
        secret: CredentialSecret,
    ) -> Result<(), SecurityError>;
    fn load(&self, reference: &CredentialRef) -> Result<Option<CredentialSecret>, SecurityError>;
    fn delete(&mut self, reference: &CredentialRef) -> Result<bool, SecurityError>;
}

/// Non-persistent credential service for tests and local orchestration.
#[derive(Debug, Default)]
pub struct InMemoryCredentialService {
    secrets: BTreeMap<CredentialRef, CredentialSecret>,
}

impl InMemoryCredentialService {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl CredentialService for InMemoryCredentialService {
    fn store(
        &mut self,
        reference: CredentialRef,
        secret: CredentialSecret,
    ) -> Result<(), SecurityError> {
        self.secrets.insert(reference, secret);
        Ok(())
    }

    fn load(&self, reference: &CredentialRef) -> Result<Option<CredentialSecret>, SecurityError> {
        Ok(self.secrets.get(reference).cloned())
    }

    fn delete(&mut self, reference: &CredentialRef) -> Result<bool, SecurityError> {
        Ok(self.secrets.remove(reference).is_some())
    }
}

/// Security and credential errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SecurityError {
    InvalidCredentialRef,
    EmptySecret,
    BackendUnavailable,
    AccessDenied,
    VaultLocked,
}

impl fmt::Display for SecurityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCredentialRef => write!(formatter, "credential reference cannot be empty"),
            Self::EmptySecret => write!(formatter, "secret cannot be empty"),
            Self::BackendUnavailable => write!(formatter, "credential backend unavailable"),
            Self::AccessDenied => write!(formatter, "credential access denied"),
            Self::VaultLocked => write!(formatter, "credential vault is locked"),
        }
    }
}

impl std::error::Error for SecurityError {}

#[cfg(test)]
mod tests {
    use super::{
        CredentialRef, CredentialSecret, CredentialService, InMemoryCredentialService,
        MasterPasswordVault, SecretString, SecurityError,
    };

    #[test]
    fn secret_string_redacts_debug_and_display() {
        let secret = SecretString::new("hunter2").expect("secret should be valid");

        assert_eq!(secret.expose(), "hunter2");
        assert_eq!(format!("{secret}"), "REDACTED");
        assert_eq!(format!("{secret:?}"), "SecretString(REDACTED)");
        assert!(!format!("{secret:?}").contains("hunter2"));
    }

    #[test]
    fn credential_reference_rejects_empty_values() {
        assert!(CredentialRef::new("   ").is_err());
    }

    #[test]
    fn memory_credential_service_stores_loads_and_deletes_secrets() {
        let mut service = InMemoryCredentialService::new();
        let reference = CredentialRef::new("keychain://site-a").expect("valid ref");
        let secret =
            CredentialSecret::Password(SecretString::new("password").expect("valid secret"));

        service
            .store(reference.clone(), secret.clone())
            .expect("store secret");

        assert_eq!(service.load(&reference).expect("load secret"), Some(secret));
        assert!(service.delete(&reference).expect("delete secret"));
        assert!(service.load(&reference).expect("load deleted").is_none());
    }

    #[test]
    fn master_password_vault_tracks_locked_state() {
        let mut vault = MasterPasswordVault::new();

        assert_eq!(vault.require_unlocked(), Err(SecurityError::VaultLocked));

        vault.unlock(SecretString::new("master-password").expect("valid master"));
        assert!(vault.is_unlocked());
        assert_eq!(vault.require_unlocked(), Ok(()));

        vault.lock();
        assert!(!vault.is_unlocked());
    }
}
