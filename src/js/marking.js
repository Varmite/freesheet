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
        const ok = given !== '' && (q.answers || []).some((a) => normText(a) === given);
        return { score: ok ? max : 0, max, correct: ok, needsSelfMark: false };
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

  return { marksOf, normText, parseNumber, isAnswered, mark, capsLevel, formatMarks };
});
