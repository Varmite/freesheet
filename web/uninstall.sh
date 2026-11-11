#!/usr/bin/env bash
# Removes the freesheet-web service. Your settings (.env) and everyone's progress (data/)
# are kept unless you choose to delete them.
set -euo pipefail
cd "$(dirname "$0")"
SERVICE=freesheet-web

if systemctl list-unit-files "$SERVICE.service" >/dev/null 2>&1; then
  sudo systemctl disable --now "$SERVICE" 2>/dev/null || true
  sudo rm -f "/etc/systemd/system/$SERVICE.service"
  sudo systemctl daemon-reload
  echo "Removed the $SERVICE service."
fi

read -r -p "Also delete everyone's progress (web/data) and settings (web/.env)? This cannot be undone. [y/N] " a
if [[ "$a" =~ ^[Yy] ]]; then
  rm -rf data .env
  echo "Deleted."
else
  echo "Kept web/data and web/.env."
fi
echo "Certificates from setup-https.sh (acme.sh, the DuckDNS updater) are left in place; see SETUP.md to remove them."
