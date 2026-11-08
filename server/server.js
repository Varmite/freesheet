#!/usr/bin/env node
'use strict';
// FreeSheet as a website: serves the same screens as the desktop app (src/), the notes,
// questions and exams (content/) and the read-aloud audio (audio/), and keeps each
// visitor's progress. Needs Node.js 20 or newer and nothing else. See server/README.md.
//
// Whose progress is it? Web pages cannot read a device's or operating system's real ID,
// so each browser is given a random UUID in a cookie the first time it visits. Signing in
// with Google moves progress from that browser to the account, so it follows the learner.
//
// Everything is stored as one JSON file per learner in DATA_DIR.

const http = require('http');
const fs = require('fs');
const fsp = fs.promises;
const path = require('path');
const crypto = require('crypto');
const zlib = require('zlib');

const ROOT = path.join(__dirname, '..');
const SRC = path.join(ROOT, 'src');
const CONTENT = path.join(ROOT, 'content');
const AUDIO = path.join(ROOT, 'audio');
const VERSION = JSON.parse(fs.readFileSync(path.join(ROOT, 'package.json'), 'utf8')).version;

const env = process.env;
const cfg = {
  port: Number(env.PORT || 8080),
  host: env.HOST || '0.0.0.0',
  publicUrl: (env.PUBLIC_URL || '').replace(/\/+$/, ''),
  dataDir: env.DATA_DIR || path.join(__dirname, 'data'),
  trustProxy: env.TRUST_PROXY === '1',
  maxDataBytes: Number(env.MAX_DATA_MB || 2048) * 1048576,
  googleId: env.GOOGLE_CLIENT_ID || '',
  googleSecret: env.GOOGLE_CLIENT_SECRET || '',
  // Only the tests point these somewhere else.
  googleAuthUrl: env.GOOGLE_AUTH_URL || 'https://accounts.google.com/o/oauth2/v2/auth',
  googleTokenUrl: env.GOOGLE_TOKEN_URL || 'https://oauth2.googleapis.com/token',
};
const googleOn = !!(cfg.googleId && cfg.googleSecret && cfg.publicUrl);

const MAX_BODY = 5 * 1048576;
const SESSION_MS = 180 * 86400e3;
const DEVICE_COOKIE_S = 400 * 86400;
const ID = /^[a-z0-9]+(-[a-z0-9]+)*$/;
const AUDIO_ID = /^[0-9a-f]{8}$/;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

// ---------------------------------------------------------------- content

const parse = (text) => JSON.parse(text.replace(/^﻿/, ''));
const readJson = (file) => parse(fs.readFileSync(file, 'utf8'));
const dirsIn = (dir) => (fs.existsSync(dir) ? fs.readdirSync(dir, { withFileTypes: true }).filter((e) => e.isDirectory()).map((e) => e.name) : []);

// The same catalogue the desktop app builds in src-tauri/src/content.rs.
function buildCatalog() {
  const grades = [];
  for (const name of dirsIn(CONTENT)) {
    const m = /^grade-(\d+)$/.exec(name);
    if (!m) continue;
    const subjects = [];
    for (const sub of dirsIn(path.join(CONTENT, name))) {
      const file = path.join(CONTENT, name, sub, 'subject.json');
      if (!fs.existsSync(file)) continue;
      try {
        const s = readJson(file);
        subjects.push({
          id: s.id ?? null, name: s.name ?? null, description: s.description || '',
          topics: (s.topics || []).map((t) => ({ id: t.id ?? null, title: t.title ?? null, term: t.term ?? null })),
          exams: (s.exams || []).map((e) => ({ id: e.id ?? null, title: e.title ?? null, durationMinutes: e.durationMinutes ?? null, totalMarks: e.totalMarks ?? null })),
        });
      } catch (e) { console.error(`Could not read ${file}: ${e.message}`); }
    }
    const key = (v) => String(v.name || '').toLowerCase();
    subjects.sort((a, b) => (key(a) < key(b) ? -1 : key(a) > key(b) ? 1 : 0));
    grades.push({ grade: Number(m[1]), subjects });
  }
  grades.sort((a, b) => a.grade - b.grade);
  const courses = [];
  for (const id of dirsIn(path.join(CONTENT, 'courses')).sort()) {
    const file = path.join(CONTENT, 'courses', id, 'course.json');
    if (!ID.test(id) || !fs.existsSync(file)) continue;
    try {
      const c = readJson(file);
      const units = c.units || [];
      courses.push({ id, name: c.name ?? null, about: c.about ?? null, grade: c.grade ?? null, level: c.level ?? null, units: units.length, lessons: units.reduce((n, u) => n + (u.lessons || []).length, 0) });
    } catch (e) { console.error(`Bad course file ${file}: ${e.message}`); }
  }
  return { grades, courses };
}

// Every word in the content, for telling spelling slips from different answers.
function buildWords() {
  const seen = new Set();
  const add = (text) => {
    for (const m of text.normalize('NFKC').toLowerCase().matchAll(/\p{L}[\p{L}']*/gu)) seen.add(m[0].replace(/'+$/, ''));
  };
  const walk = (v) => {
    if (typeof v === 'string') add(v);
    else if (Array.isArray(v)) v.forEach(walk);
    else if (v && typeof v === 'object') Object.values(v).forEach(walk);
  };
  const files = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const f = path.join(dir, e.name);
      if (e.isDirectory()) files(f);
      else if (e.name.endsWith('.json')) { try { walk(readJson(f)); } catch { /* validated elsewhere */ } }
    }
  };
  files(CONTENT);
  return [...seen];
}

const catalogBuf = Buffer.from(JSON.stringify(buildCatalog()));
const wordsBuf = Buffer.from(JSON.stringify(buildWords()));

// ---------------------------------------------------------------- responses

const SECURITY = {
  'Content-Security-Policy': "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; media-src 'self' blob:; font-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'",
  'X-Content-Type-Options': 'nosniff',
  'Referrer-Policy': 'same-origin',
};
const TYPES = {
  '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8', '.svg': 'image/svg+xml', '.png': 'image/png',
  '.woff2': 'font/woff2', '.woff': 'font/woff', '.ttf': 'font/ttf', '.ogg': 'audio/ogg',
};
const compressible = (type) => /^(text\/|application\/json|image\/svg)/.test(type);

// Files and their compressed copies are prepared once: the content is small.
const prepared = new WeakMap();
function preparedFor(buf) {
  let p = prepared.get(buf);
  if (!p) {
    p = { etag: `"${crypto.createHash('sha1').update(buf).digest('base64url')}"`, gz: null };
    prepared.set(buf, p);
  }
  return p;
}

function send(req, res, status, type, buf, headers = {}) {
  const h = { ...SECURITY, 'Content-Type': type, 'Cache-Control': 'no-cache', Vary: 'Accept-Encoding, Cookie', ...headers };
  const p = preparedFor(buf);
  h.ETag = p.etag;
  if (status === 200 && req.headers['if-none-match'] === p.etag) {
    delete h['Content-Type'];
    res.writeHead(304, h);
    return res.end();
  }
  let body = buf;
  if (buf.length > 1024 && compressible(type) && /\bgzip\b/.test(req.headers['accept-encoding'] || '')) {
    if (!p.gz) p.gz = zlib.gzipSync(buf, { level: 7 });
    body = p.gz;
    h['Content-Encoding'] = 'gzip';
  }
  h['Content-Length'] = body.length;
  res.writeHead(status, h);
  res.end(req.method === 'HEAD' ? undefined : body);
}

// Answers about one learner are never cached and are small, so they skip the file cache.
function sendJson(req, res, status, value, headers = {}) {
  const body = JSON.stringify(value);
  res.writeHead(status, { ...SECURITY, 'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store', ...headers });
  res.end(req.method === 'HEAD' ? undefined : body);
}
const fail = (req, res, status, error, headers) => sendJson(req, res, status, { error }, headers);

const fileCache = new Map();
async function readCached(file, transform) {
  const st = await fsp.stat(file);
  if (!st.isFile()) throw Object.assign(new Error('not a file'), { code: 'ENOENT' });
  const hit = fileCache.get(file);
  if (hit && hit.mtime === st.mtimeMs) return hit.buf;
  let buf = await fsp.readFile(file);
  if (transform) buf = transform(buf);
  fileCache.set(file, { mtime: st.mtimeMs, buf });
  return buf;
}

async function sendAudio(req, res, file) {
  let st;
  try { st = await fsp.stat(file); } catch { return fail(req, res, 404, 'Not found'); }
  res.writeHead(200, { ...SECURITY, 'Content-Type': TYPES['.ogg'], 'Content-Length': st.size, 'Cache-Control': 'public, max-age=604800' });
  if (req.method === 'HEAD') return res.end();
  fs.createReadStream(file).on('error', () => res.destroy()).pipe(res);
}

async function audioList(req, res, folder) {
  let names = [];
  try { names = await fsp.readdir(path.join(AUDIO, folder)); } catch { /* no audio here */ }
  sendJson(req, res, 200, names.filter((n) => n.endsWith('.ogg')).map((n) => n.slice(0, -4)).filter((n) => AUDIO_ID.test(n)).sort());
}

// ---------------------------------------------------------------- progress storage

const ownersDir = path.join(cfg.dataDir, 'owners');
fs.mkdirSync(ownersDir, { recursive: true });
const ownerFile = (id) => path.join(ownersDir, `${id}.json`);
const emptyProgress = () => ({ version: 1, settings: {}, topics: {}, exams: {}, examDrafts: {} });

let usedBytes = 0;
for (const f of fs.readdirSync(ownersDir)) usedBytes += fs.statSync(path.join(ownersDir, f)).size;

async function readOwner(id) {
  for (const file of [ownerFile(id), `${ownerFile(id)}.bak`]) {
    try {
      const r = JSON.parse(await fsp.readFile(file, 'utf8'));
      if (r && typeof r.rev === 'number' && r.progress && typeof r.progress === 'object') return r;
    } catch (e) {
      if (e.code !== 'ENOENT') console.error(`Could not read ${file}: ${e.message}`);
    }
  }
  return null;
}

// Written to a temporary file first and the last version kept, as the desktop app does.
async function writeOwner(id, record) {
  const file = ownerFile(id);
  const text = JSON.stringify(record);
  const tmp = `${file}.${crypto.randomBytes(4).toString('hex')}.tmp`;
  let before = 0;
  try { before = (await fsp.stat(file)).size; } catch { /* a new learner */ }
  await fsp.writeFile(tmp, text);
  if (before) await fsp.copyFile(file, `${file}.bak`).catch(() => {});
  await fsp.rename(tmp, file);
  usedBytes += Buffer.byteLength(text) - before;
}

async function removeOwner(id) {
  for (const f of [ownerFile(id), `${ownerFile(id)}.bak`]) {
    try { const { size } = await fsp.stat(f); await fsp.unlink(f); usedBytes -= size; } catch { /* already gone */ }
  }
}

// One change at a time for each learner.
const chains = new Map();
function serial(id, fn) {
  const next = (chains.get(id) || Promise.resolve()).catch(() => {}).then(fn);
  chains.set(id, next);
  next.finally(() => { if (chains.get(id) === next) chains.delete(id); }).catch(() => {});
  return next;
}

const revOf = (owner, n) => `${owner.slice(2, 10)}.${n}`;
const hasProgress = (p) => ['topics', 'exams', 'examDrafts', 'courses'].some((k) => p && p[k] && Object.keys(p[k]).length > 0);
const isPlain = (v) => !!v && typeof v === 'object' && !Array.isArray(v);

// ---------------------------------------------------------------- limits

const buckets = new Map();
function allow(key, max, windowMs) {
  const now = Date.now();
  const b = buckets.get(key);
  if (!b || now >= b.reset) { buckets.set(key, { n: 1, reset: now + windowMs }); return true; }
  return ++b.n <= max;
}
setInterval(() => { const now = Date.now(); for (const [k, b] of buckets) if (now >= b.reset) buckets.delete(k); }, 60e3).unref();

function clientIp(req) {
  if (cfg.trustProxy) {
    const forwarded = req.headers['cf-connecting-ip'] || String(req.headers['x-forwarded-for'] || '').split(',').pop().trim();
    if (forwarded) return forwarded;
  }
  return req.socket.remoteAddress || 'unknown';
}

// ---------------------------------------------------------------- who is this

const sessionsFile = path.join(cfg.dataDir, 'sessions.json');
const sessions = new Map();
try {
  for (const [k, v] of Object.entries(JSON.parse(fs.readFileSync(sessionsFile, 'utf8')))) if (v.exp > Date.now()) sessions.set(k, v);
} catch { /* first run */ }
function saveSessions() {
  const tmp = `${sessionsFile}.tmp`;
  fs.writeFileSync(tmp, JSON.stringify(Object.fromEntries(sessions)));
  fs.renameSync(tmp, sessionsFile);
}
const hashToken = (t) => crypto.createHash('sha256').update(t).digest('hex');

function cookies(req) {
  const out = {};
  for (const part of String(req.headers.cookie || '').split(';')) {
    const i = part.indexOf('=');
    if (i > 0) out[part.slice(0, i).trim()] = part.slice(i + 1).trim();
  }
  return out;
}

function cookie(req, name, value, maxAge) {
  const secure = req.headers['x-forwarded-proto'] === 'https' || !!req.socket.encrypted;
  return `${name}=${value}; Path=/; HttpOnly; SameSite=Lax; Max-Age=${maxAge}${secure ? '; Secure' : ''}`;
}

// The learner behind a request: their account if signed in, otherwise this browser.
function identify(req) {
  const c = cookies(req);
  const set = [];
  let device = c.fs_device;
  if (!UUID.test(device || '')) {
    device = crypto.randomUUID();
    set.push(cookie(req, 'fs_device', device, DEVICE_COOKIE_S));
  }
  const token = c.fs_session && hashToken(c.fs_session);
  const session = token && sessions.get(token);
  if (session && session.exp > Date.now()) return { owner: session.owner, device: `d-${device}`, email: session.email, token, set };
  return { owner: `d-${device}`, device: `d-${device}`, email: null, token: null, set };
}
const withCookies = (who) => (who.set.length ? { 'Set-Cookie': who.set } : {});

// A request changing something must come from this site's own pages.
function sameOrigin(req) {
  const origin = req.headers.origin;
  if (!origin) return true;
  let host;
  try { host = new URL(origin).host; } catch { return false; }
  if (host === req.headers.host) return true;
  if (cfg.trustProxy && host === req.headers['x-forwarded-host']) return true;
  return !!cfg.publicUrl && host === new URL(cfg.publicUrl).host;
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let size = 0;
    req.on('data', (c) => {
      size += c.length;
      if (size > MAX_BODY) return reject(Object.assign(new Error('too large'), { status: 413 }));
      chunks.push(c);
    });
    req.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')));
    req.on('error', reject);
  });
}

// ---------------------------------------------------------------- signing in with Google

const pendingLogins = new Map();
const b64url = (buf) => Buffer.from(buf).toString('base64url');
const callbackUrl = () => `${cfg.publicUrl}/auth/google/callback`;

function startGoogle(req, res) {
  if (!allow(`auth:${clientIp(req)}`, 30, 60e3)) return fail(req, res, 429, 'Too many attempts. Try again in a minute.');
  const state = b64url(crypto.randomBytes(24));
  const nonce = b64url(crypto.randomBytes(24));
  const verifier = b64url(crypto.randomBytes(48));
  if (pendingLogins.size > 5000) pendingLogins.clear();
  pendingLogins.set(state, { nonce, verifier, exp: Date.now() + 10 * 60e3 });
  const q = new URLSearchParams({
    client_id: cfg.googleId, redirect_uri: callbackUrl(), response_type: 'code', scope: 'openid email',
    state, nonce, code_challenge: b64url(crypto.createHash('sha256').update(verifier).digest()),
    code_challenge_method: 'S256', prompt: 'select_account',
  });
  res.writeHead(302, { ...SECURITY, Location: `${cfg.googleAuthUrl}?${q}`, 'Set-Cookie': cookie(req, 'fs_oauth', state, 600), 'Cache-Control': 'no-store' });
  res.end();
}

function page(req, res, status, message) {
  const html = `<!DOCTYPE html><html lang="en-ZA"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>FreeSheet</title><body style="font-family:system-ui,sans-serif;max-width:32rem;margin:4rem auto;padding:0 1rem"><h1>Sign-in did not finish</h1><p>${message}</p><p><a href="/">Back to FreeSheet</a></p></body></html>`;
  res.writeHead(status, { ...SECURITY, 'Content-Type': TYPES['.html'], 'Cache-Control': 'no-store' });
  res.end(html);
}

async function exchangeCode(code, verifier) {
  const r = await fetch(cfg.googleTokenUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({ code, client_id: cfg.googleId, client_secret: cfg.googleSecret, redirect_uri: callbackUrl(), grant_type: 'authorization_code', code_verifier: verifier }),
    signal: AbortSignal.timeout(15000),
  });
  if (!r.ok) throw new Error(`token endpoint answered ${r.status}`);
  const { id_token: idToken } = await r.json();
  return JSON.parse(Buffer.from(String(idToken).split('.')[1], 'base64url').toString('utf8'));
}

async function finishGoogle(req, res, url) {
  const state = url.searchParams.get('state') || '';
  const pending = pendingLogins.get(state);
  pendingLogins.delete(state);
  const clearOauth = cookie(req, 'fs_oauth', '', 0);
  if (!pending || pending.exp < Date.now() || cookies(req).fs_oauth !== state) return page(req, res, 400, 'That sign-in link has expired. Please try again.');
  const code = url.searchParams.get('code');
  if (!code) return page(req, res, 400, 'Google did not sign you in.');
  let claims;
  try {
    // The token comes straight from Google over HTTPS, so its contents are what matter.
    claims = await exchangeCode(code, pending.verifier);
  } catch (e) {
    console.error(`Google sign-in failed: ${e.message}`);
    return page(req, res, 502, 'Could not reach Google. Please try again.');
  }
  const issuers = ['accounts.google.com', 'https://accounts.google.com'];
  if (!issuers.includes(claims.iss) || claims.aud !== cfg.googleId || !(claims.exp * 1000 > Date.now()) || claims.nonce !== pending.nonce || typeof claims.sub !== 'string' || !claims.sub) {
    console.error('Google sign-in rejected: the token did not check out');
    return page(req, res, 400, 'Google’s answer could not be verified. Please try again.');
  }
  const who = identify(req);
  const owner = `a-${crypto.createHash('sha256').update(claims.sub).digest('hex').slice(0, 40)}`;
  // A new account takes this browser's progress. An account that already has progress keeps
  // it, and this browser's stays where it is rather than being overwritten.
  await serial(owner, () => serial(who.device, async () => {
    const account = await readOwner(owner);
    if (account && hasProgress(account.progress)) return;
    const device = await readOwner(who.device);
    if (!device) return;
    await writeOwner(owner, { rev: (account ? account.rev : 0) + 1, progress: device.progress });
    await removeOwner(who.device);
  }));
  const token = b64url(crypto.randomBytes(32));
  sessions.set(hashToken(token), { owner, email: typeof claims.email === 'string' ? claims.email : '', exp: Date.now() + SESSION_MS });
  saveSessions();
  res.writeHead(302, { ...SECURITY, Location: '/', 'Cache-Control': 'no-store', 'Set-Cookie': [clearOauth, cookie(req, 'fs_session', token, SESSION_MS / 1000), ...who.set] });
  res.end();
}

// ---------------------------------------------------------------- the API

function accountInfo(who) {
  return { signedIn: !!who.email || who.owner.startsWith('a-'), email: who.email, googleAvailable: googleOn };
}

async function apiProgress(req, res) {
  const who = identify(req);
  const headers = withCookies(who);
  if (req.method === 'GET') {
    const r = await readOwner(who.owner);
    return sendJson(req, res, 200, { rev: revOf(who.owner, r ? r.rev : 0), progress: r ? { ...emptyProgress(), ...r.progress } : emptyProgress(), account: accountInfo(who) }, headers);
  }
  if (req.method !== 'PUT') return fail(req, res, 405, 'Method not allowed');
  if (!sameOrigin(req)) return fail(req, res, 403, 'Wrong origin');
  if (!/^application\/json/.test(req.headers['content-type'] || '')) return fail(req, res, 415, 'Send JSON');
  if (!allow(`save:${who.owner}`, 120, 60e3)) return fail(req, res, 429, 'Saving too often', headers);
  let progress;
  try {
    progress = JSON.parse(await readBody(req));
  } catch (e) {
    return fail(req, res, e.status || 400, e.status ? 'Progress is too large' : 'Not valid JSON', { ...headers, Connection: 'close' });
  }
  if (!isPlain(progress) || !isPlain(progress.settings) || !isPlain(progress.topics) || !isPlain(progress.exams)) return fail(req, res, 400, 'Not FreeSheet progress', headers);
  const out = await serial(who.owner, async () => {
    const current = await readOwner(who.owner);
    if (req.headers['x-rev'] !== revOf(who.owner, current ? current.rev : 0)) return { status: 409 };
    if (!current) {
      if (usedBytes > cfg.maxDataBytes) return { status: 507 };
      if (!allow(`new:${clientIp(req)}`, 30, 3600e3)) return { status: 429 };
    }
    const rev = (current ? current.rev : 0) + 1;
    await writeOwner(who.owner, { rev, progress });
    return { status: 200, rev: revOf(who.owner, rev) };
  });
  if (out.status !== 200) return fail(req, res, out.status, { 409: 'Changed elsewhere', 429: 'Too many new learners from one place', 507: 'The server is full' }[out.status], headers);
  sendJson(req, res, 200, { rev: out.rev }, headers);
}

async function apiRev(req, res) {
  const who = identify(req);
  const r = await readOwner(who.owner);
  sendJson(req, res, 200, { rev: revOf(who.owner, r ? r.rev : 0) }, withCookies(who));
}

// Run before the page is drawn, so a saved theme shows from the very first paint.
async function bootScript(req, res) {
  const who = identify(req);
  const r = await readOwner(who.owner);
  const s = (r && r.progress.settings) || {};
  const pick = (k, d) => (typeof s[k] === 'string' && /^[a-z]+$/.test(s[k]) ? s[k] : d);
  const appearance = { theme: pick('theme', 'system'), accent: pick('accent', 'default'), lineSpacing: pick('lineSpacing', 'normal'), readingWidth: pick('readingWidth', 'normal'), textRendering: pick('textRendering', 'smooth') };
  const js = `window.__FREESHEET_PLATFORM__="web";window.__FREESHEET_VERSION__=${JSON.stringify(VERSION)};window.__FREESHEET_APPEARANCE__=${JSON.stringify(appearance)};`;
  res.writeHead(200, { ...SECURITY, 'Content-Type': TYPES['.js'], 'Cache-Control': 'no-store', ...withCookies(who) });
  res.end(js);
}

async function logout(req, res) {
  if (!sameOrigin(req)) return fail(req, res, 403, 'Wrong origin');
  const who = identify(req);
  if (who.token) { sessions.delete(who.token); saveSessions(); }
  sendJson(req, res, 200, { ok: true }, { 'Set-Cookie': cookie(req, 'fs_session', '', 0) });
}

// /api/topic/12/mathematics/algebra and friends. Ids are checked before they touch a path.
async function apiContent(req, res, parts) {
  const [kind, a, b, c, d] = parts;
  const grade = (g) => (/^\d{1,2}$/.test(g) && Number(g) >= 1 && Number(g) <= 12 ? Number(g) : null);
  const json = async (file) => send(req, res, 200, TYPES['.json'], await readCached(file, (buf) => (buf[0] === 0xef && buf[1] === 0xbb && buf[2] === 0xbf ? buf.subarray(3) : buf)));
  if (kind === 'catalog' && parts.length === 1) return send(req, res, 200, TYPES['.json'], catalogBuf);
  if (kind === 'words' && parts.length === 1) return send(req, res, 200, TYPES['.json'], wordsBuf);
  if ((kind === 'topic' || kind === 'exam') && parts.length === 4 && grade(a) && ID.test(b) && ID.test(c)) {
    return json(path.join(CONTENT, `grade-${grade(a)}`, b, kind === 'topic' ? 'topics' : 'exams', `${c}.json`));
  }
  if (kind === 'course' && ID.test(a || '')) {
    if (parts.length === 2) return json(path.join(CONTENT, 'courses', a, 'course.json'));
    if (parts.length === 3 && b === 'audio') return audioList(req, res, path.join('courses', a));
    if (parts.length === 4 && b === 'audio' && AUDIO_ID.test(c)) return sendAudio(req, res, path.join(AUDIO, 'courses', a, `${c}.ogg`));
  }
  if (kind === 'audio' && grade(a) && ID.test(b || '') && ID.test(c || '')) {
    const folder = path.join(`grade-${grade(a)}`, b, c);
    if (parts.length === 4) return audioList(req, res, folder);
    if (parts.length === 5 && AUDIO_ID.test(d)) return sendAudio(req, res, path.join(AUDIO, folder, `${d}.ogg`));
  }
  return fail(req, res, 404, 'Not found');
}

// ---------------------------------------------------------------- pages

const WEB_API = path.join(__dirname, 'web-api.js');

function withWebHead(buf) {
  const html = buf.toString('utf8');
  if (!html.includes('<script src="js/theme.js"></script>')) throw new Error('index.html no longer loads js/theme.js');
  return Buffer.from(html.replace('<script src="js/theme.js"></script>', '<script src="/boot.js"></script>\n  <script src="js/theme.js"></script>').replace('</head>', '  <link rel="stylesheet" href="mobile.css">\n</head>'));
}

async function serveStatic(req, res, pathname) {
  let rel;
  try { rel = decodeURIComponent(pathname); } catch { return fail(req, res, 400, 'Bad address'); }
  if (rel === '/') rel = '/index.html';
  if (rel.includes('\0') || rel.includes('\\') || rel.split('/').some((s) => s.startsWith('.'))) return fail(req, res, 404, 'Not found');
  const file = rel === '/js/api.js' ? WEB_API : path.join(SRC, rel);
  if (file !== WEB_API && !file.startsWith(SRC + path.sep)) return fail(req, res, 404, 'Not found');
  const type = TYPES[path.extname(file).toLowerCase()];
  if (!type) return fail(req, res, 404, 'Not found');
  let buf;
  try {
    buf = await readCached(file, rel === '/index.html' ? withWebHead : undefined);
  } catch (e) {
    if (e.code === 'ENOENT' || e.code === 'EISDIR') return fail(req, res, 404, 'Not found');
    throw e;
  }
  const immutable = /\.(woff2?|ttf)$/.test(file);
  const headers = immutable ? { 'Cache-Control': 'public, max-age=2592000' } : {};
  if (rel === '/index.html') Object.assign(headers, withCookies(identify(req)));
  send(req, res, 200, type, buf, headers);
}

async function route(req, res) {
  const url = new URL(req.url, 'http://localhost');
  const p = url.pathname;
  const reading = req.method === 'GET' || req.method === 'HEAD';
  if (p === '/api/progress') return apiProgress(req, res);
  if (p === '/auth/logout') return req.method === 'POST' ? logout(req, res) : fail(req, res, 405, 'Method not allowed');
  if (!reading) return fail(req, res, 405, 'Method not allowed');
  if (p === '/api/rev') return apiRev(req, res);
  if (p === '/boot.js') return bootScript(req, res);
  if (p === '/auth/google' || p === '/auth/google/callback') {
    if (!googleOn) return fail(req, res, 404, 'Google sign-in is not set up on this server');
    return p === '/auth/google' ? startGoogle(req, res) : finishGoogle(req, res, url);
  }
  if (p.startsWith('/api/')) {
    try {
      return await apiContent(req, res, p.slice(5).split('/'));
    } catch (e) {
      if (e.code === 'ENOENT') return fail(req, res, 404, 'Not found');
      throw e;
    }
  }
  return serveStatic(req, res, p);
}

const server = http.createServer((req, res) => {
  route(req, res).catch((e) => {
    console.error(`${req.method} ${req.url}: ${e.stack || e}`);
    if (!res.headersSent) fail(req, res, 500, 'Something went wrong');
    else res.destroy();
  });
});

setInterval(() => {
  const now = Date.now();
  for (const [k, v] of pendingLogins) if (v.exp < now) pendingLogins.delete(k);
  let changed = false;
  for (const [k, v] of sessions) if (v.exp < now) { sessions.delete(k); changed = true; }
  if (changed) saveSessions();
}, 3600e3).unref();

server.listen(cfg.port, cfg.host, () => {
  const { port } = server.address();
  console.log(`FreeSheet ${VERSION} is running on http://${cfg.host}:${port}`);
  console.log(`Progress is kept in ${cfg.dataDir}`);
  if (googleOn) console.log(`Google sign-in is on. Allowed redirect URI: ${callbackUrl()}`);
  else console.log('Google sign-in is off: set GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET and PUBLIC_URL to turn it on.');
  if (cfg.publicUrl && !cfg.publicUrl.startsWith('https://')) console.warn('Warning: PUBLIC_URL is not https, which Google will refuse.');
});
