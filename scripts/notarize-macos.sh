#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

DMG_PATH="${1:-}"

if [[ -z "${DMG_PATH}" ]]; then
  echo "Usage: scripts/notarize-macos.sh <path-to-dmg>" >&2
  exit 2
fi

: "${CROSSSCP_NOTARY_PROFILE:?Set CROSSSCP_NOTARY_PROFILE for xcrun notarytool}"

xcrun notarytool submit "${DMG_PATH}" --keychain-profile "${CROSSSCP_NOTARY_PROFILE}" --wait
xcrun stapler staple "${DMG_PATH}"

echo "Notarized and stapled ${DMG_PATH}"
