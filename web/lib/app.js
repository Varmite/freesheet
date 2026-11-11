'use strict';
// The web server's routes. server.js reads the settings and starts it; the tests start it
// directly with their own settings.
//
//   /login                  the sign-in page (public/login.html)
//   POST /auth/login        username and password in, session cookie out
//   POST /auth/logout       ends the session
//   /                       the app (src/index.html), signed-in only
//   /js/api.js              public/api.js instead of the desktop bridge
//   /boot.js                theme etc. before first paint, and the version
//   /api/...                content, audio and progress, signed-in only
//   everything else         files from src/ (scripts, styles, fonts)

const express = require('express');
const fs = require('fs');
const path = require('path');
const { createContent, validAudioId } = require('./content');
const { createStore, createSessions } = require('./store');
const { createAccounts } = require('./auth');

const SESSION_DAYS = 60;
const COOKIE = 'fs_session';

function createApp(cfg) {
  const ROOT = cfg.repoRoot;
  const SRC = path.join(ROOT, 'src');
  const PUBLIC = path.join(__dirname, '..', 'public');
  const VERSION = JSON.parse(fs.readFileSync(path.join(ROOT, 'package.json'), 'utf8')).version;

  const content = createContent(ROOT);
  const store = createStore(cfg.dataDir);
  const sessions = createSessions(cfg.dataDir, cfg.sessionSecret, SESSION_DAYS * 86400e3);
  const accounts = createAccounts(cfg.dataDir);

  // When someone last had FreeSheet open (for the updater: update only when idle).
  let lastSeen = 0;

  const app = express();
  app.disable('x-powered-by');
  if (cfg.trustProxy) app.set('trust proxy', 1);

  // ---------------------------------------------------------------- sessions

  function readCookie(req, name) {
    for (const part of String(req.headers.cookie || '').split(';')) {
      const i = part.indexOf('=');
      if (i > 0 && part.slice(0, i).trim() === name) return decodeURIComponent(part.slice(i + 1).trim());
    }
    return null;
  }
  function setCookie(res, value, maxAgeS) {
    const secure = cfg.secureCookies ? '; Secure' : '';
    res.append('Set-Cookie', `${COOKIE}=${encodeURIComponent(value)}; Path=/; Max-Age=${maxAgeS}; HttpOnly; SameSite=Lax${secure}`);
  }
  // req.user = { username } when signed in. A session ends if its account was removed
  // or its password changed since (./users.sh works while the server runs).
  app.use((req, res, next) => {
    req.token = readCookie(req, COOKIE);
    const s = sessions.get(req.token);
    const changed = s ? accounts.changedAt(s.username) : null;
    req.user = s && changed !== null && (s.created || 0) >= changed ? { username: s.username } : null;
    next();
  });
  const signedIn = (req, res, next) => {
    if (!req.user) return res.status(401).json({ error: 'Please sign in.' });
    lastSeen = Date.now();
    next();
  };

  // ---------------------------------------------------------------- sign-in

  app.get('/login', (req, res) => {
    if (req.user) return res.redirect('/');
    res.set('Cache-Control', 'no-store');
    res.sendFile(path.join(PUBLIC, 'login.html'));
  });
  app.post('/auth/login', express.json({ limit: '10kb' }), async (req, res) => {
    const { username, password } = req.body || {};
    if (typeof username !== 'string' || typeof password !== 'string' || !username || !password || password.length > 200) {
      return res.status(400).json({ error: 'Type your username and password.' });
    }
    const user = accounts.check(username, password);
    if (!user) {
      // A short pause makes guessing passwords slow.
      await new Promise((r) => setTimeout(r, 800));
      return res.status(401).json({ error: 'Wrong username or password.' });
    }
    setCookie(res, await sessions.create(user), SESSION_DAYS * 86400);
    res.json({ ok: true });
  });

  app.post('/auth/logout', async (req, res) => {
    await sessions.remove(req.token);
    setCookie(res, '', 0);
    res.json({ ok: true });
  });

  // ---------------------------------------------------------------- the app page

  // index.html with the web additions: boot.js before the theme script, the phone
  // layout on small or touch screens, and the home-screen manifest.
  const indexHtml = (() => {
    const html = fs.readFileSync(path.join(SRC, 'index.html'), 'utf8');
    const themeTag = '<script src="js/theme.js"></script>';
    if (!html.includes(themeTag)) throw new Error('src/index.html no longer loads js/theme.js; update web/lib/app.js');
    return html
      .replace(themeTag, `<script src="/boot.js"></script>\n  ${themeTag}`)
      .replace('</head>', [
        '  <link rel="stylesheet" href="mobile.css" media="(max-width: 720px), (hover: none) and (pointer: coarse)">',
        '  <link rel="manifest" href="/manifest.webmanifest">',
        '  <meta name="theme-color" content="#1b1c1f">',
        '  <meta name="apple-mobile-web-app-capable" content="yes">',
        '  <link rel="apple-touch-icon" href="/icon-192.png">',
        '</head>'].join('\n'));
  })();
  const sendIndex = (req, res) => {
    if (!req.user) return res.redirect('/login');
    lastSeen = Date.now();
    res.set('Cache-Control', 'no-store').type('html').send(indexHtml);
  };
  app.get(['/', '/index.html'], sendIndex);

  // Runs before the page is drawn, so the saved theme shows from the first paint.
  app.get('/boot.js', async (req, res) => {
    let s = {};
    if (req.user) s = (await store.load(req.user.username)).progress.settings || {};
    const pick = (k, d) => (typeof s[k] === 'string' && /^[a-z]+$/.test(s[k]) ? s[k] : d);
    const appearance = { theme: pick('theme', 'system'), accent: pick('accent', 'default'), lineSpacing: pick('lineSpacing', 'normal'), readingWidth: pick('readingWidth', 'normal'), textRendering: pick('textRendering', 'smooth') };
    res.set('Cache-Control', 'no-store').type('js').send(
      `window.__FREESHEET_PLATFORM__="web";window.__FREESHEET_VERSION__=${JSON.stringify(VERSION)};` +
      `window.__FREESHEET_APPEARANCE__=${JSON.stringify(appearance)};` +
      `window.__FREESHEET_ACCOUNT__=${JSON.stringify(req.user ? { username: req.user.username } : null)};`);
  });

  app.get('/js/api.js', (req, res) => res.set('Cache-Control', 'no-cache').sendFile(path.join(PUBLIC, 'api.js')));
  for (const f of ['manifest.webmanifest', 'icon-192.png', 'icon-512.png', 'login.css']) {
    app.get(`/${f}`, (req, res) => res.sendFile(path.join(PUBLIC, f)));
  }

  // ---------------------------------------------------------------- content and audio

  const api = express.Router();
  api.use(signedIn);
  const notFound = (res) => res.status(404).json({ error: 'Not found' });
  const jsonOr404 = (res, v) => (v == null ? notFound(res) : res.json(v));

  api.get('/catalog', (req, res) => res.json(content.catalog));
  api.get('/words', (req, res) => res.json(content.words));
  api.get('/ping', (req, res) => res.json({ ok: true })); // "someone is here" (see updater)
  api.get('/topic/:g/:s/:t', (req, res) => jsonOr404(res, content.item(req.params.g, req.params.s, 'topics', req.params.t)));
  api.get('/exam/:g/:s/:x', (req, res) => jsonOr404(res, content.item(req.params.g, req.params.s, 'exams', req.params.x)));
  api.get('/course/:c', (req, res) => jsonOr404(res, content.course(req.params.c)));
  api.get('/course/:c/audio', (req, res) => jsonOr404(res, content.audioList(content.courseAudioFolder(req.params.c))));
  api.get('/audio/:g/:s/:t', (req, res) => jsonOr404(res, content.audioList(content.topicAudioFolder(req.params.g, req.params.s, req.params.t))));

  // Audio files support Range requests (needed by iPhone Safari to play and seek).
  function sendAudio(res, file) {
    if (!file) return notFound(res);
    res.sendFile(file, { acceptRanges: true, maxAge: '7d', headers: { 'Content-Type': 'audio/ogg' } });
  }
  api.get('/course/:c/audio/:a', (req, res) => sendAudio(res, validAudioId(req.params.a) && content.audioFile(content.courseAudioFolder(req.params.c), req.params.a)));
  api.get('/audio/:g/:s/:t/:a', (req, res) => sendAudio(res, validAudioId(req.params.a) && content.audioFile(content.topicAudioFolder(req.params.g, req.params.s, req.params.t), req.params.a)));

  // ---------------------------------------------------------------- progress

  api.get('/progress', async (req, res) => {
    const { progress, rev } = await store.load(req.user.username);
    res.set('Cache-Control', 'no-store').json({ progress, rev, account: { username: req.user.username } });
  });
  api.put('/progress', express.json({ limit: '2mb' }), async (req, res) => {
    const data = req.body;
    if (!data || typeof data !== 'object' || Array.isArray(data)) return res.status(400).json({ error: 'Progress must be a JSON object.' });
    const base = req.get('X-Rev');
    const r = await store.save(req.user.username, data, base == null || base === '' ? null : Number(base));
    if (!r.ok) return res.status(409).json({ error: 'A newer save exists.', rev: r.rev });
    res.json({ ok: true, rev: r.rev });
  });

  app.use('/api', api);
  app.use('/api', (req, res) => notFound(res));

  // ---------------------------------------------------------------- the app's files

  // Everything else comes from src/. express.static refuses ../ (also encoded) and dot files.
  app.use(express.static(SRC, { index: false, dotfiles: 'deny', fallthrough: true, maxAge: 0 }));
  app.use((req, res) => res.status(404).type('text').send('Not found'));

  // Errors: bodies too large, bad JSON, anything unexpected.
  // eslint-disable-next-line no-unused-vars
  app.use((err, req, res, next) => {
    const status = err.status || err.statusCode || 500;
    if (status >= 500) console.error(err);
    res.status(status).json({ error: status === 413 ? 'Too large.' : status < 500 ? 'Bad request.' : 'Server error.' });
  });

  return { app, lastSeen: () => lastSeen, sessions, accounts };
}

module.exports = { createApp };
