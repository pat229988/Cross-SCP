# Third-Party Licenses

## Rust Crates

| Package | Version | License | Source URL | Linking | AGPL compatibility notes | Security/update policy | Alternatives considered |
|---|---:|---|---|---|---|---|---|
| `zeroize` | 1.8.2 | Apache-2.0 OR MIT | <https://crates.io/crates/zeroize> | Statically linked Rust crate | MIT/Apache-2.0 are compatible with AGPLv3-or-later usage | Track RustSec advisories through `cargo audit`; update within compatible semver when advisories or maintenance releases appear | Manual `Drop` zeroing rejected as error-prone; `secrecy` remains a future candidate for stronger typed secret exposure patterns |
| `ssh2` | 0.9.5 | MIT OR Apache-2.0 | <https://crates.io/crates/ssh2> | Optional feature-gated Rust crate linking libssh2 through `libssh2-sys` | MIT/Apache-2.0 are compatible with AGPLv3-or-later usage; libssh2 dependency is BSD-style and must remain documented before release | Track RustSec advisories for `ssh2`, `libssh2-sys`, OpenSSL/libssh2 transitive dependencies; keep backend feature optional until parity/security review passes | `russh` and libssh FFI remain alternatives if libssh2 lacks required legacy SFTP client parity |

When adding a dependency, record:

- Package name and version.
- License and source URL.
- Static or dynamic linking mode.
- AGPL compatibility notes.
- Security/update policy.
- Replacement alternatives considered.
