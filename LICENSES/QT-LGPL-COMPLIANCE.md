# Qt LGPL Compliance Checklist

CrossSCP currently remains licensed as `AGPL-3.0-or-later`. The Qt dependency
is separate third-party software. This checklist documents the intended path
for using open-source Qt without a commercial Qt license.

This is an engineering compliance checklist, not legal advice.

## Required release posture

1. Use dynamically linked Qt libraries only.
2. Include `THIRD_PARTY_NOTICES.md` and the `LICENSES/` directory in binary
   release artifacts where practical.
3. Record the exact Qt version used for each release.
4. Provide source links for the exact Qt release series used.
5. Do not modify Qt unless modified Qt source publication is also planned.
6. Do not add DRM, signature checks, or runtime checks whose purpose is to
   prevent replacing LGPL-covered Qt libraries.

## Platform-specific expectations

### macOS

- `macdeployqt` may copy Qt frameworks into:
  `CrossSCP.app/Contents/Frameworks/`.
- Legal notices should be present in:
  `CrossSCP.app/Contents/Resources/Legal/`.
- Codesigning/notarization may be used for platform trust, but release notes
  should not claim that users are forbidden from replacing Qt frameworks with
  compatible modified versions.

### Windows

- `windeployqt` may copy Qt DLLs next to `CrossSCP.exe`.
- Legal notices should be present in a `Legal/` directory inside the deployed
  application directory and therefore included in installer/portable ZIPs.
- Users should be able to replace Qt DLLs with compatible modified DLLs.

### Linux

- Prefer depending on distro Qt packages for DEB/RPM builds.
- If bundling Qt shared objects, include the same notices and preserve user
  ability to replace the shared objects.

## Qt source links

- Qt source browser: <https://code.qt.io/cgit/qt/>
- Official Qt source archives: <https://download.qt.io/official_releases/qt/>
- Qt licensing overview: <https://www.qt.io/licensing/open-source-lgpl-obligations>

## Release checklist

Before publishing a GUI release:

- [ ] `qmake6 -query QT_VERSION` was recorded in release notes.
- [ ] macOS/Windows packages contain dynamic Qt frameworks/DLLs, not static Qt.
- [ ] `THIRD_PARTY_NOTICES.md` is included in the packaged app/legal folder.
- [ ] `LICENSES/THIRD_PARTY_LICENSES.md` is included in the packaged app/legal folder.
- [ ] Exact Qt source links are included in release notes or notices.
- [ ] No local Qt modifications were made, or modified Qt source is published.
- [ ] Users are not technically blocked from replacing Qt shared libraries.
