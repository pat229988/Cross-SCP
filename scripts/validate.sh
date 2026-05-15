#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p crossscp-protocol-sftp --features ssh2-backend
cargo clippy -p crossscp-protocol-sftp --features ssh2-backend --all-targets -- -D warnings
cmake -S . -B build/package-check
