'use strict';
// Phone preview / smoke test: serves www/ like the Android app does and opens it in a
// phone-sized, touch-emulated window, clicks through the main screens and saves PNGs.
// Run from the repo root (uses the desktop edition's Electron):
//   npx electron "Android Edition/scripts/phone-preview.js" <outDir> [grade] [subjectId]
const { app, BrowserWindow, protocol, net } = require('electron');
const { pathToFileURL } = require('url');
const path = require('path');
const fs = require('fs');
const os = require('os');

const args = process.argv.slice(2).filter((a) => !a.startsWith('--') && !a.endsWith('phone-preview.js') && a !== '.');
const outDir = path.resolve(args[0] || path.join(os.tmpdir(), 'freesheet-phone'));
const grade = Number(args[1] || 10);
const subjectId = args[2] || 'mathematics';
const WWW = path.join(__dirname, '..', 'www');
fs.mkdirSync(outDir, { recursive: true });
app.setPath('userData', fs.mkdtempSync(path.join(os.tmpdir(), 'freesheet-phone-profile-')));

const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.ogg': 'audio/ogg', '.woff2': 'font/woff2', '.woff': 'font/woff', '.ttf': 'font/ttf', '.svg': 'image/svg+xml' };
// The app is served from a custom app:// scheme (as Capacitor serves https://localhost).
protocol.registerSchemesAsPrivileged([{ scheme: 'app', privileges: { standard: true, secure: true, supportFetchAPI: true, stream: true } }]);
function serve() {
  protocol.handle('app', async (req) => {
    const rel = decodeURIComponent(new URL(req.url).pathname).replace(/^\/+/, '') || 'index.html';
    const file = path.join(WWW, rel);
    if (!file.startsWith(WWW) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) return new Response('', { status: 404 });
    const r = await net.fetch(pathToFileURL(file).href);
    return new Response(r.body, { headers: { 'Content-Type': TYPES[path.extname(file)] || 'application/octet-stream' } });
  });
}

const LOG = path.join(outDir, 'log.txt');
const _log = console.log; console.log = (...a) => { _log(...a); fs.appendFileSync(LOG, a.join(' ') + '\n'); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const errors = [];
let win;
const js = (code) => win.webContents.executeJavaScript(code);
async function shot(name) {
  await sleep(600);
  const img = await win.webContents.capturePage();
  fs.writeFileSync(path.join(outDir, `${name}.png`), img.toPNG());
  console.log('saved', name);
}
const click = (sel) => js(`(() => { const el = document.querySelector(${JSON.stringify(sel)}); if (!el) throw new Error('missing ' + ${JSON.stringify(sel)}); el.click(); return true; })()`);
const overflow = () => js(`document.documentElement.scrollWidth > window.innerWidth ? 'HORIZONTAL OVERFLOW ' + document.documentElement.scrollWidth : 'ok'`);

async function run() {
  serve();
  win = new BrowserWindow({ width: 390, height: 844, useContentSize: true, show: true, webPreferences: { sandbox: true, contextIsolation: true } });
  win.webContents.on('console-message', (ev) => { if ((ev.level === 'error' || ev.level === 3) && !/ResizeObserver loop/.test(ev.message)) errors.push(ev.message); });
  const base = 'app://freesheet/index.html';
  await win.loadURL(base);
  // A phone: touch screen, no hover, 2x pixels.
  const dbg = win.webContents.debugger;
  dbg.attach('1.3');
  await dbg.sendCommand('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 2, mobile: true });
  await dbg.sendCommand('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 5 });
  // Seed progress with a buddy so the "meet your buddy" sheet does not cover every screen.
  await js(`localStorage.setItem('freesheet-progress', JSON.stringify({ version: 1, settings: { grade: ${grade}, buddy: { kind: 'fox', color: 'coral', accessory: 'glasses', name: 'Pip', on: true } }, topics: {}, exams: {}, examDrafts: {} })); true`);
  await win.loadURL(base);
  for (let i = 0; i < 50 && !(await js(`document.documentElement.hasAttribute('data-warmed-up')`)); i++) await sleep(100);
  await sleep(800);
  await shot('01-study-home'); console.log(' overflow:', await overflow());
  await click(`[data-subject="${subjectId}"]`);
  await shot('02-subject'); console.log(' overflow:', await overflow());
  await click('[data-topic]');
  await sleep(600);
  await shot('03-notes'); console.log(' overflow:', await overflow());
  const hasAudio = await js(`!document.querySelector('#reader-bar').hidden`);
  console.log('read-aloud available:', hasAudio);
  if (hasAudio) {
    await click('#read-btn'); await sleep(2500);
    console.log('read-aloud:', await js(`document.querySelector('#read-status').textContent`));
    await shot('03a-reading');
    await click('#read-stop');
  }
  await click('.seg[data-mode="questions"]');
  await sleep(400);
  await js(`document.querySelectorAll('.q-card').forEach((c) => { const o = c.querySelectorAll('.opt'); if (o.length) o[Math.min(1, o.length - 1)].click(); }); document.querySelectorAll('.check-btn').forEach((b) => b.click()); true`);
  await shot('04-questions'); console.log(' overflow:', await overflow());
  await click('.tab[data-tab="exams"]');
  await click(`[data-subject="${subjectId}"]`);
  await click('[data-exam]');
  await sleep(500);
  await shot('05-exam-intro');
  await click('#start'); await sleep(500);
  await shot('06-exam-run'); console.log(' overflow:', await overflow());
  await click('#submit'); await sleep(400);
  await shot('07-dialog');
  await js(`(() => { const b = document.querySelector('[data-r="0"]'); if (b) b.click(); return true; })()`);
  await click('.tab[data-tab="progress"]');
  await shot('08-progress'); console.log(' overflow:', await overflow());
  await click('#settings-btn');
  await shot('09-settings'); console.log(' overflow:', await overflow());
  await click('#settings-back');
  await click('.tab[data-tab="study"]'); await sleep(300);
  await click('.tab[data-tab="study"]'); await sleep(500);
  const course = await js(`(() => { const c = document.querySelector('[data-course]'); if (c) { c.click(); return true; } return false; })()`);
  if (course) {
    await shot('10-course'); console.log(' overflow:', await overflow());
    const lesson = await js(`(() => { const l = document.querySelector('[data-lesson]'); if (l) { l.click(); return true; } return false; })()`);
    if (lesson) { await sleep(800); await shot('11-lesson'); console.log(' overflow:', await overflow()); }
  }
  console.log('saved progress bytes:', await js(`(localStorage.getItem('freesheet-progress') || '').length`));
  console.log('renderer errors:', errors.length ? errors.join('\n') : 'none');
  app.exit(errors.length ? 2 : 0);
}

app.whenReady().then(() => run().catch((err) => { console.error(err); app.exit(1); }));
