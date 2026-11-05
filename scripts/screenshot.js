'use strict';
// UI smoke test: launches the real app with a throw-away profile, clicks through
// the main screens and saves PNG screenshots.
// Usage: npx electron scripts/screenshot.js <outDir> [grade] [subjectId]
const { app, BrowserWindow } = require('electron');
const path = require('path');
const fs = require('fs');
const os = require('os');

const args = process.argv.slice(2).filter((a) => !a.startsWith('--') && !a.endsWith('screenshot.js') && a !== '.');
const outDir = path.resolve(args[0] || path.join(os.tmpdir(), 'freesheet-shots'));
const grade = Number(args[1] || 10);
const subjectId = args[2] || 'mathematics';
fs.mkdirSync(outDir, { recursive: true });
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'freesheet-profile-'));
app.setPath('userData', profile);
// A study buddy is already set up, so the "meet your buddy" sheet doesn't cover the screens.
const buddy = { kind: 'fox', color: 'coral', accessory: 'glasses', name: 'Pip', on: true };
fs.writeFileSync(path.join(profile, 'progress.json'), JSON.stringify({ version: 1, settings: { grade, buddy }, topics: {}, exams: {}, examDrafts: {} }));

const errors = [];
app.on('browser-window-created', (_e, win) => {
  win.webContents.on('console-message', (ev) => {
    const level = ev.level !== undefined ? ev.level : ev;
    if (level === 'error' || level === 3) errors.push(ev.message);
  });
  win.webContents.once('did-finish-load', () => run(win).catch((err) => { console.error(err); app.exit(1); }));
});

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function js(win, code) { return win.webContents.executeJavaScript(code); }
async function shot(win, name) {
  await sleep(500);
  const img = await win.webContents.capturePage();
  fs.writeFileSync(path.join(outDir, `${name}.png`), img.toPNG());
  console.log('saved', name);
}
const click = (sel) => `(() => { const el = document.querySelector(${JSON.stringify(sel)}); if (!el) throw new Error('missing ' + ${JSON.stringify(sel)}); el.click(); return true; })()`;

async function run(win) {
  win.setSize(1200, 900);
  await sleep(1200);
  // Let the app's one-off transition warm-up finish first: it holds input while it runs.
  for (let i = 0; i < 50 && !(await js(win, `document.documentElement.hasAttribute('data-warmed-up')`)); i++) await sleep(100);
  await shot(win, '01-study-home');
  console.log('streak before:', await js(win, `document.querySelector('#streak-btn').className + ' ' + document.querySelector('#streak-btn').textContent`));
  await js(win, click(`[data-subject="${subjectId}"]`));
  await shot(win, '02-subject');
  await js(win, click('[data-topic]'));
  await sleep(500);
  await sleep(400);
  await shot(win, '03-notes');
  // read-aloud: if this topic has audio, start it and confirm it is actually playing
  const hasAudio = await js(win, `!document.querySelector('#reader-bar').hidden`);
  if (hasAudio) {
    await js(win, click('#read-btn'));
    await sleep(2500);
    const st = await js(win, `[...document.querySelectorAll('audio')].length + ' ' + document.querySelector('#read-status').textContent + ' | ' + document.querySelector('#read-btn').textContent`);
    console.log('read-aloud:', st);
    await shot(win, '03a-reading');
    await js(win, click('#read-stop'));
  } else console.log('read-aloud: no audio for this topic');
  // notes are revealed one section at a time
  await js(win, click('#more-btn'));
  await sleep(600);
  await shot(win, '03b-notes-continued');
  await js(win, click('.seg[data-mode="questions"]'));
  await sleep(300);
  // answer the first questions: pick option B on MCQs, then check
  await js(win, `document.querySelectorAll('.q-card').forEach((c, i) => { const o = c.querySelectorAll('.opt'); if (o.length) o[Math.min(1, o.length - 1)].click(); }); true`);
  await js(win, `document.querySelectorAll('.q-card .text-in').forEach((t) => { t.value = '5'; t.dispatchEvent(new Event('input')); }); true`);
  await js(win, `document.querySelectorAll('.q-card .text-area').forEach((t) => { t.value = 'My working'; t.dispatchEvent(new Event('input')); }); true`);
  await js(win, `document.querySelectorAll('.check-btn').forEach((b) => b.click()); true`);
  console.log('streak after practice:', await js(win, `document.querySelector('#streak-btn').className + ' ' + document.querySelector('#streak-btn').textContent + ' | toast: ' + !!document.querySelector('.streak-toast')`));
  await shot(win, '04-questions-checked');
  await js(win, `document.querySelectorAll('.mark-btn[data-mark="1"]').forEach((b) => b.click()); true`);
  await js(win, `window.scrollTo(0, document.body.scrollHeight); true`);
  await shot(win, '05-practice-done');
  await js(win, click('.tab[data-tab="exams"]'));
  await shot(win, '06-exam-home');
  await js(win, click(`[data-subject="${subjectId}"]`));
  await js(win, click('[data-exam]'));
  await sleep(500);
  await shot(win, '07-exam-intro');
  await js(win, click('#start'));
  await sleep(500);
  await js(win, `document.querySelectorAll('.q-card').forEach((c) => { const o = c.querySelectorAll('.opt'); if (o.length) o[0].click(); }); document.querySelectorAll('.q-card .text-in').forEach((t) => { t.value = '3'; t.dispatchEvent(new Event('input')); }); document.querySelectorAll('.q-card .text-area').forEach((t) => { t.value = 'answer'; t.dispatchEvent(new Event('input')); }); true`);
  await shot(win, '08-exam-run');
  await js(win, click('#submit'));
  await sleep(300);
  await shot(win, '09-exam-submit-dialog');
  await js(win, click('[data-r="1"]'));
  await sleep(500);
  // award full marks on every written answer (the highest mark button on each card)
  await js(win, `document.querySelectorAll('.q-card').forEach((c) => { const b = c.querySelectorAll('.mark-btn'); if (b.length) b[b.length - 1].click(); }); true`);
  await shot(win, '10-exam-marking');
  await js(win, `(() => { const f = document.querySelector('#finish'); if (f) f.click(); return true; })()`);
  await sleep(500);
  await shot(win, '11-exam-result');
  await js(win, click('#review'));
  await sleep(500);
  await shot(win, '12-exam-review');
  await js(win, click('#streak-btn'));
  await shot(win, '12b-streak');
  await js(win, click('[data-r="1"]'));
  await js(win, click('.tab[data-tab="progress"]'));
  await shot(win, '13-progress');
  console.log('streak saved:', JSON.stringify(JSON.parse(fs.readFileSync(path.join(profile, 'progress.json'), 'utf8')).streak));
  const saved = fs.readFileSync(path.join(profile, 'progress.json'), 'utf8');
  console.log('progress bytes saved:', saved.length);
  console.log('renderer errors:', errors.length ? errors.join('\n') : 'none');
  app.exit(errors.length ? 2 : 0);
}

require('../main.js');
