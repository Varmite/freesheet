// Hints for practice questions (never offered in exams). Shared by the app
// (window.Hints) and tests (require).
//
// Hints get stronger one level at a time:
//   - a clue from the notes: the sentence in this topic's notes that best matches the
//     question (scored by shared rare words with the question, its answer and worked
//     solution), with the answer blanked out, plus a link to that notes section
//   - multiple choice: rule out wrong options one at a time, down to two
//   - true/false: what to watch for in the statement
//   - typed answers: the answer's letter pattern, then its first half
//   - numbers: the first step of the worked solution (answer hidden), then a range
//   - matching: one correct pair at a time
//   - written answers: how many points to make, key words from the memo, the first point
// A hint never contains the answer itself (checked across all content by tests).
(function (root, factory) {
  if (typeof module === 'object' && module.exports) {
    module.exports = factory(require('./markup.js'), require('./sections.js'), require('./marking.js'));
  } else {
    root.Hints = factory(root.Markup, root.Sections, root.Marking);
  }
})(typeof self !== 'undefined' ? self : this, function (M, S, K) {
  'use strict';

  const LETTERS = 'ABCDEFGH';
  const BLANK = '⟦blank⟧';
  const BLANK_HTML = '<span class="hint-blank" aria-label="hidden answer">   </span>';
  const STOP = new Set(('a an the of to in on at for and or but is are was were be been being it its this that these those with as by ' +
    'from which what who whom whose when where why how do does did not no yes than then so such can could will would should may might ' +
    'must has have had into about over under between your you we they he she i their there here also each one two three more most ' +
    'very only just use used using called answer question give write following true false correct both other same make made').split(' '));

  // ---------------------------------------------------------------- text helpers

  function plain(raw) {
    return String(raw == null ? '' : raw).replace(/\$[^$]*\$/g, ' ').replace(/\*\*|\*/g, '');
  }
  function stem(w) { return w.length > 4 ? w.replace(/(ings|ing|ed|es|s)$/, '') : w; }
  function tokens(raw) {
    const out = [];
    for (const w of plain(raw).toLowerCase().match(/\p{L}[\p{L}'-]*/gu) || []) {
      const t = w.replace(/^'+|'+$/g, '');
      if (t.length >= 3 && !STOP.has(t)) out.push(stem(t));
    }
    return out;
  }

  // Split markup text into sentences without breaking inside $maths$.
  function sentences(raw) {
    const maths = [];
    const t = String(raw).replace(/\$[^$]*\$/g, (m) => { maths.push(m); return `⁠${maths.length - 1}⁠`; });
    return t.split(/(?<=[.!?])\s+(?=[A-Z⁠(*"'])|\n+/)
      .map((s) => s.replace(/⁠(\d+)⁠/g, (_, i) => maths[Number(i)]).trim())
      .filter(Boolean);
  }

  // The explaining text of a block, with how much to trust it as a clue. Worked
  // examples are left out: they are about other problems, so they mislead as clues.
  function blockStrings(b) {
    const w = (arr, weight) => arr.filter(Boolean).map((raw) => ({ raw, weight }));
    switch (b && b.type) {
      case 'paragraph': case 'passage': return w([b.text], 1);
      case 'list': return w(b.items || [], 1);
      // (Bold can't span maths in the markup, so a term with maths in it isn't bolded.)
      case 'definition': return w([`${String(b.term).includes('$') ? b.term : `**${b.term}**`}: ${b.text}`], 1.25);
      case 'callout': return w([b.text], 1);
      case 'formula': return b.caption ? w([`${b.caption}: $${b.latex}$`], 1.1) : [];
      case 'table': return w((b.rows || []).map((r) => (r || []).map((c) => String(c).trim()).filter(Boolean).join('; ')), 0.6);
      case 'diagram': return b.caption ? w([b.caption], 0.8) : [];
      default: return [];
    }
  }

  // ---------------------------------------------------------------- notes index

  const indexes = new WeakMap();
  function indexOf(topic) {
    if (indexes.has(topic)) return indexes.get(topic);
    const sents = [];
    const df = new Map();
    S.split(topic.notes || []).forEach((sec, si) => {
      for (const b of sec.blocks) {
        for (const { raw: str, weight } of blockStrings(b)) {
          for (const raw of sentences(str)) {
            const toks = new Set(tokens(raw));
            if (toks.size < 4) continue; // too little explaining text (headings, rows of numbers)
            sents.push({ raw, section: si, toks, weight });
            for (const t of toks) df.set(t, (df.get(t) || 0) + 1);
          }
        }
      }
    });
    const n = Math.max(1, sents.length);
    const idf = (t) => Math.log(1 + n / (df.get(t) || 0.5));
    const idx = { sents, idf };
    indexes.set(topic, idx);
    return idx;
  }

  // ---------------------------------------------------------------- answers (to hide)

  function numberForms(v) {
    const n = Number(v);
    if (!Number.isFinite(n)) return [];
    const s = String(n);
    return [...new Set([s, s.replace('.', ','), n.toFixed(2), n.toFixed(2).replace('.', ',')])];
  }
  function answerTexts(q) {
    switch (q.type) {
      case 'mcq': return [plain(q.options[q.answer])].filter((t) => t.trim().length >= 2);
      case 'short': return (q.answers || []).map(plain);
      case 'numeric': return numberForms(q.answer);
      default: return [];
    }
  }

  // Blank every occurrence of the answer outside maths. Returns null if the answer
  // still shows (for example inside a formula), so the clue isn't used.
  function hideAnswer(raw, q) {
    const answers = answerTexts(q).map((a) => a.trim()).filter((a) => a.length >= 2).sort((a, b) => b.length - a.length);
    if (!answers.length) return raw;
    let out = '';
    for (const seg of M.segments(raw)) {
      if (seg.math) { out += `$${seg.text}$`; continue; }
      let t = seg.text;
      for (const a of answers) {
        const esc = a.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
        const re = /^\d/.test(a) ? new RegExp(`(?<![\\d,.])${esc}(?![\\d])`, 'g') : new RegExp(`\\b${esc}\\b`, 'gi');
        t = t.replace(re, BLANK);
      }
      out += t;
    }
    // The answer written inside a formula can't be blanked: don't use this clue.
    const squash = (t) => String(t).toLowerCase().replace(/\s+|\\[a-z]+|[{}]/g, '');
    const mathText = M.segments(out).filter((sg) => sg.math).map((sg) => squash(sg.text)).join(' ');
    if (answers.some((a) => !/^\d/.test(a) && squash(a).length >= 2 && mathText.includes(squash(a)))) return null;
    const lower = plain(out).toLowerCase();
    if (answers.some((a) => (/^\d/.test(a) ? new RegExp(`(?<![\\d,.])${a.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(?![\\d])`).test(lower) : lower.includes(a.toLowerCase())))) return null;
    if (q.type === 'numeric' && M.segments(out).some((s) => s.math && numberForms(q.answer).some((f) => s.text.includes(f)))) return null;
    return out;
  }

  // Render a clue. Bold markers that can't pair up (bold split across sentences, or
  // spanning maths) would show as literal "**", so any left over are dropped.
  function renderRaw(raw) { return M.inline(raw).replace(/\*\*/g, '').split(BLANK).join(BLANK_HTML); }

  // ---------------------------------------------------------------- the clue from the notes

  function clueFor(q, topic) {
    if (!topic || !topic.notes) return null;
    const idx = indexOf(topic);
    if (!idx.sents.length) return null;
    const weights = new Map();
    const add = (raw, w) => { for (const t of tokens(raw)) weights.set(t, Math.max(weights.get(t) || 0, w)); };
    add(q.prompt, 1);
    if (q.type === 'mcq') add(q.options[q.answer], 1.5);
    if (q.type === 'short') (q.answers || []).forEach((a) => add(a, 1.5));
    if (q.type === 'match') (q.pairs || []).forEach((p) => { add(p.left, 0.7); add(p.right, 0.7); });
    if (q.type === 'open') add(q.memo, 0.6);
    if (q.explanation) add(q.explanation, 0.8);
    const scored = idx.sents.map((s) => {
      let score = 0;
      for (const t of s.toks) if (weights.has(t)) score += idx.idf(t) * weights.get(t);
      return { s, score: (score * s.weight) / Math.sqrt(s.toks.size) };
    }).filter((x) => x.score > 1.6).sort((a, b) => b.score - a.score);
    for (const { s } of scored.slice(0, 6)) {
      const hidden = hideAnswer(s.raw, q);
      if (hidden == null) continue;
      // A clue that repeats the question adds nothing.
      if (plain(hidden).trim().toLowerCase() === plain(q.prompt).trim().toLowerCase()) continue;
      return { raw: hidden, section: s.section };
    }
    return null;
  }

  // ---------------------------------------------------------------- per-type hints

  function seededOrder(arr, seed) {
    let h = 2166136261;
    for (const c of String(seed)) { h ^= c.charCodeAt(0); h = Math.imul(h, 16777619); }
    const a = arr.slice();
    for (let i = a.length - 1; i > 0; i--) {
      h = (Math.imul(h, 1103515245) + 12345) >>> 0;
      const j = h % (i + 1);
      [a[i], a[j]] = [a[j], a[i]];
    }
    return a;
  }

  function fmt(n) {
    const r = Math.abs(n) >= 100 ? Math.round(n) : Math.round(n * 100) / 100;
    return String(r).replace('.', ',');
  }

  // A range around a number that contains it without giving it away.
  function rangeFor(v) {
    const n = Number(v);
    if (!Number.isFinite(n)) return null;
    if (n === 0) return [-1, 1];
    const mag = Math.pow(10, Math.floor(Math.log10(Math.abs(n))));
    const step = mag / 2;
    let lo = Math.floor((n - Math.abs(n) * 0.35) / step) * step;
    let hi = Math.ceil((n + Math.abs(n) * 0.35) / step) * step;
    if (lo === n) lo -= step;
    if (hi === n) hi += step;
    return [lo, hi];
  }

  function shapeOf(answer, revealHalf) {
    return plain(answer).trim().split(/\s+/).map((w) => {
      const keep = revealHalf ? Math.ceil(w.length / 2) : 1;
      return [...w].map((ch, i) => (i < keep || !/\p{L}/u.test(ch) ? ch : '_')).join(' ');
    }).join(' ');
  }

  function firstStep(explanation, q) {
    if (!explanation) return null;
    const parts = sentences(explanation).flatMap((s) => s.split(/;\s+|,\s+so\s+/));
    if (parts.length < 2) return null;
    const hidden = hideAnswer(parts[0], q);
    if (hidden == null || /=\s*$/.test(plain(hidden))) return null;
    return hidden;
  }

  const ABSOLUTE = /\b(always|never|all|none|only|every|everyone|no one|cannot|must)\b/i;

  function build(q, topic) {
    const out = [];
    const clue = clueFor(q, topic);
    const clueHint = () => clue && { kind: 'clue', section: clue.section, html: `From your notes: “${renderRaw(clue.raw)}”` };
    switch (q.type) {
      case 'mcq': {
        if (clue) out.push(clueHint());
        const wrong = seededOrder(q.options.map((_, i) => i).filter((i) => i !== q.answer), q.id);
        const out2 = wrong.slice(0, Math.max(0, q.options.length - 2));
        out2.forEach((i, k) => out.push({
          kind: 'eliminate',
          eliminate: out2.slice(0, k + 1),
          html: k === 0 ? `It isn't <strong>${LETTERS[i]}</strong>. I've crossed it out.` : `It isn't <strong>${LETTERS[i]}</strong> either. Choose from what's left.`,
        }));
        break;
      }
      case 'truefalse': {
        if (clue) out.push(clueHint());
        const m = ABSOLUTE.exec(plain(q.prompt));
        out.push({
          kind: 'tip',
          html: m
            ? `Watch the word “<strong>${M.escapeHtml(m[1].toLowerCase())}</strong>”. A single exception would make the statement false.`
            : 'Read the statement word by word and check each part. If any part is wrong, the whole statement is false.',
        });
        break;
      }
      case 'short': {
        if (clue) out.push(clueHint());
        const a = (q.answers || [])[0];
        if (a) {
          const letters = plain(a).replace(/[^\p{L}]/gu, '').length;
          const wordCount = plain(a).trim().split(/\s+/).length;
          const hidden = plain(a).trim().split(/\s+/).reduce((n, w) => n + Math.max(0, w.replace(/[^\p{L}]/gu, '').length - 1), 0);
          if (hidden >= 2) out.push({ kind: 'shape', html: `The answer has ${wordCount > 1 ? `${wordCount} words, ` : ''}${letters} letters: <span class="hint-shape">${M.escapeHtml(shapeOf(a, false))}</span>` });
          if (hidden >= 4) out.push({ kind: 'shape', html: `It starts like this: <span class="hint-shape">${M.escapeHtml(shapeOf(a, true))}</span>` });
        }
        break;
      }
      case 'numeric': {
        if (clue) out.push(clueHint());
        const step = firstStep(q.explanation, q);
        if (step) out.push({ kind: 'step', html: `Start like this: ${renderRaw(step)}` });
        const r = rangeFor(q.answer);
        if (r) out.push({ kind: 'range', html: `The answer is between <strong>${fmt(r[0])}</strong> and <strong>${fmt(r[1])}</strong>${q.unit ? ' ' + M.inline(q.unit) : ''}.` });
        break;
      }
      case 'match': {
        if (clue) out.push(clueHint());
        const order = seededOrder((q.pairs || []).map((_, i) => i), q.id);
        order.slice(0, Math.max(0, (q.pairs || []).length - 1)).forEach((i, k) => {
          const p = q.pairs[i];
          out.push({ kind: 'pair', pair: i, html: `${k ? 'And ' : ''}${M.inline(p.left)} goes with <strong>${M.inline(p.right)}</strong>.` });
        });
        break;
      }
      case 'open': {
        const marks = K.marksOf(q);
        out.push({ kind: 'tip', html: `This is worth ${marks} mark${marks === 1 ? '' : 's'}, so aim for about ${marks} clear point${marks === 1 ? '' : 's'}${clue ? '' : '.'}` +
          (clue ? `. Your notes say: “${renderRaw(clue.raw)}”` : ''), section: clue ? clue.section : undefined });
        const seen = new Set();
        const words = (plain(q.memo).match(/\p{L}[\p{L}-]{4,}/gu) || [])
          .map((w) => w.toLowerCase()).filter((w) => !STOP.has(w) && !seen.has(w) && seen.add(w));
        const keys = seededOrder(words.sort((a, b) => b.length - a.length).slice(0, 6), q.id);
        if (keys.length >= 2) out.push({ kind: 'keywords', html: `Words worth using: ${keys.map((w) => `<span class="hint-chip">${M.escapeHtml(w)}</span>`).join(' ')}` });
        const first = String(q.memo || '').split('\n')[0].replace(/✓\s*\(\d+\)|\(\d+\)\s*$/g, '').trim();
        if (first) out.push({ kind: 'point', html: `One point the memo wants: ${M.inline(first)}` });
        break;
      }
      default: break;
    }
    // Always something to offer.
    if (!out.length) out.push({ kind: 'tip', html: 'This one is a key term from this topic. Skim the headings and bold words in the notes: the answer is one of them.' });
    return out;
  }

  const cache = new WeakMap();
  function forQuestion(q, topic) {
    if (!cache.has(q)) cache.set(q, build(q, topic));
    return cache.get(q);
  }

  // The notes section that best explains a question (for "show me in the notes").
  function relatedSection(q, topic) {
    const c = clueFor(q, topic);
    return c ? c.section : null;
  }

  return { forQuestion, relatedSection, _answerTexts: answerTexts, _plain: plain };
});
