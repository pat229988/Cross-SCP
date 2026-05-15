# CrossSCP Signing and Distribution Trust

This project can build unsigned test artifacts from `dev`, but public artifacts
must be signed through each platform's trust model before normal users can open
or install them without scary warnings.

## macOS DMG trust

macOS Gatekeeper blocks unsigned or ad-hoc signed apps downloaded from the
internet. A DMG is broadly usable only when the app is:

1. Signed with an Apple Developer ID Application certificate.
2. Built with the hardened runtime.
3. Submitted to Apple notarization.
4. Stapled after notarization succeeds.

Required GitHub repository secrets for tagged release builds:

```text
CROSSSCP_SIGN_IDENTITY              # Example: Developer ID Application: Name (TEAMID)
MACOS_CERTIFICATE_P12               # base64-encoded Developer ID .p12
MACOS_CERTIFICATE_PASSWORD          # password used when exporting the .p12
APPLE_ID                            # Apple ID email
APPLE_TEAM_ID                       # Developer team ID
APPLE_APP_SPECIFIC_PASSWORD         # app-specific password for notarytool
```

Export a certificate for GitHub Actions:

```bash
# Export from Keychain Access as .p12 first, then base64 encode it.
base64 -i DeveloperIDApplication.p12 | pbcopy
```

Tagged macOS release workflows intentionally fail when these secrets are absent,
because uploading an unsigned DMG creates the exact Gatekeeper block users saw.
Manual/dev builds may still be unsigned for smoke testing.

## Windows setup EXE trust

Windows SmartScreen warns about unsigned installers and may continue warning for
new certificates until reputation is established. The best public path is an EV
or OV code-signing certificate with Authenticode timestamping.

Required GitHub repository secrets for tagged Windows builds on hosted runners:

```text
WINDOWS_CODE_SIGN_CERT_PFX          # base64-encoded Authenticode .pfx
WINDOWS_CODE_SIGN_CERT_PASSWORD     # PFX password
```

Alternative for self-hosted runners with certs already installed:

```text
CROSSSCP_SIGN_CERT_THUMBPRINT       # certificate thumbprint in Windows cert store
```

The workflow signs `CrossSCP.exe`, `crossscp-cli.exe`, and the final NSIS setup
EXE. The portable ZIP itself is not signed, but the executable files inside it
are signed when credentials are available.

## Ubuntu DEB trust

A standalone `.deb` can include checksums and optional detached GPG signatures,
but Ubuntu does not treat random downloaded `.deb` files as trusted just because
they are signed. For broad trust, publish through a signed APT repository or a
Launchpad PPA.

Optional secrets for detached artifact signatures:

```text
LINUX_GPG_PRIVATE_KEY               # armored private key for artifact signing
LINUX_GPG_PASSPHRASE                # key passphrase, if any
```

## Flatpak trust

A one-file `.flatpak` bundle is useful for beta testing but is still an external
bundle from the user's perspective. For broad Linux trust and better UX, publish
through Flathub or a signed Flatpak repository.

The current workflow can emit `.flatpak`, `.sha256`, and optional `.asc` detached
signatures. Flathub submission remains the recommended public distribution path.
