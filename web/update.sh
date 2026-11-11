#!/usr/bin/env bash
# Updates FreeSheet to the newest version on GitHub and restarts it.
# (It also updates itself when no one is using it: AUTO_UPDATE in .env.)
set -euo pipefail
cd "$(dirname "$0")"
git pull --ff-only
npm ci --omit=dev --no-audit --no-fund
sudo systemctl restart freesheet-web
echo "Updated to $(git log -1 --format='%h %s')"
