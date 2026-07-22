// SPDX-License-Identifier: AGPL-3.0-or-later

//! Initial CrossSCP CLI scaffold and GUI service bridge.

use crossscp_config::{FileSessionStore, SessionStore};
use crossscp_core::{FileConflictPolicy, MaskDecision, MaskSet, SessionProfile, SessionProtocol};
use crossscp_core::{ProtocolCapabilities, RemoteFileSystem};
use crossscp_protocol_ftp::{
    resolve_ftp_credentials, FtpAdapter, FtpConnectionConfig, FtpError, FtpsMode,
};
use crossscp_protocol_local::LocalFileSystem;
use crossscp_protocol_scp::{
    resolve_scp_credentials, ScpAdapter, ScpConnectionConfig, ScpError, ScpTransferSummary,
};
use crossscp_protocol_sftp::LiveSftpTestConfig;
use crossscp_transfer::{OverwriteMode, TransferDirection, TransferOptions, TransferQueue};
use std::io::Write;
use std::time::Duration;

#[cfg(feature = "ssh2-backend")]
use crossscp_protocol_sftp::ssh2_backend::Ssh2Backend;
#[cfg(feature = "ssh2-backend")]
use crossscp_protocol_sftp::{
    resolve_sftp_credentials, SftpAdapter, SftpConnectionConfig, SftpError, SftpFileProgress,
};
use crossscp_security::{
    CredentialRef, CredentialSecret, CredentialService, InMemoryCredentialService, SecretString,
};

fn main() {
    let mut args = std::env::args().skip(1);

    match args.next().as_deref() {
        Some("--version") | Some("version") => print_version(),
        Some("mask-check") => run_mask_check(args.collect()),
        Some("local-profile") => print_local_profile(args.next()),
        Some("local-list") => run_local_list(args.next()),
        Some("local-copy") => run_local_copy(args.collect()),
        Some("session-list") => run_session_list(args.next()),
        Some("session-save") => run_session_save(args.collect()),
        Some("session-delete") => run_session_delete(args.collect()),
        Some("sftp-list") => run_sftp_list(args.collect()),
        Some("sftp-upload") => run_sftp_transfer(
            args.collect(),
            SftpTransferKind::Upload,
            FileConflictPolicy::Replace,
            false,
        ),
        Some("sftp-download") => run_sftp_transfer(
            args.collect(),
            SftpTransferKind::Download,
            FileConflictPolicy::Replace,
            false,
        ),
        Some("sftp-mkdir") => run_sftp_mkdir(args.collect()),
        Some("sftp-delete") => run_sftp_delete(args.collect()),
        Some("remote-capabilities") => run_remote_capabilities(args.collect()),
        Some("remote-list") => run_remote_list(args.collect()),
        Some("remote-upload") => run_remote_transfer(args.collect(), SftpTransferKind::Upload),
        Some("remote-download") => run_remote_transfer(args.collect(), SftpTransferKind::Download),
        Some("remote-mkdir") => run_remote_mkdir(args.collect()),
        Some("remote-delete") => run_remote_delete(args.collect()),
        Some("remote-rename") => run_remote_rename(args.collect()),
        Some("sftp-live-config") => print_sftp_live_config(),
        _ => print_help(),
    }
}

fn print_version() {
    println!("crossscp 0.1.0");
}

fn print_help() {
    println!("CrossSCP clean-room CLI scaffold");
    println!("Usage:");
    println!("  crossscp --version");
    println!("  crossscp local-profile [name]");
    println!("  crossscp local-list <path>");
    println!("  crossscp local-copy <source> <destination> [ask|always|never|if-newer|resume]");
    println!("  crossscp session-list <sessions.tsv>");
    println!("  crossscp session-save <sessions.tsv> [protocol] <name> <host> <port> <username> <remote-path> <credential-ref>");
    println!("  crossscp session-delete <sessions.tsv> <name>");
    println!("  crossscp sftp-list <host> <port> <username> <remote-path>  # password from CROSSSCP_SFTP_PASSWORD or key from CROSSSCP_SFTP_KEY_PATH, requires ssh2-backend");
    println!("  crossscp sftp-upload <host> <port> <username> <local-path> <remote-path>  # requires ssh2-backend");
    println!("  crossscp sftp-download <host> <port> <username> <remote-path> <local-path>  # requires ssh2-backend");
    println!(
        "  crossscp sftp-mkdir <host> <port> <username> <remote-path>  # requires ssh2-backend"
    );
    println!(
        "  crossscp sftp-delete <host> <port> <username> <remote-path>  # requires ssh2-backend"
    );
    println!("  crossscp remote-capabilities --protocol <sftp|scp|ftp|ftps|webdav|s3|local>");
    println!("  crossscp remote-list --protocol <sftp|ftp|ftps> --host <host> --port <port> --username <user> --path <remote-path>");
    println!("  crossscp remote-upload --protocol <sftp|scp|ftp|ftps> --host <host> --port <port> --username <user> --local <path> --remote <path> [--conflict <keep-existing|replace|keep-both>]");
    println!("  crossscp remote-download --protocol <sftp|scp|ftp|ftps> --host <host> --port <port> --username <user> --remote <path> --local <path>");
    println!("  crossscp remote-mkdir --protocol <sftp|ftp|ftps> --host <host> --port <port> --username <user> --path <remote-path>");
    println!("  crossscp remote-delete --protocol <sftp|ftp|ftps> --host <host> --port <port> --username <user> --path <remote-path>");
    println!("  crossscp remote-rename --protocol <ftp|ftps> --host <host> --port <port> --username <user> --from <old-path> --to <new-path>");
    println!("  crossscp mask-check <include-pattern> <exclude-pattern> <path>");
    println!("  crossscp sftp-live-config");
}

#[derive(Debug, Default)]
struct RemoteCommandArgs {
    protocol: Option<SessionProtocol>,
    host: Option<String>,
    port: Option<String>,
    username: Option<String>,
    path: Option<String>,
    local: Option<String>,
    remote: Option<String>,
    conflict: Option<FileConflictPolicy>,
    from: Option<String>,
    to: Option<String>,
}

struct RemoteTransferSpec {
    local: String,
    remote: String,
    conflict_policy: Option<FileConflictPolicy>,
}

fn parse_remote_args(args: Vec<String>) -> Result<RemoteCommandArgs, String> {
    let mut parsed = RemoteCommandArgs::default();
    let mut iter = args.into_iter();
    while let Some(flag) = iter.next() {
        let value = iter
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--protocol" => {
                parsed.protocol = Some(
                    value
                        .parse()
                        .map_err(|_| format!("unsupported protocol: {value}"))?,
                );
            }
            "--host" | "--url" | "--endpoint" => parsed.host = Some(value),
            "--port" => parsed.port = Some(value),
            "--username" => parsed.username = Some(value),
            "--path" => parsed.path = Some(value),
            "--local" => parsed.local = Some(value),
            "--remote" => parsed.remote = Some(value),
            "--conflict" => parsed.conflict = Some(parse_file_conflict_policy(&value)?),
            "--from" => parsed.from = Some(value),
            "--to" => parsed.to = Some(value),
            "--bucket" | "--region" | "--root" | "--prefix" => {}
            _ => return Err(format!("unknown remote option: {flag}")),
        }
    }
    Ok(parsed)
}

fn remote_protocol(args: &RemoteCommandArgs) -> Result<SessionProtocol, String> {
    args.protocol
        .clone()
        .ok_or_else(|| "--protocol is required".to_string())
}

fn remote_host_port_username(args: &RemoteCommandArgs) -> Result<(String, String, String), String> {
    let protocol = remote_protocol(args)?;
    let host = args
        .host
        .clone()
        .ok_or_else(|| "--host is required".to_string())?;
    let port = args
        .port
        .clone()
        .or_else(|| protocol.default_port().map(|port| port.to_string()))
        .ok_or_else(|| "--port is required".to_string())?;
    let username = args
        .username
        .clone()
        .ok_or_else(|| "--username is required".to_string())?;
    Ok((host, port, username))
}

fn capabilities_for(protocol: &SessionProtocol) -> ProtocolCapabilities {
    match protocol {
        SessionProtocol::Sftp => ProtocolCapabilities::sftp(),
        SessionProtocol::Scp => ProtocolCapabilities::scp_transfer_only(),
        SessionProtocol::Ftp => ProtocolCapabilities::ftp_like(false),
        SessionProtocol::Ftps => ProtocolCapabilities::ftp_like(true),
        SessionProtocol::WebDav => ProtocolCapabilities::webdav(),
        SessionProtocol::S3 => ProtocolCapabilities::s3(),
        SessionProtocol::Local => ProtocolCapabilities::empty(),
    }
}

fn run_remote_capabilities(args: Vec<String>) {
    let parsed = parse_remote_args(args).unwrap_or_else(|message| exit_error(&message, 2));
    let protocol = remote_protocol(&parsed).unwrap_or_else(|message| exit_error(&message, 2));
    let caps = capabilities_for(&protocol);
    println!("protocol\t{}", protocol.as_str());
    println!("display_name\t{}", protocol.display_name());
    println!("default_port\t{}", protocol.default_port().unwrap_or(0));
    println!("can_list\t{}", caps.can_list);
    println!("can_upload\t{}", caps.can_upload);
    println!("can_download\t{}", caps.can_download);
    println!("can_delete\t{}", caps.can_delete);
    println!("can_mkdir\t{}", caps.can_mkdir);
    println!("can_rename\t{}", caps.can_rename);
    println!("can_recursive_transfer\t{}", caps.can_recursive_transfer);
    println!("uses_object_prefixes\t{}", caps.uses_object_prefixes);
    println!("supports_tls_policy\t{}", caps.supports_tls_policy);
    println!(
        "supports_http_version_policy\t{}",
        caps.supports_http_version_policy
    );
}

fn run_remote_list(args: Vec<String>) {
    let parsed = parse_remote_args(args).unwrap_or_else(|message| exit_error(&message, 2));
    let protocol = remote_protocol(&parsed).unwrap_or_else(|message| exit_error(&message, 2));
    let (host, port, username) =
        remote_host_port_username(&parsed).unwrap_or_else(|message| exit_error(&message, 2));
    let path = parsed.path.unwrap_or_else(|| "/".to_string());
    match protocol {
        SessionProtocol::Sftp => run_sftp_list(vec![host, port, username, path]),
        SessionProtocol::Ftp | SessionProtocol::Ftps => {
            run_ftp_list(protocol, host, port, username, path)
        }
        SessionProtocol::Scp => exit_error("SCP is transfer-only; remote-list is unsupported", 1),
        other => unsupported_live_protocol(other),
    }
}

fn run_remote_transfer(args: Vec<String>, kind: SftpTransferKind) {
    let parsed = parse_remote_args(args).unwrap_or_else(|message| exit_error(&message, 2));
    let protocol = remote_protocol(&parsed).unwrap_or_else(|message| exit_error(&message, 2));
    let (host, port, username) =
        remote_host_port_username(&parsed).unwrap_or_else(|message| exit_error(&message, 2));
    if matches!(kind, SftpTransferKind::Download) && parsed.conflict.is_some() {
        exit_error("--conflict is only valid for uploads", 2);
    }
    let requested_conflict_policy = parsed.conflict;
    let conflict_policy = requested_conflict_policy.unwrap_or_default();
    let local = parsed
        .local
        .unwrap_or_else(|| exit_error("--local is required", 2));
    let remote = parsed
        .remote
        .unwrap_or_else(|| exit_error("--remote is required", 2));
    match protocol {
        SessionProtocol::Sftp => match kind {
            SftpTransferKind::Upload => run_sftp_transfer(
                vec![host, port, username, local, remote],
                kind,
                conflict_policy,
                true,
            ),
            SftpTransferKind::Download => run_sftp_transfer(
                vec![host, port, username, remote, local],
                kind,
                conflict_policy,
                false,
            ),
        },
        SessionProtocol::Ftp | SessionProtocol::Ftps => run_ftp_transfer(
            protocol,
            host,
            port,
            username,
            RemoteTransferSpec {
                local,
                remote,
                conflict_policy: requested_conflict_policy,
            },
            kind,
        ),
        SessionProtocol::Scp if conflict_policy != FileConflictPolicy::Replace => exit_error(
            "SCP cannot inspect remote conflicts; use SFTP for keep-existing or keep-both",
            1,
        ),
        SessionProtocol::Scp => run_scp_transfer(host, port, username, local, remote, kind),
        other => unsupported_live_protocol(other),
    }
}

fn run_remote_mkdir(args: Vec<String>) {
    let parsed = parse_remote_args(args).unwrap_or_else(|message| exit_error(&message, 2));
    let protocol = remote_protocol(&parsed).unwrap_or_else(|message| exit_error(&message, 2));
    let (host, port, username) =
        remote_host_port_username(&parsed).unwrap_or_else(|message| exit_error(&message, 2));
    let path = parsed
        .path
        .unwrap_or_else(|| exit_error("--path is required", 2));
    match protocol {
        SessionProtocol::Sftp => run_sftp_mkdir(vec![host, port, username, path]),
        SessionProtocol::Ftp | SessionProtocol::Ftps => {
            run_ftp_mkdir(protocol, host, port, username, path)
        }
        SessionProtocol::Scp => exit_error("SCP is transfer-only; remote-mkdir is unsupported", 1),
        other => unsupported_live_protocol(other),
    }
}

fn run_remote_delete(args: Vec<String>) {
    let parsed = parse_remote_args(args).unwrap_or_else(|message| exit_error(&message, 2));
    let protocol = remote_protocol(&parsed).unwrap_or_else(|message| exit_error(&message, 2));
    let (host, port, username) =
        remote_host_port_username(&parsed).unwrap_or_else(|message| exit_error(&message, 2));
    let path = parsed
        .path
        .unwrap_or_else(|| exit_error("--path is required", 2));
    match protocol {
        SessionProtocol::Sftp => run_sftp_delete(vec![host, port, username, path]),
        SessionProtocol::Ftp | SessionProtocol::Ftps => {
            run_ftp_delete(protocol, host, port, username, path)
        }
        SessionProtocol::Scp => exit_error("SCP is transfer-only; remote-delete is unsupported", 1),
        other => unsupported_live_protocol(other),
    }
}

fn run_remote_rename(args: Vec<String>) {
    let parsed = parse_remote_args(args).unwrap_or_else(|message| exit_error(&message, 2));
    let protocol = remote_protocol(&parsed).unwrap_or_else(|message| exit_error(&message, 2));
    let (host, port, username) =
        remote_host_port_username(&parsed).unwrap_or_else(|message| exit_error(&message, 2));
    let from = parsed
        .from
        .unwrap_or_else(|| exit_error("--from is required", 2));
    let to = parsed
        .to
        .unwrap_or_else(|| exit_error("--to is required", 2));
    match protocol {
        SessionProtocol::Ftp | SessionProtocol::Ftps => {
            run_ftp_rename(protocol, host, port, username, from, to)
        }
        SessionProtocol::Sftp => exit_error("remote-rename is not available for SFTP yet", 1),
        other => unsupported_live_protocol(other),
    }
}

fn unsupported_live_protocol(protocol: SessionProtocol) -> ! {
    exit_error(
        &format!(
            "{} adapter is not implemented yet; currently SFTP, SCP, FTP, and explicit FTPS are live",
            protocol.display_name()
        ),
        1,
    )
}

fn run_scp_transfer(
    host: String,
    port: String,
    username: String,
    local: String,
    remote: String,
    kind: SftpTransferKind,
) {
    let mut adapter = match connect_scp(&host, &port, &username) {
        Ok(adapter) => adapter,
        Err(error) => exit_error(&format!("scp transfer failed: {error}"), 1),
    };
    let result = match kind {
        SftpTransferKind::Upload => {
            adapter.upload_file_with_progress(&local, &remote, print_transfer_progress)
        }
        SftpTransferKind::Download => {
            adapter.download_file_with_progress(&remote, &local, print_transfer_progress)
        }
    };
    match result {
        Ok(progress) => print_scp_progress(progress),
        Err(error) => exit_error(&format!("scp transfer failed: {error}"), 1),
    }
    let _ = adapter.disconnect();
}

fn connect_scp(host: &str, port: &str, username: &str) -> Result<ScpAdapter, ScpError> {
    let port = port
        .parse::<u16>()
        .map_err(|_| ScpError::Backend(format!("invalid port: {port}")))?;
    let reference = CredentialRef::new("env://CROSSSCP_REMOTE_PASSWORD")?;
    let mut credentials = InMemoryCredentialService::new();
    credentials.store(reference.clone(), ssh_secret_from_env("SCP")?)?;
    let config = ScpConnectionConfig {
        host: host.to_string(),
        port,
        username: non_empty(username.to_string()),
        initial_remote_path: None,
        credential_ref: Some(reference),
    };
    let auth = resolve_scp_credentials(&config, &credentials)?;
    let mut adapter = ScpAdapter::new(config, auth).with_timeout(sftp_timeout());
    adapter.connect()?;
    Ok(adapter)
}

fn print_scp_progress(progress: ScpTransferSummary) {
    println!(
        "completed\t{}\t{}\t{}",
        progress.source,
        progress.destination,
        progress.bytes_total.unwrap_or(progress.bytes_done)
    );
}

fn run_ftp_list(
    protocol: SessionProtocol,
    host: String,
    port: String,
    username: String,
    path: String,
) {
    let mut adapter = match connect_ftp(protocol, &host, &port, &username) {
        Ok(adapter) => adapter,
        Err(error) => exit_error(&format!("ftp-list failed: {error}"), 1),
    };
    match adapter.list_directory(&path) {
        Ok(entries) => {
            for entry in entries {
                let kind = if entry.is_directory { "dir" } else { "file" };
                println!(
                    "{kind}\t{}\t{}\t{}",
                    entry.size.unwrap_or(0),
                    entry.path,
                    entry.name
                );
            }
        }
        Err(error) => exit_error(&format!("ftp-list failed: {error}"), 1),
    }
    let _ = adapter.disconnect();
}

fn run_ftp_transfer(
    protocol: SessionProtocol,
    host: String,
    port: String,
    username: String,
    transfer: RemoteTransferSpec,
    kind: SftpTransferKind,
) {
    let mut adapter = match connect_ftp(protocol, &host, &port, &username) {
        Ok(adapter) => adapter,
        Err(error) => exit_error(&format!("ftp transfer failed: {error}"), 1),
    };
    let result = match kind {
        SftpTransferKind::Upload => match transfer.conflict_policy {
            Some(conflict_policy) => {
                adapter.upload_path_with_policy(&transfer.local, &transfer.remote, conflict_policy)
            }
            None => adapter.upload_path(&transfer.local, &transfer.remote),
        },
        SftpTransferKind::Download => adapter.download_path(&transfer.remote, &transfer.local),
    };
    match result {
        Ok(progress) => {
            println!(
                "completed\t{}\t{}\t{}",
                progress.source,
                progress.destination,
                progress.bytes_total.unwrap_or(progress.bytes_done)
            );
        }
        Err(error) => exit_error(&format!("ftp transfer failed: {error}"), 1),
    }
    let _ = adapter.disconnect();
}

fn run_ftp_mkdir(
    protocol: SessionProtocol,
    host: String,
    port: String,
    username: String,
    path: String,
) {
    let mut adapter = match connect_ftp(protocol, &host, &port, &username) {
        Ok(adapter) => adapter,
        Err(error) => exit_error(&format!("ftp-mkdir failed: {error}"), 1),
    };
    match adapter.create_directory(&path) {
        Ok(()) => println!("created\t{path}"),
        Err(error) => exit_error(&format!("ftp-mkdir failed: {error}"), 1),
    }
    let _ = adapter.disconnect();
}

fn run_ftp_delete(
    protocol: SessionProtocol,
    host: String,
    port: String,
    username: String,
    path: String,
) {
    let mut adapter = match connect_ftp(protocol, &host, &port, &username) {
        Ok(adapter) => adapter,
        Err(error) => exit_error(&format!("ftp-delete failed: {error}"), 1),
    };
    match adapter.delete_path_recursive(&path) {
        Ok(()) => println!("deleted\t{path}"),
        Err(error) => exit_error(&format!("ftp-delete failed: {error}"), 1),
    }
    let _ = adapter.disconnect();
}

fn run_ftp_rename(
    protocol: SessionProtocol,
    host: String,
    port: String,
    username: String,
    from: String,
    to: String,
) {
    let mut adapter = match connect_ftp(protocol, &host, &port, &username) {
        Ok(adapter) => adapter,
        Err(error) => exit_error(&format!("ftp-rename failed: {error}"), 1),
    };
    match adapter.rename(&from, &to) {
        Ok(()) => println!("renamed\t{from}\t{to}"),
        Err(error) => exit_error(&format!("ftp-rename failed: {error}"), 1),
    }
    let _ = adapter.disconnect();
}

fn connect_ftp(
    protocol: SessionProtocol,
    host: &str,
    port: &str,
    username: &str,
) -> Result<FtpAdapter, FtpError> {
    let port = port
        .parse::<u16>()
        .map_err(|_| FtpError::Backend(format!("invalid port: {port}")))?;
    let reference = CredentialRef::new("env://CROSSSCP_REMOTE_PASSWORD")?;
    let mut credentials = InMemoryCredentialService::new();
    credentials.store(reference.clone(), ftp_secret_from_env()?)?;
    let config = FtpConnectionConfig {
        protocol: protocol.clone(),
        host: host.to_string(),
        port,
        username: non_empty(username.to_string()),
        passive_mode: true,
        ftps_mode: (protocol == SessionProtocol::Ftps).then_some(FtpsMode::Explicit),
        initial_remote_path: None,
        credential_ref: Some(reference),
    };
    let auth = resolve_ftp_credentials(&config, &credentials)?;
    let mut adapter = FtpAdapter::new(config, auth);
    adapter.connect()?;
    Ok(adapter)
}

fn ftp_secret_from_env() -> Result<CredentialSecret, FtpError> {
    let password = std::env::var("CROSSSCP_REMOTE_PASSWORD")
        .or_else(|_| std::env::var("CROSSSCP_FTP_PASSWORD"))
        .or_else(|_| std::env::var("CROSSSCP_FTPS_PASSWORD"))
        .map_err(|_| FtpError::Backend("CROSSSCP_REMOTE_PASSWORD is required".to_string()))?;
    Ok(CredentialSecret::Password(SecretString::new(password)?))
}

fn print_local_profile(name: Option<String>) {
    let profile = SessionProfile::local(name.unwrap_or_else(|| "local".to_string()));
    println!("local profile: {}", profile.name);
}

fn run_mask_check(args: Vec<String>) {
    if args.len() != 3 {
        print_help();
        std::process::exit(2);
    }

    let masks = MaskSet::new().include(&args[0]).exclude(&args[1]);
    match masks.decide(&args[2]) {
        MaskDecision::Included => println!("included"),
        MaskDecision::Excluded => println!("excluded"),
    }
}

fn run_local_list(path: Option<String>) {
    let Some(path) = path else {
        print_help();
        std::process::exit(2);
    };
    let adapter = LocalFileSystem::new();
    match adapter.list_directory(&path) {
        Ok(entries) => {
            for entry in entries {
                let kind = if entry.is_directory { "dir" } else { "file" };
                println!("{kind}\t{}\t{}", entry.size.unwrap_or(0), entry.name);
            }
        }
        Err(error) => {
            eprintln!("local-list failed: {error}");
            std::process::exit(1);
        }
    }
}

fn run_local_copy(args: Vec<String>) {
    if !(2..=3).contains(&args.len()) {
        print_help();
        std::process::exit(2);
    }

    let overwrite_mode = args.get(2).map_or(Ok(OverwriteMode::Always), |value| {
        parse_overwrite_mode(value)
    });
    let overwrite_mode = match overwrite_mode {
        Ok(mode) => mode,
        Err(message) => exit_error(&message, 2),
    };

    let options = TransferOptions {
        overwrite_mode,
        ..TransferOptions::default()
    };
    let mut queue = TransferQueue::new();
    let job_id = queue.enqueue(
        TransferDirection::LocalCopy,
        args[0].clone(),
        args[1].clone(),
        options,
    );

    let adapter = LocalFileSystem::new();
    match adapter.execute_next_local_copy(&mut queue) {
        Ok(Some(progress)) => println!(
            "completed\t{}\t{}\t{}",
            job_id.as_u64(),
            progress.bytes_done,
            progress.bytes_total.unwrap_or(progress.bytes_done)
        ),
        Ok(None) => println!("skipped\t{}\t0\t0", job_id.as_u64()),
        Err(error) => exit_error(&format!("local-copy failed: {error}"), 1),
    }
}

fn run_session_list(path: Option<String>) {
    let Some(path) = path else {
        print_help();
        std::process::exit(2);
    };
    let store = match FileSessionStore::open(path) {
        Ok(store) => store,
        Err(error) => exit_error(&format!("session-list failed: {error}"), 1),
    };
    let profiles = match store.list() {
        Ok(profiles) => profiles,
        Err(error) => exit_error(&format!("session-list failed: {error}"), 1),
    };
    for profile in profiles {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            profile.name,
            protocol_label(&profile.protocol),
            profile.host,
            profile.port.unwrap_or(22),
            profile.username.unwrap_or_default(),
            profile
                .initial_remote_path
                .unwrap_or_else(|| "/".to_string()),
            profile.credential_ref.unwrap_or_default()
        );
    }
}

fn run_session_save(args: Vec<String>) {
    if args.len() != 7 && args.len() != 8 {
        print_help();
        std::process::exit(2);
    }
    let (protocol, name_idx, host_idx, port_idx, username_idx, path_idx, credential_idx) =
        if args.len() == 8 {
            let protocol = args[1]
                .parse::<SessionProtocol>()
                .unwrap_or_else(|_| exit_error("session-save failed: invalid protocol", 2));
            (protocol, 2, 3, 4, 5, 6, 7)
        } else {
            (SessionProtocol::Sftp, 1, 2, 3, 4, 5, 6)
        };
    let port = match args[port_idx].parse::<u16>() {
        Ok(port) => port,
        Err(_) => exit_error("session-save failed: port must be 1-65535", 2),
    };
    let profile = SessionProfile {
        name: args[name_idx].clone(),
        protocol,
        host: args[host_idx].clone(),
        port: Some(port),
        username: non_empty(args[username_idx].clone()),
        initial_remote_path: non_empty(args[path_idx].clone()),
        credential_ref: non_empty(args[credential_idx].clone()),
    };
    let mut store = match FileSessionStore::open(&args[0]) {
        Ok(store) => store,
        Err(error) => exit_error(&format!("session-save failed: {error}"), 1),
    };
    if let Err(error) = store.save(profile) {
        exit_error(&format!("session-save failed: {error}"), 1);
    }
    println!("saved\t{}", args[name_idx]);
}

fn run_session_delete(args: Vec<String>) {
    if args.len() != 2 {
        print_help();
        std::process::exit(2);
    }
    let mut store = match FileSessionStore::open(&args[0]) {
        Ok(store) => store,
        Err(error) => exit_error(&format!("session-delete failed: {error}"), 1),
    };
    match store.remove(&args[1]) {
        Ok(true) => println!("deleted\t{}", args[1]),
        Ok(false) => println!("missing\t{}", args[1]),
        Err(error) => exit_error(&format!("session-delete failed: {error}"), 1),
    }
}

#[derive(Clone, Copy)]
enum SftpTransferKind {
    Upload,
    Download,
}

#[cfg(feature = "ssh2-backend")]
fn run_sftp_list(args: Vec<String>) {
    if args.len() != 4 {
        print_help();
        std::process::exit(2);
    }
    let mut adapter = match connect_sftp(&args[0], &args[1], &args[2]) {
        Ok(adapter) => adapter,
        Err(error) => exit_error(&format!("sftp-list failed: {error}"), 1),
    };
    match adapter.list_directory(&args[3]) {
        Ok(entries) => {
            for entry in entries {
                let kind = if entry.is_directory { "dir" } else { "file" };
                println!(
                    "{kind}\t{}\t{}\t{}",
                    entry.size.unwrap_or(0),
                    entry.path,
                    entry.name
                );
            }
        }
        Err(error) => exit_error(&format!("sftp-list failed: {error}"), 1),
    }
}

#[cfg(not(feature = "ssh2-backend"))]
fn run_sftp_list(_args: Vec<String>) {
    exit_error("sftp-list requires crossscp-cli --features ssh2-backend", 1);
}

#[cfg(feature = "ssh2-backend")]
fn run_sftp_transfer(
    args: Vec<String>,
    kind: SftpTransferKind,
    conflict_policy: FileConflictPolicy,
    exact_destination: bool,
) {
    if args.len() != 5 {
        print_help();
        std::process::exit(2);
    }
    let mut adapter = match connect_sftp(&args[0], &args[1], &args[2]) {
        Ok(adapter) => adapter,
        Err(error) => exit_error(&format!("sftp transfer failed: {error}"), 1),
    };
    let result = match kind {
        SftpTransferKind::Upload if exact_destination => adapter
            .backend_mut()
            .upload_file_to_exact_destination_with_progress(
                &args[3],
                &args[4],
                conflict_policy,
                print_transfer_progress,
            ),
        SftpTransferKind::Upload => adapter.backend_mut().upload_file_with_progress_policy(
            &args[3],
            &args[4],
            conflict_policy,
            print_transfer_progress,
        ),
        SftpTransferKind::Download => adapter.backend_mut().download_file_with_progress(
            &args[3],
            &args[4],
            print_transfer_progress,
        ),
    };
    match result {
        Ok(progress) => print_sftp_progress(progress),
        Err(error) => exit_error(&format!("sftp transfer failed: {error}"), 1),
    }
}

fn print_transfer_progress(bytes_done: u64, bytes_total: Option<u64>) {
    eprintln!("progress\t{bytes_done}\t{}", bytes_total.unwrap_or(0));
    let _ = std::io::stderr().flush();
}

#[cfg(feature = "ssh2-backend")]
fn run_sftp_mkdir(args: Vec<String>) {
    if args.len() != 4 {
        print_help();
        std::process::exit(2);
    }
    let mut adapter = match connect_sftp(&args[0], &args[1], &args[2]) {
        Ok(adapter) => adapter,
        Err(error) => exit_error(&format!("sftp-mkdir failed: {error}"), 1),
    };
    match adapter.create_directory(&args[3]) {
        Ok(()) => println!("created\t{}", args[3]),
        Err(error) => exit_error(&format!("sftp-mkdir failed: {error}"), 1),
    }
}

#[cfg(feature = "ssh2-backend")]
fn run_sftp_delete(args: Vec<String>) {
    if args.len() != 4 {
        print_help();
        std::process::exit(2);
    }
    let mut adapter = match connect_sftp(&args[0], &args[1], &args[2]) {
        Ok(adapter) => adapter,
        Err(error) => exit_error(&format!("sftp-delete failed: {error}"), 1),
    };
    match adapter.delete_path(&args[3]) {
        Ok(()) => println!("deleted\t{}", args[3]),
        Err(error) => exit_error(&format!("sftp-delete failed: {error}"), 1),
    }
}

#[cfg(not(feature = "ssh2-backend"))]
fn run_sftp_delete(_args: Vec<String>) {
    exit_error(
        "sftp-delete requires crossscp-cli --features ssh2-backend",
        1,
    );
}

#[cfg(not(feature = "ssh2-backend"))]
fn run_sftp_mkdir(_args: Vec<String>) {
    exit_error(
        "sftp-mkdir requires crossscp-cli --features ssh2-backend",
        1,
    );
}

#[cfg(not(feature = "ssh2-backend"))]
fn run_sftp_transfer(
    _args: Vec<String>,
    _kind: SftpTransferKind,
    _conflict_policy: FileConflictPolicy,
    _exact_destination: bool,
) {
    exit_error(
        "sftp transfer requires crossscp-cli --features ssh2-backend",
        1,
    );
}

#[cfg(feature = "ssh2-backend")]
fn connect_sftp(
    host: &str,
    port: &str,
    username: &str,
) -> Result<SftpAdapter<Ssh2Backend>, SftpError> {
    let port = port
        .parse::<u16>()
        .map_err(|_| SftpError::Backend(format!("invalid port: {port}")))?;
    let reference = CredentialRef::new("env://CROSSSCP_REMOTE_PASSWORD")?;
    let mut credentials = InMemoryCredentialService::new();
    let secret = sftp_secret_from_env()?;
    credentials.store(reference.clone(), secret)?;
    let config = SftpConnectionConfig {
        host: host.to_string(),
        port,
        username: non_empty(username.to_string()),
        initial_remote_path: None,
        credential_ref: Some(reference),
    };
    let auth = resolve_sftp_credentials(&config, &credentials)?;
    let mut adapter = SftpAdapter::new(config, Ssh2Backend::new(auth).with_timeout(sftp_timeout()));
    adapter.connect()?;
    Ok(adapter)
}

fn sftp_timeout() -> Duration {
    std::env::var("CROSSSCP_SFTP_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(300))
}

#[cfg(feature = "ssh2-backend")]
fn sftp_secret_from_env() -> Result<CredentialSecret, SftpError> {
    ssh_secret_from_env("SFTP").map_err(|error| SftpError::Backend(error.to_string()))
}

fn ssh_secret_from_env(protocol: &str) -> Result<CredentialSecret, ScpError> {
    let protocol_key_path = format!("CROSSSCP_{protocol}_KEY_PATH");
    let protocol_passphrase = format!("CROSSSCP_{protocol}_KEY_PASSPHRASE");
    let protocol_password = format!("CROSSSCP_{protocol}_PASSWORD");
    if let Ok(private_key_path) = std::env::var("CROSSSCP_REMOTE_PRIVATE_KEY_PATH")
        .or_else(|_| std::env::var(&protocol_key_path))
    {
        if !private_key_path.trim().is_empty() {
            let passphrase = std::env::var("CROSSSCP_REMOTE_PRIVATE_KEY_PASSPHRASE")
                .or_else(|_| std::env::var(&protocol_passphrase))
                .ok()
                .filter(|value| !value.is_empty())
                .map(SecretString::new)
                .transpose()?;
            return Ok(CredentialSecret::PrivateKey {
                private_key_path,
                passphrase,
            });
        }
    }

    let password = std::env::var("CROSSSCP_REMOTE_PASSWORD")
        .or_else(|_| std::env::var(&protocol_password))
        .map_err(|_| {
            ScpError::Backend(
                format!("CROSSSCP_REMOTE_PASSWORD/{protocol_password} or CROSSSCP_REMOTE_PRIVATE_KEY_PATH/{protocol_key_path} is required"),
            )
        })?;
    Ok(CredentialSecret::Password(SecretString::new(password)?))
}

#[cfg(feature = "ssh2-backend")]
fn print_sftp_progress(progress: SftpFileProgress) {
    println!(
        "completed\t{}\t{}\t{}",
        progress.source,
        progress.destination,
        progress.bytes_total.unwrap_or(progress.bytes_done)
    );
}

fn print_sftp_live_config() {
    match LiveSftpTestConfig::from_env() {
        Ok(Some(config)) => {
            println!("host={}", config.host);
            println!("port={}", config.port);
            println!("username={}", config.username);
            println!("credential_ref={}", config.credential_ref);
            println!("list_path={}", config.initial_list_path());
            if let Some((local, remote)) = config.transfer_paths() {
                println!("local_file={local}");
                println!("remote_file={remote}");
            }
        }
        Ok(None) => {
            println!("live SFTP config not set");
        }
        Err(error) => {
            eprintln!("invalid live SFTP config: {error}");
            std::process::exit(1);
        }
    }
}

fn parse_overwrite_mode(value: &str) -> Result<OverwriteMode, String> {
    match value {
        "ask" => Ok(OverwriteMode::Ask),
        "always" => Ok(OverwriteMode::Always),
        "never" => Ok(OverwriteMode::Never),
        "if-newer" => Ok(OverwriteMode::IfNewer),
        "resume" => Ok(OverwriteMode::Resume),
        _ => Err(format!("unknown overwrite mode: {value}")),
    }
}

fn parse_file_conflict_policy(value: &str) -> Result<FileConflictPolicy, String> {
    match value {
        "keep-existing" => Ok(FileConflictPolicy::KeepExisting),
        "replace" => Ok(FileConflictPolicy::Replace),
        "keep-both" => Ok(FileConflictPolicy::KeepBoth),
        _ => Err(format!("unknown file conflict policy: {value}")),
    }
}

fn protocol_label(protocol: &SessionProtocol) -> &'static str {
    match protocol {
        SessionProtocol::Sftp => "sftp",
        SessionProtocol::Scp => "scp",
        SessionProtocol::Ftp => "ftp",
        SessionProtocol::Ftps => "ftps",
        SessionProtocol::WebDav => "webdav",
        SessionProtocol::S3 => "s3",
        SessionProtocol::Local => "local",
    }
}

fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn exit_error(message: &str, code: i32) -> ! {
    eprintln!("{message}");
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use crossscp_core::FileConflictPolicy;

    use super::parse_remote_args;

    #[test]
    fn remote_upload_parses_keep_both_conflict_policy() {
        let parsed = parse_remote_args(vec![
            "--protocol".to_string(),
            "sftp".to_string(),
            "--conflict".to_string(),
            "keep-both".to_string(),
        ])
        .expect("remote arguments parse");

        assert_eq!(parsed.conflict, Some(FileConflictPolicy::KeepBoth));
    }

    #[test]
    fn remote_upload_rejects_unknown_conflict_policy() {
        let error = parse_remote_args(vec![
            "--conflict".to_string(),
            "rename-randomly".to_string(),
        ])
        .expect_err("unknown policy must fail");

        assert!(error.contains("unknown file conflict policy"));
    }
}
