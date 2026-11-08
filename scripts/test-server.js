#!/usr/bin/env node
'use strict';
// Starts the website (server/server.js) with a stand-in for Google and checks that learners
// are kept apart, saves can't clobber newer ones, files stay private and sign-in is strict.
const assert = require('assert');
const crypto = require('crypto');
const fs = require('fs');
const http = require('http');
const os = require('os');
const path = require('path');
const { spawn } = require('child_process');

const ROOT = path.join(__dirname, '..');
const dataDir = fs.mkdtempSync(path.join(os.tmpdir(), 'freesheet-server-'));
const CLIENT = 'test-client';

// A pretend Google: the test decides what its token says.
let claims = {};
let lastTokenRequest = null;
const google = http.createServer((req, res) => {
  let body = '';
  req.on('data', (c) => { body += c; });
  req.on('end', () => {
    lastTokenRequest = new URLSearchParams(body);
    const token = ['x', Buffer.from(JSON.stringify(claims)).toString('base64url'), 'y'].join('.');
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ id_token: token }));
  });
});

// A browser: keeps its own cookies.
class Client {
  constructor(base) { this.base = base; this.jar = new Map(); }
  async go(url, opts = {}) {
    const headers = { ...(opts.headers || {}) };
    if (this.jar.size) headers.Cookie = [...this.jar].map(([k, v]) => `${k}=${v}`).join('; ');
    const r = await fetch(url.startsWith('http') ? url : this.base + url, { redirect: 'manual', ...opts, headers });
    for (const c of r.headers.getSetCookie()) {
      const [pair] = c.split(';');
      const i = pair.indexOf('=');
      if (/Max-Age=0/.test(c)) this.jar.delete(pair.slice(0, i)); else this.jar.set(pair.slice(0, i), pair.slice(i + 1));
    }
    return r;
  }
  load() { return this.go('/api/progress').then((r) => r.json()); }
  save(rev, data, extra = {}) {
    return this.go('/api/progress', { method: 'PUT', headers: { 'Content-Type': 'application/json', 'X-Rev': rev, ...extra }, body: JSON.stringify(data) });
  }
  async signIn(sub, tweak = {}) {
    const start = await this.go('/auth/google');
    assert.strictEqual(start.status, 302);
    const q = new URL(start.headers.get('location')).searchParams;
    claims = { iss: 'https://accounts.google.com', aud: CLIENT, exp: Math.floor(Date.now() / 1000) + 600, nonce: q.get('nonce'), sub, email: `${sub}@example.com`, ...tweak };
    const r = await this.go(`/auth/google/callback?code=abc&state=${q.get('state')}`);
    return { r, challenge: q.get('code_challenge') };
  }
}

const progress = (topics) => ({ version: 1, settings: { grade: 12 }, topics, exams: {}, examDrafts: {} });

async function run(base) {
  // ---- the site itself
  const a = new Client(base);
  let r = await a.go('/');
  assert.strictEqual(r.status, 200);
  const html = await r.text();
  assert(html.includes('/boot.js') && html.includes('mobile.css'), 'index gets the web additions');
  assert(r.headers.get('content-security-policy').includes("script-src 'self'"));
  assert(!r.headers.get('content-security-policy').includes('api.github.com'));
  assert(/HttpOnly/.test(r.headers.getSetCookie()[0]) && /SameSite=Lax/.test(r.headers.getSetCookie()[0]), 'cookie is protected');
  assert(/^[0-9a-f-]{36}$/.test(a.jar.get('fs_device')), 'browser gets a UUID');
  assert((await (await a.go('/js/api.js')).text()).includes('web: true'), 'the web bridge replaces api.js');
  const catalog = await (await a.go('/api/catalog')).json();
  const g = catalog.grades.find((x) => x.grade === 12);
  const s = g.subjects.find((x) => x.id === 'mathematics');
  assert(catalog.courses.length > 0 && s.topics.length > 0);
  const t = s.topics[0];
  assert.strictEqual((await (await a.go(`/api/topic/12/mathematics/${t.id}`)).json()).id, t.id);
  assert.strictEqual((await (await a.go(`/api/exam/12/mathematics/${s.exams[0].id}`)).json()).id, s.exams[0].id);
  assert((await (await a.go('/api/words')).json()).length > 1000);
  const list = await (await a.go(`/api/audio/12/mathematics/${t.id}`)).json();
  assert(list.length > 0, 'audio is listed');
  r = await a.go(`/api/audio/12/mathematics/${t.id}/${list[0]}`);
  assert.strictEqual(r.headers.get('content-type'), 'audio/ogg');
  assert.strictEqual(Buffer.from(await r.arrayBuffer()).subarray(0, 4).toString(), 'OggS');
  r = await a.go('/api/catalog', { headers: { 'Accept-Encoding': 'gzip' } });
  const etag = r.headers.get('etag');
  await r.arrayBuffer();
  assert.strictEqual((await a.go('/api/catalog', { headers: { 'If-None-Match': etag } })).status, 304);

  // ---- nothing outside the public folders
  for (const u of ['/../package.json', '/%2e%2e/package.json', '/js/..%2f..%2fpackage.json', '/..%5cpackage.json', '/.git/config', '/js/%2e%2e/%2e%2e/server/.env',
    '/api/topic/12/..%2f..%2f/x', '/api/topic/13/mathematics/x', '/api/audio/12/mathematics/x/..%2f..', '/api/course/..%2f..', '/server.js', '/data/sessions.json']) {
    r = await a.go(u);
    assert.strictEqual(r.status, 404, `${u} must not be served`);
    assert(!(await r.text()).includes('"devDependencies"'), `${u} leaked package.json`);
  }

  // ---- each browser has its own progress
  const b = new Client(base);
  let la = await a.load();
  let lb = await b.load();
  assert.deepStrictEqual(la.progress.topics, {});
  r = await a.save(la.rev, progress({ t: { notesRead: true } }));
  assert.strictEqual(r.status, 200);
  const rev2 = (await r.json()).rev;
  assert.deepStrictEqual((await a.load()).progress.topics, { t: { notesRead: true } });
  assert.deepStrictEqual((await b.load()).progress.topics, {}, 'another browser sees nothing');

  // ---- stale saves are refused, bad ones too
  assert.strictEqual((await a.save(la.rev, progress({ old: 1 }))).status, 409, 'a stale tab cannot overwrite');
  assert.strictEqual((await (await a.go('/api/rev')).json()).rev, rev2);
  assert.strictEqual((await a.save(rev2, { nope: true })).status, 400);
  assert.strictEqual((await a.save(rev2, [])).status, 400);
  assert.strictEqual((await a.save(rev2, progress({}), { Origin: 'https://evil.example' })).status, 403, 'another site cannot save');
  r = await a.go('/api/progress', { method: 'PUT', headers: { 'Content-Type': 'text/plain', 'X-Rev': rev2 }, body: '{}' });
  assert.strictEqual(r.status, 415);
  r = await a.go('/api/progress', { method: 'PUT', headers: { 'Content-Type': 'application/json', 'X-Rev': rev2 }, body: JSON.stringify(progress({ big: 'x'.repeat(6 * 1048576) })) }).catch(() => ({ status: 413 }));
  assert.strictEqual(r.status, 413);
  assert.deepStrictEqual((await a.load()).progress.topics, { t: { notesRead: true } }, 'failed saves change nothing');

  // ---- a saved theme reaches the first paint, safely
  r = await a.save(rev2, { ...progress({ t: { notesRead: true } }), settings: { theme: 'sepia', accent: '</script><x>' } });
  assert.strictEqual(r.status, 200);
  const boot = await (await a.go('/boot.js')).text();
  assert(boot.includes('"theme":"sepia"') && boot.includes('"accent":"default"'), 'odd values are dropped');

  // ---- signing in: strict about what comes back
  const c = new Client(base);
  await c.load();
  assert.strictEqual((await c.go('/auth/google/callback?code=abc&state=made-up')).status, 400, 'unknown state');
  for (const [tweak, why] of [[{ aud: 'someone-else' }, 'audience'], [{ iss: 'https://evil.example' }, 'issuer'], [{ exp: 1000 }, 'expiry'], [{ nonce: 'wrong' }, 'nonce']]) {
    const { r: bad } = await c.signIn('mallory', tweak);
    assert.strictEqual(bad.status, 400, `bad ${why} must be refused`);
    assert(!c.jar.has('fs_session'), `no session after bad ${why}`);
  }
  // State from one browser can't be finished in another (login CSRF).
  const start = await c.go('/auth/google');
  const stolen = new URL(start.headers.get('location')).searchParams.get('state');
  assert.strictEqual((await new Client(base).go(`/auth/google/callback?code=abc&state=${stolen}`)).status, 400);

  // ---- a new account takes this browser's progress
  la = await a.load();
  const { r: ok, challenge } = await a.signIn('alice');
  assert.strictEqual(ok.status, 302);
  assert.strictEqual(crypto.createHash('sha256').update(lastTokenRequest.get('code_verifier')).digest('base64url'), challenge, 'PKCE');
  assert.strictEqual(lastTokenRequest.get('client_secret'), 'test-secret');
  assert(a.jar.has('fs_session'));
  la = await a.load();
  assert.strictEqual(la.account.email, 'alice@example.com');
  assert.deepStrictEqual(la.progress.topics, { t: { notesRead: true } }, 'progress moved to the account');

  // ---- settings alone (a grade, a theme) also come along to a new account
  const fresh = new Client(base);
  const f0 = await fresh.load();
  assert.strictEqual((await fresh.save(f0.rev, { ...progress({}), settings: { grade: 9, theme: 'forest' } })).status, 200);
  await fresh.signIn('newcomer');
  assert.strictEqual((await fresh.load()).progress.settings.theme, 'forest');

  // ---- importing replaces the account's progress, and it follows her to a phone
  r = await a.save(la.rev, progress({ imported: { best: 9 } }));
  assert.strictEqual(r.status, 200);
  const phone = new Client(base);
  const before = await phone.load();
  const phoneSave = await phone.save(before.rev, progress({ phoneOnly: 1 }));
  assert.strictEqual(phoneSave.status, 200);
  await phone.signIn('alice');
  const onPhone = await phone.load();
  assert.deepStrictEqual(onPhone.progress.topics, { imported: { best: 9 } }, 'the account wins over a browser with its own progress');
  assert.strictEqual(onPhone.account.signedIn, true);

  // ---- two devices: the one left behind can't overwrite the newer save
  const stale = onPhone.rev;
  r = await a.load();
  assert.strictEqual((await a.save(r.rev, progress({ newer: 1 }))).status, 200);
  assert.strictEqual((await phone.save(stale, progress({ older: 1 }))).status, 409);

  // ---- signing out
  assert.strictEqual((await phone.go('/auth/logout', { method: 'POST', headers: { Origin: 'https://evil.example' } })).status, 403);
  const oldSession = phone.jar.get('fs_session');
  assert.strictEqual((await phone.go('/auth/logout', { method: 'POST' })).status, 200);
  const signedOut = await phone.load();
  assert.strictEqual(signedOut.account.signedIn, false);
  assert.deepStrictEqual(signedOut.progress.topics, { phoneOnly: 1 }, 'the browser kept its own progress');
  const replay = new Client(base);
  replay.jar.set('fs_session', oldSession);
  assert.strictEqual((await replay.load()).account.signedIn, false, 'a signed-out session is dead');

  // ---- nothing readable about the session token on disk
  const onDisk = fs.readFileSync(path.join(dataDir, 'sessions.json'), 'utf8');
  assert(!onDisk.includes(a.jar.get('fs_session')), 'session tokens are stored hashed');
}

async function main() {
  await new Promise((r) => google.listen(0, '127.0.0.1', r));
  const g = `http://127.0.0.1:${google.address().port}`;
  const child = spawn(process.execPath, [path.join(ROOT, 'server', 'server.js')], {
    env: { ...process.env, PORT: '0', HOST: '127.0.0.1', DATA_DIR: dataDir, PUBLIC_URL: 'https://site.example', GOOGLE_CLIENT_ID: CLIENT, GOOGLE_CLIENT_SECRET: 'test-secret', GOOGLE_AUTH_URL: `${g}/auth`, GOOGLE_TOKEN_URL: `${g}/token` },
    stdio: ['ignore', 'pipe', 'inherit'],
  });
  try {
    const base = await new Promise((resolve, reject) => {
      child.stdout.on('data', (d) => { const m = /running on (http:\/\/[\d.:]+)/.exec(String(d)); if (m) resolve(m[1]); });
      child.on('exit', (code) => reject(new Error(`server exited (${code})`)));
    });
    await run(base);
    console.log('Website checks passed.');
  } finally {
    child.removeAllListeners('exit');
    child.kill();
    google.close();
    await new Promise((r) => setTimeout(r, 200));
    fs.rmSync(dataDir, { recursive: true, force: true });
  }
}

main().catch((e) => { console.error(e); process.exit(1); });
