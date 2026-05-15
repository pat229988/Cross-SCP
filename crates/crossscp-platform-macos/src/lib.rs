// SPDX-License-Identifier: AGPL-3.0-or-later

//! macOS platform service scaffolds.
//!
//! Real Keychain integration is intentionally not implemented in this phase so
//! platform APIs do not leak into core crates before design review.

use crossscp_security::{CredentialRef, CredentialSecret, CredentialService, SecurityError};

/// Placeholder for a future macOS Keychain-backed credential service.
#[derive(Clone, Copy, Debug, Default)]
pub struct MacOsKeychainCredentialService;

impl MacOsKeychainCredentialService {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl CredentialService for MacOsKeychainCredentialService {
    fn store(
        &mut self,
        _reference: CredentialRef,
        _secret: CredentialSecret,
    ) -> Result<(), SecurityError> {
        Err(SecurityError::BackendUnavailable)
    }

    fn load(&self, _reference: &CredentialRef) -> Result<Option<CredentialSecret>, SecurityError> {
        Err(SecurityError::BackendUnavailable)
    }

    fn delete(&mut self, _reference: &CredentialRef) -> Result<bool, SecurityError> {
        Err(SecurityError::BackendUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use crossscp_security::{CredentialRef, CredentialService};

    use super::MacOsKeychainCredentialService;

    #[test]
    fn macos_keychain_scaffold_reports_unavailable() {
        let service = MacOsKeychainCredentialService::new();
        let reference = CredentialRef::new("keychain://site").expect("valid ref");

        assert!(service.load(&reference).is_err());
    }
}
