# CrossSCP

CrossSCP is a cross-platform file transfer client built with Rust and Qt. It provides a legacy SFTP client-inspired dual-pane interface for local and remote SFTP file operations.

## Highlights

- Dual-pane local and remote SFTP browsing
- Password and SSH private-key authentication
- Upload and download files or folders
- Multiple file/folder selection for batch transfers
- Create and delete local or remote folders/files
- Session-only transfer queue and activity logs
- macOS `.app` and `.dmg` packaging

## Creator

Created by **Pratik Patel**.

- GitHub: [pat229988](https://github.com/pat229988)
- Repository: [Cross-SCP](https://github.com/pat229988/Cross-SCP.git)

## macOS Build

The canonical packaged artifact is:

```bash
dist/macos/CrossSCP-macos-arm64.dmg
```

To rebuild locally on macOS:

```bash
bash scripts/package-gui.sh
```

## GitHub Pages

A simple static landing page is available under `docs/index.html`. Enable GitHub Pages for the repository and select the `docs/` folder as the Pages source.
