// FreeSheet motion, modelled on iOS.
//
// Springs: every preset is a real damped spring sampled into a CSS linear() easing
// (also exposed to CSS as --spring / --bounce for transitions), so motion settles
// the way iOS motion does.
//
// Page changes:
//   - tapping a card or row grows it into the new page while the old one sinks back
//     and dims; going back shrinks the page into the card it came from
//   - other forward/back moves slide like a navigation stack (also the topic pager)
//   - tab switches and in-place changes cross-fade; the new page's cards settle in,
//     progress bars fill and numbers count up
// In-page: segmented controls have a sliding thumb, answers bounce (right) or shake
// (wrong), selections pop, filtered lists reflow, disclosures and new content ease in.
//
// Smoothness (measured with Chromium's frame tracing at 180 Hz):
//   - Moving things animate only transform and opacity, which the compositor runs
//     at the display's full refresh rate. Anything it can't animate (corner clip,
//     shadow) is kept out of those animations, because one such property in a
//     keyframe drags the whole animation onto the main thread.
//   - Pages only ever scale uniformly; scaling them non-uniformly made Chromium
//     re-rasterise them every few frames. The zoom's rounded window is a clip on an
//     empty container instead.
//   - main keeps its own compositor layer (CSS), so animations ending on it don't
//     force a full repaint, and frozen page copies skip layout for what's off screen.
//   - Motion blur is kept to small moving things; blurring whole pages costs frames.
//
// Turned off by the "Animations" setting or by the OS reduced-motion preference.
(function () {
  'use strict';

  const reduce = window.matchMedia('(prefers-reduced-motion: reduce)');
  let wanted = true;

  function enabled() { return wanted && !reduce.matches; }
  function sync() { document.documentElement.classList.toggle('motion', enabled()); }
  function setEnabled(v) { wanted = v !== false; sync(); }
  reduce.addEventListener('change', sync);
  sync();

  // ---------------------------------------------------------------- springs

  // A damped spring (response = period in seconds, damping ratio < 1), sampled
  // until it settles. at(u) gives the spring's value at fraction u of the duration.
  function spring(response, damping) {
    const w = (2 * Math.PI) / response, z = damping, wd = w * Math.sqrt(1 - z * z);
    const x = (t) => 1 - Math.exp(-z * w * t) * (Math.cos(wd * t) + ((z * w) / wd) * Math.sin(wd * t));
    let T = 0.15;
    for (; T < 3; T += 0.02) {
      let settled = true;
      for (let t = T; t < T + 0.25; t += 0.01) if (Math.abs(1 - x(t)) > 0.0015) { settled = false; break; }
      if (settled) break;
    }
    const N = 60, pts = [];
    for (let i = 0; i <= N; i++) pts.push(i === 0 ? 0 : i === N ? 1 : +x((T * i) / N).toFixed(4));
    return { easing: `linear(${pts.join(', ')})`, duration: Math.round(T * 1000), at: (u) => (u <= 0 ? 0 : u >= 1 ? 1 : x(u * T)) };
  }

  const ZOOM = spring(0.5, 0.88);    // card opening / closing
  const PUSH = spring(0.44, 0.985);  // navigation slide, no overshoot
  const SNAPPY = spring(0.34, 0.84); // small things: reveals, thumbs, reflow
  const BOUNCY = spring(0.4, 0.62);  // pops and right answers

  const rootStyle = document.documentElement.style;
  rootStyle.setProperty('--spring', SNAPPY.easing);
  rootStyle.setProperty('--spring-d', SNAPPY.duration + 'ms');
  rootStyle.setProperty('--bounce', BOUNCY.easing);
  rootStyle.setProperty('--bounce-d', BOUNCY.duration + 'ms');

  // Note: an animation that includes any property the compositor cannot animate
  // (border-radius, box-shadow, ...) runs entirely on the main thread, transform
  // and all. Keep those in separate animations (opts.keep adds one without
  // cancelling the element's others).
  function play(el, frames, opts) {
    if (!el || !enabled() || !el.animate) return null;
    if (!(opts && opts.keep)) for (const a of el.getAnimations()) if (!(a instanceof CSSTransition) && !(a instanceof CSSAnimation)) a.cancel();
    el.style.willChange = 'transform, opacity';
    const { keep, ...rest } = opts || {};
    const a = el.animate(frames, { easing: SNAPPY.easing, duration: SNAPPY.duration, ...rest });
    const done = () => { if (!el.getAnimations().length) el.style.willChange = ''; };
    a.finished.then(done, done);
    return a;
  }
  const sp = (s, extra) => ({ easing: s.easing, duration: s.duration, ...extra });

  // ---------------------------------------------------------------- page layers

  function topEdge() {
    const bar = document.querySelector('.topbar');
    return bar ? bar.getBoundingClientRect().bottom : 0;
  }
  function visibleBox() {
    const top = topEdge();
    return { left: 0, top, width: document.documentElement.clientWidth, height: window.innerHeight - top };
  }

  // Off-screen parts of a frozen copy keep their size but skip layout and paint,
  // so freezing a long page is cheap.
  const NO_PRUNE = /^(TR|TD|TH|TBODY|THEAD|TFOOT|CAPTION|COLGROUP|COL)$/;
  function prune(o, c, lo, hi, depth) {
    const oc = o.children, cc = c.children;
    for (let i = 0; i < oc.length && i < cc.length; i++) {
      const oe = oc[i];
      if (NO_PRUNE.test(oe.tagName)) continue;
      const r = oe.getBoundingClientRect();
      if (r.top > hi || r.bottom < lo) {
        if (!oe.offsetHeight) continue;
        const cs = getComputedStyle(oe);
        if (cs.display === 'inline' || cs.display === 'contents' || cs.display.startsWith('table')) continue;
        const px = (p) => parseFloat(cs[p]) || 0;
        const h = oe.offsetHeight - px('paddingTop') - px('paddingBottom') - px('borderTopWidth') - px('borderBottomWidth');
        const w = oe.offsetWidth - px('paddingLeft') - px('paddingRight') - px('borderLeftWidth') - px('borderRightWidth');
        cc[i].style.contentVisibility = 'hidden';
        cc[i].style.containIntrinsicSize = `${Math.max(0, w)}px ${Math.max(0, h)}px`;
      } else if (depth < 6 && oe.children.length > 1) prune(oe, cc[i], lo, hi, depth + 1);
    }
  }

  // A frozen copy of `src` in a fixed box covering the visible area under the top bar.
  function makeLayer(src, box) {
    const r = src.getBoundingClientRect();
    const layer = document.createElement('div');
    layer.className = 'nav-layer';
    layer.setAttribute('aria-hidden', 'true');
    layer.inert = true;
    layer.style.cssText = `left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px`;
    const copy = src.cloneNode(true);
    copy.removeAttribute('id');
    copy.classList.remove('nav-front');
    for (const e of copy.querySelectorAll('[id]')) e.removeAttribute('id');
    const ox = r.left - box.left, oy = r.top - box.top;
    copy.style.cssText = `position:absolute;left:${ox}px;top:${oy}px;width:${r.width}px;margin:0;transform-origin:${-ox}px ${-oy}px`;
    prune(src, copy, box.top - 80, box.top + box.height + 80, 0);
    const scrim = document.createElement('div');
    scrim.className = 'nav-scrim';
    layer.append(copy, scrim);
    document.body.appendChild(layer);
    return { layer, copy, scrim, box };
  }

  function fixedScrim(z) {
    const s = document.createElement('div');
    s.className = 'nav-scrim fixed';
    s.style.zIndex = z;
    document.body.appendChild(s);
    return s;
  }

  // ---------------------------------------------------------------- page transitions

  let origin = null;          // the card/row just tapped
  let hint = null;            // 'forward' | 'back' for same-level moves (pager)
  const trail = new Map();    // page key -> selector of the card that was opened from it
  let active = null;          // cleanup for the transition in progress

  function selectorFor(el) {
    const parts = Object.entries(el.dataset).map(([k, v]) =>
      `[data-${k.replace(/[A-Z]/g, (c) => '-' + c.toLowerCase())}="${CSS.escape(v)}"]`);
    if (parts.length) return '.' + CSS.escape(el.classList[0]) + parts.join('');
    return el.id ? '#' + CSS.escape(el.id) : null;
  }

  // Called on tap (capture phase), before the page changes.
  function noteOrigin(el) {
    origin = { el, rect: el.getBoundingClientRect(), radius: parseFloat(getComputedStyle(el).borderTopLeftRadius) || 16, selector: selectorFor(el), at: performance.now() };
  }
  function takeOrigin() {
    const o = origin;
    origin = null;
    return o && performance.now() - o.at < 2000 && o.rect.width > 0 ? o : null;
  }
  function setHint(dir) { hint = { dir, at: performance.now() }; }

  function finishActive() { if (active) { const f = active; active = null; f(); } }

  // Freeze the current page before it is replaced. It covers the new page while
  // that renders, then takes part in the transition.
  function capture(main) {
    finishActive();
    return enabled() ? makeLayer(main, visibleBox()) : null;
  }
  function discard(tr) { if (tr) tr.layer.remove(); }

  // kind: 'push' | 'pop' | 'tab' | 'swap' | 'first'
  function navigate(main, tr, { kind, fromKey, toKey }) {
    if (!tr) finishActive(); // (with tr, capture() already did this)
    const o = takeOrigin();
    const h = hint && performance.now() - hint.at < 2000 ? hint.dir : null;
    hint = null;
    if (!enabled()) { discard(tr); return; }
    const shift = thumbsFrom(main, tr);
    if (!tr) return run(main, null, fade(main, null, true, 0));
    if (kind === 'push' && o) {
      if (o.selector) trail.set(fromKey, o.selector);
      return run(main, tr, zoomOpen(main, tr, o));
    }
    if (kind === 'pop') {
      const sel = trail.get(toKey);
      const card = sel && main.querySelector(sel);
      if (card) {
        const r = card.getBoundingClientRect(), box = visibleBox();
        if (r.width && r.top >= box.top && r.bottom <= box.top + box.height) return run(main, tr, zoomClose(main, tr, card, r));
      }
      return run(main, tr, slide(main, tr, false));
    }
    if (kind === 'push') return run(main, tr, slide(main, tr, true));
    if (h) return run(main, tr, slide(main, tr, h === 'forward'));
    return run(main, tr, fade(main, tr, kind !== 'swap', shift));
  }

  // When the transition's main animation ends (or the next navigation starts),
  // remove the frozen pages and helpers.
  function run(main, tr, t) {
    const extras = t.extras || [];
    const cleanup = () => {
      for (const a of main.getAnimations()) if (!(a instanceof CSSTransition)) a.cancel();
      main.classList.remove('nav-front');
      main.style.visibility = '';
      main.style.transformOrigin = '';
      main.style.boxShadow = '';
      if (tr) tr.layer.remove();
      for (const e of extras) e.remove();
    };
    active = cleanup;
    const finish = () => {
      if (active !== cleanup) return;
      active = null;
      cleanup();
      if (t.after) t.after();
    };
    if (t.anim) t.anim.finished.then(finish, finish); else finish();
    // Belt and braces: even if the animation never reports finishing, its layers go.
    setTimeout(finish, 2500);
  }

  // Animate a layer's visible window from rect `from` to rect `to` (viewport
  // coordinates) with corner radius rFrom -> rTo, the page inside scaling with it.
  // The page copy only ever scales uniformly (a compositor transform, rasterised
  // once). The rounded window is a clip on the layer, which holds no content of
  // its own, so updating it each frame is cheap. Scaling the page non-uniformly
  // and counter-scaling it instead made Chromium re-rasterise it as it grew.
  // Frames are sampled from the spring.
  function morph(t, from, to, rFrom, rTo, s, opacityAt) {
    const box = t.box, N = 48, move = [], clip = [], fades = [];
    for (let i = 0; i <= N; i++) {
      const u = i / N, p = s.at(u);
      const x = from.left + (to.left - from.left) * p;
      const y = from.top + (to.top - from.top) * p;
      const w = Math.max(1, from.width + (to.width - from.width) * p);
      const h = Math.max(1, from.height + (to.height - from.height) * p);
      const k = w / box.width;
      const rad = Math.max(0, rFrom + (rTo - rFrom) * Math.min(1, p));
      move.push({ offset: u, transform: `translate(${x - box.left}px, ${y - box.top}px) scale(${k})` });
      const top = y - box.top, left = x - box.left;
      clip.push({ offset: u, clipPath: `inset(${top}px ${box.width - left - w}px ${box.height - top - h}px ${left}px round ${rad}px)` });
      if (opacityAt) fades.push({ offset: u, opacity: opacityAt(u) });
    }
    const a = play(t.copy, move, { duration: s.duration, easing: 'linear' });
    play(t.layer, clip, { duration: s.duration, easing: 'linear' });
    if (opacityAt) play(t.layer, fades, { duration: s.duration, easing: 'linear', keep: true });
    return a;
  }

  // The tapped card grows to become the new page.
  function zoomOpen(main, tr, o) {
    const box = tr.box, c = o.rect;
    if (o.selector) { const twin = tr.copy.querySelector(o.selector); if (twin) twin.style.visibility = 'hidden'; }
    // A copy of the new page grows out of the card. The real page stays rendered
    // underneath the opaque old-page layer, so it is ready the moment the animation
    // ends. (Animating the real page with a clip instead was measured: it saves a
    // little on the first open but drops frames mid-animation on every open.)
    tr.layer.style.zIndex = '15';
    const fresh = makeLayer(main, box);
    fresh.layer.style.zIndex = '16';
    const anim = morph(fresh, c, box, o.radius, 0, ZOOM);

    // The page behind sinks back and dims, as the home screen does on iOS. Only the
    // copy inside the layer scales, so the layer itself keeps covering the real page.
    const cx = tr.copy.style.left, cy = tr.copy.style.top;
    tr.copy.style.transformOrigin = `${box.width / 2 - parseFloat(cx)}px ${box.height / 2 - parseFloat(cy)}px`;
    play(tr.copy, [{ transform: 'scale(1)' }, { transform: 'scale(0.92)' }], sp(ZOOM));
    play(tr.scrim, [{ opacity: 0 }, { opacity: 1 }], sp(ZOOM));

    // A copy of the card rides along on top, fading as the page shows through.
    // It is small, so it can afford a touch of motion blur.
    const ghost = o.el.cloneNode(true);
    ghost.removeAttribute('id');
    ghost.classList.add('nav-ghost');
    ghost.style.cssText = `position:fixed;left:${c.left}px;top:${c.top}px;width:${c.width}px;height:${c.height}px;margin:0;transform-origin:0 0`;
    document.body.appendChild(ghost);
    const gf = [];
    for (let i = 0; i <= 24; i++) {
      const u = i / 24, p = ZOOM.at(u);
      const x = c.left + (box.left - c.left) * p, y = c.top + (box.top - c.top) * p;
      const k = (c.width + (box.width - c.width) * p) / c.width;
      gf.push({ offset: u, transform: `translate(${x - c.left}px, ${y - c.top}px) scale(${k})`, opacity: Math.max(0, 1 - u / 0.22), filter: `blur(${(Math.max(0, 1 - u / 0.22) * 1.5).toFixed(2)}px)` });
    }
    play(ghost, gf, { duration: ZOOM.duration, easing: 'linear' });
    return { anim, extras: [fresh.layer, ghost] };
  }

  // Going back: the page shrinks into the card it was opened from.
  function zoomClose(main, tr, card, c) {
    const box = tr.box;
    const radius = parseFloat(getComputedStyle(card).borderTopLeftRadius) || 16;
    tr.layer.style.zIndex = '17';
    const anim = morph(tr, box, c, 0, radius, ZOOM, (u) => (u < 0.45 ? 1 : Math.max(0, 1 - (u - 0.45) / 0.3)));
    const scrim = fixedScrim('16');
    play(scrim, [{ opacity: 1 }, { opacity: 0 }], sp(ZOOM));
    const m = main.getBoundingClientRect();
    main.style.transformOrigin = `${box.width / 2 - m.left}px ${box.top + box.height / 2 - m.top}px`;
    play(main, [{ transform: 'scale(0.92)' }, { transform: 'scale(1)' }], sp(ZOOM));
    return { anim, extras: [scrim] };
  }

  // Navigation-stack slide: forward comes in from the right over the old page,
  // back uncovers the parent page as the current one slides away.
  function slide(main, tr, forward) {
    // The shadow is a static style, not a keyframe, so the slide stays on the compositor.
    const shadow = '-14px 0 36px rgba(0, 0, 0, 0.16)';
    let anim;
    if (forward) {
      tr.layer.style.zIndex = '15';
      main.classList.add('nav-front');
      main.style.boxShadow = shadow;
      anim = play(main, [{ transform: 'translate3d(100%, 0, 0)' }, { transform: 'translate3d(0, 0, 0)' }], sp(PUSH));
      play(tr.layer, [{ transform: 'translate3d(0, 0, 0)' }, { transform: 'translate3d(-25%, 0, 0)' }], sp(PUSH));
      play(tr.scrim, [{ opacity: 0 }, { opacity: 1 }], sp(PUSH));
      return { anim, after: () => enterDetails(main) };
    }
    tr.layer.style.zIndex = '17';
    tr.layer.style.boxShadow = shadow;
    anim = play(tr.layer, [{ transform: 'translate3d(0, 0, 0)' }, { transform: 'translate3d(100%, 0, 0)' }], sp(PUSH));
    const scrim = fixedScrim('16');
    play(scrim, [{ opacity: 1 }, { opacity: 0 }], sp(PUSH));
    play(main, [{ transform: 'translate3d(-25%, 0, 0)' }, { transform: 'translate3d(0, 0, 0)' }], sp(PUSH));
    return { anim, extras: [scrim] };
  }

  // Tab switches and in-place changes: a quick cross-fade, nudged sideways when a
  // segmented control moved, with the new page's cards settling in.
  function fade(main, tr, stagger, shift) {
    // The old page fades out quickly and the new one fades in just behind it, so
    // two pages of text are never both clearly visible at once.
    let anim = null;
    if (tr) {
      tr.layer.style.zIndex = '17';
      anim = play(tr.layer, [{ opacity: 1 }, { opacity: 0 }], { duration: 140, easing: 'cubic-bezier(0.4, 0, 1, 1)', fill: 'forwards' });
    }
    const a2 = play(main, [
      { transform: `translate3d(${shift * 28}px, ${shift ? 0 : 6}px, 0) scale(${shift ? 1 : 0.99})` },
      { transform: 'translate3d(0, 0, 0) scale(1)' },
    ], sp(ZOOM));
    play(main, [{ opacity: 0 }, { opacity: 1 }], { duration: 240, delay: tr ? 70 : 0, easing: 'cubic-bezier(0, 0, 0.2, 1)', fill: 'backwards', keep: true });
    if (stagger) staggerIn(main);
    enterDetails(main);
    return { anim: anim || a2 };
  }

  function staggerIn(root) {
    let i = 0;
    for (const it of root.querySelectorAll('.grid > .card, .rows > li:not([hidden]), .facts > div, .grade-buttons > .btn, .continue')) {
      if (i >= 16) break;
      play(it, [{ opacity: 0, transform: 'translate3d(0, 16px, 0) scale(0.97)' }, { opacity: 1, transform: 'translate3d(0, 0, 0) scale(1)' }],
        sp(SNAPPY, { delay: 20 + i * 22, fill: 'backwards' }));
      i++;
    }
  }

  // Progress bars fill and headline numbers count up.
  function enterDetails(root) {
    for (const b of root.querySelectorAll('.bar > span')) {
      b.style.transformOrigin = 'left center';
      play(b, [{ transform: 'scaleX(0)' }, { transform: 'scaleX(1)' }], sp(ZOOM, { delay: 120, fill: 'backwards' }));
    }
    countUp(root, 100);
  }

  const NUMBERS = '.facts strong, .head-stats strong, .result-big, .streak-num';
  function countUp(root, delay) {
    if (!enabled() || !root) return;
    for (const el of root.querySelectorAll(NUMBERS)) {
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      let tn;
      while ((tn = walker.nextNode()) && !/\d/.test(tn.data));
      if (!tn) continue;
      const m = /\d+(?![.,]\d)/.exec(tn.data);
      const n = m ? Number(m[0]) : 0;
      if (!n || n > 100000) continue;
      const before = tn.data.slice(0, m.index), after = tn.data.slice(m.index + m[0].length);
      const node = tn, start = performance.now() + (delay || 0), dur = Math.min(900, 450 + n * 6);
      node.data = before + '0' + after;
      const step = (now) => {
        if (!node.isConnected) return;
        const u = Math.min(1, Math.max(0, (now - start) / dur));
        node.data = before + Math.round(n * (1 - Math.pow(1 - u, 3))) + after;
        if (u < 1) requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
    }
  }

  // ---------------------------------------------------------------- sliding indicators

  // A highlight that springs between options: the top-bar tab pill and the thumb
  // of every segmented control.
  function indicator(container, cls) {
    let ind = container.querySelector(':scope > .' + cls);
    if (!ind) {
      ind = document.createElement('span');
      ind.className = cls;
      ind.setAttribute('aria-hidden', 'true');
      container.prepend(ind);
      container.classList.add('has-indicator');
      new ResizeObserver(() => { if (container.isConnected) placeIndicator(container, ind, false); }).observe(container);
    }
    return ind;
  }

  function placeIndicator(container, ind, animate, fromBtn) {
    const btn = container.querySelector(':scope > .active');
    if (!btn) { ind.style.opacity = '0'; return; }
    const to = `translate3d(${btn.offsetLeft}px, ${btn.offsetTop}px, 0)`;
    const from = fromBtn ? `translate3d(${fromBtn.offsetLeft}px, ${fromBtn.offsetTop}px, 0)` : ind.style.transform;
    const fromW = fromBtn ? fromBtn.offsetWidth : parseFloat(ind.style.width);
    ind.style.opacity = '1';
    ind.style.transform = to;
    ind.style.width = btn.offsetWidth + 'px';
    ind.style.height = btn.offsetHeight + 'px';
    if (animate && from && from !== to && enabled()) {
      // Width is animated as scaleX against the new width so the move stays on the compositor.
      const sx = (fromW || btn.offsetWidth) / btn.offsetWidth;
      ind.style.transformOrigin = 'left top';
      play(ind, [
        { transform: `${from} scaleX(${sx})`, filter: 'blur(0px)' },
        { filter: 'blur(0.8px)', offset: 0.25 },
        { transform: `${to} scaleX(1)`, filter: 'blur(0px)' },
      ], sp(SNAPPY));
    }
  }

  function moveTabPill(tabs, animate) {
    if (tabs) placeIndicator(tabs, indicator(tabs, 'tab-pill'), animate);
  }

  // Give every segmented control under root a thumb.
  function decorate(root) {
    for (const sc of root.querySelectorAll('.segmented')) placeIndicator(sc, indicator(sc, 'seg-thumb'), false);
  }

  // Select a segment in place: move .active and slide the thumb across.
  function selectSegment(btn) {
    const sc = btn.parentElement;
    const prev = sc.querySelector(':scope > .seg.active');
    if (prev === btn) return;
    if (prev) prev.classList.remove('active');
    btn.classList.add('active');
    placeIndicator(sc, indicator(sc, 'seg-thumb'), true, prev);
    press(btn);
  }

  // After a re-render, slide each segmented thumb from where the old page had it.
  // Returns -1/0/1: which way the selection moved, to nudge the page the same way.
  function thumbsFrom(main, tr) {
    if (!tr) return 0;
    const olds = tr.copy.querySelectorAll('.segmented'), news = main.querySelectorAll('.segmented');
    let shift = 0;
    news.forEach((sc, i) => {
      const old = olds[i];
      if (!old) return;
      const oi = [...old.querySelectorAll(':scope > .seg')].findIndex((b) => b.classList.contains('active'));
      const segs = [...sc.querySelectorAll(':scope > .seg')];
      const ni = segs.findIndex((b) => b.classList.contains('active'));
      if (oi < 0 || ni < 0 || oi === ni || segs.length !== old.querySelectorAll(':scope > .seg').length) return;
      placeIndicator(sc, indicator(sc, 'seg-thumb'), true, segs[oi]);
      if (!shift) shift = ni > oi ? 1 : -1;
    });
    return shift;
  }

  // ---------------------------------------------------------------- small things

  // Something new appearing inside the current page (feedback, a notes section).
  function reveal(el, delay) {
    return play(el, [
      { opacity: 0, transform: 'translate3d(0, 14px, 0) scale(0.98)', filter: 'blur(2px)' },
      { filter: 'blur(0px)', offset: 0.35 },
      { opacity: 1, transform: 'translate3d(0, 0, 0) scale(1)', filter: 'blur(0px)' },
    ], sp(SNAPPY, { delay: delay || 0, fill: 'backwards' }));
  }

  // A selection landing: a quick squeeze that springs back.
  function press(el) {
    return play(el, [{ transform: 'scale(0.92)' }, { transform: 'scale(1)' }], sp(BOUNCY));
  }

  function pop(el) {
    countUp(el, 80);
    return play(el, [{ transform: 'scale(0.85)', opacity: 0 }, { transform: 'scale(1)', opacity: 1 }], sp(BOUNCY));
  }

  // A checked answer: right answers bounce, wrong ones shake like a rejected passcode.
  function verdict(card) {
    if (!card || !enabled()) return;
    const flash = (cls) => { card.classList.remove('flash-good', 'flash-bad'); void card.offsetWidth; card.classList.add(cls); };
    if (card.classList.contains('is-correct')) {
      flash('flash-good');
      play(card, [{ transform: 'scale(0.97)' }, { transform: 'scale(1)' }], sp(BOUNCY));
    } else if (card.classList.contains('is-wrong')) {
      flash('flash-bad');
      play(card, [
        { transform: 'translateX(0)' }, { transform: 'translateX(-11px)', offset: 0.12 }, { transform: 'translateX(9px)', offset: 0.3 },
        { transform: 'translateX(-6px)', offset: 0.48 }, { transform: 'translateX(4px)', offset: 0.66 }, { transform: 'translateX(-2px)', offset: 0.82 },
        { transform: 'translateX(0)' },
      ], { duration: 480, easing: 'ease-out' });
    } else {
      play(card, [{ transform: 'scale(0.985)' }, { transform: 'scale(1)' }], sp(SNAPPY));
    }
  }

  // Run mutate() (which shows/hides items) and let the items that stay glide to
  // their new places while newly shown ones fade in.
  function flip(items, mutate) {
    if (!enabled()) { mutate(); return; }
    const before = new Map();
    for (const el of items) if (!el.hidden) before.set(el, el.getBoundingClientRect());
    mutate();
    const delta = new Map();
    for (const el of items) {
      if (el.hidden) continue;
      const b = before.get(el), r = el.getBoundingClientRect();
      delta.set(el, b ? [b.left - r.left, b.top - r.top] : null);
    }
    for (const [el, d] of delta) {
      if (!d) { play(el, [{ opacity: 0, transform: 'scale(0.97)' }, { opacity: 1, transform: 'scale(1)' }], sp(SNAPPY)); continue; }
      let [dx, dy] = d;
      // A parent that moves carries its children; only animate the difference.
      for (let p = el.parentElement; p; p = p.parentElement) {
        const pd = delta.get(p);
        if (pd) { dx -= pd[0]; dy -= pd[1]; break; }
      }
      if (Math.abs(dx) > 0.5 || Math.abs(dy) > 0.5) play(el, [{ transform: `translate(${dx}px, ${dy}px)` }, { transform: 'translate(0, 0)' }], sp(SNAPPY));
    }
  }

  // iOS alerts grow in from slightly larger than full size and fade out in place.
  function dialogIn(back) {
    if (!back) return;
    play(back, [{ opacity: 0 }, { opacity: 1 }], { duration: 220, easing: 'linear' });
    play(back.firstElementChild, [{ opacity: 0, transform: 'scale(1.15)' }, { opacity: 1, transform: 'scale(1)' }], sp(SNAPPY));
    // The streak dialog: days pop in one by one and the flame bounces.
    back.querySelectorAll('.streak-day').forEach((d, i) =>
      play(d, [{ opacity: 0, transform: 'scale(0.6)' }, { opacity: 1, transform: 'scale(1)' }], sp(BOUNCY, { delay: 120 + i * 45, fill: 'backwards' })));
    const flame = back.querySelector('.streak-hero-flame');
    if (flame) play(flame, [{ transform: 'scale(0.5) rotate(-12deg)' }, { transform: 'scale(1) rotate(0deg)' }], sp(BOUNCY, { delay: 80, fill: 'backwards' }));
    for (const b of back.querySelectorAll('.bar > span')) {
      b.style.transformOrigin = 'left center';
      play(b, [{ transform: 'scaleX(0)' }, { transform: 'scaleX(1)' }], sp(ZOOM, { delay: 200, fill: 'backwards' }));
    }
    countUp(back, 120);
  }

  // Resolves once the dialog has animated out (immediately when motion is off).
  function dialogOut(back) {
    if (!back || !enabled()) return Promise.resolve();
    back.style.pointerEvents = 'none';
    play(back.firstElementChild, [{ transform: 'scale(1)' }, { transform: 'scale(0.96)' }], { duration: 200, easing: 'ease-out', fill: 'forwards' });
    const a = play(back, [{ opacity: 1 }, { opacity: 0 }], { duration: 200, easing: 'ease-out', fill: 'forwards' });
    // A timer backs up "finished", which Chromium holds back while the window is hidden.
    return a ? Promise.race([a.finished.catch(() => {}), new Promise((r) => setTimeout(r, 260))]) : Promise.resolve();
  }

  // Disclosures (<details>) ease their content in when opened.
  document.addEventListener('toggle', (e) => {
    const d = e.target;
    if (d instanceof HTMLDetailsElement && d.open) {
      for (const c of d.children) if (c.tagName !== 'SUMMARY') reveal(c);
    }
  }, true);

  // ---------------------------------------------------------------- warm-up rehearsal
  //
  // Measured at 180 Hz: the first card zoom of a session drops ~35 frames, but once
  // any real zoom has run, later ones drop 1-2, whichever page they open. Showing
  // pages first doesn't help, and neither does imitating the zoom's effects; only
  // the real transition warms what it needs. So the app runs one real zoom in and
  // out when it is idle after starting (see rehearseTransitions in app.js), hidden
  // under an exact frozen copy of the page so nothing on screen changes. While
  // rehearsing, the transition's layers are lifted above that cover at 0.4%
  // opacity: still drawn, so the GPU does the work, but far too faint to see.

  // An opaque frozen copy of the page that sits over everything below the top bar
  // and takes the input while it is up.
  function cover(main) {
    const t = makeLayer(main, visibleBox());
    t.layer.classList.add('nav-cover');
    t.layer.inert = false;
    return t;
  }
  function setRehearsing(on) { document.documentElement.classList.toggle('rehearsing', !!on); }
  // Nothing transitioning or moving: a frozen copy of the page would match it exactly.
  // (Never-ending idle animations, like the study buddy breathing, don't count.)
  function quiet() {
    return !active && !document.getAnimations().some((a) => a.playState === 'running' &&
      !(a.effect && a.effect.getComputedTiming && a.effect.getComputedTiming().iterations === Infinity));
  }

  window.Motion = {
    cover, setRehearsing, quiet, settle: finishActive, rehearsalStep: ZOOM.duration + 60,
    enabled, setEnabled, noteOrigin, setHint, capture, discard, navigate, decorate, selectSegment,
    reveal, press, pop, verdict, flip, countUp, dialogIn, dialogOut, moveTabPill,
  };
})();
