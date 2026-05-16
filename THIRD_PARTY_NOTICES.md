# Third-Party Notices

CrossSCP itself is licensed under `AGPL-3.0-or-later`; see `LICENSE`.

This file records third-party components that may be included in binary
packages or required at runtime. It is intended for distribution with release
artifacts alongside the `LICENSES/` directory.

## Qt 6

CrossSCP's GUI uses Qt 6 modules dynamically linked at runtime:

- Qt Core
- Qt Gui
- Qt Qml
- Qt Quick
- Qt Quick Controls / Dialogs / Layouts, as deployed by Qt tooling

Open-source Qt is available under LGPL/GPL terms, with commercial licensing
available from The Qt Company. CrossSCP does not assume a commercial Qt
license. Release packages must follow the open-source Qt terms applicable to
the Qt build used for that release.

Qt source code is available from:

- <https://code.qt.io/cgit/qt/>
- <https://download.qt.io/official_releases/qt/>

Record the exact Qt version used for each release in release notes. Current
development builds should be treated as using the Qt version reported by:

```bash
qmake6 -query QT_VERSION
```

### Dynamic linking and replacement rights

CrossSCP release packages must use dynamically linked Qt libraries:

- macOS: Qt `.framework` bundles under `CrossSCP.app/Contents/Frameworks/`
- Windows: Qt `*.dll` files next to `CrossSCP.exe`
- Linux DEB/RPM: system Qt packages where practical, or shared `*.so` files

Users may replace the Qt shared libraries included with CrossSCP with
compatible modified versions, subject to normal platform loader requirements.
CrossSCP must not add technical measures intended to prevent replacement of the
LGPL-covered Qt libraries.

If CrossSCP ever modifies Qt itself, the modified Qt source must be made
available under the applicable Qt open-source license terms.

See `LICENSES/QT-LGPL-COMPLIANCE.md` for the release checklist.

## Rust crates and native libraries

Rust crate license details are tracked in `LICENSES/THIRD_PARTY_LICENSES.md`.
The optional SFTP backend uses the Rust `ssh2` crate over libssh2 and native
OpenSSL/zlib dependencies. Release notes should record exact native library
versions when those libraries are bundled.
