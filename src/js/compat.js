// FreeSheet on old Android WebViews. Loaded first, and written in old-style JavaScript so
// that it runs even where the rest of the app cannot.
//
// On Android the app is drawn by the phone's "Android System WebView" (Chrome on Android
// 7 to 9), which updates through the Play Store. A phone that has never updated it may
// have one from 2017. This file:
//   - fills in a few small things slightly old WebViews lack, so the app runs on them;
//   - where the WebView is too old to run the app at all, shows how to update it instead
//     of leaving a blank screen.
(function () {
  'use strict';

  // ---- small fills (Chromium 64 to 68)
  if (!Array.prototype.flat) {
    Object.defineProperty(Array.prototype, 'flat', {
      configurable: true, writable: true,
      value: function (depth) {
        var d = depth === undefined ? 1 : Math.floor(depth), out = [];
        (function add(list, left) {
          for (var i = 0; i < list.length; i++) {
            if (!(i in list)) continue;
            if (Array.isArray(list[i]) && left > 0) add(list[i], left - 1); else out.push(list[i]);
          }
        })(this, d);
        return out;
      },
    });
  }
  if (!Array.prototype.flatMap) {
    Object.defineProperty(Array.prototype, 'flatMap', {
      configurable: true, writable: true,
      value: function (fn, self) { return Array.prototype.map.call(this, fn, self).flat(); },
    });
  }

  // Only inside the Android app (MainActivity.kt provides FreeSheetAndroid), not the website.
  if (!window.FreeSheetAndroid) return;

  // ---- too old to run at all?
  function capable() {
    try {
      // The bridge to the Rust side, and the newest things the app's own code relies on.
      if (!window.__TAURI__ || !window.__TAURI__.core) return false;
      if (!window.Promise || !Promise.prototype.finally || !window.ResizeObserver) return false;
      if (!(window.CSS && CSS.supports && CSS.supports('display', 'grid') && CSS.supports('color', 'var(--x)'))) return false;
      new RegExp('(?<=a)\\p{L}', 'u');
      return true;
    } catch (e) {
      return false;
    }
  }
  if (capable()) return;

  function notice() {
    var dark = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
    var box = document.createElement('div');
    box.id = 'compat-notice';
    box.setAttribute('role', 'alert');
    box.style.cssText = 'position:fixed;top:0;left:0;right:0;bottom:0;z-index:99999;overflow:auto;padding:32px 24px;' +
      'font:17px/1.5 sans-serif;background:' + (dark ? '#1a1b1e' : '#faf8f4') + ';color:' + (dark ? '#dcdcd8' : '#2c2a26');
    var h = document.createElement('h1');
    h.textContent = 'FreeSheet needs one update first';
    h.style.cssText = 'font-size:22px;margin:0 0 12px';
    var p1 = document.createElement('p');
    p1.textContent = 'This phone’s “Android System WebView” is too old to show FreeSheet. It is a free part of Android that apps use to draw their screens.';
    var p2 = document.createElement('p');
    p2.textContent = 'Open the Play Store, update “Android System WebView” (on Android 7, 8 or 9: update “Chrome”), then open FreeSheet again. Your progress is safe.';
    box.appendChild(h); box.appendChild(p1); box.appendChild(p2);
    if (window.FreeSheetAndroid && window.FreeSheetAndroid.openWebViewUpdate) {
      var b = document.createElement('button');
      b.textContent = 'Open the Play Store';
      b.style.cssText = 'font:inherit;font-weight:600;margin-top:8px;padding:12px 20px;border:0;border-radius:10px;background:#1f5fcc;color:#fff';
      b.onclick = function () { window.FreeSheetAndroid.openWebViewUpdate(); };
      box.appendChild(b);
    }
    document.body.appendChild(box);
  }
  if (document.body) notice(); else document.addEventListener('DOMContentLoaded', notice);
})();
