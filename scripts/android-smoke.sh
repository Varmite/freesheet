#!/usr/bin/env bash
# Android smoke test (run by .github/workflows/android-smoke.yml inside an emulator):
#   bash scripts/android-smoke.sh <debug apk> <out-dir>
# Installs the APK, drives the app through the WebView's DevTools (scripts/android-smoke.js),
# presses the real Back and Home keys, and checks that progress is saved and nothing crashed.
# Screenshots and the log are saved in <out-dir>.
set -u
APK="$1"; OUT="$2"; mkdir -p "$OUT"
PKG=za.freesheet.app
FAIL=0
shot() { adb exec-out screencap -p > "$OUT/$1.png"; }

adb install -r "$APK" || exit 1
adb logcat -c
adb shell am start -n "$PKG/.MainActivity"
sleep 15
PID=$(adb shell pidof "$PKG" | tr -d '\r')
[ -n "$PID" ] || { echo "FAIL  app did not start"; exit 1; }
adb forward tcp:9222 "localabstract:webview_devtools_remote_$PID"
shot 01-start

node scripts/android-smoke.js 9222 start || FAIL=1
shot 02-topic

adb shell input keyevent KEYCODE_BACK
node scripts/android-smoke.js 9222 back || FAIL=1
shot 03-after-back

# Leaving the app saves progress.
adb shell input keyevent KEYCODE_HOME
sleep 3
adb shell run-as "$PKG" find . -name 'progress.json*' > "$OUT/progress-files.txt" 2>&1
if grep -q 'progress.json$' "$OUT/progress-files.txt"; then
  echo "PASS  progress saved when leaving the app: $(tr '\r\n' '  ' < "$OUT/progress-files.txt")"
else
  echo "FAIL  no progress.json after leaving the app"; cat "$OUT/progress-files.txt"; FAIL=1
fi
adb shell pidof "$PKG" > /dev/null && echo "PASS  app still running in the background" || { echo "FAIL  app died"; FAIL=1; }

adb logcat -d > "$OUT/logcat.txt"
if grep -E "panicked|FATAL EXCEPTION" "$OUT/logcat.txt"; then FAIL=1; else echo "PASS  no crashes in the log"; fi
echo "Result: $([ $FAIL = 0 ] && echo PASS || echo FAIL)"
exit $FAIL
