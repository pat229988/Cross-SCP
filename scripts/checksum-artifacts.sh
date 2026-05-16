#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: scripts/checksum-artifacts.sh <artifact-file-or-dir> [...]" >&2
  exit 2
fi

hash_file() {
  local file="$1"
  if [[ "${file}" == *.sha256 ]]; then
    return 0
  fi

  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "${file}" > "${file}.sha256"
  else
    sha256sum "${file}" > "${file}.sha256"
  fi
  echo "Wrote ${file}.sha256"
}

for path in "$@"; do
  if [[ -d "${path}" ]]; then
    while IFS= read -r -d '' file; do
      hash_file "${file}"
    done < <(find "${path}" -type f ! -name '*.sha256' -print0)
  elif [[ -f "${path}" ]]; then
    hash_file "${path}"
  else
    echo "Skipping missing artifact path: ${path}" >&2
  fi
done
