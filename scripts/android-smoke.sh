#!/usr/bin/env bash
# Android smoke test (run by .github/workflows/android-smoke.yml inside an emulator):
#   bash scripts/android-smoke.sh <apk> <out-dir>
# Installs the APK, taps through the app by on-screen text and saves a screenshot, the
# screen's text (uiautomator) and the log after each step. Fails if a step's text is missing
# or the app crashes.
set -u
APK="$1"; OUT="$2"; mkdir -p "$OUT"
PKG=za.freesheet.app
FAIL=0

adb install -r "$APK" || exit 1
adb logcat -c
adb shell am start -n "$PKG/.MainActivity"
sleep 25

dump() { # dump <name>: screenshot + screen text
  adb exec-out screencap -p > "$OUT/$1.png"
  adb shell uiautomator dump /sdcard/ui.xml > /dev/null 2>&1
  adb shell cat /sdcard/ui.xml > "$OUT/$1.xml"
}

# Centre of the first element whose text or description contains $1.
find_text() {
  python3 - "$1" "$2" <<'EOF'
import re, sys, xml.etree.ElementTree as ET
needle, f = sys.argv[1], sys.argv[2]
try:
    root = ET.parse(f).getroot()
except Exception:
    sys.exit(1)
for n in root.iter('node'):
    if needle in (n.get('text') or '') or needle in (n.get('content-desc') or ''):
        x1, y1, x2, y2 = map(int, re.findall(r'\d+', n.get('bounds')))
        if x2 > x1 and y2 > y1:
            print((x1 + x2) // 2, (y1 + y2) // 2)
            sys.exit(0)
sys.exit(1)
EOF
}

# step <name> <text to tap> [required=1]
step() {
  dump "$1-before"
  local xy
  if xy=$(find_text "$2" "$OUT/$1-before.xml"); then
    echo "[$1] tap '$2' at $xy"
    adb shell input tap $xy
    sleep 4
  else
    echo "[$1] '$2' not found on screen"
    [ "${3:-1}" = 1 ] && FAIL=1
  fi
}

alive() {
  if ! adb shell pidof "$PKG" > /dev/null; then echo "App is not running ($1)"; FAIL=1; fi
}

alive start
step 01-welcome "Grade 10"
step 02-buddy "Skip for now" 0
step 03-subjects "Mathematics"
step 04-topics "Algebraic Expressions"
dump 05-topic
if grep -q "Listen to these notes" "$OUT/05-topic.xml"; then echo "Read-aloud audio found"; else echo "No Listen button (audio list from the APK failed?)"; FAIL=1; fi
step 06-listen "Listen to these notes"
sleep 4
dump 07-playing
adb shell input keyevent KEYCODE_BACK; sleep 3
dump 08-after-back
alive "after back"
if grep -q "Algebraic Expressions" "$OUT/08-after-back.xml"; then echo "Back went up to the topic list"; else echo "Back did not go to the topic list"; FAIL=1; fi

# Leaving the app saves progress.
adb shell input keyevent KEYCODE_HOME; sleep 3
adb shell run-as "$PKG" find . -name 'progress.json*' > "$OUT/progress-files.txt" 2>&1
cat "$OUT/progress-files.txt"
grep -q progress.json "$OUT/progress-files.txt" || { echo "No progress.json saved"; FAIL=1; }

adb logcat -d > "$OUT/logcat.txt"
grep -E "panicked|FATAL EXCEPTION|RustStdoutStderr" "$OUT/logcat.txt" | head -40
grep -q -E "panicked|FATAL EXCEPTION" "$OUT/logcat.txt" && FAIL=1
echo "Result: $([ $FAIL = 0 ] && echo PASS || echo FAIL)"
exit $FAIL
