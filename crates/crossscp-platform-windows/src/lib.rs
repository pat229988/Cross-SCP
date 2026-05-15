// SPDX-License-Identifier: AGPL-3.0-or-later

//! Windows platform service scaffolds.
//!
//! Real Credential Manager / DPAPI integration is intentionally not implemented
//! in this phase so platform APIs stay isolated behind reviewed traits.

use crossscp_security::{CredentialRef, CredentialSecret, CredentialService, SecurityError};

/// Placeholder for a future Windows Credential Manager / DPAPI-backed service.
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsCredentialService;

impl WindowsCredentialService {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl CredentialService for WindowsCredentialService {
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

    use super::WindowsCredentialService;

    #[test]
    fn windows_credential_scaffold_reports_unavailable() {
        let service = WindowsCredentialService::new();
        let reference = CredentialRef::new("wincred://site").expect("valid ref");

        assert!(service.load(&reference).is_err());
    }
}
