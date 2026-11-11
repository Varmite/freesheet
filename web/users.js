#!/usr/bin/env node
'use strict';
// FreeSheet accounts, managed on the laptop (there is no sign-up page):
//
//   ./users.sh list                 who has an account
//   ./users.sh add <name>           make an account (asks for a password)
//   ./users.sh passwd <name>        change a password (signs that person out everywhere)
//   ./users.sh remove <name>        delete an account (their progress is kept in data/users/<name>)
//
// The server notices changes straight away; no restart needed.

const fs = require('fs');
const path = require('path');
const { createAccounts, normalise, validUsername, MIN_PASSWORD } = require('./lib/auth');

// Same .env reading as server.js, just for DATA_DIR.
const envFile = path.join(__dirname, '.env');
if (!process.env.DATA_DIR && fs.existsSync(envFile)) {
  const m = /^\s*DATA_DIR\s*=\s*(.*?)\s*$/m.exec(fs.readFileSync(envFile, 'utf8'));
  if (m && m[1]) process.env.DATA_DIR = m[1].replace(/^(['"])(.*)\1$/, '$2');
}
const dataDir = path.resolve(__dirname, process.env.DATA_DIR || 'data');
const accounts = createAccounts(dataDir);

// Reads a line without showing what is typed. FREESHEET_PASSWORD may give it instead
// (for scripts and tests).
let piped = null; // lines from a pipe, when not typed
function askHidden(prompt) {
  if (process.env.FREESHEET_PASSWORD) return Promise.resolve(process.env.FREESHEET_PASSWORD);
  return new Promise((resolve) => {
    const stdin = process.stdin;
    process.stdout.write(prompt);
    if (!stdin.isTTY) {
      // Piped in: all of it is read once, then handed out a line at a time.
      if (!piped) {
        piped = new Promise((done) => {
          let buf = '';
          stdin.setEncoding('utf8');
          stdin.on('data', (d) => { buf += d; });
          stdin.on('end', () => done(buf.split(/\r?\n/)));
        });
      }
      piped.then((lines) => { process.stdout.write('\n'); resolve(lines.length ? lines.shift() : ''); });
      return;
    }
    let pw = '';
    stdin.setRawMode(true);
    stdin.setEncoding('utf8');
    stdin.resume();
    const onKey = (ch) => {
      for (const c of ch) {
        if (c === '\r' || c === '\n') {
          stdin.setRawMode(false); stdin.pause(); stdin.removeListener('data', onKey);
          process.stdout.write('\n');
          return resolve(pw);
        }
        if (c === '\u0003') { process.stdout.write('\n'); process.exit(130); } // Ctrl+C
        if (c === '\u007f' || c === '\b') { if (pw) { pw = pw.slice(0, -1); process.stdout.write('\b \b'); } continue; }
        pw += c;
        process.stdout.write('*');
      }
    };
    stdin.on('data', onKey);
  });
}

async function newPassword() {
  // Typed at the keyboard: ask again after a mistake. From a script: give up.
  const typed = process.stdin.isTTY && !process.env.FREESHEET_PASSWORD;
  for (;;) {
    const a = await askHidden(`Password (at least ${MIN_PASSWORD} characters): `);
    const b = process.env.FREESHEET_PASSWORD ? a : await askHidden('Type it again: ');
    const problem = a.length < MIN_PASSWORD ? 'Too short.' : a !== b ? "They don't match." : '';
    if (!problem) return a;
    console.log(typed ? `${problem} Try again.` : problem);
    if (!typed) process.exit(1);
  }
}

(async () => {
  const [cmd, rawName] = process.argv.slice(2);
  const name = normalise(rawName);
  const needName = () => {
    if (!rawName) { console.error(`Which account? e.g. ./users.sh ${cmd} thandi`); process.exit(2); }
  };
  switch (cmd) {
    case 'list': {
      const all = accounts.list();
      console.log(all.length ? all.join('\n') : 'No accounts yet. Make one with: ./users.sh add <name>');
      break;
    }
    case 'add': {
      needName();
      if (!validUsername(name)) { console.error('Usernames are 1 to 32 lower-case letters, digits, - or _ (e.g. thandi or sipho-m).'); process.exit(2); }
      if (accounts.exists(name)) { console.error(`${name} already has an account. To change the password: ./users.sh passwd ${name}`); process.exit(1); }
      accounts.set(name, await newPassword());
      console.log(`Made the account "${name}". Sign in at your FreeSheet address with that name and password.`);
      break;
    }
    case 'passwd': {
      needName();
      if (!accounts.exists(name)) { console.error(`There is no account called ${name}. See: ./users.sh list`); process.exit(1); }
      accounts.set(name, await newPassword());
      console.log(`Changed the password for "${name}". They have been signed out on every device.`);
      break;
    }
    case 'remove': {
      needName();
      if (!accounts.remove(name)) { console.error(`There is no account called ${name}.`); process.exit(1); }
      console.log(`Removed "${name}". Their progress is still in ${path.join(dataDir, 'users', name)} (delete that folder to remove it too).`);
      break;
    }
    default:
      console.log('Usage: ./users.sh list | add <name> | passwd <name> | remove <name>');
      process.exit(cmd ? 2 : 0);
  }
})();
