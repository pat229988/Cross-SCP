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

# macdeployqt is not fully idempotent when a previously signed/deployed bundle
# is reused. Remove stale deployed Qt payloads and make the bundle writable so
# the platform plugin/framework copy is atomic and does not silently leave a
# partially deployed app that aborts during Qt platform initialization.
chmod -R u+rwX "${APP_PATH}"
rm -rf "${APP_PATH}/Contents/Frameworks" \
       "${APP_PATH}/Contents/PlugIns" \
       "${APP_PATH}/Contents/Resources/qml"

MACDEPLOYQT_ARGS=("${APP_PATH}" -verbose=1 -qmldir="apps/crossscp-gui/qml")
if [[ -n "${QT_LIB_DIR}" ]]; then
  MACDEPLOYQT_ARGS+=("-libpath=${QT_LIB_DIR}")
fi
MACDEPLOYQT_LOG="$(mktemp)"
set +e
"${MACDEPLOYQT}" "${MACDEPLOYQT_ARGS[@]}" 2>&1 | tee "${MACDEPLOYQT_LOG}"
MACDEPLOYQT_STATUS=${PIPESTATUS[0]}
set -e
if grep -q "file copy failed" "${MACDEPLOYQT_LOG}"; then
  echo "macdeployqt failed to copy required files; refusing to package a partial app bundle." >&2
  rm -f "${MACDEPLOYQT_LOG}"
  exit 1
fi
if [[ ${MACDEPLOYQT_STATUS} -ne 0 ]]; then
  echo "macdeployqt exited with status ${MACDEPLOYQT_STATUS}; continuing because no required file copy failure was detected." >&2
fi
rm -f "${MACDEPLOYQT_LOG}"

if [[ ! -f "${APP_PATH}/Contents/PlugIns/platforms/libqcocoa.dylib" ]]; then
  echo "macdeployqt did not deploy the Cocoa platform plugin." >&2
  exit 1
fi

# Some Homebrew Qt helper frameworks retain absolute install names after
# macdeployqt copies them. If left untouched, dyld can load both bundled Qt and
# /opt/homebrew Qt at runtime, which causes duplicate Objective-C classes and
# can abort during platform initialization. Normalize all Qt framework IDs and
# Qt framework references to the bundle-local Frameworks directory.
APP_PATH_FOR_FIXUP="${APP_PATH}" python3 - <<'PY'
import os
import re
import subprocess

app_path = os.environ["APP_PATH_FOR_FIXUP"]
contents = os.path.join(app_path, "Contents")
frameworks = os.path.join(contents, "Frameworks")
qt_framework_ref = re.compile(r"(/[^\s]+/(Qt[^/]+\.framework)/Versions/A/(Qt[^\s]+))")


def is_macho(path):
    try:
        with open(path, "rb") as handle:
            magic = handle.read(4)
    except OSError:
        return False
    return magic in {
        b"\xfe\xed\xfa\xce", b"\xce\xfa\xed\xfe",
        b"\xfe\xed\xfa\xcf", b"\xcf\xfa\xed\xfe",
        b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca",
    }


def run(*args):
    subprocess.run(args, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


for entry in os.listdir(frameworks):
    if not entry.startswith("Qt") or not entry.endswith(".framework"):
        continue
    framework_name = entry
    library_name = entry[:-len(".framework")]
    library_path = os.path.join(frameworks, entry, "Versions", "A", library_name)
    if os.path.exists(library_path):
        new_id = f"@executable_path/../Frameworks/{framework_name}/Versions/A/{library_name}"
        run("install_name_tool", "-id", new_id, library_path)

for dirpath, _, filenames in os.walk(contents):
    for filename in filenames:
        path = os.path.join(dirpath, filename)
        if not is_macho(path):
            continue
        try:
            output = subprocess.check_output(["otool", "-L", path], text=True, stderr=subprocess.DEVNULL)
        except subprocess.CalledProcessError:
            continue
        replacements = []
        for line in output.splitlines()[1:]:
            match = qt_framework_ref.search(line)
            if not match:
                continue
            old_ref, framework_name, library_name = match.groups()
            if old_ref.startswith("@executable_path/") or old_ref.startswith("@rpath/"):
                continue
            new_ref = f"@executable_path/../Frameworks/{framework_name}/Versions/A/{library_name}"
            if old_ref != new_ref:
                replacements.append((old_ref, new_ref))
        for old_ref, new_ref in replacements:
            run("install_name_tool", "-change", old_ref, new_ref, path)
PY

# CrossSCP does not use QtPdf. Homebrew's qpdf/pdfquick plugins can keep
# optional QtPdf framework references that macdeployqt does not bundle for this
# app, causing dyld to fall back to /opt/homebrew Qt and load duplicate QtCore
# and QtGui. Remove these optional plugins from the package.
rm -f "${APP_PATH}/Contents/PlugIns/imageformats/libqpdf.dylib" \
      "${APP_PATH}/Contents/PlugIns/quick/libpdfquickplugin.dylib"
rm -rf "${APP_PATH}/Contents/Resources/qml/QtQuick/Pdf"

mkdir -p "${APP_PATH}/Contents/Resources"
cat > "${APP_PATH}/Contents/Resources/qt.conf" <<'EOF'
[Paths]
Plugins = PlugIns
Qml2Imports = Resources/qml
Imports = Resources/qml
EOF
cp "${VOLUME_ICON}" "${APP_PATH}/Contents/Resources/CrossSCP.icns"
mkdir -p "${APP_PATH}/Contents/Resources/Legal"
cp LICENSE THIRD_PARTY_NOTICES.md "${APP_PATH}/Contents/Resources/Legal/"
cp -R LICENSES "${APP_PATH}/Contents/Resources/Legal/"
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
