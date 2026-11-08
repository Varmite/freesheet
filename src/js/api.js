// FreeSheet: the bridge to the Rust back-end (src-tauri/). It gives the rest of the app
// window.api, the same on every platform (Windows, macOS, Linux and Android).
// Content, audio and progress are handled in Rust; this file only forwards calls.
(function () {
  'use strict';
  const T = window.__TAURI__;
  const invoke = T.core.invoke;
  // Set by the Rust side before any page script runs: win32 | darwin | linux | android.
  const platform = window.__FREESHEET_PLATFORM__ || 'linux';
  const android = platform === 'android';

  if (android) {
    document.documentElement.classList.add('android-edition');
    const css = document.createElement('link');
    css.rel = 'stylesheet';
    css.href = 'mobile.css';
    document.head.appendChild(css);
  }

  // Audio arrives as raw bytes (an ArrayBuffer), not JSON.
  const bytes = (p) => p.then((b) => new Uint8Array(b));

  // Saves still on their way to the back-end; closing waits for them.
  const pending = new Set();
  function track(p) {
    pending.add(p);
    p.finally(() => pending.delete(p)).catch(() => {});
    return p;
  }

  window.api = {
    platform,
    getCatalog: () => invoke('catalog'),
    contentWords: () => invoke('content_words'),
    getCourse: (id) => invoke('course', { id }),
    courseAudioList: (id) => invoke('course_audio_list', { id }),
    courseAudio: (id, audio) => bytes(invoke('course_audio', { id, audio })),
    getTopic: (grade, subjectId, topicId) => invoke('topic', { grade: Number(grade), subjectId, topicId }),
    getExam: (grade, subjectId, examId) => invoke('exam', { grade: Number(grade), subjectId, examId }),
    listAudio: (grade, subjectId, topicId) => invoke('audio_list', { grade: Number(grade), subjectId, topicId }),
    getAudio: (grade, subjectId, topicId, id) => bytes(invoke('audio', { grade: Number(grade), subjectId, topicId, id })),
    loadProgress: () => invoke('progress_load'),
    saveProgress: (data) => track(invoke('progress_save', { data })),
    // Used while the app closes: starts the save; the close waits for it (see below).
    saveProgressSync: (data) => { track(invoke('progress_save', { data })); return true; },
    progressLocation: () => invoke('progress_location'),
    exportProgress: (data) => invoke('progress_export', { data }),
    importProgress: () => invoke('progress_import'),
    openDataFolder: () => invoke('open_data_folder'),
    version: () => invoke('app_version'),
    smoothTextActive: () => invoke('smooth_text_active'),
    relaunch: () => invoke('relaunch'),
    openReleasePage: (url) => invoke('open_release_page', { url }),
  };

  // ---------- closing ----------
  // The back-end holds the window open and sends "freesheet:closing"; the app saves
  // (as it would on beforeunload), and once the saves are written the window closes.
  async function saveNow() {
    window.dispatchEvent(new Event('beforeunload'));
    await Promise.allSettled([...pending]);
  }
  T.event.listen('freesheet:closing', async () => {
    await saveNow();
    invoke('close_window');
  });

  if (!android) return;

  // ---------- Android ----------

  // Going to the home screen or another app: save now (this also keeps an exam in
  // progress so it can be resumed).
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') saveNow();
  });

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
    saveNow().then(() => invoke('minimize_app'));
  }
  if (T.app && T.app.onBackButtonPress) T.app.onBackButtonPress(back);
})();
