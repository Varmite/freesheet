#!/usr/bin/env node
'use strict';
// Builds the FreeSheet Android Edition APK into ../build/android.
//   npm run build            (from "Android Edition")
// Needs the Android SDK (ANDROID_HOME) and JDK 21. Without them, build on GitHub instead:
//   gh workflow run android.yml --ref master
// Signing: uses keystore/keystore.properties when present (see README.md), else the debug key.

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const HERE = path.join(__dirname, '..');
const ROOT = path.join(HERE, '..');
const ANDROID = path.join(HERE, 'android');
const OUT = path.join(ROOT, 'build', 'android');
const version = require(path.join(ROOT, 'package.json')).version;
const win = process.platform === 'win32';

function run(cmd, args, opts = {}) {
  console.log(`> ${cmd} ${args.join(' ')}`);
  const r = spawnSync(cmd, args, { stdio: 'inherit', shell: win, ...opts });
  if (r.status !== 0) process.exit(r.status || 1);
}

if (!process.env.ANDROID_HOME && !process.env.ANDROID_SDK_ROOT && !fs.existsSync(path.join(ANDROID, 'local.properties'))) {
  console.error('The Android SDK was not found (set ANDROID_HOME). Build on GitHub instead: gh workflow run android.yml --ref master');
  process.exit(1);
}

const env = { ...process.env };
const props = path.join(HERE, 'keystore', 'keystore.properties');
if (fs.existsSync(props) && !env.FREESHEET_KEYSTORE) {
  for (const line of fs.readFileSync(props, 'utf8').split(/\r?\n/)) {
    const m = /^(\w+)=(.*)$/.exec(line.trim());
    if (m) env[m[1]] = m[1] === 'FREESHEET_KEYSTORE' ? path.resolve(HERE, m[2]) : m[2];
  }
}
if (!env.FREESHEET_KEYSTORE) console.warn('No release key found: the APK will be signed with the debug key.');

run('node', [path.join(__dirname, 'prepare-web.js')]);
run('npx', ['cap', 'sync', 'android'], { cwd: HERE });
run(win ? 'gradlew.bat' : './gradlew', ['assembleRelease', '--no-daemon'], { cwd: ANDROID, env });

const apk = path.join(ANDROID, 'app', 'build', 'outputs', 'apk', 'release', 'app-release.apk');
fs.mkdirSync(OUT, { recursive: true });
const dest = path.join(OUT, `FreeSheet-${version}-android.apk`);
fs.copyFileSync(apk, dest);
console.log(`\nAPK ready: ${dest} (${(fs.statSync(dest).size / 1048576).toFixed(0)} MB)`);
