#!/usr/bin/env bash
# HTTPS for FreeSheet with a free DuckDNS name and a free Let's Encrypt certificate.
# Run after install.sh, from the web/ folder:  ./setup-https.sh
#
# What it does (SETUP.md steps 2 and 3 explain each part):
#  1. keeps <name>.duckdns.org pointing at your home's internet address (a cron job
#     every 5 minutes, so it follows when your address changes);
#  2. gets a certificate with acme.sh using the DNS challenge through DuckDNS, so only
#     port 5000 needs forwarding (not 80 or 443);
#  3. installs it in web/certs and tells FreeSheet to use it; acme.sh renews it by
#     itself every ~60 days and signals FreeSheet to load the new one.
set -euo pipefail
cd "$(dirname "$0")"
WEB_DIR="$(pwd)"
SERVICE=freesheet-web

bold() { printf '\n\033[1m%s\033[0m\n' "$*"; }
ask() { local a; read -r -p "$1: " a; echo "$a"; }

if [ "$(id -u)" = 0 ]; then echo "Run this as your normal user (not with sudo)."; exit 1; fi
for c in curl crontab openssl; do
  command -v "$c" >/dev/null || { echo "Installing $c..."; sudo apt-get install -y curl cron openssl; break; }
done

bold "DuckDNS"
echo "Sign in at https://www.duckdns.org, add a subdomain, and copy your token (SETUP.md step 2)."
NAME="$(ask "Your DuckDNS subdomain (just the name, e.g. reevesfamily)")"
NAME="${NAME%.duckdns.org}"
TOKEN="$(ask "Your DuckDNS token")"
EMAIL="$(ask "Your email address (Let's Encrypt sends expiry warnings here)")"
[[ "$NAME" =~ ^[a-z0-9-]+$ ]] || { echo "That subdomain doesn't look right."; exit 1; }
DOMAIN="$NAME.duckdns.org"

# 1. The address updater: ~/duckdns/duck.sh, run by cron every 5 minutes.
mkdir -p "$HOME/duckdns"
cat > "$HOME/duckdns/duck.sh" <<EOF
#!/bin/sh
# Tells DuckDNS this laptop's current internet address (empty ip= means "the one you see").
echo url="https://www.duckdns.org/update?domains=$NAME&token=$TOKEN&ip=" | curl -s -k -o "$HOME/duckdns/duck.log" -K -
EOF
chmod 700 "$HOME/duckdns/duck.sh"
( crontab -l 2>/dev/null | grep -v 'duckdns/duck.sh' ; echo "*/5 * * * * $HOME/duckdns/duck.sh >/dev/null 2>&1" ) | crontab -
"$HOME/duckdns/duck.sh"
if [ "$(cat "$HOME/duckdns/duck.log")" != "OK" ]; then
  echo "DuckDNS said: $(cat "$HOME/duckdns/duck.log"). Check the subdomain and token, then run this again."
  exit 1
fi
echo "$DOMAIN now points at this home's internet address (checked every 5 minutes)."

# 2. acme.sh and the certificate.
bold "Certificate"
if [ ! -x "$HOME/.acme.sh/acme.sh" ]; then
  curl -fsSL https://get.acme.sh | sh -s email="$EMAIL"
fi
ACME="$HOME/.acme.sh/acme.sh"
export DuckDNS_Token="$TOKEN"
echo "Asking Let's Encrypt for a certificate. This takes a few minutes (DuckDNS is slow to update)."
"$ACME" --issue --server letsencrypt --dns dns_duckdns --dnssleep 120 -d "$DOMAIN" || {
  rc=$?
  # 2 means "already have a valid certificate", which is fine.
  [ "$rc" = 2 ] || { echo "Getting the certificate failed (see the messages above, and SETUP.md)."; exit 1; }
}

# 3. Install it where FreeSheet reads it; on renewal, signal FreeSheet to reload it.
mkdir -p "$WEB_DIR/certs"
"$ACME" --install-cert -d "$DOMAIN" \
  --key-file "$WEB_DIR/certs/key.pem" \
  --fullchain-file "$WEB_DIR/certs/fullchain.pem" \
  --reloadcmd "kill -HUP \$(systemctl show -p MainPID --value $SERVICE) 2>/dev/null || true"
chmod 600 "$WEB_DIR/certs/key.pem"

node lib/envfile.js .env HTTPS_CERT "$WEB_DIR/certs/fullchain.pem"
node lib/envfile.js .env HTTPS_KEY "$WEB_DIR/certs/key.pem"
sudo systemctl restart "$SERVICE"
sleep 2

PORT="$(grep -E '^PORT=' .env | cut -d= -f2)"; PORT="${PORT:-5000}"
bold "Done. FreeSheet is at  https://$DOMAIN:$PORT"
echo
echo "In Google Cloud (SETUP.md step 1), add this under Authorised JavaScript origins:"
echo "    https://$DOMAIN:$PORT"
echo
echo "The certificate renews by itself (acme.sh's daily cron job)."
