#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

required=(
  CROSSSCP_SFTP_TEST_HOST
  CROSSSCP_SFTP_TEST_USERNAME
  CROSSSCP_SFTP_TEST_CREDENTIAL_REF
)

missing=0
for name in "${required[@]}"; do
  if [[ -z "${!name:-}" ]]; then
    echo "Missing ${name}; live SFTP smoke test is skipped." >&2
    missing=1
  fi
done

if [[ "${missing}" -ne 0 ]]; then
  exit 0
fi

echo "Running feature-gated SFTP backend build validation..."
cargo check -p crossscp-protocol-sftp --features ssh2-backend
cargo check -p crossscp-cli --features ssh2-backend

echo "Live SFTP environment is configured."
echo "Host: ${CROSSSCP_SFTP_TEST_HOST}:${CROSSSCP_SFTP_TEST_PORT:-22}"
echo "List path: ${CROSSSCP_SFTP_TEST_LIST_PATH:-.}"

echo "Running live SFTP list smoke check..."
cargo run -p crossscp-cli --features ssh2-backend -- \
  sftp-list \
  "${CROSSSCP_SFTP_TEST_HOST}" \
  "${CROSSSCP_SFTP_TEST_PORT:-22}" \
  "${CROSSSCP_SFTP_TEST_USERNAME}" \
  "${CROSSSCP_SFTP_TEST_LIST_PATH:-.}"

if [[ -n "${CROSSSCP_SFTP_TEST_LOCAL_FILE:-}" && -n "${CROSSSCP_SFTP_TEST_REMOTE_FILE:-}" ]]; then
  echo "Running live SFTP upload smoke check..."
  cargo run -p crossscp-cli --features ssh2-backend -- \
    sftp-upload \
    "${CROSSSCP_SFTP_TEST_HOST}" \
    "${CROSSSCP_SFTP_TEST_PORT:-22}" \
    "${CROSSSCP_SFTP_TEST_USERNAME}" \
    "${CROSSSCP_SFTP_TEST_LOCAL_FILE}" \
    "${CROSSSCP_SFTP_TEST_REMOTE_FILE}"

  download_target="${CROSSSCP_SFTP_TEST_DOWNLOAD_FILE:-${CROSSSCP_SFTP_TEST_LOCAL_FILE}.downloaded}"
  echo "Running live SFTP download smoke check to ${download_target}..."
  cargo run -p crossscp-cli --features ssh2-backend -- \
    sftp-download \
    "${CROSSSCP_SFTP_TEST_HOST}" \
    "${CROSSSCP_SFTP_TEST_PORT:-22}" \
    "${CROSSSCP_SFTP_TEST_USERNAME}" \
    "${CROSSSCP_SFTP_TEST_REMOTE_FILE}" \
    "${download_target}"
else
  echo "Transfer paths not configured; upload/download live checks are skipped."
fi
