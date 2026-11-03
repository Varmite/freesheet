'use strict';
// Unit tests for answer marking (node scripts/test-marking.js).
const assert = require('assert');
const K = require('../src/js/marking.js');
const M = require('../src/js/markup.js');
let n = 0;
function t(name, fn) { fn(); n++; }

t('parseNumber SA formats', () => {
  const cases = [['3,5', 3.5], ['3.5', 3.5], ['12 500', 12500], ['1 250,75', 1250.75], ['R150', 150], ['R 1 250,00', 1250],
    ['-2', -2], ['\u22122,5', -2.5], ['3/4', 0.75], ['1e-3', 0.001], ['45%', 45], ['5 m', 5], ['9,8 m/s2', 9.8], ['1,250.50', 1250.5],
    ['0,125', 0.125], ['12°', 12], ['  7 ', 7]];
  for (const [s, v] of cases) assert.ok(Math.abs(K.parseNumber(s) - v) < 1e-9, `${s} -> ${K.parseNumber(s)} expected ${v}`);
  for (const s of ['', 'abc', '3 4', '1/0']) assert.ok(Number.isNaN(K.parseNumber(s)), `${s} should be NaN`);
});
t('short answers', () => {
  const q = { type: 'short', answers: ['photosynthesis', 'the base'] };
  assert.ok(K.mark(q, ' Photosynthesis. ').correct);
  assert.ok(K.mark(q, 'base').correct);
  assert.ok(!K.mark(q, 'respiration').correct);
  assert.ok(!K.mark(q, '').correct);
});
t('numeric tolerance', () => {
  const q = { type: 'numeric', answer: 0.125, tolerance: 0.0001, marks: 2 };
  assert.strictEqual(K.mark(q, '0,125').score, 2);
  assert.strictEqual(K.mark(q, '1/8').score, 2);
  assert.strictEqual(K.mark(q, '0,13').score, 0);
  assert.strictEqual(K.mark({ type: 'numeric', answer: 3 }, '3,004').score, 1);
});
t('mcq/truefalse/match/open', () => {
  assert.ok(K.mark({ type: 'mcq', options: ['a', 'b'], answer: 1 }, 1).correct);
  assert.ok(!K.mark({ type: 'mcq', options: ['a', 'b'], answer: 1 }, 0).correct);
  assert.ok(K.mark({ type: 'truefalse', answer: false }, false).correct);
  assert.ok(!K.mark({ type: 'truefalse', answer: false }, undefined).correct);
  const m = { type: 'match', marks: 3, pairs: [{ left: 'a', right: '1' }, { left: 'b', right: '2' }, { left: 'c', right: '3' }] };
  assert.strictEqual(K.mark(m, ['1', '2', '3']).score, 3);
  assert.strictEqual(K.mark(m, ['1', '3', '2']).score, 1);
  const o = { type: 'open', marks: 4, memo: 'x' };
  assert.ok(K.mark(o, { text: 'hi' }).needsSelfMark);
  assert.strictEqual(K.mark(o, { text: 'hi' }, 3).score, 3);
  assert.strictEqual(K.mark(o, { text: 'hi' }, 9).score, 4);
});
t('caps levels', () => {
  assert.strictEqual(K.capsLevel(80).level, 7);
  assert.strictEqual(K.capsLevel(79).level, 6);
  assert.strictEqual(K.capsLevel(29).level, 1);
});
t('markup', () => {
  assert.strictEqual(M.inline('**a** and *b*'), '<strong>a</strong> and <em>b</em>');
  assert.strictEqual(M.inline('<script>'), '&lt;script&gt;');
  assert.strictEqual(M.inline('costs US\$5'), 'costs US$5');
  assert.ok(M.segments('a $x^2$ b').some((s) => s.math && s.text === 'x^2'));
  assert.ok(M.segments('price $5').some((s) => s.unclosed));
  assert.strictEqual(M.inline('2 * 3 * 4'), '2 * 3 * 4');
});
console.log(`${n} marking/markup test groups passed.`);
