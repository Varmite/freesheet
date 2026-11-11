'use strict';
// Server tests: run with `npm test` in web/. They start the app in this process with a
// temporary data folder and the TEST_MODE sign-in stand-in (lib/auth.js).

const { test, before, after } = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const http = require('http');
const { createApp } = require('../lib/app');
const { signTestToken } = require('../lib/auth');
const { shouldUpdate } = require('../lib/updater');

const ROOT = path.join(__dirname, '..', '..');
const CLIENT = 'test-client.apps.googleusercontent.com';
const SECRET = 'unit-test-secret';
const dataDir = fs.mkdtempSync(path.join(os.tmpdir(), 'freesheet-web-'));

function makeServer(extra = {}) {
  const cfg = {
    repoRoot: ROOT, dataDir, googleClientId: CLIENT, allowedEmails: [], sessionSecret: 'session-secret',
    secureCookies: false, testMode: true, testSecret: SECRET, ...extra,
  };
  const { app } = createApp(cfg);
  const server = http.createServer(app);
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve({ server, base: `http://127.0.0.1:${server.address().port}` })));
}

let srv;
before(async () => { srv = await makeServer(); });
after(() => { srv.server.close(); fs.rmSync(dataDir, { recursive: true, force: true }); });

const now = () => Math.floor(Date.now() / 1000);
const claims = (over = {}) => ({ iss: 'https://accounts.google.com', aud: CLIENT, sub: '1001', email: 'a@example.com', email_verified: true, name: 'A', exp: now() + 600, ...over });

async function signIn(base, over, secret = SECRET) {
  const r = await fetch(`${base}/auth/google`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ credential: signTestToken(claims(over), secret) }) });
  const set = r.headers.get('set-cookie') || '';
  return { status: r.status, body: await r.json(), cookie: (/fs_session=[^;]*/.exec(set) || [''])[0] };
}
const get = (base, p, cookie) => fetch(base + p, { headers: cookie ? { cookie } : {}, redirect: 'manual' });

test('signed-out visitors are sent to the login page and kept out of the API', async () => {
  const home = await get(srv.base, '/');
  assert.strictEqual(home.status, 302);
  assert.strictEqual(home.headers.get('location'), '/login');
  assert.strictEqual((await get(srv.base, '/login')).status, 200);
  for (const p of ['/api/progress', '/api/catalog', '/api/topic/10/mathematics/exponents']) {
    assert.strictEqual((await get(srv.base, p)).status, 401, p);
  }
  const put = await fetch(`${srv.base}/api/progress`, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: '{}' });
  assert.strictEqual(put.status, 401);
});

test('the session cookie is HttpOnly, SameSite=Lax and lasts about 60 days', async () => {
  const r = await fetch(`${srv.base}/auth/google`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ credential: signTestToken(claims(), SECRET) }) });
  const c = r.headers.get('set-cookie');
  assert.match(c, /HttpOnly/);
  assert.match(c, /SameSite=Lax/);
  assert.match(c, /Max-Age=5184000/);
});

test('forged, expired, wrong-audience and unverified tokens are refused', async () => {
  assert.strictEqual((await signIn(srv.base, {}, 'not-the-secret')).status, 401, 'forged signature');
  assert.strictEqual((await signIn(srv.base, { exp: now() - 10 })).status, 401, 'expired');
  assert.strictEqual((await signIn(srv.base, { aud: 'someone-else' })).status, 401, 'wrong audience');
  assert.strictEqual((await signIn(srv.base, { iss: 'https://evil.example' })).status, 401, 'wrong issuer');
  assert.strictEqual((await signIn(srv.base, { email_verified: false })).status, 401, 'unverified email');
  // A token with its payload swapped for another person's keeps the old signature.
  const good = signTestToken(claims(), SECRET).split('.');
  const swapped = [good[0], Buffer.from(JSON.stringify(claims({ sub: '9999' }))).toString('base64url'), good[2]].join('.');
  const r = await fetch(`${srv.base}/auth/google`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ credential: swapped }) });
  assert.strictEqual(r.status, 401, 'tampered payload');
  // A made-up cookie is not a session.
  assert.strictEqual((await get(srv.base, '/api/progress', 'fs_session=made-up')).status, 401);
});

test('without TEST_MODE, Google itself checks the token (a fake one fails)', async () => {
  const real = await makeServer({ testMode: false });
  try {
    const fake = signTestToken(claims(), SECRET);
    const r = await fetch(`${real.base}/auth/google`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ credential: fake }) });
    assert.strictEqual(r.status, 401);
    assert.strictEqual((await get(real.base, '/test/token')).status, 404, 'no test token route outside test mode');
  } finally { real.server.close(); }
});

test('ALLOWED_EMAILS keeps everyone else out', async () => {
  const strict = await makeServer({ allowedEmails: ['me@example.com', 'sister@example.com'] });
  try {
    assert.strictEqual((await signIn(strict.base, { email: 'stranger@example.com', sub: '5' })).status, 401);
    assert.strictEqual((await signIn(strict.base, { email: 'Sister@Example.com', sub: '6' })).status, 200);
  } finally { strict.server.close(); }
});

test('two learners have separate progress, and saves survive', async () => {
  const a = await signIn(srv.base, { sub: '111', email: 'a@example.com' });
  const b = await signIn(srv.base, { sub: '222', email: 'b@example.com' });
  const fresh = await (await get(srv.base, '/api/progress', a.cookie)).json();
  assert.deepStrictEqual(fresh.progress, { version: 1, settings: {}, topics: {}, exams: {}, examDrafts: {} });
  const put = (cookie, rev, data) => fetch(`${srv.base}/api/progress`, { method: 'PUT', headers: { 'Content-Type': 'application/json', 'X-Rev': String(rev), cookie }, body: JSON.stringify(data) });
  assert.strictEqual((await put(a.cookie, 0, { version: 1, settings: { grade: 10 }, topics: { 'a/b/c': { attempts: 1 } }, exams: {}, examDrafts: {} })).status, 200);
  assert.strictEqual((await put(b.cookie, 0, { version: 1, settings: { grade: 8 }, topics: {}, exams: {}, examDrafts: {} })).status, 200);
  const pa = await (await get(srv.base, '/api/progress', a.cookie)).json();
  const pb = await (await get(srv.base, '/api/progress', b.cookie)).json();
  assert.strictEqual(pa.progress.settings.grade, 10);
  assert.strictEqual(pb.progress.settings.grade, 8);
  assert.deepStrictEqual(pb.progress.topics, {});
  // Stored by Google account id, with a backup of the previous save.
  assert.ok(fs.existsSync(path.join(dataDir, 'users', '111', 'progress.json')));
  assert.ok(fs.existsSync(path.join(dataDir, 'users', '111', 'account.json')));
  assert.strictEqual((await put(a.cookie, pa.rev, { ...pa.progress, settings: { grade: 11 } })).status, 200);
  assert.ok(fs.existsSync(path.join(dataDir, 'users', '111', 'progress.json.bak')));
  // An old page (stale rev) can't overwrite a newer save.
  assert.strictEqual((await put(a.cookie, pa.rev, { ...pa.progress, settings: { grade: 9 } })).status, 409);
  // Sessions are kept on disk, so a restarted server still knows the learner.
  const again = await makeServer();
  try {
    const r = await get(again.base, '/api/progress', a.cookie);
    assert.strictEqual(r.status, 200);
    assert.strictEqual((await r.json()).progress.settings.grade, 11);
  } finally { again.server.close(); }
  // Signing out ends the session.
  await fetch(`${srv.base}/auth/logout`, { method: 'POST', headers: { cookie: a.cookie } });
  assert.strictEqual((await get(srv.base, '/api/progress', a.cookie)).status, 401);
});

test('progress must be a JSON object and at most 2 MB', async () => {
  const { cookie } = await signIn(srv.base, { sub: '333' });
  const put = (body) => fetch(`${srv.base}/api/progress`, { method: 'PUT', headers: { 'Content-Type': 'application/json', cookie }, body });
  assert.strictEqual((await put('[1,2]')).status, 400);
  assert.strictEqual((await put('{oops')).status, 400);
  assert.strictEqual((await put(JSON.stringify({ topics: {}, exams: {}, pad: 'x'.repeat(2.2 * 1048576) }))).status, 413);
});

test('catalog, every topic and every exam load as JSON', async () => {
  const { cookie } = await signIn(srv.base, { sub: '444' });
  const json = async (p) => { const r = await get(srv.base, p, cookie); assert.strictEqual(r.status, 200, p); return r.json(); };
  const cat = await json('/api/catalog');
  assert.ok(cat.grades.length >= 5);
  let n = 0;
  for (const g of cat.grades) {
    for (const s of g.subjects) {
      assert.ok(s.id && s.name && Array.isArray(s.topics) && Array.isArray(s.exams));
      for (const t of s.topics) { const v = await json(`/api/topic/${g.grade}/${s.id}/${t.id}`); assert.strictEqual(v.id, t.id); n++; }
      for (const x of s.exams) { const v = await json(`/api/exam/${g.grade}/${s.id}/${x.id}`); assert.strictEqual(v.id, x.id); n++; }
      assert.ok(Array.isArray(await json(`/api/audio/${g.grade}/${s.id}/${s.topics[0].id}`)));
    }
  }
  for (const c of cat.courses) { assert.ok((await json(`/api/course/${c.id}`)).units); assert.ok(Array.isArray(await json(`/api/course/${c.id}/audio`))); }
  assert.ok(n > 300, `${n} topics and exams`);
  const words = await json('/api/words');
  assert.ok(words.length > 1000 && words.includes('photosynthesis'));
});

test('audio is served with range requests', async () => {
  const { cookie } = await signIn(srv.base, { sub: '555' });
  const ids = await (await get(srv.base, '/api/audio/9/technology/preservation-of-metals', cookie)).json();
  assert.ok(ids.length > 0);
  const url = `/api/audio/9/technology/preservation-of-metals/${ids[0]}`;
  const r = await fetch(srv.base + url, { headers: { cookie, Range: 'bytes=0-99' } });
  assert.strictEqual(r.status, 206);
  assert.match(r.headers.get('content-range'), /^bytes 0-99\/\d+$/);
  assert.strictEqual(r.headers.get('content-type'), 'audio/ogg');
  assert.strictEqual((await r.arrayBuffer()).byteLength, 100);
  const whole = await get(srv.base, url, cookie);
  assert.strictEqual(whole.status, 200);
  assert.strictEqual(whole.headers.get('accept-ranges'), 'bytes');
});

test('path traversal is refused, plain and encoded', async () => {
  const { cookie } = await signIn(srv.base, { sub: '666' });
  const bad = [
    '/api/topic/10/..%2F..%2F..%2Fweb/package',
    '/api/topic/10/%2e%2e/%2e%2e',
    '/api/topic/10/mathematics/..%2F..%2Fsubject',
    '/api/topic/10/mathematics/..%252F..%252Fsubject',
    '/api/topic/13/mathematics/exponents',
    '/api/topic/1e1/mathematics/exponents',
    '/api/exam/10/mathematics/%2e%2e%5c%2e%2e%5cpackage',
    '/api/course/..%2F..%2Fpackage',
    '/api/audio/9/technology/preservation-of-metals/..%2F..%2F..%2F..%2Fpackage',
    '/api/audio/9/technology/%2e%2e/%2e%2e',
    '/api/audio/9/technology/preservation-of-metals/ABCDEF12',
    '/api/course/afrikaans-gr9/audio/..%2f..%2fpackage',
  ];
  for (const p of bad) {
    const r = await get(srv.base, p, cookie);
    assert.ok(r.status === 404 || r.status === 400, `${p} gave ${r.status}`);
    const text = await r.text();
    assert.ok(!text.includes('"dependencies"') && !text.includes('"devDependencies"'), `${p} leaked a file`);
  }
  // The static files from src/ can't be escaped either.
  for (const p of ['/..%2fpackage.json', '/%2e%2e/package.json', '/js/..%2f..%2fpackage.json', '/js/%2e%2e%2f%2e%2e%2fweb%2f.env', '/.git/config', '/..\\package.json']) {
    const r = await get(srv.base, p, cookie);
    assert.ok(r.status >= 400, `${p} gave ${r.status}`);
    assert.ok(!(await r.text()).includes('"dependencies"'), `${p} leaked a file`);
  }
});

test('the app page is the web version: web api.js, boot script, phone layout', async () => {
  const { cookie } = await signIn(srv.base, { sub: '777', email: 'page@example.com' });
  const html = await (await get(srv.base, '/', cookie)).text();
  assert.match(html, /<script src="\/boot.js"><\/script>/);
  assert.match(html, /mobile\.css" media=/);
  assert.match(html, /manifest\.webmanifest/);
  const api = await (await get(srv.base, '/js/api.js', cookie)).text();
  assert.match(api, /web edition/);
  assert.ok(!api.includes('__TAURI__'), 'the web api.js must not use Tauri');
  const boot = await (await get(srv.base, '/boot.js', cookie)).text();
  assert.match(boot, /__FREESHEET_PLATFORM__="web"/);
  assert.match(boot, /page@example\.com/);
  assert.strictEqual((await get(srv.base, '/styles.css')).status, 200);
});

test('updates wait until nobody has used FreeSheet for a while', () => {
  const idleMs = 15 * 60e3;
  const t = 1_000_000_000;
  assert.strictEqual(shouldUpdate({ behind: false, lastSeen: 0, now: t, idleMs }), false);
  assert.strictEqual(shouldUpdate({ behind: true, lastSeen: t - 60e3, now: t, idleMs }), false);
  assert.strictEqual(shouldUpdate({ behind: true, lastSeen: t - idleMs, now: t, idleMs }), true);
});
