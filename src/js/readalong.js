// FreeSheet read-along: highlights the word (and its sentence) being spoken while
// notes audio plays.
//
// The audio has no word timings, so they are worked out here:
//  1. tts/generate.py reads each notes block as its own line and puts 0.35 s of
//     silence after every chunk it speaks. Decoding the audio and finding those
//     silences gives the time span of each block (a long block can be several chunks).
//  2. Within a block, time is shared between the words on screen in proportion to
//     how long each one takes to say (maths is weighed by its spoken form, so
//     "x^2" counts as "x squared"), with extra time for pauses at punctuation.
// The current sentence is tinted with the CSS Custom Highlight API and a marker
// glides from word to word.
(function () {
  'use strict';

  const M = window.Markup;
  const S = window.Sections;
  const HAS_HL = typeof CSS !== 'undefined' && CSS.highlights && typeof Highlight === 'function';
  const RENDERED = new Set(['heading', 'paragraph', 'list', 'definition', 'example', 'callout', 'table', 'formula', 'passage', 'diagram']);
  const LINE_SEL = 'p, li, td, th, caption, figcaption, h3, .n-def-term, .n-def-text, .n-label, .n-answer, .n-passage-text, .n-caption, .katex-display';

  // ---------------------------------------------------------------- audio

  let decoder = null;

  // Returns { segments: [{start, end}] } – the stretches of speech between the gaps.
  async function analyse(bytes) {
    if (!decoder) decoder = new OfflineAudioContext(1, 1, 24000);
    const copy = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    const audio = await decoder.decodeAudioData(copy);
    const data = audio.getChannelData(0);
    const hop = Math.round(audio.sampleRate * 0.01); // 10 ms frames
    const n = Math.floor(data.length / hop);
    const rms = new Float32Array(n);
    for (let f = 0; f < n; f++) {
      let s = 0;
      for (let k = f * hop, e = k + hop; k < e; k++) s += data[k] * data[k];
      rms[f] = Math.sqrt(s / hop);
    }
    const loud = Array.from(rms).sort((a, b) => a - b)[Math.floor(n * 0.9)] || 0;
    const thr = Math.max(loud * 0.03, 1e-4);
    const minGap = 22; // frames: just under the generator's 35-frame gap
    const sec = (f) => (f * hop) / audio.sampleRate;
    const segments = [];
    let start = -1, quiet = 0;
    for (let f = 0; f < n; f++) {
      if (rms[f] > thr) { if (start < 0) start = f; quiet = 0; }
      else if (start >= 0 && ++quiet >= minGap) {
        segments.push({ start: sec(start), end: sec(f - quiet + 1) });
        start = -1; quiet = 0;
      }
    }
    if (start >= 0) segments.push({ start: sec(start), end: sec(n - quiet) });
    return { segments };
  }

  // ---------------------------------------------------------------- words on screen

  // Source strings of a block in the order they appear on screen, to recover the
  // TeX behind each rendered formula.
  function sources(b) {
    switch (b.type) {
      case 'list': return b.items || [];
      case 'definition': return [b.term, b.text];
      case 'callout': return [b.label || 'Note', b.text];
      case 'example': return [b.title || 'Worked example', b.problem, ...(b.steps || []), b.answer];
      case 'table': return [b.caption, ...(b.rows || []).flat()]; // the header row is not read out on its own
      case 'formula': return [b.caption];
      case 'passage': return [b.title, b.text];
      case 'diagram': return [b.caption];
      default: return [b.text];
    }
  }

  // Punctuation at the end of a word, allowing closing quotes and brackets after it.
  const END_STOP = /[.!?]["')\]]*$/;
  const END_COMMA = /[,;:]["')\]]*$/;
  const END_SENTENCE = /[.!?:]["')\]]*$/;
  const END_PAUSE = /[.,;:!?]["')\]]*$/;

  function wordWeight(w) {
    const spoken = S.plain(w).length || 1;
    const pause = END_STOP.test(w) ? 3 : END_COMMA.test(w) ? 1 : 0;
    return spoken + 1 + pause;
  }

  function blockTokens(el, b) {
    const tex = b.type === 'formula' ? [b.latex] : [];
    for (const src of sources(b)) {
      if (src == null) continue;
      for (const seg of M.segments(String(src))) if (seg.math) tex.push(seg.text);
    }
    const out = [];
    let mi = 0;
    const lineOf = (node) => { const l = node.closest(LINE_SEL); return l && el.contains(l) ? l : el; };
    (function walk(node) {
      for (let c = node.firstChild; c; c = c.nextSibling) {
        if (c.nodeType === 1) {
          if (c.tagName === 'THEAD' || c.tagName === 'BUTTON' || c.tagName === 'IMG') continue;
          if (c.classList.contains('katex')) {
            const t = tex[mi++];
            const r = document.createRange();
            r.selectNode(c);
            out.push({ range: r, weight: (t != null ? S.latex(t).length : 10) + 1, line: lineOf(c), stop: false, pause: false });
            continue;
          }
          walk(c);
        } else if (c.nodeType === 3) {
          const re = /\S+/g;
          let m;
          while ((m = re.exec(c.data))) {
            const r = document.createRange();
            r.setStart(c, m.index);
            r.setEnd(c, m.index + m[0].length);
            out.push({ range: r, weight: wordWeight(m[0]), line: lineOf(c.parentElement), stop: END_SENTENCE.test(m[0]), pause: END_PAUSE.test(m[0]) });
          }
        }
      }
    })(el);
    return out;
  }

  // ---------------------------------------------------------------- timing

  // The generator's gap between spoken chunks measures about 1.3 s once the voice's
  // own lead-in and tail are added; pauses inside a chunk (commas, full stops) stay
  // under about 0.8 s.
  const CHUNK_GAP = 0.95;

  function chunksOf(segs) {
    const out = [];
    segs.forEach((s, i) => {
      if (!i || s.start - segs[i - 1].end >= CHUNK_GAP) out.push([]);
      out[out.length - 1].push(s);
    });
    return out;
  }

  // Share out `items` (each an array of speech segments, in order) as one contiguous
  // run per block, so each run's length best matches the block's share of the text.
  function assign(units, items) {
    const B = units.length, N = items.length;
    if (!B || N < B) return null;
    const pre = [0];
    for (const it of items) pre.push(pre[pre.length - 1] + (it[it.length - 1].end - it[0].start));
    const chars = units.reduce((a, u) => a + u.chars, 0);
    const exp = units.map((u) => (pre[N] * u.chars) / chars);
    const cost = Array.from({ length: B + 1 }, () => new Float64Array(N + 1).fill(Infinity));
    const from = Array.from({ length: B + 1 }, () => new Int32Array(N + 1));
    cost[0][0] = 0;
    for (let b = 1; b <= B; b++) {
      for (let s = b; s <= N - (B - b); s++) {
        for (let k = b - 1; k < s; k++) {
          if (cost[b - 1][k] === Infinity) continue;
          const d = pre[s] - pre[k] - exp[b - 1];
          const c = cost[b - 1][k] + (d * d) / Math.max(exp[b - 1], 0.3);
          if (c < cost[b][s]) { cost[b][s] = c; from[b][s] = k; }
        }
      }
    }
    const groups = new Array(B);
    for (let b = B, s = N; b > 0; b--) { const k = from[b][s]; groups[b - 1] = items.slice(k, s).flat(); s = k; }
    return groups;
  }

  // Spread tokens over the speech in `segs` by weight, skipping the silences between.
  function spread(toks, segs) {
    const W = toks.reduce((a, t) => a + t.weight, 0) || 1;
    const D = segs.reduce((a, s) => a + s.end - s.start, 0);
    let acc = 0;
    for (const t of toks) {
      let x = (acc / W) * D;
      t.t = segs[segs.length - 1].end;
      for (const s of segs) {
        const d = s.end - s.start;
        if (x <= d) { t.t = s.start + x; break; }
        x -= d;
      }
      acc += t.weight;
    }
  }

  // Give each token of one block a start time. The pauses inside the block's audio
  // are matched to punctuation in the text, so timing re-syncs at every pause
  // instead of drifting across a long paragraph.
  function place(toks, segs) {
    const K = segs.length;
    const cands = [];
    for (let j = 1; j < toks.length; j++) if (toks[j - 1].pause) cands.push(j);
    if (K < 2 || cands.length < K - 1) return spread(toks, segs);

    const wp = [0];
    for (const t of toks) wp.push(wp[wp.length - 1] + t.weight);
    const W = wp[toks.length];
    const durs = segs.map((s) => s.end - s.start);
    const D = durs.reduce((a, d) => a + d, 0);
    // target[k]: where (in weight) the cut after segment k should fall.
    const target = [];
    for (let k = 0, acc = 0; k < K - 1; k++) { acc += durs[k]; target.push((W * acc) / D); }

    // dp[k][c]: best cost with cut k placed at cands[c]; cuts must increase.
    const C = cands.length;
    const dp = Array.from({ length: K - 1 }, () => new Float64Array(C).fill(Infinity));
    const back = Array.from({ length: K - 1 }, () => new Int32Array(C).fill(-1));
    for (let c = 0; c < C; c++) dp[0][c] = (wp[cands[c]] - target[0]) ** 2;
    for (let k = 1; k < K - 1; k++) {
      let best = Infinity, bestC = -1;
      for (let c = 0; c < C; c++) {
        if (c > 0 && dp[k - 1][c - 1] < best) { best = dp[k - 1][c - 1]; bestC = c - 1; }
        if (bestC >= 0) { dp[k][c] = best + (wp[cands[c]] - target[k]) ** 2; back[k][c] = bestC; }
      }
    }
    let c = -1, best = Infinity;
    for (let i = 0; i < C; i++) if (dp[K - 2][i] < best) { best = dp[K - 2][i]; c = i; }
    if (c < 0) return spread(toks, segs);
    const cuts = new Array(K - 1);
    for (let k = K - 2; k >= 0; k--) { cuts[k] = cands[c]; c = back[k][c]; }
    const bounds = [0, ...cuts, toks.length];
    for (let k = 0; k < K; k++) spread(toks.slice(bounds[k], bounds[k + 1]), [segs[k]]);
  }

  function build(sectionEl, sec, timing) {
    const els = Array.from(sectionEl.children).filter((c) => !c.classList.contains('section-play') && !c.classList.contains('read-word'));
    const units = [];
    let ei = 0;
    for (const b of sec.blocks) {
      if (!b || typeof b !== 'object' || !RENDERED.has(b.type)) continue;
      const el = els[ei++];
      if (!el) break;
      const speech = S.blockSpeech(b);
      if (!speech) continue;
      const toks = blockTokens(el, b);
      toks.forEach((t, i) => { t.pause = t.pause || i === toks.length - 1 || toks[i + 1].line !== t.line; });
      if (toks.length) units.push({ toks, chars: speech.length });
    }
    const segs = timing.segments;
    if (!units.length || !segs.length) return null;
    // Prefer the long chunk gaps as block boundaries; fall back to any pause.
    const groups = assign(units, chunksOf(segs)) || assign(units, segs.map((s) => [s]));
    if (groups) units.forEach((u, i) => place(u.toks, groups[i]));
    else spread(units.flatMap((u) => u.toks), segs); // too few pauses found: spread over the whole clip
    const toks = units.flatMap((u) => u.toks);

    // Sentences: break at full stops and wherever a new line of text starts.
    const sentences = [];
    toks.forEach((t, i) => {
      const prev = toks[i - 1];
      if (!prev || prev.stop || prev.line !== t.line) sentences.push({ first: i, last: i });
      else sentences[sentences.length - 1].last = i;
      t.sentence = sentences.length - 1;
    });
    for (const s of sentences) {
      const a = toks[s.first].range, b = toks[s.last].range;
      s.range = document.createRange();
      s.range.setStart(a.startContainer, a.startOffset);
      s.range.setEnd(b.endContainer, b.endOffset);
    }
    return { toks, sentences };
  }

  // ---------------------------------------------------------------- following along

  // Starts highlighting for `audio`. getSection() returns the section's current
  // element (it is looked up again if the notes are redrawn mid-read).
  function follow(audio, getSection, sec, timing) {
    let el = null, plan = null, cur = -1, curSentence = -1, raf = 0, stopped = false;
    let marker = null, userScrolledAt = 0;

    const onUser = () => { userScrolledAt = performance.now(); };
    const onKey = (e) => { if (['PageUp', 'PageDown', 'ArrowUp', 'ArrowDown', 'Home', 'End', ' '].includes(e.key)) onUser(); };
    window.addEventListener('wheel', onUser, { passive: true });
    window.addEventListener('touchmove', onUser, { passive: true });
    window.addEventListener('keydown', onKey);
    window.addEventListener('resize', reposition);

    function rebuild() {
      el = getSection();
      plan = el ? build(el, sec, timing) : null;
      cur = -1; curSentence = -1;
      if (marker) marker.remove();
      marker = null;
      if (el && plan) {
        marker = document.createElement('span');
        marker.className = 'read-word';
        marker.setAttribute('aria-hidden', 'true');
        el.appendChild(marker);
      }
    }

    function tick() {
      raf = 0;
      if (stopped) return;
      if (!el || !el.isConnected) rebuild();
      if (plan) {
        const t = audio.currentTime + 0.04; // a hair early reads as "on time"
        const toks = plan.toks;
        let lo = 0, hi = toks.length - 1, idx = -1;
        while (lo <= hi) { const m = (lo + hi) >> 1; if (toks[m].t <= t) { idx = m; lo = m + 1; } else hi = m - 1; }
        if (idx !== cur) { cur = idx; paint(); }
      }
      if (!audio.paused) raf = requestAnimationFrame(tick);
    }

    function paint() {
      if (cur < 0) { clearMarks(); return; }
      const tok = plan.toks[cur];
      if (HAS_HL && tok.sentence !== curSentence) {
        curSentence = tok.sentence;
        CSS.highlights.set('read-line', new Highlight(plan.sentences[curSentence].range));
      }
      reposition(true);
      keepInView(tok.range.getBoundingClientRect());
    }

    function reposition(animate) {
      if (!marker || !plan || cur < 0) return;
      const r = plan.toks[cur].range.getBoundingClientRect();
      const box = el.getBoundingClientRect();
      if (!r.width && !r.height) return;
      const x = r.left - box.left - 3, y = r.top - box.top - 1;
      const glide = animate === true && marker.style.opacity === '1' && window.Motion && Motion.enabled();
      marker.classList.toggle('glide', glide);
      marker.style.transform = `translate3d(${x}px, ${y}px, 0)`;
      marker.style.width = r.width + 6 + 'px';
      marker.style.height = r.height + 2 + 'px';
      marker.style.opacity = '1';
    }

    function keepInView(r) {
      if (performance.now() - userScrolledAt < 4000) return;
      const top = 150, bottom = window.innerHeight - 90;
      if (r.top >= top && r.bottom <= bottom) return;
      window.scrollBy({ top: r.top - window.innerHeight * 0.38, behavior: window.Motion && Motion.enabled() ? 'smooth' : 'auto' });
    }

    function clearMarks() {
      if (HAS_HL) CSS.highlights.delete('read-line');
      curSentence = -1;
      if (marker) marker.style.opacity = '0';
    }

    const start = () => { if (!raf && !stopped) raf = requestAnimationFrame(tick); };
    audio.addEventListener('play', start);
    audio.addEventListener('seeked', start);
    start();

    return {
      stop() {
        stopped = true;
        if (raf) cancelAnimationFrame(raf);
        audio.removeEventListener('play', start);
        audio.removeEventListener('seeked', start);
        window.removeEventListener('wheel', onUser);
        window.removeEventListener('touchmove', onUser);
        window.removeEventListener('keydown', onKey);
        window.removeEventListener('resize', reposition);
        clearMarks();
        if (marker) marker.remove();
      },
    };
  }

  window.ReadAlong = { analyse, follow };
})();
