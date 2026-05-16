#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

bash scripts/validate.sh
bash scripts/live-sftp-smoke.sh

echo "Release check skeleton completed. Review packaging/README.md blockers before release."
