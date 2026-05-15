# CrossSCP Packaging Guide

This directory contains productization notes and packaging skeletons.

## Current package state

CrossSCP is moving out of core MVP scaffolding into productization. The current package setup provides:

- Cargo workspace validation for Rust crates.
- Optional Qt GUI bundle target through CMake.
- Product icons copied into `apps/crossscp-gui/resources/icons/`.
- macOS app bundle metadata skeleton.
- Windows icon/version resource skeleton.
- CPack archive/DMG skeleton.

## Default validation

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p crossscp-protocol-sftp --features ssh2-backend
cargo clippy -p crossscp-protocol-sftp --features ssh2-backend --all-targets -- -D warnings
cmake -S . -B build/package-check
```

## GUI package build

Requires Qt 6 development tools in `PATH`/`CMAKE_PREFIX_PATH`.

```bash
cmake -S . -B build/gui -DCROSSSCP_BUILD_GUI=ON -DCMAKE_BUILD_TYPE=Release
cmake --build build/gui --config Release
cmake --build build/gui --target package --config Release
```

## Release blockers

- Real Qt deployment tooling must be added per platform (`macdeployqt`, `windeployqt`).
- Code signing/notarization is not configured.
- Windows installer technology is not finalized (WiX/MSIX/NSIS pending).
- License inventory must be reviewed with `cargo deny` or equivalent.
- Live SFTP integration tests are still gated/manual.

See also:

- `DOC/V1_RELEASE_PLAN.md` for the release acceptance criteria and implementation phase plan.
- `DOC/PLATFORM_TOOL_CHECKLIST.md` for platform-specific build, deployment, signing, and packaging tools.

## Helper scripts

```bash
scripts/validate.sh
scripts/release-check.sh
scripts/generate-macos-icons.sh
scripts/package-gui.sh
scripts/deploy-macos.sh
scripts/notarize-macos.sh <dmg>
```

Windows deployment helper:

```powershell
pwsh scripts/deploy-windows.ps1
```
