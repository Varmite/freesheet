// FreeSheet as a website: the same window.api the desktop app gets from src/js/api.js, but
// talking to server/server.js. The server sends this file in place of js/api.js.
(function () {
  'use strict';

  // Safari has no requestIdleCallback.
  if (!window.requestIdleCallback) window.requestIdleCallback = (fn) => setTimeout(fn, 1);

  async function get(url) {
    const r = await fetch(url);
    if (!r.ok) throw new Error(`Could not load ${url} (${r.status})`);
    return r;
  }
  const json = (url) => get(url).then((r) => r.json());
  const bytes = (url) => get(url).then((r) => r.arrayBuffer()).then((b) => new Uint8Array(b));
  const id = encodeURIComponent;

  // Saves go one at a time, each naming the version it replaces, so a tab left open on
  // old progress can't overwrite what was saved since on another device.
  let rev = null;
  let reloading = false;
  let queue = Promise.resolve();
  let inFlight = 0;

  function reloadForNewer() {
    if (reloading) return;
    reloading = true;
    location.reload();
  }

  function save(data) {
    if (reloading) return Promise.resolve(true);
    const body = JSON.stringify(data);
    inFlight++;
    const run = async () => {
      // keepalive lets a save finish as the page closes, but only for small bodies.
      const r = await fetch('/api/progress', { method: 'PUT', headers: { 'Content-Type': 'application/json', 'X-Rev': rev }, body, keepalive: body.length < 60000 });
      if (r.status === 409) { reloadForNewer(); return true; }
      if (!r.ok) throw new Error(`Could not save (${r.status})`);
      rev = (await r.json()).rev;
      return true;
    };
    const result = queue.then(run, run);
    queue = result.catch(() => {}).finally(() => { inFlight--; });
    return result;
  }

  const api = {
    web: true,
    platform: 'web',
    account: { signedIn: false, email: null, googleAvailable: false },
    deviceName: 'this device',
    getCatalog: () => json('/api/catalog'),
    contentWords: () => json('/api/words'),
    getCourse: (c) => json(`/api/course/${id(c)}`),
    courseAudioList: (c) => json(`/api/course/${id(c)}/audio`),
    courseAudio: (c, a) => bytes(`/api/course/${id(c)}/audio/${id(a)}`),
    getTopic: (g, s, t) => json(`/api/topic/${id(g)}/${id(s)}/${id(t)}`),
    getExam: (g, s, x) => json(`/api/exam/${id(g)}/${id(s)}/${id(x)}`),
    listAudio: (g, s, t) => json(`/api/audio/${id(g)}/${id(s)}/${id(t)}`),
    getAudio: (g, s, t, a) => bytes(`/api/audio/${id(g)}/${id(s)}/${id(t)}/${id(a)}`),
    async loadProgress() {
      const r = await json('/api/progress');
      rev = r.rev;
      api.account = r.account;
      api.deviceName = r.account.signedIn ? 'your account' : 'this device';
      return r.progress;
    },
    saveProgress: save,
    saveProgressSync(data) { save(data).catch(() => {}); return true; },
    progressLocation: async () => (api.account.signedIn
      ? 'Your progress is saved to your account and follows you to any device you sign in on.'
      : 'Your progress is saved for this browser on this device. Sign in to keep it on any device.'),
    async exportProgress(data) {
      const url = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' }));
      const a = document.createElement('a');
      a.href = url;
      a.download = `freesheet-progress-${new Date().toISOString().slice(0, 10)}.json`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 10000);
      return { ok: true };
    },
    importProgress() {
      return new Promise((resolve) => {
        const input = document.createElement('input');
        input.type = 'file';
        input.accept = '.json,application/json';
        input.hidden = true;
        document.body.appendChild(input);
        const finish = (v) => { input.remove(); resolve(v); };
        input.addEventListener('cancel', () => finish({ ok: false }));
        input.addEventListener('change', async () => {
          const file = input.files[0];
          if (!file) return finish({ ok: false });
          try {
            const v = JSON.parse((await file.text()).replace(/^﻿/, ''));
            if (!v || typeof v !== 'object' || !v.topics || !v.exams) return finish({ ok: false, error: 'That file is not a FreeSheet progress file.' });
            finish({ ok: true, data: { version: 1, settings: {}, topics: {}, exams: {}, examDrafts: {}, ...v } });
          } catch (e) {
            finish({ ok: false, error: 'That file could not be read.' });
          }
        });
        input.click();
      });
    },
    signIn() { location.href = '/auth/google'; },
    async signOut() {
      await fetch('/auth/logout', { method: 'POST' });
      reloading = true;
      location.reload();
    },
    openDataFolder: async () => {},
    version: async () => window.__FREESHEET_VERSION__,
    smoothTextActive: async () => true,
    relaunch: async () => location.reload(),
    openReleasePage: async () => {},
    downloadUpdate: async () => {},
    cancelUpdate: async () => {},
    onUpdateProgress: () => {},
    applyUpdate: async () => {},
  };
  window.api = api;

  // Going to another app or tab: save now, as the phone app does. Coming back: pick up
  // anything saved from another device in the meantime.
  document.addEventListener('visibilitychange', async () => {
    if (document.visibilityState === 'hidden') {
      window.dispatchEvent(new Event('beforeunload'));
      return;
    }
    try {
      const now = await json('/api/rev');
      if (rev !== null && now.rev !== rev && inFlight === 0) reloadForNewer();
    } catch (e) { /* offline for a moment */ }
  });
})();
