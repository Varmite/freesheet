'use strict';
// Sets KEY=value in a .env file, replacing the existing line or adding one, and keeping
// everything else. Used by server.js and, from the command line, by the shell scripts
// (so values with odd characters don't trip up sed):
//   node lib/envfile.js .env KEY value
const fs = require('fs');

function setEnv(file, key, value) {
  if (!/^[A-Z0-9_]+$/.test(key || '')) throw new Error(`bad key ${key}`);
  const text = fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '';
  const line = `${key}=${String(value == null ? '' : value).replace(/[\r\n]/g, '')}`;
  const re = new RegExp(`^${key}=.*$`, 'm');
  fs.writeFileSync(file, re.test(text) ? text.replace(re, () => line) : `${text.replace(/\n*$/, '\n')}${line}\n`);
}

if (require.main === module) {
  const [file, key, value] = process.argv.slice(2);
  try { setEnv(file, key, value); } catch (e) { console.error(`usage: envfile.js FILE KEY VALUE (${e.message})`); process.exit(2); }
}

module.exports = { setEnv };
