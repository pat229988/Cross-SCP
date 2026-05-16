// SPDX-License-Identifier: AGPL-3.0-or-later

//! Optional `ssh2`/libssh2 backend candidate for the first live SFTP POC.

use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossscp_security::CredentialSecret;

use crate::{
    SftpAuthMaterial, SftpBackend, SftpConnectionConfig, SftpError, SftpFileProgress,
    SftpRemoteFile,
};

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
            timeout: Duration::from_secs(10),
            session: None,
        }
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
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
        let mut entries = Vec::new();

        for (entry_path, stat) in sftp.readdir(Path::new(path))? {
            let name = entry_name(&entry_path);
            if name == "." || name == ".." {
                continue;
            }

            let permissions = stat.perm;
            entries.push(SftpRemoteFile {
                name,
                path: remote_child_path(path, &entry_path),
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
        let destination = resolve_upload_destination(&sftp, local_path, remote_path);
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
        let remote_stat = sftp.stat(Path::new(remote_path)).ok();
        if remote_stat
            .as_ref()
            .and_then(|stat| stat.perm)
            .is_some_and(is_directory_perm)
        {
            let destination = resolve_download_directory_destination(remote_path, local_path);
            let bytes_done =
                download_directory_recursive(&sftp, remote_path, Path::new(&destination))?;
            return Ok(SftpFileProgress {
                source: remote_path.to_string(),
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
        let mut remote_file = sftp.open(Path::new(remote_path))?;
        let mut local_file = fs::File::create(local_path)?;
        let bytes_done = std::io::copy(&mut remote_file, &mut local_file)?;

        Ok(SftpFileProgress {
            source: remote_path.to_string(),
            destination: local_path.to_string(),
            bytes_done,
            bytes_total: remote_stat.and_then(|stat| stat.size),
        })
    }

    fn create_directory(&mut self, remote_path: &str) -> Result<(), SftpError> {
        let session = self.session.as_ref().ok_or(SftpError::NotConnected)?;
        let sftp = session.sftp()?;
        ensure_remote_directory(&sftp, remote_path)
    }

    fn delete_path(&mut self, remote_path: &str) -> Result<(), SftpError> {
        let session = self.session.as_ref().ok_or(SftpError::NotConnected)?;
        let sftp = session.sftp()?;
        delete_remote_path_recursive(&sftp, remote_path)
    }
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

fn resolve_upload_destination(sftp: &ssh2::Sftp, local_path: &str, remote_path: &str) -> String {
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
        return format!("{}{}", trimmed_remote, local_name);
    }

    if sftp
        .stat(Path::new(trimmed_remote))
        .ok()
        .and_then(|stat| stat.perm)
        .is_some_and(is_directory_perm)
    {
        if trimmed_remote == "/" {
            format!("/{local_name}")
        } else {
            format!("{trimmed_remote}/{local_name}")
        }
    } else {
        trimmed_remote.to_string()
    }
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

fn ensure_remote_directory(sftp: &ssh2::Sftp, remote_path: &str) -> Result<(), SftpError> {
    if sftp
        .stat(Path::new(remote_path))
        .ok()
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
        if sftp.stat(Path::new(&current)).is_err() {
            sftp.mkdir(Path::new(&current), 0o755).map_err(|error| {
                SftpError::Backend(format!("remote mkdir failed for '{current}': {error}"))
            })?;
        }
    }
    Ok(())
}

fn upload_directory_recursive(
    sftp: &ssh2::Sftp,
    local_dir: &Path,
    remote_dir: &str,
) -> Result<u64, SftpError> {
    ensure_remote_directory(sftp, remote_dir)?;
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

fn create_remote_file_for_replace(
    sftp: &ssh2::Sftp,
    remote_path: &str,
) -> Result<ssh2::File, SftpError> {
    let path = Path::new(remote_path);
    if sftp
        .stat(path)
        .ok()
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
