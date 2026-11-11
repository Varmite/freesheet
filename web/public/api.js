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
    // The app's Settings shows "Signed in <email>" with a Sign out button; here it's the username.
    account: { signedIn: true, email: account.username || '', googleAvailable: true },
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
        if (r.account) api.account.email = r.account.username;
        return r.progress;
      });
    },
    saveProgress: function (data) { return save(data, false); },
    // Called as the page closes or is hidden: start the save; keepalive carries it.
    saveProgressSync: function (data) { save(data, true).catch(function () {}); return true; },
    progressLocation: function () {
      return Promise.resolve('Your progress is saved to your account (' + (api.account.email || 'signed in') + ') on this FreeSheet server, so it follows you to any phone or computer.');
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

  // ---------- "FreeSheet was updated"
  // The server sets __FREESHEET_UPDATED__ (in /boot.js) the first time a learner opens
  // the site after it updated itself. Show a small note for a few seconds.
  function showUpdatedNote() {
    var style = document.createElement('style');
    style.textContent =
      '.web-updated{position:fixed;left:50%;top:calc(64px + env(safe-area-inset-top, 0px));z-index:10000;' +
      'display:flex;align-items:center;-webkit-transform:translate(-50%,-12px);transform:translate(-50%,-12px);opacity:0;' +
      'max-width:calc(100vw - 1.5rem);padding:0.45rem 0.45rem 0.45rem 0.9rem;border-radius:999px;' +
      'background:var(--bg);color:var(--text);border:1px solid var(--border-strong);' +
      'box-shadow:0 8px 24px rgba(0,0,0,0.16);font-size:0.9rem;line-height:1.3;' +
      'transition:opacity .25s ease,transform .25s ease}' +
      '.web-updated.show{opacity:1;-webkit-transform:translate(-50%,0);transform:translate(-50%,0)}' +
      '.web-updated b{font-weight:650;margin-right:0.3rem}' +
      '.web-updated button{font:inherit;font-size:1.05rem;line-height:1;margin-left:0.5rem;width:1.8rem;height:1.8rem;' +
      'border:0;border-radius:50%;background:none;color:var(--muted);cursor:pointer;flex:none}';
    document.head.appendChild(style);
    var note = document.createElement('div');
    note.className = 'web-updated';
    note.setAttribute('role', 'status');
    note.innerHTML = '<span><b>FreeSheet was updated.</b>You’re on the newest version.</span>' +
      '<button type="button" aria-label="Close">×</button>';
    document.body.appendChild(note);
    var timer;
    var close = function () {
      clearTimeout(timer);
      note.classList.remove('show');
      setTimeout(function () { note.remove(); style.remove(); }, 300);
    };
    note.querySelector('button').addEventListener('click', close);
    setTimeout(function () { note.classList.add('show'); }, 600);
    timer = setTimeout(close, 7000);
  }
  if (window.__FREESHEET_UPDATED__) {
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', showUpdatedNote);
    else showUpdatedNote();
  }

  // ---------- the phone's status bar matches the theme
  var themeMeta = null;
  function syncThemeColor() {
    if (!document.body) return;
    themeMeta = themeMeta || document.querySelector('meta[name="theme-color"]');
    var bg = getComputedStyle(document.body).backgroundColor;
    if (themeMeta && bg && themeMeta.content !== bg) themeMeta.content = bg;
  }
  document.addEventListener('DOMContentLoaded', function () {
    syncThemeColor();
    new MutationObserver(function () { setTimeout(syncThemeColor, 50); })
      .observe(document.documentElement, { attributes: true, attributeFilter: ['class', 'style', 'data-theme'] });
    if (window.matchMedia) {
      var mq = window.matchMedia('(prefers-color-scheme: dark)');
      if (mq.addEventListener) mq.addEventListener('change', syncThemeColor);
    }
  });

  // "Someone is using FreeSheet": the server only installs updates when no page has
  // pinged it for a while (web/lib/updater.js).
  setInterval(function () {
    if (document.visibilityState === 'visible') fetch('/api/ping', { credentials: 'same-origin' }).catch(function () {});
  }, 2 * 60e3);
})();
