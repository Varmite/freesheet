#!/usr/bin/env bash
# FreeSheet web edition: installs it as a service that starts with the laptop.
# For Linux Mint, Ubuntu and Debian. Run from the web/ folder:  ./install.sh
# Safe to run again.
set -euo pipefail
cd "$(dirname "$0")"
WEB_DIR="$(pwd)"
SERVICE=freesheet-web

bold() { printf '\n\033[1m%s\033[0m\n' "$*"; }
ask() { local a=""; read -r -p "$1${2:+ [$2]}: " a || true; echo "${a:-${2:-}}"; }
confirm() { local a; read -r -p "$1 [y/N] " a; [[ "$a" =~ ^[Yy] ]]; }
envget() { grep -E "^$1=" .env 2>/dev/null | head -1 | cut -d= -f2- || true; }

if [ "$(id -u)" = 0 ]; then
  echo "Run this as your normal user (not with sudo). It asks for your password when it needs it."
  exit 1
fi

# ---- 1. Node.js 18 or newer
node_ok() { command -v node >/dev/null 2>&1 && [ "$(node -p 'process.versions.node.split(".")[0]')" -ge 18 ]; }
if ! node_ok; then
  bold "FreeSheet needs Node.js 18 or newer (found: $(node -v 2>/dev/null || echo none))."
  if confirm "Install Node.js 22 (LTS) from NodeSource now? This uses sudo."; then
    sudo apt-get update
    sudo apt-get install -y ca-certificates curl gnupg
    curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash -
    sudo apt-get install -y nodejs
  else
    echo "Install Node.js 18 or newer, then run ./install.sh again."
    exit 1
  fi
fi
echo "Node.js $(node -v)"

# ---- 2. Packages
bold "Installing packages"
npm ci --omit=dev --no-audit --no-fund

# ---- 3. Settings (.env)
bold "Settings"
[ -f .env ] || cp .env.example .env
chmod 600 .env
if [ -z "$(envget SESSION_SECRET)" ]; then
  node lib/envfile.js .env SESSION_SECRET "$(node -e 'console.log(require("crypto").randomBytes(32).toString("hex"))')"
fi
PORT="$(envget PORT)"; PORT="${PORT:-5000}"

# ---- 4. Accounts (usernames and passwords, kept on this laptop)
bold "Accounts"
if [ -n "$(node users.js list | grep -v '^No accounts')" ]; then
  echo "Accounts so far: $(node users.js list | tr '
' ' ')"
fi
while :; do
  NAME="$(ask "Make an account? Type a username (e.g. thandi), or press Enter to skip")"
  [ -z "$NAME" ] && break
  node users.js add "$NAME" || true
done
echo "Later: ./users.sh add <name>, ./users.sh passwd <name>, ./users.sh list"

# ---- 5. Is the port free?
if command -v ss >/dev/null && ss -ltn "sport = :$PORT" | grep -q LISTEN; then
  if ! systemctl is-active --quiet "$SERVICE"; then
    bold "Port $PORT is already used by another program:"
    sudo ss -ltnp "sport = :$PORT" || true
    echo "Stop it first (see SETUP.md, \"Port 5000 is busy\"), then run ./install.sh again."
    exit 1
  fi
fi

# ---- 6. The service: runs as you, restarts if it stops, starts when the laptop boots.
bold "Setting up the $SERVICE service (needs sudo)"
NODE_BIN="$(command -v node)"
sudo tee "/etc/systemd/system/$SERVICE.service" >/dev/null <<EOF
[Unit]
Description=FreeSheet web edition
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$USER
WorkingDirectory=$WEB_DIR
ExecStart=$NODE_BIN $WEB_DIR/server.js
Environment=NODE_ENV=production
Environment=PATH=$(dirname "$NODE_BIN"):/usr/local/bin:/usr/bin:/bin
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload
sudo systemctl enable "$SERVICE" >/dev/null
sudo systemctl restart "$SERVICE"
sleep 2

if ! systemctl is-active --quiet "$SERVICE"; then
  bold "The service did not start. Its log:"
  sudo journalctl -u "$SERVICE" -n 30 --no-pager
  exit 1
fi

# ---- 7. Done
SCHEME=http; [ -n "$(envget HTTPS_CERT)" ] && SCHEME=https
IP="$(hostname -I 2>/dev/null | awk '{print $1}')"
bold "FreeSheet is running."
echo "  On this laptop:        $SCHEME://localhost:$PORT"
[ -n "$IP" ] && echo "  On your home Wi-Fi:    $SCHEME://$IP:$PORT"
if [ "$SCHEME" = http ]; then
  echo
  echo "Without HTTPS, passwords travel unencrypted. Before using it over the internet, run"
  echo "./setup-https.sh (free DuckDNS name and certificate; SETUP.md steps 2 and 3)."
fi
echo
echo "Log:      journalctl -u $SERVICE -f"
echo "Restart:  sudo systemctl restart $SERVICE"
