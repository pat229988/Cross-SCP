// SPDX-License-Identifier: AGPL-3.0-or-later

//! Prompt broker contracts for UI/CLI-independent user decisions.
//!
//! Protocol adapters and transfer executors must not directly depend on Qt,
//! terminal input, or platform dialogs. Instead, they ask a broker supplied by
//! the caller.

use std::fmt;

/// Request to confirm replacement of an existing destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverwritePrompt {
    pub source: String,
    pub destination: String,
    pub source_size: Option<u64>,
    pub destination_size: Option<u64>,
}

/// Request for a credential secret. Actual secret values should be returned via
/// future secret wrapper types; this phase models prompt control flow only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialPrompt {
    pub profile_name: String,
    pub username: Option<String>,
    pub message: String,
}

/// Request to verify a remote host key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostKeyPrompt {
    pub host: String,
    pub fingerprint: String,
    pub algorithm: String,
    pub expected_fingerprint: Option<String>,
    pub expected_algorithm: Option<String>,
}

/// Prompt request variants shared by protocol adapters and transfer executors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PromptRequest {
    Overwrite(OverwritePrompt),
    Credential(CredentialPrompt),
    HostKey(HostKeyPrompt),
    Confirm { title: String, message: String },
}

/// User/broker response to a prompt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PromptResponse {
    Accept,
    Reject,
    Cancel,
    AcceptAll,
    RejectAll,
    RememberAccepted,
}

/// Prompt broker failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PromptError {
    Unavailable,
    Cancelled,
    UnsupportedPrompt,
}

impl fmt::Display for PromptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => write!(formatter, "prompt broker unavailable"),
            Self::Cancelled => write!(formatter, "prompt cancelled"),
            Self::UnsupportedPrompt => write!(formatter, "prompt type unsupported"),
        }
    }
}

impl std::error::Error for PromptError {}

/// UI/CLI-independent prompt broker.
pub trait PromptBroker {
    fn prompt(&self, request: PromptRequest) -> Result<PromptResponse, PromptError>;
}

/// Non-interactive broker useful for tests and unattended operations.
#[derive(Clone, Debug)]
pub struct FixedPromptBroker {
    response: Result<PromptResponse, PromptError>,
}

impl FixedPromptBroker {
    #[must_use]
    pub fn accept() -> Self {
        Self {
            response: Ok(PromptResponse::Accept),
        }
    }

    #[must_use]
    pub fn reject() -> Self {
        Self {
            response: Ok(PromptResponse::Reject),
        }
    }

    #[must_use]
    pub fn cancel() -> Self {
        Self {
            response: Ok(PromptResponse::Cancel),
        }
    }
}

impl PromptBroker for FixedPromptBroker {
    fn prompt(&self, _request: PromptRequest) -> Result<PromptResponse, PromptError> {
        self.response.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{FixedPromptBroker, PromptBroker, PromptRequest, PromptResponse};

    #[test]
    fn fixed_prompt_broker_returns_configured_response() {
        let broker = FixedPromptBroker::accept();

        assert_eq!(
            broker.prompt(PromptRequest::Confirm {
                title: "Confirm".to_string(),
                message: "Continue?".to_string(),
            }),
            Ok(PromptResponse::Accept)
        );
    }
}
