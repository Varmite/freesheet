#!/usr/bin/env node
'use strict';
// Drives FreeSheet inside an Android emulator through the WebView's DevTools (debug APK only).
// Used by scripts/android-smoke.sh:   node scripts/android-smoke.js <port> <phase>
//   phase "start": (exits 4 if the WebView is too old and the app says so) checks the Rust back-end (content, audio from the APK, word list), picks a
//                  grade, skips the buddy setup and opens a maths topic with read-aloud audio.
//   phase "back":  checks that the Android back button went up from the topic to the topic list.
//   phase "tour":  visits Settings, Exams, Progress and a language lesson, with a screenshot of
//                  each (into <out-dir>), checking nothing is wider than the screen; then, where
//                  the WebView can animate, turns animations on and changes pages with them.
//   phase "crash": kills the WebView's renderer process, as Android does when short of memory.
//   phase "alive": checks the app came back after that, with the learner's grade still set.
// Prints one line per check and exits 1 if any fails. Errors the page throws meanwhile fail too.

const { execSync } = require('child_process');
const [, , port, phase, outDir = '.'] = process.argv;
let failed = 0;
const check = (ok, what, detail = '') => {
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${what}${detail ? `: ${detail}` : ''}`);
  if (!ok) failed++;
};

async function connect() {
  for (let i = 0; i < 30; i++) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
      const page = list.find((t) => t.type === 'page');
      if (page) {
        const ws = new WebSocket(page.webSocketDebuggerUrl);
        await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
        return ws;
      }
    } catch (e) { /* not up yet */ }
    await new Promise((r) => setTimeout(r, 2000));
  }
  throw new Error('No DevTools page found');
}

(async () => {
  const ws = await connect();
  let id = 0;
  const waits = new Map();
  const pageErrors = [];
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (waits.has(d.id)) { waits.get(d.id)(d); waits.delete(d.id); }
    if (d.method === 'Runtime.exceptionThrown') {
      const x = d.params.exceptionDetails;
      pageErrors.push(`${(x.exception && x.exception.description) || x.text} (${x.url || ''}:${x.lineNumber})`);
    }
  };
  ws.send(JSON.stringify({ id: ++id, method: 'Runtime.enable' }));
  const evaluate = (expression) => new Promise((resolve) => {
    const i = ++id;
    waits.set(i, (d) => resolve(d.result.exceptionDetails
      ? { error: (d.result.exceptionDetails.exception || {}).description || d.result.exceptionDetails.text }
      : d.result.result.value));
    ws.send(JSON.stringify({ id: i, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }));
  });
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const run = async (what, expr, test) => {
    const v = await evaluate(expr);
    const ok = v !== undefined && v !== null && !v.error && test(v);
    check(ok, what, JSON.stringify(v).slice(0, 300));
    return v;
  };
  const click = (sel) => evaluate(`(() => { const el = [...document.querySelectorAll(${JSON.stringify(sel)})].find((e) => e.offsetParent !== null); if (el) el.click(); return !!el; })()`);
  const shot = (name) => { try { execSync(`adb exec-out screencap -p > "${outDir}/${name}.png"`, { shell: '/bin/bash' }); } catch (e) { console.log(`INFO  no screenshot ${name}: ${e.message}`); } };
  // Nothing on the page may be wider than the screen (the page itself never scrolls sideways).
  const fits = (where) => run(`${where}: fits the screen's width`, '({ page: document.documentElement.scrollWidth, screen: window.innerWidth })', (v) => v.page <= v.screen + 1);
  const clickText = (sel, re) => evaluate(`(() => { const el = [...document.querySelectorAll(${JSON.stringify(sel)})].find((e) => ${re}.test(e.textContent) && e.offsetParent !== null); if (el) el.click(); return !!el; })()`);

  if (phase === 'start') {
    console.log(`INFO  WebView: ${JSON.stringify(await evaluate('navigator.userAgent'))}`);
    console.log(`INFO  screen: ${JSON.stringify(await evaluate('({ w: innerWidth, h: innerHeight, dpr: devicePixelRatio, memory: navigator.deviceMemory, cores: navigator.hardwareConcurrency, notice: !!document.getElementById("compat-notice") })'))}`);
    // A WebView too old to run the app (as on a phone that has never updated it) must say so.
    if (await evaluate('!!document.getElementById("compat-notice")')) {
      await run('old WebView: the notice explains what to update', 'document.getElementById("compat-notice").innerText', (v) => /Android System WebView/.test(v) && /Play Store/.test(v));
      shot('01-webview-notice');
      ws.close();
      process.exit(failed ? 1 : 4);
    }
    for (let i = 0; i < 20 && !(await evaluate('!!(window.api && document.querySelector("#main .page"))')); i++) await sleep(1000);
    await run('platform', 'api.platform', (v) => v === 'android');
    await run('phone layout', 'document.documentElement.classList.contains("android-edition") && !!document.querySelector("link[href=\\"mobile.css\\"]")', (v) => v === true);
    await run('catalogue', 'api.getCatalog().then((c) => ({ grades: c.grades.length, courses: c.courses.length }))', (v) => v.grades >= 5 && v.courses >= 5);
    await run('topic', 'api.getTopic(10, "mathematics", "algebraic-expressions").then((t) => t.title)', (v) => /Algebraic/.test(v));
    await run('word list', 'api.contentWords().then((w) => w.length)', (v) => v > 1000);
    await run('version', 'api.version()', (v) => /^\d+\.\d+\.\d+/.test(v));
    await run('read-aloud audio from the APK', `(async () => {
      const ids = await api.listAudio(10, 'english-home-language', 'drama');
      const b = await api.getAudio(10, 'english-home-language', 'drama', ids[0]);
      const a = new Audio(URL.createObjectURL(new Blob([b], { type: 'audio/ogg' })));
      const duration = await new Promise((r) => { a.onloadedmetadata = () => r(a.duration); a.onerror = () => r(-1); });
      return { ids: ids.length, bytes: b.byteLength, ogg: String.fromCharCode(...b.slice(0, 4)), duration };
    })()`, (v) => v.ids > 0 && v.ogg === 'OggS' && v.duration > 0);
    await run('bad ids are refused', 'api.getTopic(10, "../x", "y").then(() => "loaded", (e) => "refused: " + e)', (v) => /^refused/.test(v));

    check(await clickText('#main [data-grade]', /Grade 10/), 'tap Grade 10');
    await sleep(2500);
    if (await clickText('.bs-skip, button', /Skip for now/)) { check(true, 'skip buddy setup'); await sleep(1500); }
    check(await clickText('#main .card', /Mathematics(?! Lit)/), 'open Mathematics');
    await sleep(2000);
    check(await clickText('#main .card, #main .row, #main a, #main button', /Algebraic Expressions/), 'open Algebraic Expressions');
    await sleep(2500);
    await run('Listen button (topic audio found)', '[...document.querySelectorAll("#main button")].some((b) => /Listen to these notes/.test(b.textContent))', (v) => v === true);
    await run('maths typeset', 'document.querySelectorAll("#main .katex").length', (v) => v > 0);
    // The buddy's speech bubble would take the first Back press (it closes first, by design).
    await evaluate('(() => { const x = document.querySelector(".buddy-bubble:not([hidden]) .buddy-x"); if (x) x.click(); return !!x; })()');
    await run('top bar clear of the status bar', 'document.querySelector(".topbar").getBoundingClientRect().top', (v) => v >= 0);
    await fits('notes');
  } else if (phase === 'back') {
    await sleep(1500);
    await run('back went up to the topic list', `(() => ({
      topic: [...document.querySelectorAll("#main button")].some((b) => /Listen to these notes/.test(b.textContent)),
      list: /Algebraic Expressions/.test(document.querySelector("#main").innerText),
    }))()`, (v) => !v.topic && v.list);
  } else if (phase === 'tour') {
    const phone = await evaluate('innerWidth <= 720');
    await fits('topic list');
    shot('04-topic-list');

    check(await click('#settings-btn'), 'open Settings');
    await sleep(2000);
    await fits('Settings');
    shot('05-settings');
    const motion = await run('animations', '({ supported: Motion.supported, lowEnd: Motion.lowEnd, on: Motion.enabled() })', (v) => typeof v.supported === 'boolean' && (v.on ? v.supported : true));
    await evaluate('window.scrollTo(0, document.documentElement.scrollHeight)');
    await sleep(500);
    shot('06-settings-end');
    check(await click('#main #settings-back'), 'leave Settings');
    await sleep(1500);

    check(await click('.tab[data-tab="exams"]'), 'Exams tab');
    await sleep(2000);
    await fits('Exams');
    shot('07-exams');
    check(await click('.tab[data-tab="progress"]'), 'Progress tab');
    await sleep(2000);
    await fits('Progress');
    shot('08-progress');

    check(await click('.tab[data-tab="study"]'), 'Study tab');
    await sleep(2000);
    await fits('Study');
    shot('09-study');
    if (await click('#main [data-course]')) {
      await sleep(2500);
      await fits('course');
      check(await click('#main #go-next'), 'start a language lesson');
      await sleep(3000);
      await run('lesson fills the screen (no tab bar on a phone)', `({
        lesson: !!document.querySelector('#main .lesson'), cls: document.body.classList.contains('in-lesson'),
        tabs: getComputedStyle(document.querySelector('.tabs')).display,
      })`, (v) => v.lesson && v.cls && (!phone || v.tabs === 'none'));
      await fits('lesson');
      shot('10-lesson');
      check(await click('#main #quit'), 'quit the lesson');
      await sleep(1500);
      if (await clickText('#dialog-root button', /^Quit$/)) await sleep(1500);
      await run('tab bar back after the lesson', `({ cls: document.body.classList.contains('in-lesson'), tabs: getComputedStyle(document.querySelector('.tabs')).display })`, (v) => !v.cls && v.tabs !== 'none');
    } else check(false, 'open a language course');

    // With animations on (where this WebView can run them): page changes must still land.
    if (motion && motion.supported) {
      await click('#settings-btn');
      await sleep(2000);
      check(await click('#main [data-anim="1"]'), 'turn animations on');
      await sleep(1000);
      await run('animations are on', 'Motion.enabled()', (v) => v === true);
      await click('#main #settings-back');
      await sleep(2500);
      await click('.tab[data-tab="study"]');
      await sleep(2500);
      check(await clickText('#main .card', /Mathematics(?! Lit)/), 'open Mathematics (animated)');
      await sleep(3000);
      check(await clickText('#main .card, #main .row, #main a, #main button', /Algebraic Expressions/), 'open Algebraic Expressions (animated)');
      await sleep(3500);
      await run('the topic opened (animated)', '[...document.querySelectorAll("#main button")].some((b) => /Listen to these notes/.test(b.textContent))', (v) => v === true);
      shot('11-animated-topic');
    } else console.log('INFO  this WebView cannot run the animations: they stay off');
  } else if (phase === 'crash') {
    const r = await new Promise((resolve) => {
      const i = ++id;
      waits.set(i, resolve);
      ws.send(JSON.stringify({ id: i, method: 'Page.crash' }));
      setTimeout(() => resolve({ sent: true }), 4000);
    });
    if (r.error) { console.log(`SKIP  cannot kill the renderer here: ${r.error.message}`); process.exit(3); }
    console.log('INFO  renderer killed');
    process.exit(0);
  } else if (phase === 'alive') {
    for (let i = 0; i < 30 && !(await evaluate('!!(window.api && document.querySelector("#main .page"))')); i++) await sleep(1000);
    await run('the app is back after losing its renderer', '!!document.querySelector("#main .page")', (v) => v === true);
    await run('the grade is still set', 'document.querySelector("#grade-select").value', (v) => String(v) === '10');
    await sleep(1000);
    shot('13-after-renderer-loss');
  }
  for (const e of pageErrors) check(false, 'page error', e);
  ws.close();
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
