// SPDX-License-Identifier: AGPL-3.0-or-later

//! Protocol-neutral traits and remote file metadata.

use crate::session::SessionProfile;

/// Runtime protocol adapter kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Protocol {
    Sftp,
    Scp,
    Ftp,
    Ftps,
    WebDav,
    S3,
    Local,
}

/// Capabilities advertised by a protocol adapter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProtocolCapabilities {
    pub can_resume: bool,
    pub can_preserve_timestamps: bool,
    pub can_chmod: bool,
    pub can_symlink: bool,
    pub can_hash: bool,
}

/// Minimal protocol-neutral remote file model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteFile {
    pub path: String,
    pub name: String,
    pub size: Option<u64>,
    pub is_directory: bool,
    pub is_symlink: bool,
    pub permissions: Option<String>,
}

/// Options for creating a directory.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CreateDirectoryOptions {
    pub create_parents: bool,
}

/// Options for removing files/directories.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RemoveOptions {
    pub recursive: bool,
}

/// Options for renaming/moving files.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenameOptions {
    pub overwrite: bool,
}

/// Synchronous placeholder for the eventual async protocol abstraction.
///
/// The implementation plan requires async protocol adapters. This trait is a
/// dependency-free scaffold so domain models and tests can start before the
/// async runtime/trait crate decision is finalized and recorded.
pub trait RemoteFileSystem {
    type Error;

    fn connect(&mut self, profile: &SessionProfile) -> Result<(), Self::Error>;
    fn disconnect(&mut self) -> Result<(), Self::Error>;
    fn list_directory(&self, path: &str) -> Result<Vec<RemoteFile>, Self::Error>;
    fn create_directory(
        &self,
        path: &str,
        options: CreateDirectoryOptions,
    ) -> Result<(), Self::Error>;
    fn remove(&self, path: &str, options: RemoveOptions) -> Result<(), Self::Error>;
    fn rename(&self, from: &str, to: &str, options: RenameOptions) -> Result<(), Self::Error>;
    fn capabilities(&self) -> ProtocolCapabilities;
}
