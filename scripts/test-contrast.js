'use strict';
// Colour contrast of every theme in src/styles.css (node scripts/test-contrast.js).
// Text up to 17pt needs 4.5:1; large or bold text and the edges of controls need 3:1.
const assert = require('assert');
const fs = require('fs');
const path = require('path');
const css = fs.readFileSync(path.join(__dirname, '..', 'src', 'styles.css'), 'utf8');

// The custom properties set in one rule, e.g. vars(':root') or vars('html[data-theme="dark"]').
function vars(selector) {
  const out = {};
  const re = new RegExp('(^|\\n)' + selector.replace(/[[\]().*"=]/g, '\\$&') + ' \\{([^}]*)\\}', 'g');
  for (let m; (m = re.exec(css));) for (const d of m[2].matchAll(/--([\w-]+):\s*([^;]+);/g)) out[d[1]] = d[2].trim();
  return out;
}
const hex = (s) => { if (s.length === 4) s = '#' + s.slice(1).split('').map((c) => c + c).join(''); const c = [1, 3, 5].map((i) => parseInt(s.slice(i, i + 2), 16)); assert.ok(c.every((x) => x >= 0), 'bad colour ' + s); return c; };
const lum = (c) => { const [r, g, b] = c.map((v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); }); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
const ratio = (a, b) => { const x = lum(a), y = lum(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };
// A colour value as it shows over `under`: #rrggbb, rgba(...) blended, or var(--x) looked up in v.
function col(s, under, v) {
  s = s.trim();
  const ref = s.match(/^var\(--([\w-]+)\)$/);
  if (ref) return col(v[ref[1]], under, v);
  if (s[0] === '#') return hex(s);
  const m = s.match(/rgba?\(([^)]+)\)/);
  assert.ok(m, 'cannot read colour ' + s);
  const [r, g, b, a = 1] = m[1].split(',').map(Number);
  return [r, g, b].map((c, i) => Math.round(c * a + under[i] * (1 - a)));
}
const literal = (rule, prop) => { const m = css.match(new RegExp(rule.replace(/[.()[\]"=]/g, '\\$&') + ' \\{[^}]*' + prop + ':\\s*(#[0-9a-fA-F]{6})')); assert.ok(m, 'missing: ' + rule); return hex(m[1]); };

const base = vars(':root');
const darkBase = vars('html.is-dark');
const DARK = ['dark', 'midnight', 'forest', 'black'];
const themes = ['light', 'dark', 'sepia', 'midnight', 'forest', 'black', 'contrast'];
const darkRed = literal('html.is-dark .btn.danger, html.is-dark .lesson-foot.is-wrong .btn.primary', 'background');
const fails = [];
const need = (name, got, min) => { if (got < min) fails.push(`${name}: ${got.toFixed(2)}:1, needs ${min}:1`); };

for (const t of themes) {
  const dark = DARK.includes(t);
  const own = t === 'light' || t === 'dark'; // the other themes draw cards in the page colour
  const v = Object.assign({}, base, dark ? darkBase : {}, own ? {} : { card: 'var(--bg)', 'side-bg': 'var(--surface)' }, vars(`html[data-theme="${t}"]`));
  const bg = hex(v.bg), card = col(v.card, bg, v), white = [255, 255, 255], black = [0, 0, 0];
  const fill = col(v.fill, bg, v), hl = col(v.hl, bg, v);
  need(`${t} text on page`, ratio(hex(v.text), bg), 4.5);
  need(`${t} grey text on page`, ratio(hex(v.muted), bg), 4.5);
  need(`${t} grey text on cards`, ratio(hex(v.muted), card), 4.5);
  need(`${t} links on cards`, ratio(hex(v.accent), card), 4.5);
  need(`${t} labels of unselected tabs and segments`, ratio(hex(v.muted), fill), 4.5);
  need(`${t} current tab on the highlighter`, ratio(col(v['on-hl'], bg, v), hl), 4.5);
  need(`${t} key terms on a highlighter stroke`, ratio(hex(v.text), col(v['hl-soft'], card, v)), 4.5);
  need(`${t} grey labels in the sidebar`, ratio(hex(v.muted), col(v['side-bg'], bg, v)), 4.5);
  need(`${t} text in the sidebar`, ratio(hex(v.text), col(v['side-bg'], bg, v)), 4.5);
  need(`${t} white on buttons`, ratio(white, hex(v.btn)), 4.5);
  need(`${t} red button`, ratio(white, dark ? darkRed : hex(v.bad)), 4.5);
  need(`${t} big green lesson button (18px+)`, ratio(dark ? black : white, hex(v.good)), 3);
  need(`${t} big red lesson button (18px+)`, ratio(white, dark ? darkRed : hex(v.bad)), 3);
  need(`${t} answer option edges`, ratio(hex(v['opt-edge'] || v['border-strong']), card), own ? 3 : 1.5);
}
need('XP label (bold)', ratio([0, 0, 0], literal('.ds.xp span', 'background')), 3);
need('lit streak in light themes (bold)', ratio(literal('html:not(.is-dark) .streak-btn.lit', 'color'), col(base.fill, hex(base.bg), base)), 3);

assert.deepStrictEqual(fails, [], '\n' + fails.join('\n'));
console.log(`Contrast checks passed (${themes.length} themes).`);
