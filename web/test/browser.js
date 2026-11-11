#!/usr/bin/env node
'use strict';
// End-to-end check in real browsers (Playwright): signs in with a test account,
// loads the catalog, opens a topic, answers a practice question, reloads to check the
// answer was saved, and fetches audio with a Range request. Runs in Chromium (desktop)
// and WebKit with an iPhone profile (Safari's engine).
//
//   npx playwright install chromium webkit     (once)
//   npm run test:browser

const { spawn } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');
const assert = require('assert');
const { chromium, webkit, devices } = require('playwright');
const { createAccounts } = require('../lib/auth');

const PORT = 5099;
const BASE = `http://127.0.0.1:${PORT}`;
const dataDir = fs.mkdtempSync(path.join(os.tmpdir(), 'freesheet-web-e2e-'));
const accounts = createAccounts(dataDir);
const PASSWORD = 'browser-test-pw';

function startServer() {
  const child = spawn(process.execPath, [path.join(__dirname, '..', 'server.js')], {
    env: { ...process.env, SESSION_SECRET: 'e2e', DATA_DIR: dataDir, PORT: String(PORT), HOST: '127.0.0.1', AUTO_UPDATE: '0', HTTPS_CERT: '', HTTPS_KEY: '' },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  return new Promise((resolve, reject) => {
    child.stdout.on('data', (d) => { if (String(d).includes('FreeSheet web edition on')) resolve(child); });
    child.stderr.on('data', (d) => process.stderr.write(d));
    child.on('exit', (code) => reject(new Error(`server exited (${code})`)));
  });
}

async function run(name, browserType, contextOptions, sub) {
  const browser = await browserType.launch();
  const context = await browser.newContext(contextOptions);
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const step = (s) => console.log(`  ${name}: ${s}`);

  // Signed out: the app sends us to the login page.
  await page.goto(BASE + '/');
  await page.waitForURL('**/login');
  // A wrong password is refused, the right one lets us in.
  accounts.set(sub, PASSWORD);
  await page.fill('#username', sub);
  await page.fill('#password', 'not-it');
  await page.click('#go');
  await page.waitForSelector('#error:not([hidden])');
  await page.fill('#password', PASSWORD);
  await page.click('#go');
  await page.waitForURL(BASE + '/');
  step('signed in');

  // First visit: choose a grade. The buddy may ask to be set up: skip it.
  await page.getByRole('button', { name: 'Grade 9', exact: true }).click();
  const skip = page.getByRole('button', { name: 'Skip for now' });
  if (await skip.waitFor({ timeout: 4000 }).then(() => true, () => false)) await skip.click();
  await page.waitForSelector('.card[data-subject="technology"]');
  step('catalog loaded');

  // Open a topic: its notes appear.
  await page.click('.card[data-subject="technology"]');
  await page.click('.row[data-topic="preservation-of-metals"]');
  await page.waitForSelector('.note-section');
  step('topic opened');

  // Answer the first practice question (multiple choice) and check it.
  await page.click('.seg[data-mode="questions"]');
  const card = page.locator('.q-card').first();
  await card.locator('.opt').nth(2).click();
  await card.locator('.check-btn').click();
  await page.waitForSelector('.q-card.is-correct, .q-card.is-wrong');
  const verdict = await card.getAttribute('class');
  step(`answered a question (${/is-correct/.test(verdict) ? 'correct' : 'wrong'})`);

  // Saved? Wait for the save, reload, and look again.
  await page.waitForTimeout(1500);
  await page.reload();
  // The app reopens on the Study home; 'Continue where you left off' leads back.
  await page.click('#continue');
  await page.waitForSelector('.segmented');
  if (!(await page.locator('.seg[data-mode="questions"].active').count())) await page.click('.seg[data-mode="questions"]');
  await page.waitForSelector('.q-card');
  const after = await page.locator('.q-card').first().getAttribute('class');
  assert.match(after, /is-correct|is-wrong/, 'the checked answer survived a reload');
  const saved = await page.evaluate(() => fetch('/api/progress').then((r) => r.json()));
  const tp = saved.progress.topics['9/technology/preservation-of-metals'];
  assert.ok(tp && tp.checked && Object.keys(tp.checked).length === 1, 'progress on the server');
  step('progress saved and survived a reload');

  // Audio with a Range request, as Safari's player makes them.
  const audio = await page.evaluate(async () => {
    const ids = await fetch('/api/audio/9/technology/preservation-of-metals').then((r) => r.json());
    const r = await fetch(`/api/audio/9/technology/preservation-of-metals/${ids[0]}`, { headers: { Range: 'bytes=0-1023' } });
    return { status: r.status, range: r.headers.get('content-range'), size: (await r.arrayBuffer()).byteLength, count: ids.length };
  });
  assert.strictEqual(audio.status, 206);
  assert.strictEqual(audio.size, 1024);
  step(`audio: ${audio.count} sections, Range request answered ${audio.status} (${audio.range})`);

  // The phone layout is used on the phone profile only.
  const mobile = await page.evaluate(() => [...document.styleSheets].some((s) => s.href && s.href.endsWith('mobile.css') && matchMedia(s.media.mediaText).matches));
  step(`phone layout ${mobile ? 'on' : 'off'}`);
  assert.strictEqual(mobile, !!contextOptions.isMobile);

  // Sign out from Settings returns to the login page.
  await page.click('#settings-btn');
  await page.click('#sign-out');
  await page.waitForURL('**/login');
  step('signed out');

  assert.deepStrictEqual(errors, [], `page errors: ${errors.join('; ')}`);
  await browser.close();
}

(async () => {
  const server = await startServer();
  try {
    await run('Chromium (desktop)', chromium, { viewport: { width: 1200, height: 800 } }, 'desktop-tester');
    await run('WebKit (iPhone 13)', webkit, { ...devices['iPhone 13'] }, 'phone-tester');
    console.log('Browser tests passed.');
  } finally {
    server.kill();
    fs.rmSync(dataDir, { recursive: true, force: true });
  }
})().catch((e) => { console.error(e); process.exit(1); });
