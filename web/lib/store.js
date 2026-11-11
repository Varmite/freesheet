'use strict';
// Where things are kept on disk (all under DATA_DIR):
//   users/<google sub>/progress.json   that learner's progress (+ .bak, the previous save)
//   users/<google sub>/account.json    their email and name, for reference
//   sessions.json                      who is signed in on which browser
//
// Progress is written like src-tauri/src/progress.rs: to a temporary file first, the old
// file kept as .bak, then renamed into place, so a crash or power cut never loses it.

const fs = require('fs');
const fsp = fs.promises;
const path = require('path');
const crypto = require('crypto');

// The same empty progress the Rust code returns for a new learner (progress::empty).
const emptyProgress = () => ({ version: 1, settings: {}, topics: {}, exams: {}, examDrafts: {} });

// Google's "sub" is a number as a string. Anything else could name another folder.
const validSub = (sub) => typeof sub === 'string' && /^[0-9]{1,64}$/.test(sub);

async function writeSafely(file, text) {
  await fsp.mkdir(path.dirname(file), { recursive: true });
  const tmp = `${file}.tmp`;
  await fsp.writeFile(tmp, text);
  try { await fsp.copyFile(file, `${file}.bak`); } catch (e) { if (e.code !== 'ENOENT') throw e; }
  await fsp.rename(tmp, file);
}

async function readJson(file) {
  return JSON.parse((await fsp.readFile(file, 'utf8')).replace(/^﻿/, ''));
}

function createStore(dataDir) {
  const usersDir = path.join(dataDir, 'users');
  fs.mkdirSync(usersDir, { recursive: true });
  const userDir = (sub) => {
    if (!validSub(sub)) throw new Error('bad user id');
    return path.join(usersDir, sub);
  };

  // One write at a time per learner, in order.
  const queues = new Map();
  function serial(sub, fn) {
    const prev = queues.get(sub) || Promise.resolve();
    const next = prev.then(fn, fn);
    queues.set(sub, next.catch(() => {}));
    return next;
  }

  // { progress, rev }. rev counts saves, so a browser holding old progress can be told
  // that a newer save exists (another phone) instead of overwriting it.
  async function load(sub) {
    const file = path.join(userDir(sub), 'progress.json');
    for (const f of [file, `${file}.bak`]) {
      try {
        const v = await readJson(f);
        if (v && typeof v === 'object' && !Array.isArray(v)) {
          const { _rev, ...progress } = v;
          return { progress: Object.assign(emptyProgress(), progress), rev: Number(_rev) || 0 };
        }
      } catch (e) {
        if (e.code !== 'ENOENT') console.error(`Progress file ${f} unreadable: ${e.message}`);
      }
    }
    return { progress: emptyProgress(), rev: 0 };
  }

  // Saves unless baseRev is given and is not the latest; returns { ok, rev }.
  function save(sub, progress, baseRev) {
    return serial(sub, async () => {
      const cur = await load(sub);
      if (baseRev != null && baseRev !== cur.rev) return { ok: false, rev: cur.rev };
      const rev = cur.rev + 1;
      await writeSafely(path.join(userDir(sub), 'progress.json'), JSON.stringify({ ...progress, _rev: rev }));
      return { ok: true, rev };
    });
  }

  async function saveAccount(sub, info) {
    await writeSafely(path.join(userDir(sub), 'account.json'), JSON.stringify({ ...info, lastSignIn: new Date().toISOString() }, null, 2));
  }

  return { load, save, saveAccount, userDir };
}

// ---------------------------------------------------------------- sessions
// A session is a random token in an HttpOnly cookie. The file stores only an HMAC of
// it (keyed with SESSION_SECRET), so a copy of the data folder can't be used to sign in.

function createSessions(dataDir, secret, maxAgeMs) {
  const file = path.join(dataDir, 'sessions.json');
  const key = (token) => crypto.createHmac('sha256', secret).update(String(token)).digest('hex');
  let sessions = {};
  try { sessions = JSON.parse(fs.readFileSync(file, 'utf8')); } catch { /* none yet */ }

  let writing = Promise.resolve();
  function persist() {
    const now = Date.now();
    for (const [k, s] of Object.entries(sessions)) if (!(s.expires > now)) delete sessions[k];
    const text = JSON.stringify(sessions);
    writing = writing.then(() => writeSafely(file, text)).catch((e) => console.error(`Could not save sessions: ${e.message}`));
    return writing;
  }

  return {
    async create(user) {
      const token = crypto.randomBytes(32).toString('base64url');
      sessions[key(token)] = { sub: user.sub, email: user.email, name: user.name || '', expires: Date.now() + maxAgeMs };
      await persist();
      return token;
    },
    get(token) {
      if (!token) return null;
      const s = sessions[key(token)];
      return s && s.expires > Date.now() ? s : null;
    },
    async remove(token) {
      if (token && sessions[key(token)]) { delete sessions[key(token)]; await persist(); }
    },
    flush: () => writing,
  };
}

module.exports = { createStore, createSessions, emptyProgress, validSub };
