'use strict';
// Accounts: a username and password each, kept on this laptop in DATA_DIR/accounts.json.
// Passwords are never stored, only a salted scrypt hash of them. Accounts are made on the
// laptop with ./users.sh (there is no sign-up page, so strangers can't make one).

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

// Usernames are also folder names (DATA_DIR/users/<name>/), so only these characters.
const USERNAME = /^[a-z0-9][a-z0-9_-]{0,31}$/;
const normalise = (name) => String(name || '').trim().toLowerCase();
const validUsername = (name) => USERNAME.test(name);
const MIN_PASSWORD = 6;

// scrypt with Node's defaults (N=16384, r=8, p=1): about 50 ms per check.
function hash(password, salt) {
  return crypto.scryptSync(String(password).normalize('NFC'), salt, 32).toString('hex');
}

function createAccounts(dataDir) {
  const file = path.join(dataDir, 'accounts.json');
  // Read fresh each time: ./users.sh changes the file while the server runs.
  const load = () => {
    try { return JSON.parse(fs.readFileSync(file, 'utf8')); } catch { return {}; }
  };
  const write = (all) => {
    fs.mkdirSync(dataDir, { recursive: true });
    fs.writeFileSync(`${file}.tmp`, JSON.stringify(all, null, 2), { mode: 0o600 });
    fs.renameSync(`${file}.tmp`, file);
  };

  // A wrong username costs as long as a wrong password, so nobody can tell which it was.
  const dummySalt = crypto.randomBytes(16).toString('hex');

  return {
    file,
    list: () => Object.keys(load()).sort(),
    exists: (name) => Object.prototype.hasOwnProperty.call(load(), normalise(name)),
    // When the password was last set (ms), or null if there is no such account.
    changedAt(name) { const x = load()[normalise(name)]; return x ? Date.parse(x.changed || x.created) || 0 : null; },
    // Returns { username } or null.
    check(name, password) {
      const user = normalise(name);
      const a = load()[user];
      const got = Buffer.from(hash(password, a ? a.salt : dummySalt), 'hex');
      const want = Buffer.from(a ? a.hash : hash('x', dummySalt), 'hex');
      return a && crypto.timingSafeEqual(got, want) ? { username: user } : null;
    },
    set(name, password) {
      const user = normalise(name);
      if (!validUsername(user)) throw new Error('Usernames are 1 to 32 lower-case letters, digits, - or _, starting with a letter or digit.');
      if (String(password).length < MIN_PASSWORD) throw new Error(`Passwords need at least ${MIN_PASSWORD} characters.`);
      const all = load();
      const salt = crypto.randomBytes(16).toString('hex');
      const now = new Date().toISOString();
      // changed: sessions from before a password change are no longer accepted.
      all[user] = { salt, hash: hash(password, salt), created: (all[user] && all[user].created) || now, changed: now };
      write(all);
      return user;
    },
    remove(name) {
      const all = load();
      const user = normalise(name);
      if (!all[user]) return false;
      delete all[user];
      write(all);
      return true;
    },
  };
}

module.exports = { createAccounts, validUsername, normalise, MIN_PASSWORD };
