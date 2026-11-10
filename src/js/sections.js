// Splits topic notes into sections (one per heading) and turns each section into plain text
// for text-to-speech. Works in the browser (window.Sections) and in Node (require), so the app
// and scripts/tts/extract.js always agree on section boundaries and audio file names.
(function (root, factory) {
  if (typeof module === 'object' && module.exports) module.exports = factory(require('./markup.js'));
  else root.Sections = factory(root.Markup);
})(typeof self !== 'undefined' ? self : this, function (Markup) {
  'use strict';

  // Bump when speech conversion changes, so all audio is regenerated.
  const SPEECH_VERSION = 1;

  // ---------- sections

  function split(notes) {
    const out = [];
    let cur = null;
    for (const b of notes || []) {
      if (!cur || (b.type === 'heading' && cur.blocks.length)) {
        cur = { blocks: [] };
        out.push(cur);
      }
      cur.blocks.push(b);
    }
    return out.map((s, i) => {
      const text = sectionSpeech(s.blocks);
      return { index: i, blocks: s.blocks, title: s.blocks[0].type === 'heading' ? s.blocks[0].text : '', text, id: hash(SPEECH_VERSION + '|' + text) };
    });
  }

  function hash(str) {
    // FNV-1a 32-bit, as 8 hex digits.
    let h = 0x811c9dc5;
    for (let i = 0; i < str.length; i++) {
      h ^= str.charCodeAt(i);
      h = Math.imul(h, 0x01000193) >>> 0;
    }
    return h.toString(16).padStart(8, '0');
  }

  // ---------- maths to words

  const SYMBOLS = {
    '\\times': ' times ', '\\cdot': ' times ', '\\div': ' divided by ', '\\pm': ' plus or minus ',
    '\\leq': ' is less than or equal to ', '\\le': ' is less than or equal to ',
    '\\geq': ' is greater than or equal to ', '\\ge': ' is greater than or equal to ',
    '\\neq': ' is not equal to ', '\\approx': ' is approximately ', '\\infty': ' infinity ',
    '\\rightarrow': ' gives ', '\\to': ' to ', '\\Rightarrow': ' therefore ', '\\therefore': ' therefore ',
    '\\pi': ' pi ', '\\theta': ' theta ', '\\alpha': ' alpha ', '\\beta': ' beta ', '\\Delta': ' delta ',
    '\\angle': ' angle ', '\\triangle': ' triangle ', '\\parallel': ' is parallel to ', '\\perp': ' is perpendicular to ',
    '\\circ': ' degrees ', '\\%': ' percent ', '\\sin': ' sine ', '\\cos': ' cos ', '\\tan': ' tan ',
    '\\log': ' log ', '\\ldots': ' and so on ', '\\dots': ' and so on ', '\\in': ' in ', '\\cup': ' union ', '\\cap': ' intersection ',
  };

  // Read the {...} group starting at s[i] (which must be '{'); returns [content, nextIndex].
  function group(s, i) {
    if (s[i] !== '{') {
      // Single token: a command or one character.
      const m = /^\\[a-zA-Z]+|^./.exec(s.slice(i));
      return [m ? m[0] : '', i + (m ? m[0].length : 0)];
    }
    let depth = 0;
    for (let j = i; j < s.length; j++) {
      if (s[j] === '{') depth++;
      else if (s[j] === '}' && --depth === 0) return [s.slice(i + 1, j), j + 1];
    }
    return [s.slice(i + 1), s.length];
  }

  function power(e) {
    const t = e.trim();
    if (t === '2') return ' squared ';
    if (t === '3') return ' cubed ';
    if (t === '\\circ') return ' degrees ';
    return ` to the power ${latex(t)} `;
  }

  function latex(tex) {
    let s = String(tex);
    let out = '';
    let i = 0;
    while (i < s.length) {
      const rest = s.slice(i);
      let m;
      if ((m = /^\\[dt]?frac/.exec(rest))) {
        const [a, j] = group(s, i + m[0].length);
        const [b, k] = group(s, j);
        out += ` ${latex(a)} over ${latex(b)} `;
        i = k;
      } else if ((m = /^\\sqrt/.exec(rest))) {
        let j = i + m[0].length;
        let root = '';
        if (s[j] === '[') { const e = s.indexOf(']', j); root = s.slice(j + 1, e); j = e + 1; }
        const [a, k] = group(s, j);
        out += root ? ` the ${root === '3' ? 'cube' : root + 'th'} root of ${latex(a)} ` : ` the square root of ${latex(a)} `;
        i = k;
      } else if ((m = /^\\(text|mathrm|textbf|mathbf|operatorname)/.exec(rest))) {
        const [a, j] = group(s, i + m[0].length);
        out += ` ${a.replace(/\\,|\\ /g, ' ')} `;
        i = j;
      } else if ((m = /^\\(left|right|big|Big|bigg|displaystyle|quad|qquad)\b/.exec(rest))) {
        i += m[0].length;
      } else if ((m = /^\\(overline|hat|vec|bar)/.exec(rest))) {
        const [a, j] = group(s, i + m[0].length);
        out += ` ${latex(a)} `;
        i = j;
      } else if ((m = /^\\(xrightarrow)(\[[^\]]*\])?/.exec(rest))) {
        const [, j] = group(s, i + m[0].length);
        out += ' gives ';
        i = j;
      } else if ((m = /^\\(underbrace|overbrace)/.exec(rest))) {
        const [a, j] = group(s, i + m[0].length);
        let k = j;
        if (s[k] === '_' || s[k] === '^') k = group(s, k + 1)[1];
        out += ` ${latex(a)} `;
        i = k;
      } else if ((m = /^\\[a-zA-Z]+/.exec(rest))) {
        out += SYMBOLS[m[0]] != null ? SYMBOLS[m[0]] : ' ';
        i += m[0].length;
      } else if (rest[0] === '^') {
        const [a, j] = group(s, i + 1);
        out += power(a);
        i = j;
      } else if (rest[0] === '_') {
        const [a, j] = group(s, i + 1);
        out += ` ${latex(a)} `;
        i = j;
      } else if ((m = /^\{,\}/.exec(rest))) {
        out += ' comma ';
        i += 3;
      } else if ((m = /^\\[,;:! ]/.exec(rest)) || rest[0] === '~') {
        out += ' ';
        i += m ? m[0].length : 1;
      } else if (rest[0] === '\\' && rest[1] === '\\') {
        out += ', ';
        i += 2;
      } else if (rest[0] === '{' || rest[0] === '}') {
        i++;
      } else {
        const ch = rest[0];
        const said = ({ '=': ' equals ', '+': ' plus ', '-': ' minus ', '<': ' is less than ', '>': ' is greater than ',
          '(': ' ', ')': ' ', '[': ' ', ']': ' ', '|': ' ', '&': ' ', '/': ' over ', ';': ', ', '!': ' factorial ' })[ch];
        out += said === undefined ? ch : said;
        i++;
      }
    }
    return out.replace(/\s+/g, ' ').trim();
  }

  // ---------- plain text to speech-friendly text

  const UNICODE = [
    [/²/g, ' squared'], [/³/g, ' cubed'], [/⁻¹/g, ' to the minus one'], [/°C/g, ' degrees Celsius'], [/°/g, ' degrees'],
    [/×/g, ' times '], [/÷/g, ' divided by '], [/±/g, ' plus or minus '], [/≈/g, ' approximately '], [/≤/g, ' less than or equal to '],
    [/≥/g, ' greater than or equal to '], [/→/g, ' to '], [/←/g, ' '], [/✓/g, ''], [/★/g, ''], [/☆/g, ''], [/√/g, ' square root of '],
    [/π/g, ' pi '], [/Ω/g, ' ohms'], [/µF/g, ' microfarads'], [/Ø/g, 'diameter '], [/–|—/g, ', '], [/…/g, '...'],
    [/[₀₁₂₃₄₅₆₇₈₉]/g, (d) => ' ' + '₀₁₂₃₄₅₆₇₈₉'.indexOf(d)],
  ];

  function plain(text) {
    let s = '';
    for (const seg of Markup.segments(text)) s += seg.math ? ` ${latex(seg.text)} ` : seg.text;
    s = s.replace(/\*\*|\*/g, '').replace(/\\n/g, ' ');
    for (const [re, rep] of UNICODE) s = s.replace(re, rep);
    // South African number formats: thousands spaces (12 500), decimal commas (3,5), rand amounts (R1 250,00).
    s = s.replace(/(\d) (?=\d{3}\b)/g, '$1');
    s = s.replace(/US\$\s?(\d+)/g, (m, n) => `${n} US dollar${n === '1' ? '' : 's'}`);
    s = s.replace(/_{3,}/g, ' ').replace(/ = /g, ' equals ');
    s = s.replace(/\bR(\d+)(?:,(\d{2}))?\b/g, (m, r, c) => (c && c !== '00' ? `${r} rand ${Number(c)} cents` : `${r} rand`));
    s = s.replace(/(\d),(\d)/g, '$1 comma $2');
    s = s.replace(/\bkm\/h\b/g, 'kilometres per hour').replace(/\bm\/s\b/g, 'metres per second').replace(/\bN\/kg\b/g, 'newtons per kilogram');
    s = s.replace(/\be\.g\.\s*/gi, 'for example, ').replace(/\bi\.e\.\s*/gi, 'that is, ').replace(/\betc\./gi, 'and so on.');
    s = s.replace(/\s+([,.;:!?])/g, '$1').replace(/\s+/g, ' ').trim();
    return /^[\s,.;:!?]*$/.test(s) ? '' : s;
  }

  function sentence(t) {
    const s = plain(t);
    return !s ? '' : /[.!?:]$/.test(s) ? s : s + '.';
  }

  function blockSpeech(b) {
    switch (b.type) {
      case 'heading': return sentence(b.text);
      case 'paragraph': return sentence(b.text);
      case 'list': return (b.items || []).map((it, i) => (b.ordered ? `${i + 1}. ` : '') + sentence(it)).join(' ');
      case 'definition': return `${sentence(b.term).replace(/\.$/, ':')} ${sentence(b.text)}`;
      case 'callout': return `${sentence(b.label).replace(/\.$/, ':')} ${sentence(b.text)}`;
      case 'example': {
        const parts = [sentence(b.title || 'Example'), sentence(b.problem)];
        (b.steps || []).forEach((st) => parts.push(sentence(st)));
        if (b.answer) parts.push('Answer: ' + sentence(b.answer));
        return parts.join(' ');
      }
      case 'formula': return sentence(latex(b.latex)) + (b.caption ? ' ' + sentence(b.caption) : '');
      case 'passage': return (b.title ? sentence(b.title) + ' ' : '') + sentence(b.text);
      case 'diagram': return 'Diagram: ' + sentence(b.caption);
      case 'table': {
        const parts = [];
        if (b.caption) parts.push('Table: ' + sentence(b.caption));
        const heads = (b.headers || []).map((h) => plain(String(h)));
        for (const row of b.rows || []) {
          const cells = row.map((c, i) => {
            const v = plain(String(c));
            if (!v) return '';
            return i === 0 || !heads[i] ? v : `${heads[i]}: ${v}`;
          }).filter(Boolean);
          parts.push(cells.join('; ') + '.');
        }
        return parts.join(' ');
      }
      default: return '';
    }
  }

  function sectionSpeech(blocks) {
    return blocks.map(blockSpeech).filter(Boolean).join('\n');
  }

  return { split, latex, plain, blockSpeech, hash, SPEECH_VERSION };
});
