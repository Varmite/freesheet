// Answer marking. Shared by the app (window.Marking) and tests (require).
(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory();
  else root.Marking = factory();
})(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  function marksOf(q) {
    const m = Number(q && q.marks);
    return Number.isFinite(m) && m > 0 ? m : 1;
  }

  // Strip simple markup so "**Photosynthesis**" and "photosynthesis" compare equal.
  function normText(s) {
    return String(s == null ? '' : s)
      .normalize('NFKC')
      .replace(/\$|\*\*|\*/g, '')
      .replace(/[‘’ʼ]/g, "'")
      .replace(/[“”]/g, '"')
      .replace(/[‐-―−]/g, '-')
      .toLowerCase()
      .replace(/\s+/g, ' ')
      .trim()
      .replace(/^["'(\[]+/, '')
      .replace(/[\s.,;:!?"')\]]+$/, '')
      .replace(/^(the|a|an) /, '');
  }

  // ---------- Spelling slips in short answers
  //
  // Outside language subjects a typed answer with small spelling mistakes still
  // counts ("resillience" for "resilience"). The app turns this on per question by
  // giving it a `typoWords` set: every word that appears anywhere in FreeSheet's
  // content. That set is the safeguard: a word the learner typed that is itself a
  // real word in the content ("endothermic" when the answer is "exothermic", "alkene"
  // for "alkane") is a different answer, not a typo, so it is never accepted.
  // Very short words and anything with digits must be exact.

  const LANGUAGE_SUBJECT = /language|english|afrikaans|isizulu|isixhosa|isindebele|sepedi|sesotho|setswana|siswati|tshivenda|xitsonga|french|german/;
  function isLanguageSubject(subjectId) { return LANGUAGE_SUBJECT.test(String(subjectId || '')); }

  // How many slips a word of this length may have.
  function allowedEdits(len) { return len <= 5 ? 0 : len <= 8 ? 1 : len <= 12 ? 2 : 3; } // <=5 letters (acronyms, short terms): exact

  // Edit distance counting insertions, deletions, substitutions and swapped
  // neighbours ("recieve") as one slip each. Stops early once past `max`.
  function editDistance(a, b, max) {
    if (Math.abs(a.length - b.length) > max) return max + 1;
    let prev2 = null, prev = Array.from({ length: b.length + 1 }, (_, j) => j);
    for (let i = 1; i <= a.length; i++) {
      const cur = [i];
      let best = i;
      for (let j = 1; j <= b.length; j++) {
        const cost = a[i - 1] === b[j - 1] ? 0 : 1;
        let v = Math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + cost);
        if (prev2 && i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) v = Math.min(v, prev2[j - 2] + 1);
        cur.push(v);
        if (v < best) best = v;
      }
      if (best > max) return max + 1;
      prev2 = prev;
      prev = cur;
    }
    return prev[b.length];
  }

  function words(s) { return s.replace(/[^\p{L}\p{N}']+/gu, ' ').trim().split(' ').filter(Boolean); }

  // Is `given` the expected `answer` (both already through normText) with only
  // small spelling slips? `known` is the set of real words (see above).
  function typoOf(given, answer, known) {
    if (!given || !answer || /\d/.test(given) || /\d/.test(answer)) return false;
    let g = words(given), a = words(answer);
    if (g.join('') === a.join('')) return true; // only the spacing or hyphens differ
    if (g.length !== a.length) { g = [g.join('')]; a = [a.join('')]; }
    // In a phrase the other words pin down the meaning, so the phrase gets a slip
    // budget from its total length and words of 4-5 letters may have one slip too
    // ("humman dignity"). A single word on its own keeps the stricter rule.
    const phrase = a.length > 1;
    let budget = phrase ? allowedEdits(a.join('').length) : Infinity;
    for (let i = 0; i < a.length; i++) {
      if (g[i] === a[i]) continue;
      if (known && known.has(g[i])) return false; // a real, different word: not a typo
      const len = a[i].length;
      const max = phrase ? (len <= 3 ? 0 : Math.max(1, allowedEdits(len))) : allowedEdits(len);
      const d = max ? editDistance(g[i], a[i], max) : max + 1;
      if (d > max || (budget -= d) < 0) return false;
    }
    return true;
  }

  // Parse a learner's number. Accepts SA style (12 500,75), "3.5", "-2", "R150", "3/4", "1e-3", "45%".
  function parseNumber(input) {
    if (input == null) return NaN;
    let t = String(input).normalize('NFKC').replace(/−/g, '-').replace(/[  ]/g, ' ').trim();
    if (!t) return NaN;
    t = t.replace(/^[Rr]\s*/, '').replace(/^\$\s*/, '');
    // Fraction a/b (allow spaces).
    const frac = /^([-+]?\d+(?:[.,]\d+)?)\s*\/\s*(\d+(?:[.,]\d+)?)\s*[a-zA-Z%°/·⁻²³\s]*$/.exec(t);
    if (frac) {
      const a = parseNumber(frac[1]);
      const b = parseNumber(frac[2]);
      return b === 0 ? NaN : a / b;
    }
    const m = /^([-+]?\s*(?:\d{1,3}(?: \d{3})+(?!\d)|\d+)(?:[.,]\d+)*(?:[eE][-+]?\d+)?)/.exec(t);
    if (!m) return NaN;
    const rest = t.slice(m[0].length).trim();
    // Allow a trailing unit/symbol but reject further numbers ("3 4").
    if (rest && /\d/.test(rest.replace(/[²³⁻¹]/g, ''))) {
      // e.g. "5 m/s2" -> allow digits only directly after a unit letter
      if (!/^[a-zA-Zµ°%·/.\-\s]+\d*$/.test(rest)) return NaN;
    }
    let num = m[1].replace(/\s+/g, '');
    let exp = '';
    const em = /[eE][-+]?\d+$/.exec(num);
    if (em) { exp = em[0]; num = num.slice(0, -exp.length); }
    const commas = (num.match(/,/g) || []).length;
    const dots = (num.match(/\./g) || []).length;
    if (commas && dots) {
      // Whichever separator comes last is the decimal mark.
      if (num.lastIndexOf(',') > num.lastIndexOf('.')) num = num.replace(/\./g, '').replace(',', '.');
      else num = num.replace(/,/g, '');
    } else if (commas > 1) {
      num = num.replace(/,/g, '');
    } else if (commas === 1) {
      num = num.replace(',', '.');
    } else if (dots > 1) {
      num = num.replace(/\./g, '');
    }
    const v = Number(num + exp);
    return Number.isFinite(v) ? v : NaN;
  }

  function isAnswered(q, r) {
    switch (q.type) {
      case 'mcq': return Number.isInteger(r);
      case 'truefalse': return r === true || r === false;
      case 'short':
      case 'numeric': return typeof r === 'string' && r.trim() !== '';
      case 'match': return Array.isArray(r) && r.some((x) => x != null && x !== '');
      case 'open': return !!(r && typeof r.text === 'string' && r.text.trim() !== '');
      default: return false;
    }
  }

  // Returns { score, max, correct (true|false|null), needsSelfMark }.
  function mark(q, r, selfMark) {
    const max = marksOf(q);
    switch (q.type) {
      case 'mcq': {
        const ok = Number.isInteger(r) && r === q.answer;
        return { score: ok ? max : 0, max, correct: ok, needsSelfMark: false };
      }
      case 'truefalse': {
        const ok = (r === true || r === false) && r === q.answer;
        return { score: ok ? max : 0, max, correct: ok, needsSelfMark: false };
      }
      case 'short': {
        const given = normText(r);
        let ok = given !== '' && (q.answers || []).some((a) => normText(a) === given);
        // Spelling slips (only when the app has switched them on for this question).
        let spelling = null;
        if (!ok && given && q.typoWords) {
          const hit = (q.answers || []).find((a) => typoOf(given, normText(a), q.typoWords));
          if (hit != null) { ok = true; spelling = String(hit); }
        }
        return { score: ok ? max : 0, max, correct: ok, needsSelfMark: false, spelling };
      }
      case 'numeric': {
        const v = parseNumber(r);
        const tol = Number.isFinite(Number(q.tolerance)) ? Math.abs(Number(q.tolerance)) : 0.01;
        const ok = Number.isFinite(v) && Math.abs(v - Number(q.answer)) <= tol + 1e-9;
        return { score: ok ? max : 0, max, correct: ok, needsSelfMark: false };
      }
      case 'match': {
        const pairs = q.pairs || [];
        const given = Array.isArray(r) ? r : [];
        let right = 0;
        pairs.forEach((p, i) => { if (given[i] != null && given[i] === p.right) right++; });
        const raw = pairs.length ? (max * right) / pairs.length : 0;
        const score = Math.round(raw * 2) / 2;
        return { score, max, correct: right === pairs.length, needsSelfMark: false, pairsRight: right };
      }
      case 'open': {
        if (selfMark == null || !Number.isFinite(Number(selfMark))) {
          return { score: 0, max, correct: null, needsSelfMark: true };
        }
        const s = Math.max(0, Math.min(max, Number(selfMark)));
        return { score: s, max, correct: s === max, needsSelfMark: false };
      }
      default:
        return { score: 0, max, correct: false, needsSelfMark: false };
    }
  }

  // CAPS achievement levels.
  function capsLevel(percent) {
    const p = Number(percent) || 0;
    if (p >= 80) return { level: 7, label: 'Outstanding achievement' };
    if (p >= 70) return { level: 6, label: 'Meritorious achievement' };
    if (p >= 60) return { level: 5, label: 'Substantial achievement' };
    if (p >= 50) return { level: 4, label: 'Adequate achievement' };
    if (p >= 40) return { level: 3, label: 'Moderate achievement' };
    if (p >= 30) return { level: 2, label: 'Elementary achievement' };
    return { level: 1, label: 'Not achieved' };
  }

  function formatMarks(n) {
    const v = Math.round(Number(n) * 10) / 10;
    return Number.isInteger(v) ? String(v) : String(v).replace('.', ',');
  }

  return { marksOf, normText, parseNumber, isAnswered, mark, capsLevel, formatMarks, typoOf, editDistance, isLanguageSubject };
});
