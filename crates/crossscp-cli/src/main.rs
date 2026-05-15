// SPDX-License-Identifier: AGPL-3.0-or-later

//! Initial CrossSCP CLI scaffold and GUI service bridge.

use crossscp_config::{FileSessionStore, SessionStore};
use crossscp_core::RemoteFileSystem;
use crossscp_core::{MaskDecision, MaskSet, SessionProfile, SessionProtocol};
use crossscp_protocol_local::LocalFileSystem;
use crossscp_protocol_sftp::LiveSftpTestConfig;
use crossscp_transfer::{OverwriteMode, TransferDirection, TransferOptions, TransferQueue};

#[cfg(feature = "ssh2-backend")]
use crossscp_protocol_sftp::ssh2_backend::Ssh2Backend;
#[cfg(feature = "ssh2-backend")]
use crossscp_protocol_sftp::{
    resolve_sftp_credentials, SftpAdapter, SftpConnectionConfig, SftpError, SftpFileProgress,
};
#[cfg(feature = "ssh2-backend")]
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
        Some("sftp-upload") => run_sftp_transfer(args.collect(), SftpTransferKind::Upload),
        Some("sftp-download") => run_sftp_transfer(args.collect(), SftpTransferKind::Download),
        Some("sftp-mkdir") => run_sftp_mkdir(args.collect()),
        Some("sftp-delete") => run_sftp_delete(args.collect()),
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
    println!("  crossscp session-save <sessions.tsv> <name> <host> <port> <username> <remote-path> <credential-ref>");
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
    println!("  crossscp mask-check <include-pattern> <exclude-pattern> <path>");
    println!("  crossscp sftp-live-config");
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
    if args.len() != 7 {
        print_help();
        std::process::exit(2);
    }
    let port = match args[3].parse::<u16>() {
        Ok(port) => port,
        Err(_) => exit_error("session-save failed: port must be 1-65535", 2),
    };
    let profile = SessionProfile {
        name: args[1].clone(),
        protocol: SessionProtocol::Sftp,
        host: args[2].clone(),
        port: Some(port),
        username: non_empty(args[4].clone()),
        initial_remote_path: non_empty(args[5].clone()),
        credential_ref: non_empty(args[6].clone()),
    };
    let mut store = match FileSessionStore::open(&args[0]) {
        Ok(store) => store,
        Err(error) => exit_error(&format!("session-save failed: {error}"), 1),
    };
    if let Err(error) = store.save(profile) {
        exit_error(&format!("session-save failed: {error}"), 1);
    }
    println!("saved\t{}", args[1]);
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
fn run_sftp_transfer(args: Vec<String>, kind: SftpTransferKind) {
    if args.len() != 5 {
        print_help();
        std::process::exit(2);
    }
    let mut adapter = match connect_sftp(&args[0], &args[1], &args[2]) {
        Ok(adapter) => adapter,
        Err(error) => exit_error(&format!("sftp transfer failed: {error}"), 1),
    };
    let result = match kind {
        SftpTransferKind::Upload => adapter.upload_file(&args[3], &args[4]),
        SftpTransferKind::Download => adapter.download_file(&args[3], &args[4]),
    };
    match result {
        Ok(progress) => print_sftp_progress(progress),
        Err(error) => exit_error(&format!("sftp transfer failed: {error}"), 1),
    }
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
fn run_sftp_transfer(_args: Vec<String>, _kind: SftpTransferKind) {
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
    let reference = CredentialRef::new("env://CROSSSCP_SFTP_PASSWORD")?;
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
    let mut adapter = SftpAdapter::new(config, Ssh2Backend::new(auth));
    adapter.connect()?;
    Ok(adapter)
}

#[cfg(feature = "ssh2-backend")]
fn sftp_secret_from_env() -> Result<CredentialSecret, SftpError> {
    if let Ok(private_key_path) = std::env::var("CROSSSCP_SFTP_KEY_PATH") {
        if !private_key_path.trim().is_empty() {
            let passphrase = std::env::var("CROSSSCP_SFTP_KEY_PASSPHRASE")
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

    let password = std::env::var("CROSSSCP_SFTP_PASSWORD").map_err(|_| {
        SftpError::Backend(
            "CROSSSCP_SFTP_PASSWORD or CROSSSCP_SFTP_KEY_PATH is required".to_string(),
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
