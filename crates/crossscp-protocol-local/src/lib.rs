// SPDX-License-Identifier: AGPL-3.0-or-later

//! Local filesystem protocol adapter.
//!
//! This is the first concrete protocol adapter because local browsing is a P0
//! dependency for commander/explorer UI workflows and transfer testing.

use std::fmt;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crossscp_core::{
    CreateDirectoryOptions, OverwritePrompt, PromptBroker, PromptError, PromptRequest,
    PromptResponse, ProtocolCapabilities, RemoteFile, RemoteFileSystem, RemoveOptions,
    RenameOptions, SessionProfile, SessionProtocol,
};
use crossscp_transfer::{
    decide_overwrite, OverwriteDecision, OverwriteMode, TransferDirection, TransferFileMetadata,
    TransferJob, TransferProgress, TransferQueue, TransferQueueError, TransferState,
};

/// Error type for the local filesystem adapter.
#[derive(Debug)]
pub enum LocalFsError {
    InvalidProtocol(SessionProtocol),
    DestinationExists(PathBuf),
    UnsupportedOverwriteMode(OverwriteMode),
    Prompt(PromptError),
    Io(std::io::Error),
}

impl fmt::Display for LocalFsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProtocol(protocol) => {
                write!(
                    formatter,
                    "local filesystem adapter cannot connect to {protocol:?}"
                )
            }
            Self::DestinationExists(path) => {
                write!(formatter, "destination already exists: {}", path.display())
            }
            Self::UnsupportedOverwriteMode(mode) => {
                write!(formatter, "unsupported local overwrite mode: {mode:?}")
            }
            Self::Prompt(error) => write!(formatter, "prompt error: {error}"),
            Self::Io(error) => write!(formatter, "local filesystem error: {error}"),
        }
    }
}

impl std::error::Error for LocalFsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidProtocol(_)
            | Self::DestinationExists(_)
            | Self::UnsupportedOverwriteMode(_) => None,
            Self::Prompt(error) => Some(error),
            Self::Io(error) => Some(error),
        }
    }
}

impl From<std::io::Error> for LocalFsError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Local filesystem adapter rooted at an optional base directory.
#[derive(Debug, Default)]
pub struct LocalFileSystem {
    root: Option<PathBuf>,
    connected: bool,
}

impl LocalFileSystem {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self {
            root: Some(root.into()),
            connected: false,
        }
    }

    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    fn resolve(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() {
            return path.to_path_buf();
        }

        match &self.root {
            Some(root) => root.join(path),
            None => path.to_path_buf(),
        }
    }

    /// Copy a local file, creating the destination parent directory when requested.
    pub fn copy_file(
        &self,
        source: &str,
        destination: &str,
        create_missing_directories: bool,
    ) -> Result<u64, LocalFsError> {
        let source = self.resolve(source);
        let destination = self.resolve(destination);

        if create_missing_directories {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
        }

        Ok(fs::copy(source, destination)?)
    }

    /// Execute a local-to-local copy job using transfer option semantics.
    pub fn execute_local_copy_job(
        &self,
        job: &TransferJob,
    ) -> Result<TransferProgress, LocalFsError> {
        self.execute_local_copy_job_with_prompt(job, None)
    }

    /// Execute a local-to-local copy job, optionally using a prompt broker for
    /// interactive decisions such as `OverwriteMode::Ask`.
    pub fn execute_local_copy_job_with_prompt(
        &self,
        job: &TransferJob,
        prompt_broker: Option<&dyn PromptBroker>,
    ) -> Result<TransferProgress, LocalFsError> {
        if job.direction != TransferDirection::LocalCopy {
            return Err(LocalFsError::InvalidProtocol(SessionProtocol::Local));
        }

        let source = self.resolve(&job.source);
        let destination = self.resolve(&job.destination);
        let source_metadata = transfer_metadata(&source)?;
        let destination_metadata = if destination.exists() {
            Some(transfer_metadata(&destination)?)
        } else {
            None
        };

        if let Some(parent) = destination.parent() {
            if job.options.create_missing_directories {
                fs::create_dir_all(parent)?;
            }
        }

        let decision = decide_overwrite(
            job.options.overwrite_mode,
            source_metadata,
            destination_metadata,
        );

        let bytes_done = match decision {
            OverwriteDecision::CopyFromStart => fs::copy(source, &destination)?,
            OverwriteDecision::ResumeFrom(offset) => resume_copy(&source, &destination, offset)?,
            OverwriteDecision::Skip => 0,
            OverwriteDecision::Prompt => {
                let broker = prompt_broker.ok_or(LocalFsError::Prompt(PromptError::Unavailable))?;
                match broker
                    .prompt(PromptRequest::Overwrite(OverwritePrompt {
                        source: source.to_string_lossy().into_owned(),
                        destination: destination.to_string_lossy().into_owned(),
                        source_size: Some(source_metadata.size),
                        destination_size: destination_metadata.map(|metadata| metadata.size),
                    }))
                    .map_err(LocalFsError::Prompt)?
                {
                    PromptResponse::Accept
                    | PromptResponse::AcceptAll
                    | PromptResponse::RememberAccepted => fs::copy(source, &destination)?,
                    PromptResponse::Reject | PromptResponse::RejectAll => 0,
                    PromptResponse::Cancel => {
                        return Err(LocalFsError::Prompt(PromptError::Cancelled))
                    }
                }
            }
            OverwriteDecision::Unsupported => {
                return Err(LocalFsError::UnsupportedOverwriteMode(
                    job.options.overwrite_mode,
                ));
            }
        };

        Ok(TransferProgress {
            job_id: job.id,
            bytes_done,
            bytes_total: Some(source_metadata.size),
            current_file: Some(destination.to_string_lossy().into_owned()),
        })
    }

    /// Execute the first queued local-copy job and update its queue state.
    pub fn execute_next_local_copy(
        &self,
        queue: &mut TransferQueue,
    ) -> Result<Option<TransferProgress>, LocalTransferQueueError> {
        self.execute_next_local_copy_with_prompt(queue, None)
    }

    /// Execute the first queued local-copy job using an optional prompt broker.
    pub fn execute_next_local_copy_with_prompt(
        &self,
        queue: &mut TransferQueue,
        prompt_broker: Option<&dyn PromptBroker>,
    ) -> Result<Option<TransferProgress>, LocalTransferQueueError> {
        let Some(job_id) = queue.start_next() else {
            return Ok(None);
        };

        let job = queue
            .get(job_id)
            .cloned()
            .ok_or(TransferQueueError::UnknownJob(job_id))?;

        match self.execute_local_copy_job_with_prompt(&job, prompt_broker) {
            Ok(progress) => {
                queue.set_state(job_id, TransferState::Completed)?;
                Ok(Some(progress))
            }
            Err(error) => {
                let message = error.to_string();
                queue.set_state(job_id, TransferState::Failed(message))?;
                Err(LocalTransferQueueError::Local(error))
            }
        }
    }
}

fn transfer_metadata(path: &Path) -> Result<TransferFileMetadata, LocalFsError> {
    let metadata = fs::metadata(path)?;
    Ok(TransferFileMetadata {
        size: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn resume_copy(source: &Path, destination: &Path, offset: u64) -> Result<u64, LocalFsError> {
    let mut source_file = fs::File::open(source)?;
    source_file.seek(SeekFrom::Start(offset))?;

    let mut destination_file = fs::OpenOptions::new().append(true).open(destination)?;
    let mut buffer = [0_u8; 64 * 1024];
    let mut bytes_written = 0_u64;

    loop {
        let read = source_file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        destination_file.write_all(&buffer[..read])?;
        bytes_written += read as u64;
    }

    Ok(bytes_written)
}

/// Error returned while executing a transfer queue through the local adapter.
#[derive(Debug)]
pub enum LocalTransferQueueError {
    Local(LocalFsError),
    Queue(TransferQueueError),
}

impl fmt::Display for LocalTransferQueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(error) => write!(formatter, "{error}"),
            Self::Queue(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for LocalTransferQueueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Local(error) => Some(error),
            Self::Queue(error) => Some(error),
        }
    }
}

impl From<TransferQueueError> for LocalTransferQueueError {
    fn from(error: TransferQueueError) -> Self {
        Self::Queue(error)
    }
}

impl RemoteFileSystem for LocalFileSystem {
    type Error = LocalFsError;

    fn connect(&mut self, profile: &SessionProfile) -> Result<(), Self::Error> {
        if profile.protocol != SessionProtocol::Local {
            return Err(LocalFsError::InvalidProtocol(profile.protocol.clone()));
        }

        self.connected = true;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), Self::Error> {
        self.connected = false;
        Ok(())
    }

    fn list_directory(&self, path: &str) -> Result<Vec<RemoteFile>, Self::Error> {
        let directory = self.resolve(path);
        let mut entries = Vec::new();

        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let file_type = entry.file_type()?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();

            entries.push(RemoteFile {
                path: path.to_string_lossy().into_owned(),
                name,
                size: if metadata.is_file() {
                    Some(metadata.len())
                } else {
                    None
                },
                is_directory: metadata.is_dir(),
                is_symlink: file_type.is_symlink(),
                permissions: None,
            });
        }

        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    fn create_directory(
        &self,
        path: &str,
        options: CreateDirectoryOptions,
    ) -> Result<(), Self::Error> {
        let path = self.resolve(path);
        if options.create_parents {
            fs::create_dir_all(path)?;
        } else {
            fs::create_dir(path)?;
        }
        Ok(())
    }

    fn remove(&self, path: &str, options: RemoveOptions) -> Result<(), Self::Error> {
        let path = self.resolve(path);
        let metadata = fs::symlink_metadata(&path)?;

        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            if options.recursive {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_dir(path)?;
            }
        } else {
            fs::remove_file(path)?;
        }

        Ok(())
    }

    fn rename(&self, from: &str, to: &str, options: RenameOptions) -> Result<(), Self::Error> {
        let from = self.resolve(from);
        let to = self.resolve(to);

        if !options.overwrite && to.exists() {
            return Err(LocalFsError::DestinationExists(to));
        }

        if options.overwrite && to.exists() {
            let metadata = fs::symlink_metadata(&to)?;
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                fs::remove_dir_all(&to)?;
            } else {
                fs::remove_file(&to)?;
            }
        }

        fs::rename(from, to)?;
        Ok(())
    }

    fn capabilities(&self) -> ProtocolCapabilities {
        ProtocolCapabilities {
            can_list: true,
            can_upload: true,
            can_download: true,
            can_delete: true,
            can_mkdir: true,
            can_rename: true,
            can_recursive_transfer: true,
            can_resume_upload: true,
            can_resume_download: true,
            can_preserve_timestamps: true,
            can_preserve_permissions: cfg!(unix),
            can_chmod: cfg!(unix),
            can_symlink: true,
            can_hash: false,
            can_server_side_copy: true,
            supports_random_access: true,
            uses_real_directories: true,
            uses_object_prefixes: false,
            requires_bucket: false,
            supports_tls_policy: false,
            supports_http_version_policy: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::LocalFileSystem;
    use crossscp_core::{
        CreateDirectoryOptions, FixedPromptBroker, RemoteFileSystem, RemoveOptions, RenameOptions,
        SessionProfile,
    };
    use crossscp_transfer::{
        OverwriteMode, TransferDirection, TransferOptions, TransferQueue, TransferState,
    };

    #[test]
    fn local_adapter_connects_only_to_local_profiles() {
        let mut adapter = LocalFileSystem::new();

        adapter
            .connect(&SessionProfile::local("local"))
            .expect("local profile should connect");

        assert!(adapter.is_connected());
    }

    #[test]
    fn local_adapter_lists_directory_entries_sorted_by_name() {
        let root = unique_temp_dir("crossscp-local-list");
        fs::write(root.join("b.txt"), b"b").expect("write b");
        fs::write(root.join("a.txt"), b"a").expect("write a");
        fs::create_dir(root.join("folder")).expect("create folder");

        let adapter = LocalFileSystem::with_root(&root);
        let entries = adapter.list_directory(".").expect("list directory");
        let names: Vec<_> = entries.into_iter().map(|entry| entry.name).collect();

        assert_eq!(names, vec!["a.txt", "b.txt", "folder"]);

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn local_adapter_creates_directories_with_optional_parents() {
        let root = unique_temp_dir("crossscp-local-create");
        let adapter = LocalFileSystem::with_root(&root);

        adapter
            .create_directory(
                "nested/path",
                CreateDirectoryOptions {
                    create_parents: true,
                },
            )
            .expect("create nested path");

        assert!(root.join("nested/path").is_dir());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn local_adapter_renames_without_overwriting_by_default() {
        let root = unique_temp_dir("crossscp-local-rename");
        fs::write(root.join("source.txt"), b"source").expect("write source");
        fs::write(root.join("target.txt"), b"target").expect("write target");
        let adapter = LocalFileSystem::with_root(&root);

        let result = adapter.rename(
            "source.txt",
            "target.txt",
            RenameOptions { overwrite: false },
        );

        assert!(result.is_err());
        assert_eq!(
            fs::read(root.join("target.txt")).expect("read target"),
            b"target"
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn local_adapter_renames_with_overwrite_when_requested() {
        let root = unique_temp_dir("crossscp-local-rename-overwrite");
        fs::write(root.join("source.txt"), b"source").expect("write source");
        fs::write(root.join("target.txt"), b"target").expect("write target");
        let adapter = LocalFileSystem::with_root(&root);

        adapter
            .rename(
                "source.txt",
                "target.txt",
                RenameOptions { overwrite: true },
            )
            .expect("rename with overwrite");

        assert_eq!(
            fs::read(root.join("target.txt")).expect("read target"),
            b"source"
        );
        assert!(!root.join("source.txt").exists());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn local_adapter_removes_files_and_recursive_directories() {
        let root = unique_temp_dir("crossscp-local-remove");
        fs::write(root.join("file.txt"), b"file").expect("write file");
        fs::create_dir_all(root.join("dir/child")).expect("create dir");
        fs::write(root.join("dir/child/file.txt"), b"file").expect("write nested file");
        let adapter = LocalFileSystem::with_root(&root);

        adapter
            .remove("file.txt", RemoveOptions { recursive: false })
            .expect("remove file");
        adapter
            .remove("dir", RemoveOptions { recursive: true })
            .expect("remove dir");

        assert!(!root.join("file.txt").exists());
        assert!(!root.join("dir").exists());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn local_adapter_copies_files_and_creates_destination_parents() {
        let root = unique_temp_dir("crossscp-local-copy");
        fs::write(root.join("source.txt"), b"copy me").expect("write source");
        let adapter = LocalFileSystem::with_root(&root);

        let bytes = adapter
            .copy_file("source.txt", "nested/target.txt", true)
            .expect("copy file");

        assert_eq!(bytes, 7);
        assert_eq!(
            fs::read(root.join("nested/target.txt")).expect("read target"),
            b"copy me"
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn local_transfer_executor_completes_queued_local_copy() {
        let root = unique_temp_dir("crossscp-local-transfer-complete");
        fs::write(root.join("source.txt"), b"queued copy").expect("write source");
        let adapter = LocalFileSystem::with_root(&root);
        let mut queue = TransferQueue::new();
        let id = queue.enqueue(
            TransferDirection::LocalCopy,
            "source.txt",
            "out/target.txt",
            TransferOptions {
                overwrite_mode: OverwriteMode::Always,
                ..TransferOptions::default()
            },
        );

        let progress = adapter
            .execute_next_local_copy(&mut queue)
            .expect("execute local copy")
            .expect("job should run");

        assert_eq!(progress.job_id, id);
        assert_eq!(progress.bytes_done, 11);
        assert_eq!(
            queue.get(id).expect("job exists").state,
            TransferState::Completed
        );
        assert_eq!(
            fs::read(root.join("out/target.txt")).expect("read target"),
            b"queued copy"
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn local_transfer_executor_skips_queue_job_when_destination_exists_and_never_overwrite() {
        let root = unique_temp_dir("crossscp-local-transfer-fail");
        fs::write(root.join("source.txt"), b"new").expect("write source");
        fs::write(root.join("target.txt"), b"old").expect("write target");
        let adapter = LocalFileSystem::with_root(&root);
        let mut queue = TransferQueue::new();
        let id = queue.enqueue(
            TransferDirection::LocalCopy,
            "source.txt",
            "target.txt",
            TransferOptions {
                overwrite_mode: OverwriteMode::Never,
                ..TransferOptions::default()
            },
        );

        let progress = adapter
            .execute_next_local_copy(&mut queue)
            .expect("never overwrite should skip")
            .expect("job should run");

        assert_eq!(progress.bytes_done, 0);
        assert!(matches!(
            queue.get(id).expect("job exists").state,
            TransferState::Completed
        ));
        assert_eq!(
            fs::read(root.join("target.txt")).expect("read target"),
            b"old"
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn local_transfer_executor_returns_none_for_empty_queue() {
        let root = unique_temp_dir("crossscp-local-transfer-empty");
        let adapter = LocalFileSystem::with_root(&root);
        let mut queue = TransferQueue::new();

        let progress = adapter
            .execute_next_local_copy(&mut queue)
            .expect("empty queue should not fail");

        assert!(progress.is_none());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn local_transfer_executor_ask_overwrite_accepts_via_prompt_broker() {
        let root = unique_temp_dir("crossscp-local-transfer-ask-accept");
        fs::write(root.join("source.txt"), b"new").expect("write source");
        fs::write(root.join("target.txt"), b"old").expect("write target");
        let adapter = LocalFileSystem::with_root(&root);
        let broker = FixedPromptBroker::accept();
        let mut queue = TransferQueue::new();
        let id = queue.enqueue(
            TransferDirection::LocalCopy,
            "source.txt",
            "target.txt",
            TransferOptions {
                overwrite_mode: OverwriteMode::Ask,
                ..TransferOptions::default()
            },
        );

        let progress = adapter
            .execute_next_local_copy_with_prompt(&mut queue, Some(&broker))
            .expect("ask overwrite accepted")
            .expect("job should run");

        assert_eq!(progress.bytes_done, 3);
        assert_eq!(
            queue.get(id).expect("job exists").state,
            TransferState::Completed
        );
        assert_eq!(
            fs::read(root.join("target.txt")).expect("read target"),
            b"new"
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn local_transfer_executor_ask_overwrite_rejects_via_prompt_broker() {
        let root = unique_temp_dir("crossscp-local-transfer-ask-reject");
        fs::write(root.join("source.txt"), b"new").expect("write source");
        fs::write(root.join("target.txt"), b"old").expect("write target");
        let adapter = LocalFileSystem::with_root(&root);
        let broker = FixedPromptBroker::reject();
        let mut queue = TransferQueue::new();
        let id = queue.enqueue(
            TransferDirection::LocalCopy,
            "source.txt",
            "target.txt",
            TransferOptions {
                overwrite_mode: OverwriteMode::Ask,
                ..TransferOptions::default()
            },
        );

        let progress = adapter
            .execute_next_local_copy_with_prompt(&mut queue, Some(&broker))
            .expect("ask overwrite rejected")
            .expect("job should run");

        assert_eq!(progress.bytes_done, 0);
        assert_eq!(
            queue.get(id).expect("job exists").state,
            TransferState::Completed
        );
        assert_eq!(
            fs::read(root.join("target.txt")).expect("read target"),
            b"old"
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn local_transfer_executor_ask_overwrite_without_broker_fails_job() {
        let root = unique_temp_dir("crossscp-local-transfer-ask-missing-broker");
        fs::write(root.join("source.txt"), b"new").expect("write source");
        fs::write(root.join("target.txt"), b"old").expect("write target");
        let adapter = LocalFileSystem::with_root(&root);
        let mut queue = TransferQueue::new();
        let id = queue.enqueue(
            TransferDirection::LocalCopy,
            "source.txt",
            "target.txt",
            TransferOptions {
                overwrite_mode: OverwriteMode::Ask,
                ..TransferOptions::default()
            },
        );

        let result = adapter.execute_next_local_copy(&mut queue);

        assert!(result.is_err());
        assert!(matches!(
            queue.get(id).expect("job exists").state,
            TransferState::Failed(_)
        ));
        assert_eq!(
            fs::read(root.join("target.txt")).expect("read target"),
            b"old"
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn local_transfer_executor_resumes_when_destination_is_shorter() {
        let root = unique_temp_dir("crossscp-local-transfer-resume");
        fs::write(root.join("source.txt"), b"hello world").expect("write source");
        fs::write(root.join("target.txt"), b"hello").expect("write partial target");
        let adapter = LocalFileSystem::with_root(&root);
        let mut queue = TransferQueue::new();
        let id = queue.enqueue(
            TransferDirection::LocalCopy,
            "source.txt",
            "target.txt",
            TransferOptions {
                overwrite_mode: OverwriteMode::Resume,
                ..TransferOptions::default()
            },
        );

        let progress = adapter
            .execute_next_local_copy(&mut queue)
            .expect("resume local copy")
            .expect("job should run");

        assert_eq!(progress.job_id, id);
        assert_eq!(progress.bytes_done, 6);
        assert_eq!(progress.bytes_total, Some(11));
        assert_eq!(
            fs::read(root.join("target.txt")).expect("read target"),
            b"hello world"
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn local_transfer_executor_skips_resume_when_destination_is_complete() {
        let root = unique_temp_dir("crossscp-local-transfer-resume-skip");
        fs::write(root.join("source.txt"), b"complete").expect("write source");
        fs::write(root.join("target.txt"), b"complete").expect("write target");
        let adapter = LocalFileSystem::with_root(&root);
        let mut queue = TransferQueue::new();
        let id = queue.enqueue(
            TransferDirection::LocalCopy,
            "source.txt",
            "target.txt",
            TransferOptions {
                overwrite_mode: OverwriteMode::Resume,
                ..TransferOptions::default()
            },
        );

        let progress = adapter
            .execute_next_local_copy(&mut queue)
            .expect("resume skip should not fail")
            .expect("job should run");

        assert_eq!(progress.bytes_done, 0);
        assert_eq!(
            queue.get(id).expect("job exists").state,
            TransferState::Completed
        );
        assert_eq!(
            fs::read(root.join("target.txt")).expect("read target"),
            b"complete"
        );

        fs::remove_dir_all(root).expect("cleanup");
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
