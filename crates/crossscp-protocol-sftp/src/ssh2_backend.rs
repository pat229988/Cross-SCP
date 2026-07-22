// SPDX-License-Identifier: AGPL-3.0-or-later

//! Optional `ssh2`/libssh2 backend candidate for the first live SFTP POC.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossscp_core::{numbered_conflict_path, FileConflictPolicy};
use crossscp_security::CredentialSecret;

use crate::{
    SftpAuthMaterial, SftpBackend, SftpConnectionConfig, SftpError, SftpFileProgress,
    SftpRemoteFile,
};

/// Large streaming buffer selected from CrossSCP transfer-speed experiments.
///
/// A 2 GiB upload through the jump-host test path improved from about 88 MiB/s
/// with 256 KiB to about 109 MiB/s with 8 MiB, nearly matching OpenSSH scp.
const SFTP_TRANSFER_BUFFER_SIZE: usize = 8 * 1024 * 1024;

/// Keep GUI/CLI progress responsive without emitting one event per copy buffer.
const SFTP_PROGRESS_INTERVAL_MS: u64 = 250;

/// Feature-gated backend using the Rust `ssh2` crate over libssh2.
pub struct Ssh2Backend {
    auth: SftpAuthMaterial,
    timeout: Duration,
    session: Option<ssh2::Session>,
}

impl Ssh2Backend {
    #[must_use]
    pub fn new(auth: SftpAuthMaterial) -> Self {
        Self {
            auth,
            timeout: Duration::from_secs(300),
            session: None,
        }
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn upload_file_with_progress<F>(
        &mut self,
        local_path: &str,
        remote_path: &str,
        report_progress: F,
    ) -> Result<SftpFileProgress, SftpError>
    where
        F: FnMut(u64, Option<u64>),
    {
        self.upload_file_with_progress_policy(
            local_path,
            remote_path,
            FileConflictPolicy::Replace,
            report_progress,
        )
    }

    pub fn upload_file_with_progress_policy<F>(
        &mut self,
        local_path: &str,
        remote_path: &str,
        conflict_policy: FileConflictPolicy,
        report_progress: F,
    ) -> Result<SftpFileProgress, SftpError>
    where
        F: FnMut(u64, Option<u64>),
    {
        self.upload_file_with_progress_policy_impl(
            local_path,
            remote_path,
            conflict_policy,
            false,
            report_progress,
        )
    }

    pub fn upload_file_to_exact_destination_with_progress<F>(
        &mut self,
        local_path: &str,
        remote_path: &str,
        conflict_policy: FileConflictPolicy,
        report_progress: F,
    ) -> Result<SftpFileProgress, SftpError>
    where
        F: FnMut(u64, Option<u64>),
    {
        self.upload_file_with_progress_policy_impl(
            local_path,
            remote_path,
            conflict_policy,
            true,
            report_progress,
        )
    }

    fn upload_file_with_progress_policy_impl<F>(
        &mut self,
        local_path: &str,
        remote_path: &str,
        conflict_policy: FileConflictPolicy,
        exact_destination: bool,
        mut report_progress: F,
    ) -> Result<SftpFileProgress, SftpError>
    where
        F: FnMut(u64, Option<u64>),
    {
        let session = self.session.as_ref().ok_or(SftpError::NotConnected)?;
        let local_metadata = fs::metadata(local_path)?;
        let sftp = session.sftp()?;
        let remote_path = normalize_remote_path(&sftp, remote_path);
        let requested_destination = if exact_destination {
            remote_path
        } else {
            resolve_upload_destination(&sftp, local_path, &remote_path, local_metadata.is_dir())?
        };
        if local_metadata.is_dir() {
            let bytes_total = local_directory_size(Path::new(local_path))?;
            let Some(destination) = resolve_upload_conflict_destination(
                &sftp,
                &requested_destination,
                true,
                conflict_policy,
            )?
            else {
                report_progress(bytes_total, Some(bytes_total));
                return Ok(SftpFileProgress {
                    source: local_path.to_string(),
                    destination: requested_destination,
                    bytes_done: 0,
                    bytes_total: Some(bytes_total),
                });
            };
            let mut bytes_transferred = 0;
            let mut bytes_processed = 0;
            report_progress(0, Some(bytes_total));
            let mut progress = DirectoryUploadProgress {
                bytes_total,
                bytes_transferred: &mut bytes_transferred,
                bytes_processed: &mut bytes_processed,
                report: &mut report_progress,
            };
            upload_directory_recursive_with_progress(
                &sftp,
                Path::new(local_path),
                &destination,
                &mut progress,
                conflict_policy,
            )?;
            return Ok(SftpFileProgress {
                source: local_path.to_string(),
                destination,
                bytes_done: bytes_transferred,
                bytes_total: Some(bytes_total),
            });
        }
        let bytes_total = local_metadata.len();
        let Some((destination, mut remote_file)) =
            create_remote_file_for_policy(&sftp, &requested_destination, conflict_policy)?
        else {
            report_progress(bytes_total, Some(bytes_total));
            return Ok(SftpFileProgress {
                source: local_path.to_string(),
                destination: requested_destination,
                bytes_done: 0,
                bytes_total: Some(bytes_total),
            });
        };
        let mut local_file = fs::File::open(local_path)?;
        let bytes_done = copy_with_progress(
            &mut local_file,
            &mut remote_file,
            Some(bytes_total),
            &mut report_progress,
        )?;

        Ok(SftpFileProgress {
            source: local_path.to_string(),
            destination,
            bytes_done,
            bytes_total: Some(bytes_total),
        })
    }

    pub fn download_file_with_progress<F>(
        &mut self,
        remote_path: &str,
        local_path: &str,
        mut report_progress: F,
    ) -> Result<SftpFileProgress, SftpError>
    where
        F: FnMut(u64, Option<u64>),
    {
        let session = self.session.as_ref().ok_or(SftpError::NotConnected)?;
        let sftp = session.sftp()?;
        let remote_path = normalize_remote_path(&sftp, remote_path);
        let remote_stat = sftp.stat(Path::new(&remote_path)).ok();
        if remote_stat
            .as_ref()
            .and_then(|stat| stat.perm)
            .is_some_and(is_directory_perm)
        {
            let destination = resolve_download_directory_destination(&remote_path, local_path);
            let bytes_total = remote_directory_size(&sftp, &remote_path)?;
            let mut bytes_done = 0;
            report_progress(0, Some(bytes_total));
            download_directory_recursive_with_progress(
                &sftp,
                &remote_path,
                Path::new(&destination),
                bytes_total,
                &mut bytes_done,
                &mut report_progress,
            )?;
            return Ok(SftpFileProgress {
                source: remote_path,
                destination,
                bytes_done,
                bytes_total: Some(bytes_total),
            });
        }
        if let Some(parent) = Path::new(local_path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let mut remote_file = sftp.open(Path::new(&remote_path))?;
        let mut local_file = fs::File::create(local_path)?;
        let bytes_total = remote_stat.and_then(|stat| stat.size);
        let bytes_done = copy_with_progress(
            &mut remote_file,
            &mut local_file,
            bytes_total,
            &mut report_progress,
        )?;

        Ok(SftpFileProgress {
            source: remote_path,
            destination: local_path.to_string(),
            bytes_done,
            bytes_total,
        })
    }
}

impl SftpBackend for Ssh2Backend {
    fn connect(&mut self, config: &SftpConnectionConfig) -> Result<(), SftpError> {
        let address = (config.host.as_str(), config.port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| {
                SftpError::Backend(format!(
                    "no socket address for {}:{}",
                    config.host, config.port
                ))
            })?;
        let tcp = TcpStream::connect_timeout(&address, self.timeout)?;
        tcp.set_read_timeout(Some(self.timeout))?;
        tcp.set_write_timeout(Some(self.timeout))?;

        let mut session = ssh2::Session::new()?;
        session.set_tcp_stream(tcp);
        session.handshake()?;

        let username = self
            .auth
            .username
            .as_deref()
            .or(config.username.as_deref())
            .ok_or(SftpError::MissingUsername)?;

        match &self.auth.secret {
            CredentialSecret::Password(password) => {
                session.userauth_password(username, password.expose())?;
            }
            CredentialSecret::PrivateKey {
                private_key_path,
                passphrase,
            } => {
                session.userauth_pubkey_file(
                    username,
                    None,
                    Path::new(private_key_path),
                    passphrase.as_ref().map(|secret| secret.expose()),
                )?;
            }
            CredentialSecret::PrivateKeyPassphrase(_) | CredentialSecret::Token(_) => {
                return Err(SftpError::UnsupportedAuthMethod(
                    "ssh2 backend supports password auth or private-key auth with an optional passphrase".to_string(),
                ));
            }
        }

        if !session.authenticated() {
            return Err(SftpError::AuthenticationFailed);
        }

        self.session = Some(session);
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), SftpError> {
        if let Some(session) = self.session.take() {
            session.disconnect(None, "CrossSCP disconnect", None)?;
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(ssh2::Session::authenticated)
    }

    fn list_directory(&mut self, path: &str) -> Result<Vec<SftpRemoteFile>, SftpError> {
        let session = self.session.as_ref().ok_or(SftpError::NotConnected)?;
        let sftp = session.sftp()?;
        let path = normalize_remote_path(&sftp, path);
        let mut entries = Vec::new();

        for (entry_path, stat) in sftp.readdir(Path::new(&path))? {
            let name = entry_name(&entry_path);
            if name == "." || name == ".." {
                continue;
            }

            let permissions = stat.perm;
            entries.push(SftpRemoteFile {
                name,
                path: remote_child_path(&path, &entry_path),
                size: stat.size,
                is_directory: permissions.is_some_and(is_directory_perm),
                is_symlink: permissions.is_some_and(is_symlink_perm),
                permissions,
            });
        }

        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    fn upload_file(
        &mut self,
        local_path: &str,
        remote_path: &str,
    ) -> Result<SftpFileProgress, SftpError> {
        let session = self.session.as_ref().ok_or(SftpError::NotConnected)?;
        let local_metadata = fs::metadata(local_path)?;
        let sftp = session.sftp()?;
        let remote_path = normalize_remote_path(&sftp, remote_path);
        let destination =
            resolve_upload_destination(&sftp, local_path, &remote_path, local_metadata.is_dir())?;
        if local_metadata.is_dir() {
            let bytes_done =
                upload_directory_recursive(&sftp, Path::new(local_path), &destination)?;
            return Ok(SftpFileProgress {
                source: local_path.to_string(),
                destination,
                bytes_done,
                bytes_total: Some(bytes_done),
            });
        }
        let mut remote_file = create_remote_file_for_replace(&sftp, &destination)?;
        let mut local_file = fs::File::open(local_path)?;
        let bytes_done = std::io::copy(&mut local_file, &mut remote_file)?;

        Ok(SftpFileProgress {
            source: local_path.to_string(),
            destination,
            bytes_done,
            bytes_total: Some(local_metadata.len()),
        })
    }

    fn download_file(
        &mut self,
        remote_path: &str,
        local_path: &str,
    ) -> Result<SftpFileProgress, SftpError> {
        let session = self.session.as_ref().ok_or(SftpError::NotConnected)?;
        let sftp = session.sftp()?;
        let remote_path = normalize_remote_path(&sftp, remote_path);
        let remote_stat = sftp.stat(Path::new(&remote_path)).ok();
        if remote_stat
            .as_ref()
            .and_then(|stat| stat.perm)
            .is_some_and(is_directory_perm)
        {
            let destination = resolve_download_directory_destination(&remote_path, local_path);
            let bytes_done =
                download_directory_recursive(&sftp, &remote_path, Path::new(&destination))?;
            return Ok(SftpFileProgress {
                source: remote_path.clone(),
                destination,
                bytes_done,
                bytes_total: Some(bytes_done),
            });
        }
        if let Some(parent) = Path::new(local_path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let mut remote_file = sftp.open(Path::new(&remote_path))?;
        let mut local_file = fs::File::create(local_path)?;
        let bytes_done = std::io::copy(&mut remote_file, &mut local_file)?;

        Ok(SftpFileProgress {
            source: remote_path,
            destination: local_path.to_string(),
            bytes_done,
            bytes_total: remote_stat.and_then(|stat| stat.size),
        })
    }

    fn create_directory(&mut self, remote_path: &str) -> Result<(), SftpError> {
        let session = self.session.as_ref().ok_or(SftpError::NotConnected)?;
        let sftp = session.sftp()?;
        let remote_path = normalize_remote_path(&sftp, remote_path);
        ensure_remote_directory(&sftp, &remote_path)
    }

    fn delete_path(&mut self, remote_path: &str) -> Result<(), SftpError> {
        let session = self.session.as_ref().ok_or(SftpError::NotConnected)?;
        let sftp = session.sftp()?;
        let remote_path = normalize_remote_path(&sftp, remote_path);
        delete_remote_path_recursive(&sftp, &remote_path)
    }
}

fn normalize_remote_path(sftp: &ssh2::Sftp, path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "." {
        return sftp
            .realpath(Path::new("."))
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|_| ".".to_string());
    }
    if trimmed == "~" {
        return sftp
            .realpath(Path::new("."))
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|_| ".".to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        let home = sftp
            .realpath(Path::new("."))
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|_| ".".to_string());
        return remote_join(&home, rest);
    }
    trimmed.to_string()
}

fn entry_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn remote_child_path(parent: &str, entry_path: &Path) -> String {
    let name = entry_name(entry_path);
    let clean_parent = parent.trim();
    if clean_parent.is_empty() || clean_parent == "." {
        return name;
    }
    if clean_parent == "/" {
        return format!("/{name}");
    }
    format!("{}/{name}", clean_parent.trim_end_matches('/'))
}

fn resolve_upload_destination(
    sftp: &ssh2::Sftp,
    local_path: &str,
    remote_path: &str,
    local_is_directory: bool,
) -> Result<String, SftpError> {
    let remote_is_directory = remote_stat(sftp, remote_path.trim())?
        .and_then(|stat| stat.perm)
        .is_some_and(is_directory_perm);
    Ok(resolve_upload_destination_for_type(
        local_path,
        remote_path,
        local_is_directory,
        remote_is_directory,
    ))
}

fn resolve_upload_destination_for_type(
    local_path: &str,
    remote_path: &str,
    local_is_directory: bool,
    remote_is_directory: bool,
) -> String {
    let trimmed_remote = remote_path.trim();
    let local_name = Path::new(local_path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "upload.bin".to_string());

    if trimmed_remote.is_empty() || trimmed_remote == "." {
        return local_name;
    }

    if trimmed_remote == "/" {
        return format!("/{local_name}");
    }

    if trimmed_remote.ends_with('/') {
        let directory_path = trimmed_remote.trim_end_matches('/');
        if local_is_directory && remote_basename(directory_path) == local_name {
            return directory_path.to_string();
        }
        return format!("{}{}", trimmed_remote, local_name);
    }

    if remote_is_directory {
        if local_is_directory && remote_basename(trimmed_remote) == local_name {
            trimmed_remote.to_string()
        } else {
            remote_join(trimmed_remote, &local_name)
        }
    } else {
        trimmed_remote.to_string()
    }
}

fn resolve_upload_conflict_destination(
    sftp: &ssh2::Sftp,
    remote_path: &str,
    source_is_directory: bool,
    conflict_policy: FileConflictPolicy,
) -> Result<Option<String>, SftpError> {
    let existing = remote_stat(sftp, remote_path)?;
    let Some(existing) = existing else {
        return Ok(Some(remote_path.to_string()));
    };
    let existing_is_directory = existing.perm.is_some_and(is_directory_perm);

    match conflict_policy {
        FileConflictPolicy::Replace => Ok(Some(remote_path.to_string())),
        FileConflictPolicy::KeepExisting if source_is_directory && existing_is_directory => {
            Ok(Some(remote_path.to_string()))
        }
        FileConflictPolicy::KeepExisting => Ok(None),
        FileConflictPolicy::KeepBoth => {
            next_available_remote_path(sftp, remote_path, source_is_directory).map(Some)
        }
    }
}

fn next_available_remote_path(
    sftp: &ssh2::Sftp,
    remote_path: &str,
    is_directory: bool,
) -> Result<String, SftpError> {
    for copy_number in 1..=u32::MAX {
        let candidate = numbered_conflict_path(remote_path, copy_number, is_directory);
        if remote_stat(sftp, &candidate)?.is_none() {
            return Ok(candidate);
        }
    }
    Err(SftpError::Backend(format!(
        "could not create a unique upload name for '{remote_path}'"
    )))
}

fn resolve_download_directory_destination(remote_path: &str, local_path: &str) -> String {
    let local = Path::new(local_path);
    if local.exists() && local.is_dir() {
        return local
            .join(remote_basename(remote_path))
            .to_string_lossy()
            .into_owned();
    }
    local_path.to_string()
}

fn remote_basename(remote_path: &str) -> String {
    remote_path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("download")
        .to_string()
}

fn remote_join(parent: &str, child: &str) -> String {
    if parent == "/" {
        format!("/{child}")
    } else {
        format!("{}/{child}", parent.trim_end_matches('/'))
    }
}

fn remote_stat(sftp: &ssh2::Sftp, remote_path: &str) -> Result<Option<ssh2::FileStat>, SftpError> {
    match sftp.stat(Path::new(remote_path)) {
        Ok(stat) => Ok(Some(stat)),
        Err(error) if matches!(error.code(), ssh2::ErrorCode::SFTP(2 | 10)) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn ensure_remote_directory(sftp: &ssh2::Sftp, remote_path: &str) -> Result<(), SftpError> {
    if remote_stat(sftp, remote_path)?
        .and_then(|stat| stat.perm)
        .is_some_and(is_directory_perm)
    {
        return Ok(());
    }

    let mut current = String::new();
    for part in remote_path.split('/').filter(|part| !part.is_empty()) {
        current = if remote_path.starts_with('/') && current.is_empty() {
            format!("/{part}")
        } else if current.is_empty() {
            part.to_string()
        } else {
            remote_join(&current, part)
        };
        if remote_stat(sftp, &current)?.is_none() {
            sftp.mkdir(Path::new(&current), 0o755).map_err(|error| {
                SftpError::Backend(format!("remote mkdir failed for '{current}': {error}"))
            })?;
        }
    }
    Ok(())
}

fn ensure_remote_directory_for_replace(
    sftp: &ssh2::Sftp,
    remote_path: &str,
) -> Result<(), SftpError> {
    if let Some(stat) = remote_stat(sftp, remote_path)? {
        if stat.perm.is_some_and(is_directory_perm) {
            return Ok(());
        }
        sftp.unlink(Path::new(remote_path)).map_err(|error| {
            SftpError::Backend(format!(
                "could not replace remote file '{remote_path}' with a directory: {error}"
            ))
        })?;
    }
    ensure_remote_directory(sftp, remote_path)
}

fn upload_directory_recursive(
    sftp: &ssh2::Sftp,
    local_dir: &Path,
    remote_dir: &str,
) -> Result<u64, SftpError> {
    ensure_remote_directory_for_replace(sftp, remote_dir)?;
    let mut bytes_done = 0;
    for entry in fs::read_dir(local_dir)? {
        let entry = entry?;
        let local_path = entry.path();
        let remote_path = remote_join(remote_dir, &entry.file_name().to_string_lossy());
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            bytes_done += upload_directory_recursive(sftp, &local_path, &remote_path)?;
        } else if metadata.is_file() {
            let mut remote_file = create_remote_file_for_replace(sftp, &remote_path)?;
            let mut local_file = fs::File::open(&local_path)?;
            bytes_done += std::io::copy(&mut local_file, &mut remote_file)?;
        }
    }
    Ok(bytes_done)
}

fn local_directory_size(local_dir: &Path) -> Result<u64, SftpError> {
    let mut bytes_total = 0;
    for entry in fs::read_dir(local_dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            bytes_total += local_directory_size(&entry.path())?;
        } else if metadata.is_file() {
            bytes_total += metadata.len();
        }
    }
    Ok(bytes_total)
}

struct DirectoryUploadProgress<'a, F> {
    bytes_total: u64,
    bytes_transferred: &'a mut u64,
    bytes_processed: &'a mut u64,
    report: &'a mut F,
}

fn upload_directory_recursive_with_progress<F>(
    sftp: &ssh2::Sftp,
    local_dir: &Path,
    remote_dir: &str,
    progress: &mut DirectoryUploadProgress<'_, F>,
    conflict_policy: FileConflictPolicy,
) -> Result<(), SftpError>
where
    F: FnMut(u64, Option<u64>),
{
    let Some(destination) =
        resolve_upload_conflict_destination(sftp, remote_dir, true, conflict_policy)?
    else {
        *progress.bytes_processed += local_directory_size(local_dir)?;
        (progress.report)(*progress.bytes_processed, Some(progress.bytes_total));
        return Ok(());
    };
    if conflict_policy == FileConflictPolicy::Replace {
        ensure_remote_directory_for_replace(sftp, &destination)?;
    } else {
        ensure_remote_directory(sftp, &destination)?;
    }
    for entry in fs::read_dir(local_dir)? {
        let entry = entry?;
        let local_path = entry.path();
        let remote_path = remote_join(&destination, &entry.file_name().to_string_lossy());
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            upload_directory_recursive_with_progress(
                sftp,
                &local_path,
                &remote_path,
                progress,
                conflict_policy,
            )?;
        } else if metadata.is_file() {
            let Some((_destination, mut remote_file)) =
                create_remote_file_for_policy(sftp, &remote_path, conflict_policy)?
            else {
                *progress.bytes_processed += metadata.len();
                (progress.report)(*progress.bytes_processed, Some(progress.bytes_total));
                continue;
            };
            let mut local_file = fs::File::open(&local_path)?;
            let processed_before_file = *progress.bytes_processed;
            let file_bytes_transferred = copy_with_progress(
                &mut local_file,
                &mut remote_file,
                Some(progress.bytes_total),
                &mut |file_bytes_done, _| {
                    (progress.report)(
                        processed_before_file + file_bytes_done,
                        Some(progress.bytes_total),
                    );
                },
            )?;
            *progress.bytes_transferred += file_bytes_transferred;
            *progress.bytes_processed += file_bytes_transferred;
            (progress.report)(*progress.bytes_processed, Some(progress.bytes_total));
        }
    }
    Ok(())
}

fn copy_with_progress<R, W, F>(
    reader: &mut R,
    writer: &mut W,
    bytes_total: Option<u64>,
    report_progress: &mut F,
) -> Result<u64, SftpError>
where
    R: Read,
    W: Write,
    F: FnMut(u64, Option<u64>),
{
    let mut buffer = vec![0_u8; SFTP_TRANSFER_BUFFER_SIZE];
    let progress_interval = Duration::from_millis(SFTP_PROGRESS_INTERVAL_MS);
    let mut bytes_done = 0_u64;
    let mut last_reported_bytes = bytes_done;
    let mut last_reported_at = Instant::now();
    report_progress(bytes_done, bytes_total);

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
        bytes_done += read as u64;
        if last_reported_at.elapsed() >= progress_interval {
            report_progress(bytes_done, bytes_total);
            last_reported_bytes = bytes_done;
            last_reported_at = Instant::now();
        }
    }
    writer.flush()?;
    if last_reported_bytes != bytes_done {
        report_progress(bytes_done, bytes_total);
    }
    Ok(bytes_done)
}

fn download_directory_recursive(
    sftp: &ssh2::Sftp,
    remote_dir: &str,
    local_dir: &Path,
) -> Result<u64, SftpError> {
    fs::create_dir_all(local_dir)?;
    let mut bytes_done = 0;
    for (entry_path, stat) in sftp.readdir(Path::new(remote_dir))? {
        let name = entry_name(&entry_path);
        if name == "." || name == ".." {
            continue;
        }
        let remote_path = remote_join(remote_dir, &name);
        let local_path: PathBuf = local_dir.join(&name);
        if stat.perm.is_some_and(is_directory_perm) {
            bytes_done += download_directory_recursive(sftp, &remote_path, &local_path)?;
        } else {
            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut remote_file = sftp.open(Path::new(&remote_path))?;
            let mut local_file = fs::File::create(&local_path)?;
            bytes_done += std::io::copy(&mut remote_file, &mut local_file)?;
        }
    }
    Ok(bytes_done)
}

fn remote_directory_size(sftp: &ssh2::Sftp, remote_dir: &str) -> Result<u64, SftpError> {
    let mut bytes_total = 0;
    for (entry_path, stat) in sftp.readdir(Path::new(remote_dir))? {
        let name = entry_name(&entry_path);
        if name == "." || name == ".." {
            continue;
        }
        let remote_path = remote_join(remote_dir, &name);
        if stat.perm.is_some_and(is_directory_perm) {
            bytes_total += remote_directory_size(sftp, &remote_path)?;
        } else {
            bytes_total += stat.size.unwrap_or(0);
        }
    }
    Ok(bytes_total)
}

fn download_directory_recursive_with_progress<F>(
    sftp: &ssh2::Sftp,
    remote_dir: &str,
    local_dir: &Path,
    bytes_total: u64,
    bytes_done: &mut u64,
    report_progress: &mut F,
) -> Result<(), SftpError>
where
    F: FnMut(u64, Option<u64>),
{
    fs::create_dir_all(local_dir)?;
    for (entry_path, stat) in sftp.readdir(Path::new(remote_dir))? {
        let name = entry_name(&entry_path);
        if name == "." || name == ".." {
            continue;
        }
        let remote_path = remote_join(remote_dir, &name);
        let local_path: PathBuf = local_dir.join(&name);
        if stat.perm.is_some_and(is_directory_perm) {
            download_directory_recursive_with_progress(
                sftp,
                &remote_path,
                &local_path,
                bytes_total,
                bytes_done,
                report_progress,
            )?;
        } else {
            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut remote_file = sftp.open(Path::new(&remote_path))?;
            let mut local_file = fs::File::create(&local_path)?;
            *bytes_done += copy_with_progress(
                &mut remote_file,
                &mut local_file,
                Some(bytes_total),
                &mut |file_bytes_done, _| {
                    report_progress(*bytes_done + file_bytes_done, Some(bytes_total));
                },
            )?;
            report_progress(*bytes_done, Some(bytes_total));
        }
    }
    Ok(())
}

fn delete_remote_path_recursive(sftp: &ssh2::Sftp, remote_path: &str) -> Result<(), SftpError> {
    if remote_path.trim().is_empty() || remote_path == "/" {
        return Err(SftpError::Backend(
            "refusing to delete empty path or remote root".to_string(),
        ));
    }

    let path = Path::new(remote_path);
    let stat = sftp.stat(path)?;
    if stat.perm.is_some_and(is_directory_perm) {
        for (entry_path, _stat) in sftp.readdir(path)? {
            let name = entry_name(&entry_path);
            if name == "." || name == ".." {
                continue;
            }
            let child_path = remote_join(remote_path, &name);
            delete_remote_path_recursive(sftp, &child_path)?;
        }
        sftp.rmdir(path).map_err(|error| {
            SftpError::Backend(format!("remote rmdir failed for '{remote_path}': {error}"))
        })
    } else {
        sftp.unlink(path).map_err(|error| {
            SftpError::Backend(format!("remote delete failed for '{remote_path}': {error}"))
        })
    }
}

fn create_remote_file_for_policy(
    sftp: &ssh2::Sftp,
    remote_path: &str,
    conflict_policy: FileConflictPolicy,
) -> Result<Option<(String, ssh2::File)>, SftpError> {
    match conflict_policy {
        FileConflictPolicy::Replace => create_remote_file_for_replace(sftp, remote_path)
            .map(|file| Some((remote_path.to_string(), file))),
        FileConflictPolicy::KeepExisting => {
            if remote_stat(sftp, remote_path)?.is_some() {
                return Ok(None);
            }
            match create_remote_file_exclusive(sftp, remote_path) {
                Ok(file) => Ok(Some((remote_path.to_string(), file))),
                Err(error) if is_already_exists_error(&error) => Ok(None),
                Err(error) => Err(error.into()),
            }
        }
        FileConflictPolicy::KeepBoth => {
            for copy_number in 0..=u32::MAX {
                let candidate = if copy_number == 0 {
                    remote_path.to_string()
                } else {
                    numbered_conflict_path(remote_path, copy_number, false)
                };
                if remote_stat(sftp, &candidate)?.is_some() {
                    continue;
                }
                match create_remote_file_exclusive(sftp, &candidate) {
                    Ok(file) => return Ok(Some((candidate, file))),
                    Err(error) if is_already_exists_error(&error) => continue,
                    Err(error) => return Err(error.into()),
                }
            }
            Err(SftpError::Backend(format!(
                "could not create a unique upload name for '{remote_path}'"
            )))
        }
    }
}

fn create_remote_file_exclusive(
    sftp: &ssh2::Sftp,
    remote_path: &str,
) -> Result<ssh2::File, ssh2::Error> {
    sftp.open_mode(
        Path::new(remote_path),
        ssh2::OpenFlags::WRITE | ssh2::OpenFlags::CREATE | ssh2::OpenFlags::EXCLUSIVE,
        0o644,
        ssh2::OpenType::File,
    )
}

fn is_already_exists_error(error: &ssh2::Error) -> bool {
    matches!(error.code(), ssh2::ErrorCode::SFTP(11))
}

fn create_remote_file_for_replace(
    sftp: &ssh2::Sftp,
    remote_path: &str,
) -> Result<ssh2::File, SftpError> {
    let path = Path::new(remote_path);
    if remote_stat(sftp, remote_path)?
        .and_then(|stat| stat.perm)
        .is_some_and(is_directory_perm)
    {
        return Err(SftpError::Backend(format!(
            "upload target '{remote_path}' is a directory; choose a filename inside it"
        )));
    }

    sftp.open_mode(
        path,
        ssh2::OpenFlags::WRITE | ssh2::OpenFlags::CREATE | ssh2::OpenFlags::TRUNCATE,
        0o644,
        ssh2::OpenType::File,
    )
    .or_else(|first_error| {
        if sftp.stat(path).is_ok() {
            let _ = sftp.unlink(path);
            sftp.create(path).map_err(|second_error| {
                SftpError::Backend(format!(
                    "upload replace failed for '{remote_path}': {second_error}; initial create/truncate error: {first_error}"
                ))
            })
        } else {
            Err(SftpError::Backend(format!(
                "upload create failed for '{remote_path}': {first_error}. Check the remote directory exists and is writable."
            )))
        }
    })
}

fn is_directory_perm(perm: u32) -> bool {
    (perm & 0o170000) == 0o040000
}

fn is_symlink_perm(perm: u32) -> bool {
    (perm & 0o170000) == 0o120000
}

impl From<ssh2::Error> for SftpError {
    fn from(error: ssh2::Error) -> Self {
        Self::Backend(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{
        copy_with_progress, resolve_upload_destination_for_type, SFTP_TRANSFER_BUFFER_SIZE,
    };

    #[test]
    fn same_named_upload_directory_uses_existing_directory_as_merge_root() {
        assert_eq!(
            resolve_upload_destination_for_type("/tmp/photos", "/uploads/photos", true, true,),
            "/uploads/photos"
        );
        assert_eq!(
            resolve_upload_destination_for_type("/tmp/photos", "/uploads/photos/", true, true,),
            "/uploads/photos"
        );
    }

    #[test]
    fn directory_destination_still_accepts_differently_named_source() {
        assert_eq!(
            resolve_upload_destination_for_type("/tmp/photos", "/uploads/archive", true, true,),
            "/uploads/archive/photos"
        );
        assert_eq!(
            resolve_upload_destination_for_type("/tmp/report.txt", "/uploads/archive", false, true,),
            "/uploads/archive/report.txt"
        );
    }

    #[test]
    fn fast_copy_reports_start_and_finish_without_per_buffer_noise() {
        let bytes_total = (SFTP_TRANSFER_BUFFER_SIZE * 2) as u64;
        let source = vec![7_u8; bytes_total as usize];
        let mut reader = Cursor::new(source);
        let mut writer = Vec::new();
        let mut events = Vec::new();

        let bytes_done = copy_with_progress(
            &mut reader,
            &mut writer,
            Some(bytes_total),
            &mut |done, total| events.push((done, total)),
        )
        .expect("copy succeeds");

        assert_eq!(bytes_done, bytes_total);
        assert_eq!(writer.len(), bytes_total as usize);
        assert_eq!(events.first(), Some(&(0, Some(bytes_total))));
        assert_eq!(events.last(), Some(&(bytes_total, Some(bytes_total))));
        assert!(
            events.len() <= 3,
            "fast copy should not emit one progress event per buffer: {events:?}"
        );
    }
}
