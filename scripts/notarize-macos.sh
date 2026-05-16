#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

DMG_PATH="${1:-}"

if [[ -z "${DMG_PATH}" ]]; then
  echo "Usage: scripts/notarize-macos.sh <path-to-dmg>" >&2
  exit 2
fi

if [[ -n "${CROSSSCP_NOTARY_PROFILE:-}" ]]; then
  xcrun notarytool submit "${DMG_PATH}" --keychain-profile "${CROSSSCP_NOTARY_PROFILE}" --wait
else
  : "${APPLE_ID:?Set APPLE_ID or CROSSSCP_NOTARY_PROFILE for xcrun notarytool}"
  : "${APPLE_TEAM_ID:?Set APPLE_TEAM_ID or CROSSSCP_NOTARY_PROFILE for xcrun notarytool}"
  : "${APPLE_APP_SPECIFIC_PASSWORD:?Set APPLE_APP_SPECIFIC_PASSWORD or CROSSSCP_NOTARY_PROFILE for xcrun notarytool}"
  xcrun notarytool submit "${DMG_PATH}" \
    --apple-id "${APPLE_ID}" \
    --team-id "${APPLE_TEAM_ID}" \
    --password "${APPLE_APP_SPECIFIC_PASSWORD}" \
    --wait
fi

xcrun stapler staple "${DMG_PATH}"
xcrun stapler validate "${DMG_PATH}"

echo "Notarized and stapled ${DMG_PATH}"
