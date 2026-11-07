#!/usr/bin/env bash
# Copies the Android release signing key (keystore/) into the GitHub repo's Actions
# secrets, so APKs built on GitHub are signed with the same key as local builds.
#   bash scripts/set-github-secrets.sh
set -euo pipefail
cd "$(dirname "$0")/../src-tauri/android"
REPO=Varmite/freesheet
set -a; . <(tr -d '\r' < keystore/keystore.properties); set +a
base64 -w0 keystore/freesheet-release.jks | gh secret set ANDROID_KEYSTORE_B64 -R "$REPO"
printf %s "$FREESHEET_KEYSTORE_PASSWORD" | gh secret set ANDROID_KEYSTORE_PASSWORD -R "$REPO"
printf %s "$FREESHEET_KEY_ALIAS" | gh secret set ANDROID_KEY_ALIAS -R "$REPO"
printf %s "$FREESHEET_KEY_PASSWORD" | gh secret set ANDROID_KEY_PASSWORD -R "$REPO"
gh secret list -R "$REPO"
