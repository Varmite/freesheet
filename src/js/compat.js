// FreeSheet on old Android WebViews. Loaded first, and written in old-style JavaScript so
// that it runs even where the rest of the app cannot.
//
// On Android the app is drawn by the phone's "Android System WebView" (Chrome on Android
// 7 to 9), which updates through the Play Store. A phone that has never updated it may
// have one from 2017. This file:
//   - fills in a few small things slightly old WebViews (and old browsers, on the website)
//     lack, so the app runs and looks right on them;
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

  // ---- gaps in flex layouts (Chromium before 84)
  // styles.css spaces things in rows and columns with `gap`. Older WebViews apply it only to
  // grids, so buttons, icons and labels end up touching. There, the same spacing is rebuilt
  // from the stylesheets as margins between the children. The rules go in a <style> ahead of
  // every stylesheet, so any margin the stylesheets set themselves still wins.
  function flexGapFallback() {
    var probe = document.createElement('div');
    probe.style.cssText = 'display:flex;flex-direction:column;row-gap:1px;position:absolute;visibility:hidden';
    probe.appendChild(document.createElement('div'));
    probe.appendChild(document.createElement('div'));
    document.body.appendChild(probe);
    var supported = probe.scrollHeight === 1;
    document.body.removeChild(probe);
    if (supported) return;

    document.documentElement.classList.add('no-flex-gap');
    var style = document.createElement('style');
    style.id = 'flex-gap-fallback';
    document.head.insertBefore(style, document.head.firstChild);

    function build() {
      var known = {};    // last part of a selector -> what the stylesheets say about it
      var found = [];    // rules that set a gap or change a flex container's direction
      function walk(rules, media) {
        for (var i = 0; i < rules.length; i++) {
          var r = rules[i];
          if (r.type === 4) { walk(r.cssRules, media.concat(r.media.mediaText)); continue; }
          if (r.type !== 1 || !r.style) continue;
          var st = r.style, f = { display: st.display, dir: st.flexDirection, wrap: st.flexWrap, col: st.columnGap, row: st.rowGap };
          if (!f.display && !f.dir && !f.wrap && !f.col && !f.row) continue;
          var sels = r.selectorText.split(',');
          for (var j = 0; j < sels.length; j++) {
            var sel = sels[j].trim(), key = sel.split(/[\s>+~]+/).pop();
            var k = known[key] || (known[key] = {});
            for (var name in f) if (f[name] && (!media.length || !k[name])) k[name] = f[name];
            if (f.dir || f.wrap || f.col || f.row) found.push({ sel: sel, key: key, media: media, f: f });
          }
        }
      }
      for (var s = 0; s < document.styleSheets.length; s++) {
        var sheet = document.styleSheets[s];
        if (sheet.ownerNode === style) continue;
        try { walk(sheet.cssRules, []); } catch (e) { /* a sheet that cannot be read */ }
      }
      var size = function (v) { return !v || v === 'normal' ? '0px' : v; };
      var css = [];
      for (var n = 0; n < found.length; n++) {
        var t = found[n], k2 = known[t.key] || {};
        if (k2.display !== 'flex' && k2.display !== 'inline-flex') continue;
        var dir = t.f.dir || k2.dir || 'row', wrap = t.f.wrap || k2.wrap || 'nowrap';
        var col = size(t.f.col || k2.col), row = size(t.f.row || k2.row);
        if (col === '0px' && row === '0px' && !t.f.col && !t.f.row) continue;
        var rule;
        if (wrap !== 'nowrap') rule = t.sel + ' > * { margin-right: ' + col + '; margin-bottom: ' + row + '; }';
        else if (dir.indexOf('column') === 0) rule = t.sel + ' > * + * { margin-' + (dir === 'column' ? 'top' : 'bottom') + ': ' + row + '; margin-left: 0; }';
        else rule = t.sel + ' > * + * { margin-' + (dir === 'row' ? 'left' : 'right') + ': ' + col + '; margin-top: 0; }';
        for (var m = t.media.length - 1; m >= 0; m--) rule = '@media ' + t.media[m] + ' { ' + rule + ' }';
        css.push(rule);
      }
      style.textContent = css.join('\n');
    }
    build();
    // Stylesheets that arrive later (mobile.css is added by js/api.js).
    document.addEventListener('load', function (e) {
      if (e.target && e.target.tagName === 'LINK') build();
    }, true);
    window.addEventListener('load', build);
  }
  if (document.body) flexGapFallback(); else document.addEventListener('DOMContentLoaded', flexGapFallback);

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
