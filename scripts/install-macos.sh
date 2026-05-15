#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

REPO="${CROSSSCP_REPO:-pat229988/Cross-SCP}"
VERSION="${CROSSSCP_VERSION:-latest}"
ARCH="${CROSSSCP_ARCH:-$(uname -m)}"
INSTALL_DIR="${CROSSSCP_INSTALL_DIR:-/Applications}"
APP_NAME="CrossSCP.app"
APP_DEST="${INSTALL_DIR}/${APP_NAME}"
DMG_URL="${CROSSSCP_DMG_URL:-}"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "CrossSCP macOS installer must be run on macOS." >&2
  exit 1
fi

case "${ARCH}" in
  arm64|aarch64) ARTIFACT_ARCH="arm64" ;;
  x86_64|amd64) ARTIFACT_ARCH="x64" ;;
  *) ARTIFACT_ARCH="${ARCH}" ;;
esac

require() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

require curl
require hdiutil
require ditto
require xattr
require perl

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/crossscp-install.XXXXXX")"
MOUNT_DIR="${TMP_DIR}/mnt"
DMG_PATH="${TMP_DIR}/CrossSCP.dmg"
mkdir -p "${MOUNT_DIR}"

cleanup() {
  if mount | grep -q "${MOUNT_DIR}"; then
    hdiutil detach "${MOUNT_DIR}" -quiet || true
  fi
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

curl_args=(-fsSL)
if [[ -n "${GITHUB_TOKEN:-}" ]]; then
  curl_args=(-fsSL -H "Authorization: Bearer ${GITHUB_TOKEN}" -H "Accept: application/vnd.github+json")
fi

resolve_dmg_url() {
  if [[ -n "${DMG_URL}" ]]; then
    printf '%s\n' "${DMG_URL}"
    return 0
  fi

  local api_url
  if [[ "${VERSION}" == "latest" ]]; then
    api_url="https://api.github.com/repos/${REPO}/releases/latest"
  else
    api_url="https://api.github.com/repos/${REPO}/releases/tags/${VERSION}"
  fi

  local metadata
  if ! metadata="$(curl "${curl_args[@]}" "${api_url}")"; then
    echo "Unable to read release metadata from ${api_url}." >&2
    echo "If the release is still a draft/private release, set GITHUB_TOKEN or CROSSSCP_DMG_URL." >&2
    return 1
  fi

  local preferred fallback
  preferred="$(printf '%s' "${metadata}" | perl -0ne 'while (/"browser_download_url"\s*:\s*"([^"]*macos-'"${ARTIFACT_ARCH}"'[^"]*\.dmg)"/g) { print "$1\n"; exit }')"
  fallback="$(printf '%s' "${metadata}" | perl -0ne 'while (/"browser_download_url"\s*:\s*"([^"]*macos[^"]*\.dmg)"/g) { print "$1\n"; exit }')"

  if [[ -n "${preferred}" ]]; then
    printf '%s\n' "${preferred}"
  elif [[ -n "${fallback}" ]]; then
    printf '%s\n' "${fallback}"
  else
    echo "No macOS DMG asset found in ${api_url}." >&2
    return 1
  fi
}

DMG_URL="$(resolve_dmg_url)"

echo "CrossSCP macOS tester installer"
echo "Repository: ${REPO}"
echo "Release: ${VERSION}"
echo "Artifact architecture preference: ${ARTIFACT_ARCH}"
echo "Install destination: ${APP_DEST}"
echo "DMG: ${DMG_URL}"
echo

echo "Downloading CrossSCP DMG..."
download_args=(-fL)
if [[ -n "${GITHUB_TOKEN:-}" && "${DMG_URL}" == https://github.com/* ]]; then
  download_args=(-fL -H "Authorization: Bearer ${GITHUB_TOKEN}")
fi
curl "${download_args[@]}" "${DMG_URL}" -o "${DMG_PATH}"

echo "Mounting DMG..."
hdiutil attach "${DMG_PATH}" -nobrowse -readonly -mountpoint "${MOUNT_DIR}" >/dev/null

SOURCE_APP="${MOUNT_DIR}/${APP_NAME}"
if [[ ! -d "${SOURCE_APP}" ]]; then
  SOURCE_APP="$(find "${MOUNT_DIR}" -maxdepth 2 -name "${APP_NAME}" -type d -print -quit)"
fi

if [[ -z "${SOURCE_APP}" || ! -d "${SOURCE_APP}" ]]; then
  echo "Could not find ${APP_NAME} inside the DMG." >&2
  exit 1
fi

copy_app() {
  rm -rf "${APP_DEST}"
  ditto "${SOURCE_APP}" "${APP_DEST}"
  xattr -dr com.apple.quarantine "${APP_DEST}" 2>/dev/null || true
}

echo "Installing ${APP_NAME}..."
INSTALL_PARENT="$(dirname "${INSTALL_DIR}")"
if [[ -w "${INSTALL_DIR}" || ( ! -e "${INSTALL_DIR}" && -w "${INSTALL_PARENT}" ) ]]; then
  mkdir -p "${INSTALL_DIR}"
  copy_app
else
  echo "Administrator permission is required to install into ${INSTALL_DIR}."
  sudo mkdir -p "${INSTALL_DIR}"
  sudo rm -rf "${APP_DEST}"
  sudo ditto "${SOURCE_APP}" "${APP_DEST}"
  sudo xattr -dr com.apple.quarantine "${APP_DEST}" 2>/dev/null || true
fi

echo
echo "CrossSCP installed to ${APP_DEST}"
echo "Opening CrossSCP..."
open "${APP_DEST}"
echo
echo "If macOS still blocks it, run:"
echo "  sudo xattr -dr com.apple.quarantine '${APP_DEST}'"
echo "  open '${APP_DEST}'"
