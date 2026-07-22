// SPDX-License-Identifier: AGPL-3.0-or-later

//! Protocol-neutral traits, operation models, and remote file metadata.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
    pub can_list: bool,
    pub can_upload: bool,
    pub can_download: bool,
    pub can_delete: bool,
    pub can_mkdir: bool,
    pub can_rename: bool,
    pub can_recursive_transfer: bool,
    pub can_resume_upload: bool,
    pub can_resume_download: bool,
    pub can_preserve_timestamps: bool,
    pub can_preserve_permissions: bool,
    pub can_chmod: bool,
    pub can_symlink: bool,
    pub can_hash: bool,
    pub can_server_side_copy: bool,
    pub supports_random_access: bool,
    pub uses_real_directories: bool,
    pub uses_object_prefixes: bool,
    pub requires_bucket: bool,
    pub supports_tls_policy: bool,
    pub supports_http_version_policy: bool,
}

impl ProtocolCapabilities {
    #[must_use]
    pub const fn sftp() -> Self {
        Self {
            can_list: true,
            can_upload: true,
            can_download: true,
            can_delete: true,
            can_mkdir: true,
            can_rename: false,
            can_recursive_transfer: true,
            can_resume_upload: false,
            can_resume_download: false,
            can_preserve_timestamps: false,
            can_preserve_permissions: false,
            can_chmod: false,
            can_symlink: false,
            can_hash: false,
            can_server_side_copy: false,
            supports_random_access: false,
            uses_real_directories: true,
            uses_object_prefixes: false,
            requires_bucket: false,
            supports_tls_policy: false,
            supports_http_version_policy: false,
        }
    }

    #[must_use]
    pub const fn scp_transfer_only() -> Self {
        Self {
            can_upload: true,
            can_download: true,
            can_recursive_transfer: false,
            uses_real_directories: true,
            ..Self::empty()
        }
    }

    #[must_use]
    pub const fn ftp_like(supports_tls_policy: bool) -> Self {
        Self {
            can_list: true,
            can_upload: true,
            can_download: true,
            can_delete: true,
            can_mkdir: true,
            can_rename: true,
            can_recursive_transfer: true,
            uses_real_directories: true,
            supports_tls_policy,
            ..Self::empty()
        }
    }

    #[must_use]
    pub const fn webdav() -> Self {
        Self {
            can_list: true,
            can_upload: true,
            can_download: true,
            can_delete: true,
            can_mkdir: true,
            can_rename: true,
            can_recursive_transfer: true,
            uses_real_directories: true,
            supports_tls_policy: true,
            supports_http_version_policy: true,
            ..Self::empty()
        }
    }

    #[must_use]
    pub const fn s3() -> Self {
        Self {
            can_list: true,
            can_upload: true,
            can_download: true,
            can_delete: true,
            can_mkdir: true,
            can_rename: true,
            can_recursive_transfer: true,
            can_server_side_copy: true,
            uses_object_prefixes: true,
            requires_bucket: true,
            supports_tls_policy: true,
            supports_http_version_policy: true,
            ..Self::empty()
        }
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self {
            can_list: false,
            can_upload: false,
            can_download: false,
            can_delete: false,
            can_mkdir: false,
            can_rename: false,
            can_recursive_transfer: false,
            can_resume_upload: false,
            can_resume_download: false,
            can_preserve_timestamps: false,
            can_preserve_permissions: false,
            can_chmod: false,
            can_symlink: false,
            can_hash: false,
            can_server_side_copy: false,
            supports_random_access: false,
            uses_real_directories: false,
            uses_object_prefixes: false,
            requires_bucket: false,
            supports_tls_policy: false,
            supports_http_version_policy: false,
        }
    }
}

/// Protocol-neutral error categories suitable for CLI/GUI routing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RemoteErrorKind {
    AuthFailed,
    NetworkUnreachable,
    Timeout,
    TlsCertificate,
    HostKeyTrust,
    PermissionDenied,
    NotFound,
    ConflictAlreadyExists,
    UnsupportedOperation,
    RateLimited,
    ServiceUnavailable,
    UnknownBackend,
}

/// Supported high-level remote operation kinds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RemoteOperation {
    List,
    Upload,
    Download,
    Mkdir,
    Delete,
    Rename,
    Capabilities,
}

/// Action to take when an upload target already exists.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FileConflictPolicy {
    /// Keep the destination item and skip the conflicting source file.
    KeepExisting,
    /// Replace conflicting files and merge matching directories.
    #[default]
    Replace,
    /// Keep both items by adding an incrementing number to the new name.
    KeepBoth,
}

/// Return a numbered sibling path used by [`FileConflictPolicy::KeepBoth`].
///
/// File extensions are preserved (`report (1).pdf`) while directory names are
/// suffixed directly (`photos (1)`).
#[must_use]
pub fn numbered_conflict_path(path: &str, copy_number: u32, is_directory: bool) -> String {
    let copy_number = copy_number.max(1);
    let normalized_path = if path == "/" {
        path
    } else {
        path.trim_end_matches('/')
    };
    let (parent, name) = normalized_path
        .rsplit_once('/')
        .map_or(("", normalized_path), |(parent, name)| (parent, name));
    let numbered_name = if is_directory {
        format!("{name} ({copy_number})")
    } else if let Some((stem, extension)) =
        name.rsplit_once('.').filter(|(stem, _)| !stem.is_empty())
    {
        format!("{stem} ({copy_number}).{extension}")
    } else {
        format!("{name} ({copy_number})")
    };

    if parent.is_empty() {
        if normalized_path.starts_with('/') {
            format!("/{numbered_name}")
        } else {
            numbered_name
        }
    } else {
        format!("{parent}/{numbered_name}")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteListRequest {
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteTransferRequest {
    pub local_path: String,
    pub remote_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteMkdirRequest {
    pub remote_path: String,
    pub create_parents: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteDeleteRequest {
    pub remote_path: String,
    pub recursive: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteRenameRequest {
    pub from: String,
    pub to: String,
    pub overwrite: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RemoteOperationResult {
    Entries(Vec<RemoteFile>),
    Transferred {
        bytes_done: u64,
        bytes_total: Option<u64>,
    },
    Completed,
    Capabilities(ProtocolCapabilities),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RemoteProgressEvent {
    Started {
        operation: RemoteOperation,
    },
    Bytes {
        done: u64,
        total: Option<u64>,
    },
    CurrentFile {
        path: String,
    },
    Completed,
    Failed {
        kind: RemoteErrorKind,
        message: String,
    },
    Cancelled,
}

#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
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

#[cfg(test)]
mod tests {
    use super::numbered_conflict_path;

    #[test]
    fn numbered_conflict_paths_preserve_file_extensions() {
        assert_eq!(
            numbered_conflict_path("/uploads/report.pdf", 1, false),
            "/uploads/report (1).pdf"
        );
        assert_eq!(
            numbered_conflict_path("/uploads/archive.tar.gz", 2, false),
            "/uploads/archive.tar (2).gz"
        );
        assert_eq!(
            numbered_conflict_path("/uploads/.env", 1, false),
            "/uploads/.env (1)"
        );
    }

    #[test]
    fn numbered_conflict_paths_suffix_directories() {
        assert_eq!(
            numbered_conflict_path("/uploads/photos", 3, true),
            "/uploads/photos (3)"
        );
        assert_eq!(
            numbered_conflict_path("/uploads/photos/", 1, true),
            "/uploads/photos (1)"
        );
    }
}
