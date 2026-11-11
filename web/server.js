#!/usr/bin/env node
'use strict';
// FreeSheet web edition: run it with `node server.js` (install.sh sets it up as a service).
// Settings come from web/.env (see .env.example and SETUP.md).

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const http = require('http');
const https = require('https');
const { createApp } = require('./lib/app');
const { createUpdater } = require('./lib/updater');

const WEB = __dirname;
const ENV_FILE = path.join(WEB, '.env');

// ---- .env: KEY=value lines; # comments; values may be quoted. Real environment
// variables win, so Docker and tests can override anything.
function loadEnv(file) {
  if (!fs.existsSync(file)) return;
  for (const line of fs.readFileSync(file, 'utf8').split(/\r?\n/)) {
    const m = /^\s*([A-Z0-9_]+)\s*=\s*(.*?)\s*$/.exec(line);
    if (!m || line.trim().startsWith('#')) continue;
    let v = m[2];
    if (/^(['"]).*\1$/.test(v)) v = v.slice(1, -1);
    if (process.env[m[1]] === undefined) process.env[m[1]] = v;
  }
}
loadEnv(ENV_FILE);
const env = process.env;

const dataDir = path.resolve(WEB, env.DATA_DIR || 'data');

// A session secret is needed; make one the first time and keep it in .env (or, when
// there is no .env, as in Docker, in the data folder).
if (!env.SESSION_SECRET) {
  const kept = path.join(dataDir, 'session-secret');
  if (fs.existsSync(kept)) {
    env.SESSION_SECRET = fs.readFileSync(kept, 'utf8').trim();
  } else {
    env.SESSION_SECRET = crypto.randomBytes(32).toString('hex');
    try {
      if (fs.existsSync(ENV_FILE)) {
        require('./lib/envfile').setEnv(ENV_FILE, 'SESSION_SECRET', env.SESSION_SECRET);
      } else {
        fs.mkdirSync(dataDir, { recursive: true });
        fs.writeFileSync(kept, env.SESSION_SECRET, { mode: 0o600 });
      }
      console.log('Made a new SESSION_SECRET.');
    } catch (e) {
      console.warn(`Could not save SESSION_SECRET (${e.message}); everyone will be signed out when the server restarts.`);
    }
  }
}

const httpsOn = !!(env.HTTPS_CERT && env.HTTPS_KEY);
const cfg = {
  repoRoot: path.join(WEB, '..'),
  port: Number(env.PORT || 5000),
  host: env.HOST || '0.0.0.0',
  dataDir,
  sessionSecret: env.SESSION_SECRET,
  // Secure cookies need https. Behind an https proxy (Cloudflare Tunnel), set COOKIE_SECURE=1.
  secureCookies: httpsOn || env.COOKIE_SECURE === '1',
  trustProxy: env.TRUST_PROXY === '1',
};


const { app, lastSeen, accounts } = createApp(cfg);
if (!accounts.list().length) console.warn('No accounts yet: nobody can sign in. Make one with  ./users.sh add <name>');
if (!httpsOn && env.COOKIE_SECURE !== '1') console.warn('No HTTPS: passwords travel unencrypted. Fine at home for testing; run ./setup-https.sh before using it over the internet.');

// ---- HTTP or HTTPS. Renewed certificates are picked up without a restart: on SIGHUP
// (the renewal hook in SETUP.md sends it) and by checking the file twice a day.
let server;
if (httpsOn) {
  const read = () => ({ cert: fs.readFileSync(env.HTTPS_CERT), key: fs.readFileSync(env.HTTPS_KEY) });
  server = https.createServer(read(), app);
  let certTime = fs.statSync(env.HTTPS_CERT).mtimeMs;
  const reload = (why) => {
    try {
      server.setSecureContext(read());
      certTime = fs.statSync(env.HTTPS_CERT).mtimeMs;
      console.log(`Reloaded the HTTPS certificate (${why}).`);
    } catch (e) {
      console.error(`Could not reload the HTTPS certificate: ${e.message}`);
    }
  };
  process.on('SIGHUP', () => reload('SIGHUP'));
  setInterval(() => {
    try { if (fs.statSync(env.HTTPS_CERT).mtimeMs !== certTime) reload('file changed'); } catch { /* keep the old one */ }
  }, 12 * 3600e3).unref();
} else {
  server = http.createServer(app);
}

server.on('error', (e) => {
  if (e.code === 'EADDRINUSE') {
    console.error(`Port ${cfg.port} is already in use. See which program has it with:\n  sudo ss -ltnp 'sport = :${cfg.port}'\nthen stop that program (SETUP.md, "Port 5000 is busy").`);
  } else {
    console.error(e);
  }
  process.exit(1);
});

server.listen(cfg.port, cfg.host, () => {
  console.log(`FreeSheet web edition on ${httpsOn ? 'https' : 'http'}://${cfg.host === '0.0.0.0' ? 'localhost' : cfg.host}:${cfg.port}`);
  console.log(`Data folder: ${cfg.dataDir}`);
});

// ---- automatic updates while no one is using it (lib/updater.js)
createUpdater({
  repoRoot: cfg.repoRoot,
  webDir: WEB,
  enabled: env.AUTO_UPDATE !== '0' && !fs.existsSync('/.dockerenv'),
  checkMs: Number(env.UPDATE_CHECK_MINUTES || 60) * 60e3,
  idleMs: Number(env.UPDATE_IDLE_MINUTES || 15) * 60e3,
  lastSeen,
}).start();

// Stop cleanly (systemd sends SIGTERM).
process.on('SIGTERM', () => {
  server.close(() => process.exit(0));
  setTimeout(() => process.exit(0), 3000).unref(); // open keep-alive connections
});
