#!/usr/bin/env bash
# Android smoke test (run by .github/workflows/android-smoke.yml inside an emulator):
#   bash scripts/android-smoke.sh <debug apk> <out-dir>
# Installs the APK, drives the app through the WebView's DevTools (scripts/android-smoke.js),
# presses the real Back and Home keys, checks that progress is saved, kills the WebView's
# renderer (as a phone short of memory does) to see the app start again, and checks nothing
# crashed. FONT_SCALE=1.3 tries it with Android's large text setting.
# Screenshots and the log are saved in <out-dir>.
set -u
APK="$1"; OUT="$2"; mkdir -p "$OUT"
PKG=za.freesheet.app
FAIL=0
shot() { adb exec-out screencap -p > "$OUT/$1.png"; }

adb install -r "$APK" || exit 1
if [ -n "${FONT_SCALE:-}" ]; then adb shell settings put system font_scale "$FONT_SCALE"; sleep 2; fi
adb logcat -c
adb shell am start -n "$PKG/.MainActivity"
sleep 15
PID=$(adb shell pidof "$PKG" | tr -d '\r')
[ -n "$PID" ] || { echo "FAIL  app did not start"; exit 1; }
adb forward tcp:9222 "localabstract:webview_devtools_remote_$PID"
shot 01-start

node scripts/android-smoke.js 9222 start "$OUT"; ST=$?
# 4: this emulator's WebView is too old for the app, and the app said so. Nothing more to drive.
OLD=0; if [ $ST = 4 ]; then OLD=1; elif [ $ST != 0 ]; then FAIL=1; fi
shot 02-topic

if [ $OLD = 0 ]; then
  adb shell input keyevent KEYCODE_BACK
  node scripts/android-smoke.js 9222 back || FAIL=1
  shot 03-after-back

  node scripts/android-smoke.js 9222 tour "$OUT" || FAIL=1
fi

# Leaving the app saves progress.
adb shell input keyevent KEYCODE_HOME
sleep 3
adb shell run-as "$PKG" find . -name 'progress.json*' > "$OUT/progress-files.txt" 2>&1
if [ $OLD = 1 ]; then
  echo "INFO  old WebView: there is no progress to save"
elif grep -q 'progress.json$' "$OUT/progress-files.txt"; then
  echo "PASS  progress saved when leaving the app: $(tr '\r\n' '  ' < "$OUT/progress-files.txt")"
else
  echo "FAIL  no progress.json after leaving the app"; cat "$OUT/progress-files.txt"; FAIL=1
fi
adb shell pidof "$PKG" > /dev/null && echo "PASS  app still running in the background" || { echo "FAIL  app died"; FAIL=1; }

# Losing the renderer process (Android 8+ tells the app; MainActivity.kt starts it again).
SDK=$(adb shell getprop ro.build.version.sdk | tr -d '\r')
if [ "$SDK" -ge 26 ] && [ $OLD = 0 ]; then
  adb shell am start -n "$PKG/.MainActivity"
  sleep 4
  node scripts/android-smoke.js 9222 crash; CR=$?
  if [ $CR = 0 ]; then
    sleep 12
    NEWPID=$(adb shell pidof "$PKG" | tr -d '\r')
    if [ -n "$NEWPID" ] && [ "$NEWPID" != "$PID" ]; then
      echo "PASS  the app started again by itself (process $PID -> $NEWPID)"
      adb forward --remove tcp:9222
      adb forward tcp:9222 "localabstract:webview_devtools_remote_$NEWPID"
      node scripts/android-smoke.js 9222 alive "$OUT" || FAIL=1
    else
      echo "FAIL  the app did not start again after losing its renderer (process '$NEWPID', was $PID)"; FAIL=1
      shot 12-renderer-loss-failed
    fi
  elif [ $CR != 3 ]; then FAIL=1; fi
fi

adb logcat -d > "$OUT/logcat.txt"
if grep -E "panicked|FATAL EXCEPTION" "$OUT/logcat.txt"; then FAIL=1; else echo "PASS  no crashes in the log"; fi
if grep -E "Uncaught|SyntaxError" "$OUT/logcat.txt" | head -20; then echo "INFO  the WebView logged the page errors above"; fi
echo "Result: $([ $FAIL = 0 ] && echo PASS || echo FAIL)"
exit $FAIL
