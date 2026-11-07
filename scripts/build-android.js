#!/usr/bin/env node
'use strict';
// Builds the FreeSheet Android app (an .apk for phones) into build/android, from the same
// Rust app (src-tauri/) and screens (src/) as the desktop version.
//   node scripts/build-android.js          (or: npm run build -- --android)
// Needs the Android SDK and NDK (ANDROID_HOME, NDK_HOME), JDK 17+ and Rust. Without
// them, build on GitHub instead:  gh workflow run android.yml --ref master
//
// Steps:
//   1. `tauri android init` makes the Android project in src-tauri/gen/android (once).
//   2. FreeSheet's own launcher icons (src-tauri/android/res) are copied over the defaults.
//   3. The read-aloud audio/ goes into the APK's assets (read by src-tauri/src/audio.rs).
//   4. Release signing: the key named by FREESHEET_KEYSTORE (+ _PASSWORD, FREESHEET_KEY_ALIAS,
//      FREESHEET_KEY_PASSWORD), or src-tauri/android/keystore/keystore.properties.
//      It must stay the same key, or phones cannot install updates over the old version.
//   5. `tauri android build --apk` for ARM phones (64- and 32-bit) and x86_64.

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const TAURI = path.join(ROOT, 'src-tauri');
const GEN = path.join(TAURI, 'gen', 'android');
const APP = path.join(GEN, 'app');
const OUT = path.join(ROOT, 'build', 'android');
const version = require(path.join(ROOT, 'package.json')).version;
const win = process.platform === 'win32';

// On Windows commands go through the shell (for .cmd files), so paths with spaces are quoted.
const q = (a) => (win && /\s/.test(a) && !/^".*"$/.test(a) ? `"${a}"` : a);

function run(cmd, args, opts = {}) {
  console.log(`> ${cmd} ${args.join(' ')}`);
  const r = spawnSync(q(cmd), args.map(q), { cwd: ROOT, stdio: 'inherit', shell: win, ...opts });
  if (r.status !== 0) process.exit(r.status || 1);
}
const tauriBin = path.join(ROOT, 'node_modules', '.bin', win ? 'tauri.cmd' : 'tauri');
const tauri = (args, opts) => run(tauriBin, args, opts);

function copyDir(from, to, filter) {
  fs.mkdirSync(to, { recursive: true });
  for (const e of fs.readdirSync(from, { withFileTypes: true })) {
    const a = path.join(from, e.name);
    const b = path.join(to, e.name);
    if (e.isDirectory()) copyDir(a, b, filter);
    else if (!filter || filter(e.name)) fs.copyFileSync(a, b);
  }
}

if (!process.env.ANDROID_HOME && !process.env.ANDROID_SDK_ROOT) {
  console.error('The Android SDK was not found (set ANDROID_HOME and NDK_HOME). Build on GitHub instead: gh workflow run android.yml --ref master');
  process.exit(1);
}
if (!fs.existsSync(tauriBin)) {
  console.error('The Tauri CLI is missing. Run "npm install" first.');
  process.exit(1);
}

// 1. The Android project.
if (!fs.existsSync(path.join(APP, 'build.gradle.kts'))) tauri(['android', 'init', '--ci']);

// 2. Icons.
copyDir(path.join(TAURI, 'android', 'res'), path.join(APP, 'src', 'main', 'res'));

// 3. Audio, fresh each time so deleted files do not linger.
const assets = path.join(APP, 'src', 'main', 'assets', 'audio');
fs.rmSync(assets, { recursive: true, force: true });
copyDir(path.join(ROOT, 'audio'), assets, (n) => n.endsWith('.ogg'));

// 4. Signing.
const gradle = path.join(APP, 'build.gradle.kts');
const MARK = '// FreeSheet release signing';
if (!fs.readFileSync(gradle, 'utf8').includes(MARK)) {
  fs.appendFileSync(gradle, `
${MARK} (added by scripts/build-android.js): the key named by FREESHEET_KEYSTORE,
// or the debug key when there is none (for testing only: phones cannot update it later).
android {
    val fsKeystore = System.getenv("FREESHEET_KEYSTORE")
    if (fsKeystore != null) {
        signingConfigs {
            create("freesheet") {
                storeFile = file(fsKeystore)
                storePassword = System.getenv("FREESHEET_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("FREESHEET_KEY_ALIAS")
                keyPassword = System.getenv("FREESHEET_KEY_PASSWORD")
            }
        }
    }
    buildTypes {
        getByName("release") {
            signingConfig = signingConfigs.getByName(if (fsKeystore != null) "freesheet" else "debug")
        }
    }
}
`);
}
const env = { ...process.env };
const props = path.join(TAURI, 'android', 'keystore', 'keystore.properties');
if (fs.existsSync(props) && !env.FREESHEET_KEYSTORE) {
  for (const line of fs.readFileSync(props, 'utf8').split(/\r?\n/)) {
    const m = /^(\w+)=(.*)$/.exec(line.trim());
    if (m) env[m[1]] = m[1] === 'FREESHEET_KEYSTORE' ? path.resolve(path.dirname(props), path.basename(m[2])) : m[2];
  }
}
if (!env.FREESHEET_KEYSTORE) console.warn('No release key found: the APK will be signed with the debug key.');

// 5. Build.
tauri(['android', 'build', '--apk', '--target', 'aarch64', 'armv7', 'x86_64'], { env });

const apkDir = path.join(APP, 'build', 'outputs', 'apk', 'universal', 'release');
const apk = fs.readdirSync(apkDir).find((f) => f.endsWith('.apk'));
if (!apk) { console.error(`No APK in ${apkDir}`); process.exit(1); }
fs.mkdirSync(OUT, { recursive: true });
const dest = path.join(OUT, `FreeSheet-${version}-android.apk`);
fs.copyFileSync(path.join(apkDir, apk), dest);
console.log(`\nAPK ready: ${dest} (${(fs.statSync(dest).size / 1048576).toFixed(0)} MB)`);
