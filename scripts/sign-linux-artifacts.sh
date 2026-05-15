#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: scripts/sign-linux-artifacts.sh <artifact-file-or-dir> [...]" >&2
  exit 2
fi

if [[ -z "${LINUX_GPG_PRIVATE_KEY:-}" ]]; then
  echo "Skipping Linux detached signatures: LINUX_GPG_PRIVATE_KEY is not configured."
  exit 0
fi

GNUPGHOME="${GNUPGHOME:-${RUNNER_TEMP:-/tmp}/crossscp-gnupg}"
export GNUPGHOME
mkdir -p "${GNUPGHOME}"
chmod 700 "${GNUPGHOME}"

printf '%s' "${LINUX_GPG_PRIVATE_KEY}" | gpg --batch --import

sign_file() {
  local file="$1"
  case "${file}" in
    *.sha256|*.asc) return 0 ;;
  esac

  if [[ -n "${LINUX_GPG_PASSPHRASE:-}" ]]; then
    gpg --batch --yes --pinentry-mode loopback --passphrase "${LINUX_GPG_PASSPHRASE}" --armor --detach-sign "${file}"
  else
    gpg --batch --yes --armor --detach-sign "${file}"
  fi
  echo "Wrote ${file}.asc"
}

for path in "$@"; do
  if [[ -d "${path}" ]]; then
    while IFS= read -r -d '' file; do
      sign_file "${file}"
    done < <(find "${path}" -type f ! -name '*.sha256' ! -name '*.asc' -print0)
  elif [[ -f "${path}" ]]; then
    sign_file "${path}"
  else
    echo "Skipping missing artifact path: ${path}" >&2
  fi
done
