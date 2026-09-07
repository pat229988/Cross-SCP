# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## [1.0.2] - 2026-07-22

### Added

- **Upload Conflict Handling**
  - Added Keep existing, Replace, and Keep both choices for SFTP and FTP/FTPS uploads.
  - Keep both preserves file extensions and appends an incrementing number to the new name.
  - Existing destination folders are merged so only conflicting files follow the selected policy.

- **SCP Transfer Support**
  - Added live SCP upload/download support using `ssh2`/libssh2.
  - Wired SCP into protocol-neutral CLI transfer commands and GUI transfer-only connection flow.
  - SCP reports unsupported operations clearly for remote browsing, mkdir, and delete.

- **FTP/FTPS Support**
  - Added live FTP and explicit FTPS backend support using `suppaftp`.
  - Wired protocol-neutral CLI commands and GUI remote pane/queue flows for FTP/FTPS.
  - FTPS uses certificate verification through the native TLS backend by default.

### Changed

- **SFTP Transfer Performance**
  - Tuned the Rust `ssh2` SFTP progress copy path to use an 8 MiB streaming buffer.
  - Throttled SFTP progress callbacks to reduce CLI/GUI progress-event overhead while still reporting completion.

### Fixed

- Prevented repeated uploads of the same folder from creating a nested duplicate folder.

## [1.0.1] - 2026-05-20

### Added

- **Transfer Queue UX Improvements**
  - Async transfer queue using QProcess to prevent UI hangs
  - Real-time progress bars with percentage and transfer speed display
  - Queue state management (pending, transferring, completed, failed)

- **Nested Folder Transfer Support**
  - Progress tracking for recursive folder uploads and downloads
  - Individual file progress within nested transfers
  - Proper handling of large directory structures

- **Theme Support**
  - System theme detection (light/dark mode)
  - Manual theme toggle in UI
  - Neutral theme colors (white/black) for better visibility

- **SFTP Timeout Configuration**
  - Increased default SFTP timeout to 300 seconds
  - Configurable via `CROSSSCP_SFTP_TIMEOUT` environment variable

### Fixed

- Queue header highlight visibility
- SplitView resize behavior (fixed heights instead of window-bound binding)
- Disconnect action now clears queue and session logs
- Queue watermark visibility bug
- Selection clearing when changing folders

### Changed

- Transfer queue implementation from blocking runCommand to async QProcess
- Theme colors from blue/blue-grey to neutral greys

## [1.0.0] - 2026-05-16

### Added

- Initial public beta release
- Cross-platform file transfer via SFTP (SCP is planned but not live yet)
- GUI application for macOS, Windows, Linux
- CLI tool for automation
- GitHub Pages documentation and downloads
