# CrossSCP Installation Guide

CrossSCP is currently distributed as beta/test packages. The packages are useful
for testing, but they are not yet distributed through every platform's official
trust channel.

## Quick install: macOS testers

Recommended no-Apple-Developer-account tester install:

```bash
curl -fsSL https://raw.githubusercontent.com/pat229988/Cross-SCP/dev/scripts/install-macos.sh | bash
```

What this script does:

1. Finds the latest published CrossSCP macOS DMG release asset.
2. Downloads the DMG with `curl`.
3. Mounts it.
4. Copies `CrossSCP.app` to `/Applications`.
5. Removes the `com.apple.quarantine` attribute from the installed app.
6. Opens CrossSCP.

This is the cleanest free tester path because it avoids the browser-download
quarantine flow that causes the strict Gatekeeper dialog.

### macOS installer options

Install to another folder:

```bash
curl -fsSL https://raw.githubusercontent.com/pat229988/Cross-SCP/dev/scripts/install-macos.sh \
  | CROSSSCP_INSTALL_DIR="$HOME/Applications" bash
```

Install a specific tag:

```bash
curl -fsSL https://raw.githubusercontent.com/pat229988/Cross-SCP/dev/scripts/install-macos.sh \
  | CROSSSCP_VERSION="v0.1.0-beta.3" bash
```

Install from a direct DMG URL:

```bash
curl -fsSL https://raw.githubusercontent.com/pat229988/Cross-SCP/dev/scripts/install-macos.sh \
  | CROSSSCP_DMG_URL="https://example.com/CrossSCP.dmg" bash
```

If the release is still a GitHub draft/private release, set `GITHUB_TOKEN` or use
`CROSSSCP_DMG_URL` with an accessible URL.

## macOS manual install

If you download the DMG through a browser, macOS attaches quarantine metadata.
Without Apple Developer ID notarization, recent macOS versions may show only:

```text
Move to Trash
Done
```

If that happens:

1. Drag `CrossSCP.app` to `/Applications`.
2. Run:

```bash
sudo xattr -dr com.apple.quarantine /Applications/CrossSCP.app
open /Applications/CrossSCP.app
```

If you installed to `~/Applications` instead:

```bash
xattr -dr com.apple.quarantine "$HOME/Applications/CrossSCP.app"
open "$HOME/Applications/CrossSCP.app"
```

## Why macOS behaves differently for local builds

Local builds usually launch because they were not downloaded by a browser and do
not have the `com.apple.quarantine` attribute. GitHub/browser downloads do have
that attribute, so Gatekeeper performs stricter checks.

The fully polished public macOS path is Apple Developer ID signing and Apple
notarization. That requires an Apple Developer Program subscription. CrossSCP's
free beta path uses the installer script above instead.

## Windows beta install

Download the Windows artifact from the GitHub Actions run or release:

- `CrossSCP-...-windows-x64-setup.exe` — NSIS installer
- `CrossSCP-...-windows-x64-portable.zip` — portable directory

Unsigned Windows beta builds may show Microsoft SmartScreen warnings. Choose
**More info → Run anyway** only if you trust the source and checksum.

The Windows installer and portable ZIP are built to include the required Qt DLLs
and Microsoft Visual C++ runtime DLLs, including files such as:

- `Qt6Core.dll`
- `Qt6Gui.dll`
- `Qt6Qml.dll`
- `Qt6Quick.dll`
- `vcruntime140.dll`
- `msvcp140.dll`

If Windows reports one of these DLLs is missing, the package is incomplete and
should be rebuilt from the latest workflow artifacts.

For public trust, Windows installers need Authenticode code signing. The current
workflow supports code signing when certificate secrets are configured.

## Ubuntu beta install

The `.deb` package is for Debian/Ubuntu-based systems. It is not the right
package format for Fedora.

Download the `.deb` artifact, then run:

```bash
sudo apt install ./CrossSCP-*-ubuntu-amd64.deb
```

Standalone downloaded `.deb` files are not equivalent to packages from a signed
APT repository. For public Linux distribution, a signed APT repository or PPA is
the trusted long-term path.

The `.deb` file is expected to be relatively small because it depends on system
Qt/QML packages instead of bundling the whole Qt runtime.

## Fedora beta install

Fedora uses RPM packages, not DEB packages. Download the Fedora RPM artifact,
then run:

```bash
sudo dnf install ./CrossSCP-*-fedora-x86_64.rpm
```

If DNF warns that the local RPM is unsigned, install only if you trust the
GitHub Actions artifact and checksum. For public Fedora distribution, a signed
RPM repository/COPR is the better long-term path.

The `.rpm` file is expected to be relatively small because DNF installs the
required Qt/QML runtime packages from Fedora repositories.

## Flatpak beta install

Download the `.flatpak` artifact. On Fedora, first make sure Flathub is enabled
so Flatpak can fetch the KDE runtime referenced by the CrossSCP bundle:

```bash
sudo dnf install flatpak
flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
```

Then install and run CrossSCP:

```bash
flatpak install --user ./CrossSCP-*-linux-x86_64.flatpak
flatpak run org.crossscp.CrossSCP
```

The `.flatpak` file may still be only a few MB because it contains the CrossSCP
app layer, while the shared KDE runtime is downloaded separately from Flathub.

For public Flatpak trust and better UX, CrossSCP should eventually publish
through Flathub or a signed Flatpak repository.

## Checksums and signatures

Each package workflow emits `.sha256` checksums. Verify an artifact with:

```bash
shasum -a 256 -c CrossSCP-*.sha256
```

On Linux, use `sha256sum -c CrossSCP-*.sha256` if `shasum` is unavailable.

Linux artifacts may also include `.asc` detached signatures if GPG signing
secrets are configured.
