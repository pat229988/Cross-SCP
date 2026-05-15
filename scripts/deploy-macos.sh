#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

BUILD_DIR="${BUILD_DIR:-build/gui-release}"
APP_PATH="${APP_PATH:-${BUILD_DIR}/apps/crossscp-gui/CrossSCP.app}"
DIST_DIR="${DIST_DIR:-dist/macos}"
DMG_PATH="${DMG_PATH:-${DIST_DIR}/CrossSCP-macos-arm64.dmg}"
VOLUME_ICON="${VOLUME_ICON:-apps/crossscp-gui/resources/icons/CrossSCP.icns}"
QT_BIN_DIR="${QT_BIN_DIR:-}"
QT_LIB_DIR="${QT_LIB_DIR:-}"
SIGN_IDENTITY="${CROSSSCP_SIGN_IDENTITY:-}"
ENTITLEMENTS="${CROSSSCP_MACOS_ENTITLEMENTS:-packaging/macos/entitlements.plist}"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS deployment requires Darwin." >&2
  exit 1
fi

if [[ ! -d "${APP_PATH}" ]]; then
  echo "Missing app bundle: ${APP_PATH}" >&2
  echo "Build first: scripts/package-gui.sh" >&2
  exit 1
fi

MACDEPLOYQT="${QT_BIN_DIR:+${QT_BIN_DIR}/}macdeployqt"
if [[ -z "${QT_LIB_DIR}" ]] && command -v qmake6 >/dev/null 2>&1; then
  QT_LIB_DIR="$(qmake6 -query QT_INSTALL_LIBS)"
fi

MACDEPLOYQT_ARGS=("${APP_PATH}" -verbose=1 -qmldir="apps/crossscp-gui/qml")
if [[ -n "${QT_LIB_DIR}" ]]; then
  MACDEPLOYQT_ARGS+=("-libpath=${QT_LIB_DIR}")
fi
"${MACDEPLOYQT}" "${MACDEPLOYQT_ARGS[@]}"

mkdir -p "${APP_PATH}/Contents/Resources"
cp "${VOLUME_ICON}" "${APP_PATH}/Contents/Resources/CrossSCP.icns"
/usr/libexec/PlistBuddy -c "Set :CFBundleIconFile CrossSCP.icns" "${APP_PATH}/Contents/Info.plist" 2>/dev/null \
  || /usr/libexec/PlistBuddy -c "Add :CFBundleIconFile string CrossSCP.icns" "${APP_PATH}/Contents/Info.plist"
touch "${APP_PATH}"

if [[ -n "${SIGN_IDENTITY}" ]]; then
  codesign --force --deep --options runtime --timestamp --entitlements "${ENTITLEMENTS}" --sign "${SIGN_IDENTITY}" "${APP_PATH}"
else
  echo "CROSSSCP_SIGN_IDENTITY is not set; applying ad-hoc signature for local unsigned testing."
  codesign --force --deep --sign - "${APP_PATH}"
fi

codesign --verify --deep --strict --verbose=2 "${APP_PATH}"

mkdir -p "${DIST_DIR}"
rm -f "${DMG_PATH}"

if command -v create-dmg >/dev/null 2>&1; then
  create-dmg \
    --volname "CrossSCP" \
    --volicon "${VOLUME_ICON}" \
    --window-pos 200 120 \
    --window-size 640 420 \
    --icon-size 96 \
    --app-drop-link 480 210 \
    "${DMG_PATH}" \
    "${APP_PATH}"
else
  echo "create-dmg is not installed; falling back to CPack DragNDrop output." >&2
  cmake --build "${BUILD_DIR}" --target package --config Release
  cp "${BUILD_DIR}"/*.dmg "${DMG_PATH}"
fi

if [[ -n "${SIGN_IDENTITY}" ]]; then
  codesign --force --timestamp --sign "${SIGN_IDENTITY}" "${DMG_PATH}"
  codesign --verify --verbose=2 "${DMG_PATH}"
fi

shasum -a 256 "${DMG_PATH}" > "${DMG_PATH}.sha256"

echo "macOS deployment prepared for ${APP_PATH}"
echo "DMG: ${DMG_PATH}"
echo "Checksum: ${DMG_PATH}.sha256"
