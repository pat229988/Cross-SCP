#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

BUILD_DIR="${BUILD_DIR:-build/gui-release}"
DIST_DIR="${DIST_DIR:-dist/macos}"
VERSION_TAG="${VERSION_TAG:-$(tr -d '[:space:]' < VERSION)}"
ARTIFACT_PREFIX="CrossSCP-${VERSION_TAG}-macos-arm64"
DEFAULT_DMG_PATH="${DIST_DIR}/${ARTIFACT_PREFIX}.dmg"

cmake -S . -B "${BUILD_DIR}" -DCROSSSCP_BUILD_GUI=ON -DCMAKE_BUILD_TYPE=Release
rm -rf "${BUILD_DIR}/apps/crossscp-gui/CrossSCP.app"
cmake --build "${BUILD_DIR}" --config Release --clean-first

if [[ "$(uname -s)" == "Darwin" ]]; then
  mkdir -p "${DIST_DIR}"
  rm -f "${DIST_DIR}"/CrossSCP-*-macos-arm64.dmg \
        "${DIST_DIR}"/CrossSCP-*-macos-arm64.dmg.sha256 \
        "${DIST_DIR}/CrossSCP-macos-arm64.dmg" \
        "${DIST_DIR}/CrossSCP-macos-arm64.dmg.sha256"
  BUILD_DIR="${BUILD_DIR}" DIST_DIR="${DIST_DIR}" DMG_PATH="${DMG_PATH:-${DEFAULT_DMG_PATH}}" bash scripts/deploy-macos.sh
else
  cmake --build "${BUILD_DIR}" --target package --config Release
fi
