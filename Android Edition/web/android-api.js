// FreeSheet Android Edition: the phone back-end. It provides the same window.api that
// the desktop app's preload.js gives, but reads the bundled content with fetch() and
// keeps progress on the phone. Also adds the Android back button and saves when the
// app is sent to the background. Works in a plain browser too (for testing).
(function () {
  'use strict';
  const C = window.Capacitor;
  const native = !!(C && C.isNativePlatform && C.isNativePlatform());
  const plugin = (name) => (native ? (C.Plugins && C.Plugins[name]) || (C.registerPlugin && C.registerPlugin(name)) : null);
  const App = plugin('App');
  const Filesystem = plugin('Filesystem');
  const Share = plugin('Share');

  document.documentElement.classList.add('android-edition');

  const ID_RE = /^[a-z0-9]+(-[a-z0-9]+)*$/;
  const AUDIO_RE = /^[0-9a-f]{8}$/;
  function id(v, what) { if (typeof v !== 'string' || !ID_RE.test(v)) throw new Error(`Invalid ${what}`); return v; }
  function grade(g) { const n = Number(g); if (!Number.isInteger(n) || n < 1 || n > 12) throw new Error('Invalid grade'); return n; }
  function audioId(a) { if (typeof a !== 'string' || !AUDIO_RE.test(a)) throw new Error('Invalid audio id'); return a; }

  async function getJson(url) {
    const r = await fetch(url);
    if (!r.ok) throw new Error(`Could not load ${url}`);
    return r.json();
  }
  async function getBytes(url) {
    const r = await fetch(url);
    if (!r.ok) throw new Error(`Could not load ${url}`);
    return new Uint8Array(await r.arrayBuffer());
  }
  const once = (fn) => { let p = null; return () => (p = p || fn().catch((e) => { p = null; throw e; })); };
  const catalog = once(() => getJson('data/catalog.json'));
  const audioIndex = once(() => getJson('data/audio-index.json'));
  const appInfo = once(() => getJson('data/app.json'));

  // ---------- progress ----------
  // Kept in the WebView's storage (instant, works for the synchronous save on exit) and
  // mirrored to a file in the app's private folder, in case WebView storage is cleared.
  const KEY = 'freesheet-progress';
  const FILE = 'progress.json';
  const empty = () => ({ version: 1, settings: {}, topics: {}, exams: {}, examDrafts: {} });
  const valid = (d) => d && typeof d === 'object' && !Array.isArray(d);

  function writeLocal(json) { try { localStorage.setItem(KEY, json); return true; } catch (e) { return false; } }
  let fileChain = Promise.resolve();
  function writeFile(json) {
    if (!Filesystem) return Promise.resolve();
    fileChain = fileChain.then(() => Filesystem.writeFile({ path: FILE, data: json, directory: 'DATA', encoding: 'utf8' })).catch((e) => console.warn('Progress file not written:', e));
    return fileChain;
  }

  async function loadProgress() {
    let data = null;
    try { data = JSON.parse(localStorage.getItem(KEY) || 'null'); } catch (e) { data = null; }
    if (!valid(data) && Filesystem) {
      try { data = JSON.parse((await Filesystem.readFile({ path: FILE, directory: 'DATA', encoding: 'utf8' })).data); } catch (e) { data = null; }
    }
    return valid(data) ? Object.assign(empty(), data) : empty();
  }
  async function saveProgress(data) {
    const json = JSON.stringify(data);
    if (!writeLocal(json)) throw new Error('Could not save');
    writeFile(json);
    return true;
  }
  function saveProgressSync(data) {
    const json = JSON.stringify(data);
    writeFile(json);
    return writeLocal(json);
  }

  // Export: save a copy in Documents/FreeSheet and open the share sheet (Drive, email, ...).
  async function exportProgress(data) {
    const name = `freesheet-progress-${new Date().toISOString().slice(0, 10)}.json`;
    const json = JSON.stringify(data, null, 2);
    if (!Filesystem) { // browser: plain download
      const a = document.createElement('a');
      a.href = URL.createObjectURL(new Blob([json], { type: 'application/json' }));
      a.download = name; a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 5000);
      return { ok: true };
    }
    let saved = null;
    try {
      await Filesystem.writeFile({ path: `FreeSheet/${name}`, data: json, directory: 'DOCUMENTS', encoding: 'utf8', recursive: true });
      saved = `Documents/FreeSheet/${name}`;
    } catch (e) { console.warn('Could not save to Documents:', e); }
    try {
      const tmp = await Filesystem.writeFile({ path: name, data: json, directory: 'CACHE', encoding: 'utf8' });
      await Share.share({ title: 'FreeSheet progress', files: [tmp.uri], dialogTitle: 'Save or send your progress' });
    } catch (e) {
      if (!saved) return { ok: false }; // share cancelled and nothing saved
    }
    return { ok: true, path: saved };
  }

  // Import: the system file picker.
  function importProgress() {
    return new Promise((resolve) => {
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = '.json,application/json,text/plain';
      input.style.display = 'none';
      document.body.appendChild(input);
      const done = (r) => { input.remove(); resolve(r); };
      input.addEventListener('cancel', () => done({ ok: false }));
      input.addEventListener('change', async () => {
        const f = input.files && input.files[0];
        if (!f) return done({ ok: false });
        try {
          let text = await f.text();
          if (text.charCodeAt(0) === 0xfeff) text = text.slice(1);
          const data = JSON.parse(text);
          if (!valid(data) || !data.topics || !data.exams) return done({ ok: false, error: 'That file is not a FreeSheet progress file.' });
          done({ ok: true, data: Object.assign(empty(), data) });
        } catch (e) { done({ ok: false, error: 'That file could not be read.' }); }
      });
      input.click();
    });
  }

  window.api = {
    getCatalog: () => catalog(),
    contentWords: () => getJson('data/words.json'),
    getCourse: (c) => getJson(`content/courses/${id(c, 'course')}/course.json`),
    courseAudioList: async (c) => (await audioIndex())[`courses/${id(c, 'course')}`] || [],
    courseAudio: (c, a) => getBytes(`audio/courses/${id(c, 'course')}/${audioId(a)}.ogg`),
    getTopic: async (g, s, t) => getJson(`content/grade-${grade(g)}/${id(s, 'subject')}/topics/${id(t, 'topic')}.json`),
    getExam: async (g, s, e) => getJson(`content/grade-${grade(g)}/${id(s, 'subject')}/exams/${id(e, 'exam')}.json`),
    loadProgress,
    saveProgress,
    saveProgressSync,
    progressLocation: async () => "the FreeSheet app's private storage",
    exportProgress,
    importProgress,
    openDataFolder: async () => {},
    version: async () => (await appInfo()).version,
    platform: 'android',
    smoothTextActive: async () => true,
    relaunch: async () => location.reload(),
    listAudio: async (g, s, t) => (await audioIndex())[`grade-${grade(g)}/${id(s, 'subject')}/${id(t, 'topic')}`] || [],
    getAudio: (g, s, t, a) => getBytes(`audio/grade-${grade(g)}/${id(s, 'subject')}/${id(t, 'topic')}/${audioId(a)}.ogg`),
  };

  // ---------- Android lifecycle ----------

  if (!App) return;

  // Going to the home screen or another app: save now, as closing the desktop window does
  // (this also stores an exam in progress so it can be resumed).
  App.addListener('pause', () => window.dispatchEvent(new Event('beforeunload')));

  // The back button/gesture steps back through the app like the on-screen links do.
  const visible = (el) => el && el.isConnected && el.offsetParent !== null;
  function back() {
    const q = (s) => document.querySelector(s);
    const overlay = q('#dialog-root .modal-back, .buddy-setup-back');
    if (overlay) {
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
      return;
    }
    const bubbleX = q('.buddy-bubble:not([hidden]) .buddy-x');
    if (visible(bubbleX)) return bubbleX.click();
    for (const sel of ['#main #quit', '#main #settings-back']) {
      const b = q(sel);
      if (visible(b)) return b.click();
    }
    const crumbs = document.querySelectorAll('#main .crumbs [data-crumb]');
    if (crumbs.length) return crumbs[crumbs.length - 1].click();
    const backBtn = [...document.querySelectorAll('#main .back-link, #main [data-back]')].find(visible);
    if (backBtn) return backBtn.click();
    const study = q('.tab[data-tab="study"]');
    if (study && !study.classList.contains('active') && !document.body.classList.contains('welcome-mode')) return study.click();
    window.dispatchEvent(new Event('beforeunload'));
    App.minimizeApp();
  }
  App.addListener('backButton', back);
})();
