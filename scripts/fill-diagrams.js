#!/usr/bin/env node
'use strict';
// Content-authoring helper: replaces diagram blocks whose svg is a "PLOT:{...}" spec with a real SVG.
// Run after writing content:  node scripts/fill-diagrams.js [content/grade-11 ...]
//
// Graph spec:
//   PLOT:{"x":[-4,4],"y":[-3,5],"step":1,"fns":[{"f":"x*x-2","label":"f","from":-3,"to":3}],
//         "points":[[2,3,"(2; 3)"]],"vlines":[1],"hlines":[2],"open":[[0,1]]}
//   f is a JavaScript expression in x (Math.* available: sin, cos, tan use DEGREES via sind/cosd/tand).
// Number line spec:
//   PLOT:{"nl":[-5,5],"dots":[[-2,"closed"],[3,"open"]],"shade":[[-2,3]]}   (use null for ±infinity)
const fs = require('fs');
const path = require('path');

const W = 320, H = 260, PAD = 18;

function graph(spec) {
  const [x0, x1] = spec.x, [y0, y1] = spec.y;
  const step = spec.step || 1;
  const X = (x) => PAD + ((x - x0) / (x1 - x0)) * (W - 2 * PAD);
  const Y = (y) => H - PAD - ((y - y0) / (y1 - y0)) * (H - 2 * PAD);
  const r = (n) => Math.round(n * 10) / 10;
  let s = `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 ${W} ${H}' font-family='sans-serif' font-size='10'><rect width='${W}' height='${H}' fill='white'/>`;
  // grid
  for (let x = Math.ceil(x0 / step) * step; x <= x1; x += step) s += `<line x1='${r(X(x))}' y1='${PAD}' x2='${r(X(x))}' y2='${H - PAD}' stroke='#e6e6e6'/>`;
  const stepY = spec.stepY || spec.labelStepY || step;
  for (let y = Math.ceil(y0 / stepY) * stepY; y <= y1; y += stepY) s +=`<line x1='${PAD}' y1='${r(Y(y))}' x2='${W - PAD}' y2='${r(Y(y))}' stroke='#e6e6e6'/>`;
  // axes
  const ax = Math.min(Math.max(0, y0), y1), ay = Math.min(Math.max(0, x0), x1);
  s += `<line x1='${PAD - 6}' y1='${r(Y(ax))}' x2='${W - PAD + 8}' y2='${r(Y(ax))}' stroke='black'/><text x='${W - 10}' y='${r(Y(ax)) - 4}'>x</text>`;
  s += `<line x1='${r(X(ay))}' y1='${H - PAD + 6}' x2='${r(X(ay))}' y2='${PAD - 8}' stroke='black'/><text x='${r(X(ay)) + 5}' y='${PAD - 6}'>y</text>`;
  const lab = spec.labelStep || (step * (x1 - x0 > 12 ? 2 : 1));
  for (let x = Math.ceil(x0 / lab) * lab; x <= x1; x += lab) if (Math.abs(x) > 1e-9) s += `<text x='${r(X(x))}' y='${r(Y(ax)) + 12}' text-anchor='middle'>${fmt(x, spec.xdeg)}</text>`;
  const labY = spec.labelStepY || lab;
  for (let y = Math.ceil(y0 / labY) * labY; y <= y1; y += labY) if (Math.abs(y) > 1e-9) s += `<text x='${r(X(ay)) - 4}' y='${r(Y(y)) + 3}' text-anchor='end'>${fmt(y)}</text>`;
  s += `<text x='${r(X(ay)) - 4}' y='${r(Y(ax)) + 12}' text-anchor='end'>0</text>`;
  // asymptotes / guide lines
  for (const v of spec.vlines || []) s += `<line x1='${r(X(v))}' y1='${PAD}' x2='${r(X(v))}' y2='${H - PAD}' stroke='black' stroke-dasharray='4 3'/>`;
  for (const h of spec.hlines || []) s += `<line x1='${PAD}' y1='${r(Y(h))}' x2='${W - PAD}' y2='${r(Y(h))}' stroke='black' stroke-dasharray='4 3'/>`;
  // functions
  const sind = (d) => Math.sin((d * Math.PI) / 180), cosd = (d) => Math.cos((d * Math.PI) / 180), tand = (d) => Math.tan((d * Math.PI) / 180);
  const dashes = ['', " stroke-dasharray='6 3'", " stroke-dasharray='2 3'"];
  (spec.fns || []).forEach((fn, k) => {
    // eslint-disable-next-line no-new-func
    const f = new Function('x', 'sind', 'cosd', 'tand', `with (Math) { return (${fn.f}); }`);
    const a = fn.from != null ? fn.from : x0, b = fn.to != null ? fn.to : x1;
    const n = 400;
    let d = '', pen = false, prev = null;
    for (let i = 0; i <= n; i++) {
      const x = a + ((b - a) * i) / n;
      const y = f(x, sind, cosd, tand);
      const ok = Number.isFinite(y) && y >= y0 - (y1 - y0) && y <= y1 + (y1 - y0);
      if (ok && prev !== null && Math.abs(y - prev) > (y1 - y0) * 0.8) pen = false; // jump across an asymptote
      if (ok) { const cy = Math.min(Math.max(Y(y), PAD - 2), H - PAD + 2); d += `${pen ? 'L' : 'M'}${r(X(x))},${r(cy)}`; pen = cy > PAD - 2 && cy < H - PAD + 2; if (!pen) pen = true; }
      else pen = false;
      prev = ok ? y : null;
    }
    s += `<clipPath id='c${k}'><rect x='${PAD}' y='${PAD}' width='${W - 2 * PAD}' height='${H - 2 * PAD}'/></clipPath><path d='${d}' fill='none' stroke='black' stroke-width='1.8'${dashes[k % 3]} clip-path='url(#c${k})'/>`;
    if (fn.label) {
      const lx = fn.lx != null ? fn.lx : b - (b - a) * 0.08;
      const ly = f(lx, sind, cosd, tand);
      if (Number.isFinite(ly)) {
        const right = X(lx) > W * 0.55;
        s += `<text x='${r(X(lx)) + (right ? -6 : 6)}' y='${r(Math.min(Math.max(Y(ly), PAD + 10), H - PAD - 4)) - 6}' text-anchor='${right ? 'end' : 'start'}' font-size='11' font-style='italic' paint-order='stroke' stroke='white' stroke-width='3'>${esc(fn.label)}</text>`;
      }
    }
  });
  for (const [px, py, t] of spec.points || []) {
    s += `<circle cx='${r(X(px))}' cy='${r(Y(py))}' r='3' fill='black'/>`;
    if (t) s += `<text x='${r(X(px)) + 5}' y='${r(Y(py)) - 5}'>${esc(t)}</text>`;
  }
  for (const [px, py] of spec.open || []) s += `<circle cx='${r(X(px))}' cy='${r(Y(py))}' r='3' fill='white' stroke='black'/>`;
  return s + '</svg>';
}

function numberLine(spec) {
  const [a, b] = spec.nl;
  const w = 320, h = 70, pad = 22;
  const X = (x) => pad + ((x - a) / (b - a)) * (w - 2 * pad);
  let s = `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 ${w} ${h}' font-family='sans-serif' font-size='11'><rect width='${w}' height='${h}' fill='white'/>`;
  s += `<line x1='8' y1='32' x2='${w - 8}' y2='32' stroke='black'/><polygon points='${w - 4},32 ${w - 12},28 ${w - 12},36'/><polygon points='4,32 12,28 12,36'/>`;
  for (let x = a; x <= b; x++) s += `<line x1='${X(x)}' y1='28' x2='${X(x)}' y2='36' stroke='black'/><text x='${X(x)}' y='52' text-anchor='middle'>${x}</text>`;
  for (const [from, to] of spec.shade || []) {
    const x1 = from == null ? 8 : X(from), x2 = to == null ? w - 8 : X(to);
    s += `<line x1='${x1}' y1='32' x2='${x2}' y2='32' stroke='black' stroke-width='5'/>`;
  }
  for (const [x, kind] of spec.dots || []) s += `<circle cx='${X(x)}' cy='32' r='5' fill='${kind === 'open' ? 'white' : 'black'}' stroke='black' stroke-width='2'/>`;
  return s + '</svg>';
}

function fmt(v, deg) { const t = Number.isInteger(v) ? String(v) : String(Math.round(v * 100) / 100).replace('.', ','); return deg ? t + '°' : t; }
function esc(t) { return String(t).replace(/&/g, '&amp;').replace(/</g, '&lt;'); }

function render(spec) { return spec.nl ? numberLine(spec) : graph(spec); }

function walk(p, out) {
  const st = fs.statSync(p);
  if (st.isDirectory()) for (const f of fs.readdirSync(p)) walk(path.join(p, f), out);
  else if (p.endsWith('.json')) out.push(p);
  return out;
}

if (require.main === module) {
  const targets = process.argv.slice(2);
  const files = (targets.length ? targets : [path.join(__dirname, '..', 'content')]).flatMap((t) => walk(path.resolve(t), []));
  let n = 0;
  for (const file of files) {
    const raw = fs.readFileSync(file, 'utf8');
    if (!raw.includes('PLOT:')) continue;
    const data = JSON.parse(raw);
    const fix = (blocks) => (blocks || []).forEach((b) => {
      if (b && b.type === 'diagram' && typeof b.svg === 'string' && b.svg.startsWith('PLOT:')) { b.svg = render(JSON.parse(b.svg.slice(5))); n++; }
    });
    fix(data.notes);
    (data.sections || []).forEach((sec) => fix(sec.context));
    fs.writeFileSync(file, JSON.stringify(data, null, 2) + '\n');
  }
  console.log(`Filled ${n} diagram(s).`);
}

module.exports = { render };
