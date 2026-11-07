#!/usr/bin/env node
'use strict';
// Builds FreeSheet installers into ./build, using Tauri (the Rust app in src-tauri/).
//
//   npm run build                 build for this computer's system
//   npm run build -- --win        Windows: .exe installer + .msi       (on Windows)
//   npm run build -- --mac        macOS:   .dmg + .zip, Intel and Apple Silicon (on a Mac)
//   npm run build -- --linux      Linux:   .AppImage + .deb              (on Linux)
//   npm run build -- --android    Android: .apk (needs the Android SDK + NDK; see README)
//   options: --x64 / --arm64      only that Mac architecture (CI builds each on its own machine)
//            --no-validate        skip the content check
//            --keep               do not empty ./build first
//            --set-version X.Y.Z  set the app version without being asked
//
// Version: when run in a terminal, the build first asks for the new version number
// (press Enter to accept the suggested next patch version, e.g. 1.1.0 -> 1.1.1).
// The answer is saved to package.json (which Tauri reads) and src-tauri/Cargo.toml.
// With no terminal (GitHub Actions, scripts) it does not ask.
//
// Tauri builds an app on the system it is for, so Windows installers come from Windows,
// Mac ones from a Mac and so on. To get everything at once, use the GitHub Actions
// workflows (.github/workflows/build.yml and android.yml), which run on all of them.
//
// Needs: Node.js, Rust (https://rustup.rs) and `npm install` (for the Tauri CLI).
// Linux also needs the WebKitGTK development packages (see README).
// Code signing: unsigned unless the usual Tauri signing variables are set
// (e.g. APPLE_SIGNING_IDENTITY); unsigned Mac builds are ad-hoc signed so they open.

const path = require('path');
const fs = require('fs');
const os = require('os');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const OUT = path.join(ROOT, 'build');
const TAURI = path.join(ROOT, 'src-tauri');
const argv = process.argv.slice(2);
const flag = (f) => argv.includes(f);

let setVersion = null;
for (let i = 0; i < argv.length; i++) {
  if (argv[i] === '--set-version') { setVersion = argv[i + 1] || ''; argv.splice(i, 2); break; }
  if (argv[i].startsWith('--set-version=')) { setVersion = argv[i].slice(14); argv.splice(i, 1); break; }
}

const known = ['--win', '--mac', '--linux', '--android', '--x64', '--arm64', '--no-validate', '--keep'];
const unknown = argv.filter((a) => !known.includes(a));
if (unknown.length) {
  console.error(`Unknown option(s): ${unknown.join(' ')}\nValid options: ${known.join(' ')}`);
  process.exit(2);
}

const host = process.platform; // win32 | darwin | linux
const HOST_PLATFORM = { win32: 'win', darwin: 'mac', linux: 'linux' }[host];
const pkg = require(path.join(ROOT, 'package.json'));
const win = host === 'win32';

// ---- Version number ----
const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;

function nextPatch(v) {
  const m = String(v).match(/^(\d+)\.(\d+)\.(\d+)/);
  return m ? `${m[1]}.${m[2]}.${Number(m[3]) + 1}` : '1.0.1';
}

function compareVersions(a, b) {
  const pa = a.split('-')[0].split('.').map(Number);
  const pb = b.split('-')[0].split('.').map(Number);
  for (let i = 0; i < 3; i++) if (pa[i] !== pb[i]) return pa[i] < pb[i] ? -1 : 1;
  return 0;
}

function ask(question) {
  const readline = require('readline');
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  return new Promise((resolve) => rl.question(question, (a) => { rl.close(); resolve(a.trim()); }));
}

// Writes the version into package.json, package-lock.json and Cargo.toml (keeping formatting).
function saveVersion(v) {
  const edit = (file, fn) => { if (fs.existsSync(file)) fs.writeFileSync(file, fn(fs.readFileSync(file, 'utf8'))); };
  edit(path.join(ROOT, 'package.json'), (t) => t.replace(/("version"\s*:\s*")[^"]*(")/, `$1${v}$2`));
  edit(path.join(ROOT, 'package-lock.json'), (t) => {
    const lock = JSON.parse(t);
    lock.version = v;
    if (lock.packages && lock.packages['']) lock.packages[''].version = v;
    return JSON.stringify(lock, null, 2) + '\n';
  });
  edit(path.join(TAURI, 'Cargo.toml'), (t) => t.replace(/^version = "[^"]*"/m, `version = "${v}"`));
  pkg.version = v;
}

async function chooseVersion() {
  const current = pkg.version;
  if (setVersion !== null) {
    if (!SEMVER.test(setVersion)) {
      console.error(`--set-version "${setVersion}" is not a version like 1.2.3`);
      process.exit(2);
    }
  } else if (process.stdin.isTTY && !process.env.CI) {
    const suggested = nextPatch(current);
    console.log(`\nCurrent version: ${current}`);
    for (;;) {
      const a = await ask(`New version number [${suggested}] (type ${current} to keep it): `);
      const v = a.replace(/^v/i, '') || suggested;
      if (!SEMVER.test(v)) { console.log('  Please enter a version like 1.2.3'); continue; }
      if (compareVersions(v, current) < 0) {
        const ok = await ask(`  ${v} is older than ${current}. Use it anyway? [y/N]: `);
        if (!/^y/i.test(ok)) continue;
      }
      setVersion = v;
      break;
    }
  } else {
    return;
  }
  if (setVersion !== current) {
    saveVersion(setVersion);
    console.log(`Version set to ${setVersion} (saved in package.json and src-tauri/Cargo.toml). Commit it so GitHub builds use it too.`);
  }
}

// On Windows commands go through the shell (for .cmd files), so paths with spaces are quoted.
const q = (a) => (win && /\s/.test(a) && !/^".*"$/.test(a) ? `"${a}"` : a);

function run(cmd, args, opts = {}) {
  console.log(`> ${cmd} ${args.join(' ')}`);
  const res = spawnSync(q(cmd), args.map(q), { cwd: ROOT, stdio: 'inherit', shell: win, ...opts });
  return res.status === 0;
}

function tauri(args, opts) {
  const bin = path.join(ROOT, 'node_modules', '.bin', win ? 'tauri.cmd' : 'tauri');
  if (!fs.existsSync(bin)) {
    console.error('The Tauri CLI is missing. Run "npm install" first.');
    process.exit(1);
  }
  return run(bin, args, opts);
}

function copyOut(from, name) {
  fs.mkdirSync(OUT, { recursive: true });
  const dest = path.join(OUT, name);
  fs.copyFileSync(from, dest);
  return dest;
}

function findFiles(dir, re) {
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir).filter((f) => re.test(f)).map((f) => path.join(dir, f));
}

// ---- Per platform ----

function buildWin() {
  if (!tauri(['build', '--bundles', 'nsis,msi'])) return null;
  const b = path.join(TAURI, 'target', 'release', 'bundle');
  const out = [];
  for (const f of findFiles(path.join(b, 'nsis'), /\.exe$/)) out.push(copyOut(f, `FreeSheet-Setup-${pkg.version}.exe`));
  for (const f of findFiles(path.join(b, 'msi'), /\.msi$/)) out.push(copyOut(f, `FreeSheet-${pkg.version}.msi`));
  return out;
}

function buildMac() {
  const only = ['x64', 'arm64'].filter((a) => flag(`--${a}`));
  const archs = only.length ? only : ['x64', 'arm64'];
  const TRIPLE = { x64: 'x86_64-apple-darwin', arm64: 'aarch64-apple-darwin' };
  const signing = process.env.APPLE_SIGNING_IDENTITY ? [] : ['--config', JSON.stringify({ bundle: { macOS: { signingIdentity: '-' } } })];
  const out = [];
  for (const arch of archs) {
    const triple = TRIPLE[arch];
    run('rustup', ['target', 'add', triple]);
    if (!tauri(['build', '--target', triple, '--bundles', 'app,dmg', ...signing])) return null;
    const b = path.join(TAURI, 'target', triple, 'release', 'bundle');
    for (const f of findFiles(path.join(b, 'dmg'), /\.dmg$/)) out.push(copyOut(f, `FreeSheet-${pkg.version}-mac-${arch}.dmg`));
    const app = path.join(b, 'macos', 'FreeSheet.app');
    const zip = path.join(OUT, `FreeSheet-${pkg.version}-mac-${arch}.zip`);
    if (fs.existsSync(app) && run('ditto', ['-c', '-k', '--keepParent', app, zip])) out.push(zip);
  }
  return out;
}

function buildLinux() {
  if (!tauri(['build', '--bundles', 'appimage,deb'])) return null;
  const b = path.join(TAURI, 'target', 'release', 'bundle');
  const out = [];
  for (const f of findFiles(path.join(b, 'appimage'), /\.AppImage$/)) out.push(copyOut(f, `FreeSheet-${pkg.version}-linux-x64.AppImage`));
  for (const f of findFiles(path.join(b, 'deb'), /\.deb$/)) out.push(copyOut(f, `FreeSheet-${pkg.version}-linux-x64.deb`));
  return out;
}

function buildAndroid() {
  const r = spawnSync(process.execPath, [path.join(__dirname, 'build-android.js')], { cwd: ROOT, stdio: 'inherit' });
  if (r.status !== 0) return null;
  return findFiles(path.join(OUT, 'android'), /\.apk$/);
}

async function main() {
  await chooseVersion();
  console.log(`\nFreeSheet ${pkg.version}: building on ${host} (${os.arch()}) into ${OUT}\n`);

  if (!flag('--no-validate')) {
    console.log('Checking content…');
    if (!run(process.execPath, [path.join(ROOT, 'scripts', 'validate-content.js')])) {
      console.error('\nContent has errors (see above). Fix them, or rerun with --no-validate.');
      process.exit(1);
    }
  }

  if (!flag('--keep') && fs.existsSync(OUT)) {
    try {
      fs.rmSync(OUT, { recursive: true, force: true, maxRetries: 3, retryDelay: 500 });
    } catch (err) {
      if (err.code === 'EPERM' || err.code === 'EBUSY') {
        console.error(`\nCould not clear the build folder (${OUT}): something is using it.\n` +
          'Close any FreeSheet or installer running from it, then try again (or use --keep).');
        process.exit(1);
      }
      throw err;
    }
  }

  let wanted = ['win', 'mac', 'linux', 'android'].filter((p) => flag(`--${p}`));
  if (!wanted.length) wanted = [HOST_PLATFORM];
  const LABEL = { win: 'Windows', mac: 'macOS', linux: 'Linux', android: 'Android' };
  const BUILD = { win: buildWin, mac: buildMac, linux: buildLinux, android: buildAndroid };

  const results = [];
  for (const p of wanted) {
    if (p !== 'android' && p !== HOST_PLATFORM) {
      results.push({ label: LABEL[p], status: 'skipped', note: `${LABEL[p]} apps are built on ${LABEL[p]} (or with the GitHub Actions workflow)`, files: [] });
      continue;
    }
    console.log(`\n=== ${LABEL[p]} ===\n`);
    const files = BUILD[p]();
    results.push({ label: LABEL[p], status: files ? 'built' : 'failed', files: files || [] });
  }

  console.log('\n================ Build summary ================');
  for (const r of results) console.log(`${r.label.padEnd(8)} ${r.status.toUpperCase()}${r.note ? `  (${r.note})` : ''}`);
  const installers = results.flatMap((r) => r.files).filter((f) => fs.existsSync(f))
    .map((f) => `  ${path.relative(ROOT, f)}  (${(fs.statSync(f).size / 1048576).toFixed(1)} MB)`);
  console.log(`\nInstallers:\n${installers.join('\n') || '  (none)'}\n`);
  process.exit(results.some((r) => r.status === 'failed') ? 1 : 0);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
