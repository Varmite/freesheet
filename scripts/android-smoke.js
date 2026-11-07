#!/usr/bin/env node
'use strict';
// Drives FreeSheet inside an Android emulator through the WebView's DevTools (debug APK only).
// Used by scripts/android-smoke.sh:   node scripts/android-smoke.js <port> <phase>
//   phase "start": checks the Rust back-end (content, audio from the APK, word list), picks a
//                  grade, skips the buddy setup and opens a maths topic with read-aloud audio.
//   phase "back":  checks that the Android back button went up from the topic to the topic list.
// Prints one line per check and exits 1 if any fails.

const [, , port, phase] = process.argv;
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
  ws.onmessage = (m) => { const d = JSON.parse(m.data); if (waits.has(d.id)) { waits.get(d.id)(d); waits.delete(d.id); } };
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
    const ok = v && !v.error && test(v);
    check(ok, what, JSON.stringify(v).slice(0, 300));
    return v;
  };
  const clickText = (sel, re) => evaluate(`(() => { const el = [...document.querySelectorAll(${JSON.stringify(sel)})].find((e) => ${re}.test(e.textContent) && e.offsetParent !== null); if (el) el.click(); return !!el; })()`);

  if (phase === 'start') {
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
    await run('top bar clear of the status bar', 'document.querySelector(".topbar").getBoundingClientRect().top', (v) => v >= 0);
  } else if (phase === 'back') {
    await sleep(1500);
    await run('back went up to the topic list', `(() => ({
      topic: [...document.querySelectorAll("#main button")].some((b) => /Listen to these notes/.test(b.textContent)),
      list: /Algebraic Expressions/.test(document.querySelector("#main").innerText),
    }))()`, (v) => !v.topic && v.list);
  }
  ws.close();
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
