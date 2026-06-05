// SPDX-License-Identifier: AGPL-3.0-or-later

//! Clean-room CrossSCP core domain models and behavior helpers.
//!
//! This crate must remain UI- and platform-API-free. legacy SFTP client paths may be cited
//! in tests/specs as behavioral references, but production code must not copy
//! legacy SFTP client source.

pub mod masks;
pub mod prompts;
pub mod protocol;
pub mod session;

pub use masks::{FileMask, MaskDecision, MaskSet};
pub use prompts::{
    CredentialPrompt, FixedPromptBroker, HostKeyPrompt, OverwritePrompt, PromptBroker, PromptError,
    PromptRequest, PromptResponse,
};
pub use protocol::{
    CancellationToken, CreateDirectoryOptions, Protocol, ProtocolCapabilities, RemoteDeleteRequest,
    RemoteErrorKind, RemoteFile, RemoteFileSystem, RemoteListRequest, RemoteMkdirRequest,
    RemoteOperation, RemoteOperationResult, RemoteProgressEvent, RemoteRenameRequest,
    RemoteTransferRequest, RemoveOptions, RenameOptions,
};
pub use session::{ParseSessionProtocolError, SessionProfile, SessionProtocol};
