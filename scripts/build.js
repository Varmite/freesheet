#!/usr/bin/env node
'use strict';
// Builds FreeSheet installers into ./build. Works the same on Windows, macOS and Linux.
//
//   npm run build            build every platform this computer can build
//   npm run build -- --win   Windows only  (.exe installer + .msi)
//   npm run build -- --mac   macOS only    (.dmg + .zip, Intel and Apple Silicon)
//   npm run build -- --linux Linux only    (.AppImage + .deb + .tar.gz)
//   options: --x64 / --arm64 only that architecture (CI builds each Mac arch on its own machine)
//            --no-validate   skip the content check
//            --keep          do not empty ./build first
//            --set-version X.Y.Z  set the app version without being asked
//
// Version: when run in a terminal, the build first asks for the new version number
// (press Enter to accept the suggested next patch version, e.g. 1.0.0 -> 1.0.1).
// The answer is saved to package.json and package-lock.json. With no terminal
// (GitHub Actions, scripts) it does not ask and uses package.json as it is.
//
// What can be built where (electron-builder limits):
//   Windows host: Windows, Linux .tar.gz (+ .AppImage with Developer Mode on; .deb needs Linux/WSL)
//   macOS host:   macOS, Linux, Windows .exe (the .msi needs Windows)
//   Linux host:   Linux, Windows .exe (needs Wine; the .msi needs Windows)
// To get everything at once, push to GitHub and use .github/workflows/build.yml,
// which builds on real Windows, macOS and Linux machines.
//
// Code signing: builds are unsigned unless CSC_LINK / CSC_NAME (and for macOS notarisation,
// APPLE_ID etc.) are set, in which case electron-builder signs as usual. Unsigned macOS
// builds are ad-hoc signed so they still launch on Apple Silicon.

const path = require('path');
const fs = require('fs');
const os = require('os');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const OUT = path.join(ROOT, 'build');
const argv = process.argv.slice(2);
const flag = (f) => argv.includes(f);

// --set-version X.Y.Z or --set-version=X.Y.Z
let setVersion = null;
for (let i = 0; i < argv.length; i++) {
  if (argv[i] === '--set-version') { setVersion = argv[i + 1] || ''; argv.splice(i, 2); break; }
  if (argv[i].startsWith('--set-version=')) { setVersion = argv[i].slice(14); argv.splice(i, 1); break; }
}

const known = ['--win', '--mac', '--linux', '--x64', '--arm64', '--no-validate', '--keep'];
const unknown = argv.filter((a) => !known.includes(a));
if (unknown.length) {
  console.error(`Unknown option(s): ${unknown.join(' ')}\nValid options: ${known.join(' ')}`);
  process.exit(2);
}

const host = process.platform; // win32 | darwin | linux
const pkg = require(path.join(ROOT, 'package.json'));

function commandWorks(cmd, args) {
  try {
    const res = spawnSync(cmd, args, { stdio: 'ignore' });
    return res.status === 0;
  } catch (e) {
    return false;
  }
}

// AppImage packaging creates symlinks; on Windows that needs Developer Mode or an admin prompt.
function canSymlink() {
  if (host !== 'win32') return true;
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fs-link-'));
  try {
    fs.writeFileSync(path.join(dir, 'a'), '');
    fs.symlinkSync(path.join(dir, 'a'), path.join(dir, 'b'), 'file');
    return true;
  } catch (e) {
    return false;
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

// The .deb needs a project homepage. Use package.json, else GitHub Actions, else the git remote.
function findHomepage() {
  if (pkg.homepage) return { url: pkg.homepage, inject: false };
  if (pkg.repository) return { url: String(pkg.repository.url || pkg.repository), inject: false };
  if (process.env.GITHUB_REPOSITORY) {
    const server = process.env.GITHUB_SERVER_URL || 'https://github.com';
    return { url: `${server}/${process.env.GITHUB_REPOSITORY}`, inject: true };
  }
  const res = spawnSync('git', ['config', '--get', 'remote.origin.url'], { cwd: ROOT, encoding: 'utf8' });
  const remote = res.status === 0 ? res.stdout.trim() : '';
  const m = remote.match(/^(?:git@|ssh:\/\/git@|https?:\/\/)([^/:]+)[/:](.+?)(?:\.git)?\/?$/);
  if (m) return { url: `https://${m[1]}/${m[2]}`, inject: true };
  return null;
}

const symlinks = canSymlink();
const wine = host === 'linux' ? commandWorks('wine', ['--version']) : true;
const homepage = findHomepage();
const signing = Boolean(process.env.CSC_LINK || process.env.CSC_NAME);

// Targets per platform, and which of them this host can produce.
function planFor(p) {
  if (p === 'win') {
    const notes = [];
    if (host !== 'win32') notes.push('.msi skipped (it can only be built on Windows)');
    if (!wine) notes.push('needs Wine on Linux (e.g. "sudo apt install wine"), or use the GitHub Actions workflow');
    return {
      label: 'Windows',
      targets: host === 'win32' ? ['nsis', 'msi'] : ['nsis'],
      archs: ['x64'],
      possible: wine,
      note: notes.join('; ') || null,
    };
  }
  if (p === 'mac') {
    return {
      label: 'macOS',
      targets: ['dmg', 'zip'],
      archs: ['x64', 'arm64'],
      possible: host === 'darwin',
      note: host === 'darwin'
        ? (signing ? null : 'unsigned (ad-hoc signed); on first launch right-click the app and choose Open')
        : 'macOS apps can only be built on a Mac (or with the GitHub Actions workflow)',
    };
  }
  // linux
  const targets = [];
  const notes = [];
  if (symlinks) targets.push('AppImage');
  else notes.push('.AppImage skipped (turn on Windows Developer Mode, or run as administrator, to allow it)');
  if (host === 'win32') notes.unshift('.deb skipped (needs Linux, macOS, WSL or the GitHub Actions workflow)');
  else if (!homepage) notes.unshift('.deb skipped (add "homepage" to package.json, or a git remote, to allow it)');
  else targets.push('deb');
  targets.push('tar.gz');
  return { label: 'Linux', targets, archs: ['x64'], possible: true, note: notes.join('; ') || null };
}

// Config overrides merged on top of the "build" section of package.json.
function configFor(p) {
  const config = {};
  if (p === 'linux' && homepage && homepage.inject) {
    config.extraMetadata = { homepage: homepage.url };
  }
  if (p === 'mac' && !signing) {
    config.mac = { identity: '-' }; // ad-hoc: required for Apple Silicon to open the app
  }
  if (p === 'win') {
    // Branded MSI dialogs (WiX needs absolute paths: light.exe runs in a temp folder).
    const res = path.join(ROOT, 'build-resources');
    const banner = path.join(res, 'msiBanner.bmp');
    const dialog = path.join(res, 'msiDialog.bmp');
    if (fs.existsSync(banner) && fs.existsSync(dialog)) {
      config.msi = { additionalLightArgs: [`-dWixUIBannerBmp=${banner}`, `-dWixUIDialogBmp=${dialog}`] };
    }
  }
  return config;
}

// ---- Version number ----
const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;

function nextPatch(v) {
  const m = String(v).match(/^(\d+)\.(\d+)\.(\d+)/);
  return m ? `${m[1]}.${m[2]}.${Number(m[3]) + 1}` : '1.0.1';
}

// -1 / 0 / 1, comparing only the X.Y.Z part.
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

// Writes the version into package.json and package-lock.json (keeping their formatting).
function saveVersion(v) {
  const pkgFile = path.join(ROOT, 'package.json');
  fs.writeFileSync(pkgFile, fs.readFileSync(pkgFile, 'utf8')
    .replace(/("version"\s*:\s*")[^"]*(")/, `$1${v}$2`));
  const lockFile = path.join(ROOT, 'package-lock.json');
  if (fs.existsSync(lockFile)) {
    const lock = JSON.parse(fs.readFileSync(lockFile, 'utf8'));
    lock.version = v;
    if (lock.packages && lock.packages['']) lock.packages[''].version = v;
    fs.writeFileSync(lockFile, JSON.stringify(lock, null, 2) + '\n');
  }
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
    return; // not interactive: keep package.json as it is
  }
  if (setVersion !== current) {
    saveVersion(setVersion);
    console.log(`Version set to ${setVersion} (saved in package.json). Commit it so GitHub builds use it too.`);
  }
}

function run(cmd, args) {
  const res = spawnSync(cmd, args, { cwd: ROOT, stdio: 'inherit' });
  return res.status === 0;
}

async function main() {
  await chooseVersion();
  console.log(`\nFreeSheet ${pkg.version}: building on ${host} (${os.arch()}) into ${OUT}\n`);

  let builder;
  try {
    builder = require(require.resolve('electron-builder', { paths: [ROOT] }));
  } catch (e) {
    console.error('Dependencies are missing. Run "npm install" first.');
    process.exit(1);
  }
  const { build, Platform, Arch } = builder;
  const PLATFORM = { win: Platform.WINDOWS, mac: Platform.MAC, linux: Platform.LINUX };

  if (!flag('--no-validate')) {
    console.log('Checking content…');
    if (!run(process.execPath, [path.join(ROOT, 'scripts', 'validate-content.js')])) {
      console.error('\nContent has errors (see above). Fix them, or rerun with --no-validate.');
      process.exit(1);
    }
  }

  // Read-aloud audio (made by `npm run tts`) is copied into the installers via extraResources.
  {
    let files = 0, bytes = 0;
    const walk = (d) => { if (!fs.existsSync(d)) return; for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p); else if (e.name.endsWith('.ogg')) { files++; bytes += fs.statSync(p).size; }
    } };
    walk(path.join(ROOT, 'audio'));
    console.log(files
      ? `Read-aloud audio: ${files} files (${(bytes / 1048576).toFixed(0)} MB) will be included.`
      : 'Read-aloud audio: none found (run `npm run tts` first); the Listen button will be hidden.');
  }

  if (!flag('--keep') && fs.existsSync(OUT)) {
    try {
      fs.rmSync(OUT, { recursive: true, force: true, maxRetries: 3, retryDelay: 500 });
    } catch (err) {
      if (err.code === 'EPERM' || err.code === 'EBUSY') {
        console.error(`\nCould not clear the build folder (${OUT}): something is using it.\n` +
          'Close any running FreeSheet copied from build\\, any Explorer window or installer open inside it, ' +
          'and wait for any other build to finish, then try again.');
        process.exit(1);
      }
      throw err;
    }
  }

  // Unsigned builds: do not search the keychain / certificate store for signing identities.
  if (!signing && process.env.CSC_IDENTITY_AUTO_DISCOVERY == null) {
    process.env.CSC_IDENTITY_AUTO_DISCOVERY = 'false';
  }

  let wanted = ['win', 'mac', 'linux'].filter((p) => flag(`--${p}`));
  const explicit = wanted.length > 0;
  if (!explicit) wanted = ['win', 'mac', 'linux'];

  const results = [];
  for (const p of wanted) {
    const plan = planFor(p);
    if (!plan.possible) {
      results.push({ p, label: plan.label, status: 'skipped', note: plan.note, files: [] });
      if (explicit) console.warn(`\n${plan.label}: ${plan.note}`);
      continue;
    }
    const only = ['x64', 'arm64'].filter((a) => flag(`--${a}`));
    const archs = only.length ? plan.archs.filter((a) => only.includes(a)) : plan.archs;
    if (!archs.length) {
      results.push({ p, label: plan.label, status: 'skipped', note: `no ${only.join('/')} build`, files: [] });
      continue;
    }
    console.log(`\n=== ${plan.label}: ${plan.targets.join(', ')} (${archs.join(', ')}) ===\n`);
    try {
      const files = await build({
        projectDir: ROOT,
        targets: PLATFORM[p].createTarget(plan.targets, ...archs.map((a) => Arch[a])),
        config: configFor(p),
        publish: 'never',
      });
      results.push({ p, label: plan.label, status: 'built', note: plan.note, files });
    } catch (e) {
      console.error(`\n${plan.label} build failed:\n${e && e.stack ? e.stack : e}`);
      results.push({ p, label: plan.label, status: 'failed', note: plan.note, files: [] });
    }
  }

  // Summary
  console.log('\n================ Build summary ================');
  for (const r of results) {
    console.log(`${r.label.padEnd(8)} ${r.status.toUpperCase()}${r.note ? `  (${r.note})` : ''}`);
  }
  const installers = results
    .flatMap((r) => r.files)
    .filter((f) => /\.(exe|msi|dmg|zip|AppImage|deb|tar\.gz)$/.test(f) && fs.existsSync(f))
    .map((f) => `  ${path.relative(ROOT, f)}  (${(fs.statSync(f).size / 1048576).toFixed(1)} MB)`);
  console.log(`\nInstallers:\n${installers.join('\n') || '  (none)'}\n`);

  process.exit(results.some((r) => r.status === 'failed') ? 1 : 0);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
