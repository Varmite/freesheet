// FreeSheet web edition: the same window.api the desktop app gets from src/js/api.js, but
// talking to the web server (web/lib/app.js) with fetch() instead of Tauri.
// The server sends this file in place of js/api.js.
(function () {
  'use strict';

  // Safari has no requestIdleCallback.
  if (!window.requestIdleCallback) window.requestIdleCallback = function (fn) { return setTimeout(fn, 1); };

  var account = window.__FREESHEET_ACCOUNT__ || {};
  var id = encodeURIComponent;

  // Signed out (session expired, or signed out on another tab): back to the login page.
  function toLogin() { location.href = '/login'; }

  function get(url) {
    return fetch(url, { credentials: 'same-origin' }).then(function (r) {
      if (r.status === 401) { toLogin(); throw new Error('Signed out'); }
      if (!r.ok) throw new Error('Could not load ' + url + ' (' + r.status + ')');
      return r;
    });
  }
  function json(url) { return get(url).then(function (r) { return r.json(); }); }
  // Audio arrives as raw bytes, as a Uint8Array like on the desktop.
  function bytes(url) { return get(url).then(function (r) { return r.arrayBuffer(); }).then(function (b) { return new Uint8Array(b); }); }

  // ---------- saving
  // Saves go one at a time, each naming the version it replaces (rev), so a page left
  // open on another phone can't overwrite a newer save: the server answers 409 and the
  // page reloads to pick up the newer progress.
  var rev = null;
  var reloading = false;
  var queue = Promise.resolve();

  function save(data, closing) {
    if (reloading) return Promise.resolve(true);
    var body = JSON.stringify(data);
    var run = function () {
      // keepalive lets the last save finish while the page closes (browsers allow it
      // for bodies under 64 KB).
      return fetch('/api/progress', {
        method: 'PUT',
        credentials: 'same-origin',
        headers: { 'Content-Type': 'application/json', 'X-Rev': rev == null ? '' : String(rev) },
        body: body,
        keepalive: !!closing && body.length < 60000,
      }).then(function (r) {
        if (r.status === 401) { toLogin(); throw new Error('Signed out'); }
        if (r.status === 409) { reloading = true; location.reload(); return true; }
        if (!r.ok) throw new Error('Could not save (' + r.status + ')');
        return r.json().then(function (v) { rev = v.rev; return true; });
      });
    };
    var result = queue.then(run, run);
    queue = result.catch(function () {});
    return result;
  }

  function download(name, text) {
    var url = URL.createObjectURL(new Blob([text], { type: 'application/json' }));
    var a = document.createElement('a');
    a.href = url;
    a.download = name;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(function () { URL.revokeObjectURL(url); }, 10000);
  }

  // Settings > Import progress: pick a progress.json (exported from the desktop or phone
  // app); the app then saves it to this account.
  function pickFile() {
    return new Promise(function (resolve) {
      var input = document.createElement('input');
      input.type = 'file';
      input.accept = '.json,application/json';
      input.style.display = 'none';
      document.body.appendChild(input);
      var finish = function (v) { input.remove(); resolve(v); };
      input.addEventListener('cancel', function () { finish({ ok: false }); });
      input.addEventListener('change', function () {
        var file = input.files[0];
        if (!file) return finish({ ok: false });
        file.text().then(function (text) {
          var v = JSON.parse(text.replace(/^﻿/, ''));
          if (!v || typeof v !== 'object' || !v.topics || !v.exams) return finish({ ok: false, error: 'That file is not a FreeSheet progress file.' });
          delete v._rev;
          finish({ ok: true, data: Object.assign({ version: 1, settings: {}, topics: {}, exams: {}, examDrafts: {} }, v) });
        }).catch(function () { finish({ ok: false, error: 'That file could not be read.' }); });
      });
      input.click();
    });
  }

  var noop = function () { return Promise.resolve(); };

  var api = {
    web: true,
    platform: 'web',
    deviceName: 'your account',
    account: { signedIn: true, email: account.email || '', googleAvailable: true },
    getCatalog: function () { return json('/api/catalog'); },
    contentWords: function () { return json('/api/words'); },
    getCourse: function (c) { return json('/api/course/' + id(c)); },
    courseAudioList: function (c) { return json('/api/course/' + id(c) + '/audio'); },
    courseAudio: function (c, a) { return bytes('/api/course/' + id(c) + '/audio/' + id(a)); },
    getTopic: function (g, s, t) { return json('/api/topic/' + id(g) + '/' + id(s) + '/' + id(t)); },
    getExam: function (g, s, x) { return json('/api/exam/' + id(g) + '/' + id(s) + '/' + id(x)); },
    listAudio: function (g, s, t) { return json('/api/audio/' + id(g) + '/' + id(s) + '/' + id(t)); },
    getAudio: function (g, s, t, a) { return bytes('/api/audio/' + id(g) + '/' + id(s) + '/' + id(t) + '/' + id(a)); },
    loadProgress: function () {
      return json('/api/progress').then(function (r) {
        rev = r.rev;
        if (r.account) api.account.email = r.account.email;
        return r.progress;
      });
    },
    saveProgress: function (data) { return save(data, false); },
    // Called as the page closes or is hidden: start the save; keepalive carries it.
    saveProgressSync: function (data) { save(data, true).catch(function () {}); return true; },
    progressLocation: function () {
      return Promise.resolve('Your progress is saved to your Google account (' + (api.account.email || 'signed in') + ') on this FreeSheet server, so it follows you to any phone or computer.');
    },
    exportProgress: function (data) {
      download('freesheet-progress-' + new Date().toISOString().slice(0, 10) + '.json', JSON.stringify(data, null, 2));
      return Promise.resolve({ ok: true });
    },
    importProgress: pickFile,
    signIn: toLogin,
    signOut: function () {
      reloading = true;
      return fetch('/auth/logout', { method: 'POST', credentials: 'same-origin' }).then(toLogin, toLogin);
    },
    // Desktop-only features: harmless stand-ins.
    openDataFolder: noop,
    version: function () { return Promise.resolve(window.__FREESHEET_VERSION__ || ''); },
    smoothTextActive: function () { return Promise.resolve(true); },
    relaunch: function () { location.reload(); return Promise.resolve(); },
    openReleasePage: noop,
    downloadUpdate: noop,
    cancelUpdate: noop,
    onUpdateProgress: function () {},
    applyUpdate: noop,
  };
  window.api = api;

  // Switching to another app or tab, or locking the phone: save now. (On phones the
  // page is often never "closed", so beforeunload alone is not enough.) The app's
  // beforeunload handler saves through saveProgressSync.
  document.addEventListener('visibilitychange', function () {
    if (document.visibilityState === 'hidden') window.dispatchEvent(new Event('beforeunload'));
  });
  window.addEventListener('pagehide', function () { window.dispatchEvent(new Event('beforeunload')); });

  // "Someone is using FreeSheet": the server only installs updates when no page has
  // pinged it for a while (web/lib/updater.js).
  setInterval(function () {
    if (document.visibilityState === 'visible') fetch('/api/ping', { credentials: 'same-origin' }).catch(function () {});
  }, 2 * 60e3);
})();
