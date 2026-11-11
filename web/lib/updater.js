'use strict';
// Automatic updates, done only while nobody is using FreeSheet.
//
// Every UPDATE_CHECK_MINUTES the server asks GitHub (git fetch) whether there is a newer
// version of its branch. If there is, it waits until no one has had FreeSheet open for
// UPDATE_IDLE_MINUTES (an open page pings the server every 2 minutes; see
// public/api.js), then pulls it, installs any new packages and exits. systemd
// (Restart=always) starts it again straight away, now running the new version.
//
// It never touches a checkout with local changes, and does nothing in Docker or when
// AUTO_UPDATE=0.

const { execFile } = require('child_process');
const fs = require('fs');
const path = require('path');

function run(cmd, args, cwd) {
  return new Promise((resolve, reject) => {
    execFile(cmd, args, { cwd, timeout: 10 * 60e3, maxBuffer: 16 * 1048576 }, (err, stdout, stderr) => {
      if (err) reject(new Error(`${cmd} ${args.join(' ')}: ${(stderr || err.message).trim()}`));
      else resolve(stdout.trim());
    });
  });
}

// Pure, so the tests can check it: may we update now?
function shouldUpdate({ behind, lastSeen, now, idleMs }) {
  return behind && now - lastSeen >= idleMs;
}

function createUpdater({ repoRoot, webDir, enabled, checkMs, idleMs, lastSeen, log = console.log, exit = () => process.exit(0) }) {
  let timer = null;
  let behind = false;
  let busy = false;

  const isGitCheckout = () => fs.existsSync(path.join(repoRoot, '.git'));

  async function check() {
    if (busy) return;
    busy = true;
    try {
      if (!behind) {
        await run('git', ['fetch', '--quiet'], repoRoot);
        const [here, there] = await Promise.all([
          run('git', ['rev-parse', 'HEAD'], repoRoot),
          run('git', ['rev-parse', '@{u}'], repoRoot),
        ]);
        // Behind = the remote has commits we don't (and we have none it lacks).
        behind = here !== there && (await run('git', ['merge-base', '--is-ancestor', 'HEAD', '@{u}'], repoRoot).then(() => true, () => false));
        if (behind) log('Update available; will install it once no one is using FreeSheet.');
      }
      if (!shouldUpdate({ behind, lastSeen: lastSeen(), now: Date.now(), idleMs })) return;
      if (await run('git', ['status', '--porcelain', '--untracked-files=no'], repoRoot)) {
        log('Update skipped: this checkout has local changes (run update.sh by hand).');
        behind = false;
        return;
      }
      const before = await run('git', ['rev-parse', 'HEAD:web/package-lock.json'], repoRoot).catch(() => '');
      await run('git', ['pull', '--ff-only', '--quiet'], repoRoot);
      const after = await run('git', ['rev-parse', 'HEAD:web/package-lock.json'], repoRoot).catch(() => '');
      if (before !== after) await run('npm', ['install', '--omit=dev', '--no-audit', '--no-fund'], webDir);
      log('Updated. Restarting with the new version.');
      exit();
    } catch (e) {
      log(`Update check failed (will try again later): ${e.message}`);
    } finally {
      busy = false;
    }
  }

  return {
    start() {
      if (!enabled) return log('Automatic updates are off (AUTO_UPDATE=0).');
      if (!isGitCheckout()) return log('Automatic updates are off: not a git checkout.');
      // Once an update is waiting, look for a quiet moment every few minutes.
      const tick = () => { timer = setTimeout(async () => { await check(); tick(); }, behind ? Math.min(checkMs, 5 * 60e3) : checkMs); };
      setTimeout(check, 60e3); // shortly after start-up
      tick();
    },
    stop() { clearTimeout(timer); },
    check,
  };
}

module.exports = { createUpdater, shouldUpdate };
