# CrossSCP

CrossSCP is a cross-platform file transfer client built with Rust and Qt. It provides a legacy SFTP client-inspired dual-pane interface for local and remote SFTP file operations.

## Highlights

- Dual-pane local and remote SFTP browsing
- Password and SSH private-key authentication
- Upload and download files or folders
- Multiple file/folder selection for batch transfers
- Create and delete local or remote folders/files
- Session-only transfer queue and activity logs
- macOS `.app`/`.dmg`, Windows installer/portable ZIP, and Linux DEB/RPM/Flatpak packaging

## Creator

Created by **Pratik Patel**.

- GitHub: [pat229988](https://github.com/pat229988)
- Repository: [Cross-SCP](https://github.com/pat229988/Cross-SCP.git)
- Website: [CrossSCP GitHub Pages](https://pat229988.github.io/Cross-SCP/)

## macOS Build

### Tester install without Apple Developer notarization

For macOS testers, use the terminal installer so the app can be copied and the
browser quarantine attribute can be removed automatically:

```bash
curl -fsSL https://raw.githubusercontent.com/pat229988/Cross-SCP/dev/scripts/install-macos.sh | bash
```

See [`docs/INSTALLATION.md`](docs/INSTALLATION.md) for macOS Gatekeeper details,
Windows/Linux install notes, and release trust expectations.

### Local package build

The canonical packaged artifact is:

```bash
dist/macos/CrossSCP-macos-arm64.dmg
```

To rebuild locally on macOS:

```bash
bash scripts/package-gui.sh
```

## GitHub Pages

A simple static landing page is available under `docs/index.html`. GitHub Pages
deployment is optional and may depend on the repository/account plan; packaging
and release testing do not require it.
