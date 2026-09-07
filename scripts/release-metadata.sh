#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

usage() {
  cat <<'USAGE' >&2
Usage: scripts/release-metadata.sh <target-os> <target-arch>

Exports normalized release metadata for packaging workflows. When GITHUB_ENV is
set, values are appended for later GitHub Actions steps.

Optional environment overrides:
  CROSSSCP_VERSION       Explicit release version, usually vX.Y.Z
  CROSSSCP_ARTIFACT_OS   Artifact OS label when it differs from target-os
  CROSSSCP_DIST_DIR      Output directory for artifacts
USAGE
}

if [[ $# -ne 2 ]]; then
  usage
  exit 2
fi

TARGET_OS="$1"
TARGET_ARCH="$2"
ARTIFACT_OS="${CROSSSCP_ARTIFACT_OS:-${TARGET_OS}}"
BASE_VERSION="$(tr -d '[:space:]' < VERSION)"

if [[ ! "${BASE_VERSION}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
  printf 'VERSION must contain a semver tag; found: %s\n' "${BASE_VERSION}" >&2
  exit 1
fi

if [[ "${GITHUB_REF_TYPE:-}" == "tag" && -n "${GITHUB_REF_NAME:-}" ]]; then
  if [[ "${GITHUB_REF_NAME}" != "${BASE_VERSION}" ]]; then
    printf 'Release tag %s does not match VERSION %s\n' "${GITHUB_REF_NAME}" "${BASE_VERSION}" >&2
    exit 1
  fi
  if [[ -n "${CROSSSCP_VERSION:-}" && "${CROSSSCP_VERSION}" != "${GITHUB_REF_NAME}" ]]; then
    printf 'CROSSSCP_VERSION %s does not match release tag %s\n' "${CROSSSCP_VERSION}" "${GITHUB_REF_NAME}" >&2
    exit 1
  fi
  VERSION="${GITHUB_REF_NAME}"
elif [[ -n "${CROSSSCP_VERSION:-}" ]]; then
  VERSION="${CROSSSCP_VERSION}"
else
  SHORT_SHA="$(git rev-parse --short HEAD 2>/dev/null || printf 'unknown')"
  VERSION="${BASE_VERSION}-dev-${SHORT_SHA}"
fi

COMMIT_SHA="$(git rev-parse HEAD 2>/dev/null || printf 'unknown')"

case "${TARGET_OS}" in
  macos) DEFAULT_DIST_DIR="dist/macos" ;;
  windows) DEFAULT_DIST_DIR="dist/windows" ;;
  flatpak) DEFAULT_DIST_DIR="dist/linux/flatpak" ;;
  ubuntu) DEFAULT_DIST_DIR="dist/linux/ubuntu" ;;
  *) DEFAULT_DIST_DIR="dist/${TARGET_OS}" ;;
esac

DIST_DIR="${CROSSSCP_DIST_DIR:-${DEFAULT_DIST_DIR}}"
ARTIFACT_BASE="CrossSCP-${VERSION}-${ARTIFACT_OS}-${TARGET_ARCH}"

emit() {
  printf '%s=%s\n' "$1" "$2"
}

{
  emit CROSSSCP_VERSION "${VERSION}"
  emit CROSSSCP_COMMIT_SHA "${COMMIT_SHA}"
  emit CROSSSCP_TARGET_OS "${TARGET_OS}"
  emit CROSSSCP_TARGET_ARCH "${TARGET_ARCH}"
  emit CROSSSCP_ARTIFACT_OS "${ARTIFACT_OS}"
  emit CROSSSCP_ARTIFACT_BASE "${ARTIFACT_BASE}"
  emit CROSSSCP_DIST_DIR "${DIST_DIR}"
}

if [[ -n "${GITHUB_ENV:-}" ]]; then
  {
    emit CROSSSCP_VERSION "${VERSION}"
    emit CROSSSCP_COMMIT_SHA "${COMMIT_SHA}"
    emit CROSSSCP_TARGET_OS "${TARGET_OS}"
    emit CROSSSCP_TARGET_ARCH "${TARGET_ARCH}"
    emit CROSSSCP_ARTIFACT_OS "${ARTIFACT_OS}"
    emit CROSSSCP_ARTIFACT_BASE "${ARTIFACT_BASE}"
    emit CROSSSCP_DIST_DIR "${DIST_DIR}"
  } >> "${GITHUB_ENV}"
fi
