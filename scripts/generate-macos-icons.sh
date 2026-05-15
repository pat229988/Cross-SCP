#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_PNG="${ROOT_DIR}/apps/crossscp-gui/resources/icons/crossscp-1024.png"
ICONSET_DIR="${ROOT_DIR}/apps/crossscp-gui/resources/icons/CrossSCP.iconset"
ICNS_FILE="${ROOT_DIR}/apps/crossscp-gui/resources/icons/CrossSCP.icns"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS icon generation requires Darwin sips/iconutil." >&2
  exit 1
fi

if [[ ! -f "${SOURCE_PNG}" ]]; then
  echo "Missing source icon: ${SOURCE_PNG}" >&2
  exit 1
fi

rm -rf "${ICONSET_DIR}"
mkdir -p "${ICONSET_DIR}"

sips -z 16 16 "${SOURCE_PNG}" --out "${ICONSET_DIR}/icon_16x16.png" >/dev/null
sips -z 32 32 "${SOURCE_PNG}" --out "${ICONSET_DIR}/icon_16x16@2x.png" >/dev/null
sips -z 32 32 "${SOURCE_PNG}" --out "${ICONSET_DIR}/icon_32x32.png" >/dev/null
sips -z 64 64 "${SOURCE_PNG}" --out "${ICONSET_DIR}/icon_32x32@2x.png" >/dev/null
sips -z 128 128 "${SOURCE_PNG}" --out "${ICONSET_DIR}/icon_128x128.png" >/dev/null
sips -z 256 256 "${SOURCE_PNG}" --out "${ICONSET_DIR}/icon_128x128@2x.png" >/dev/null
sips -z 256 256 "${SOURCE_PNG}" --out "${ICONSET_DIR}/icon_256x256.png" >/dev/null
sips -z 512 512 "${SOURCE_PNG}" --out "${ICONSET_DIR}/icon_256x256@2x.png" >/dev/null
sips -z 512 512 "${SOURCE_PNG}" --out "${ICONSET_DIR}/icon_512x512.png" >/dev/null
cp "${SOURCE_PNG}" "${ICONSET_DIR}/icon_512x512@2x.png"

iconutil -c icns "${ICONSET_DIR}" -o "${ICNS_FILE}"
echo "Generated ${ICNS_FILE}"
