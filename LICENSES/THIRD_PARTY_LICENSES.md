# Third-Party Licenses

## Rust Crates

| Package | Version | License | Source URL | Linking | AGPL compatibility notes | Security/update policy | Alternatives considered |
|---|---:|---|---|---|---|---|---|
| `zeroize` | 1.8.2 | Apache-2.0 OR MIT | <https://crates.io/crates/zeroize> | Statically linked Rust crate | MIT/Apache-2.0 are compatible with AGPLv3-or-later usage | Track RustSec advisories through `cargo audit`; update within compatible semver when advisories or maintenance releases appear | Manual `Drop` zeroing rejected as error-prone; `secrecy` remains a future candidate for stronger typed secret exposure patterns |
| `ssh2` | 0.9.5 | MIT OR Apache-2.0 | <https://crates.io/crates/ssh2> | Rust crate linking libssh2 through `libssh2-sys`; used by live SCP and optional SFTP backend | MIT/Apache-2.0 are compatible with AGPLv3-or-later usage; libssh2 dependency is BSD-style and must remain documented before release | Track RustSec advisories for `ssh2`, `libssh2-sys`, OpenSSL/libssh2 transitive dependencies; keep SFTP backend feature optional until parity/security review passes | `russh` and libssh FFI remain alternatives if libssh2 lacks required legacy SSH/SFTP/SCP client parity |
| `suppaftp` | 8.0.3 | MIT OR Apache-2.0 | <https://crates.io/crates/suppaftp> | Statically linked Rust crate; FTPS uses native TLS through platform/OpenSSL dependencies | MIT/Apache-2.0 are compatible with AGPLv3-or-later usage | Track RustSec advisories and upstream FTP/FTPS fixes; keep TLS certificate verification enabled by default | Chosen over ad-hoc FTP implementation to reduce protocol parsing risk; async backends remain a future option |
| `native-tls` | 0.2.18 | MIT OR Apache-2.0 | <https://crates.io/crates/native-tls> | Statically linked Rust crate wrapping platform TLS/OpenSSL/SChannel/Secure Transport | MIT/Apache-2.0 are compatible with AGPLv3-or-later usage | Track native TLS/OpenSSL advisories through packaging workflows | Chosen for first explicit FTPS implementation; rustls remains an option for stricter TLS policy control |

When adding a dependency, record:

- Package name and version.
- License and source URL.
- Static or dynamic linking mode.
- AGPL compatibility notes.
- Security/update policy.
- Replacement alternatives considered.

## GUI Runtime

CrossSCP's GUI uses Qt 6 dynamically linked at runtime. Open-source Qt is
available under LGPL/GPL terms, with commercial licensing available from The Qt
Company. CrossSCP does not assume a commercial Qt license.

Qt LGPL compliance requirements and relinking/replacement expectations are
documented in `LICENSES/QT-LGPL-COMPLIANCE.md` and `THIRD_PARTY_NOTICES.md`.
Release artifacts should include those files alongside this inventory.
