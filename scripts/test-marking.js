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
t('spelling slips in short answers', () => {
  // The real words the app passes in (normally: every word in the content).
  const known = new Set(['resilience', 'exothermic', 'endothermic', 'alkane', 'alkene', 'hypotonic', 'hypertonic', 'mitosis', 'meiosis', 'cell', 'call', 'recycle', 'self', 'esteem']);
  const q = (answers) => Object.defineProperty({ type: 'short', answers }, 'typoWords', { value: known });
  const ok = (answers, given) => K.mark(q(answers), given);
  // Accepted, with the right spelling reported back.
  assert.ok(ok(['resilience'], 'Resillience').correct);
  assert.strictEqual(ok(['resilience'], 'Resillience').spelling, 'resilience');
  assert.ok(ok(['recycle'], 'recyle').correct);
  assert.ok(ok(['photosynthesis'], 'photosynthisis').correct);
  assert.ok(ok(['mitosis'], 'mitossis').correct);
  assert.ok(ok(['self-esteem'], 'self esteem').correct);
  assert.ok(ok(['self-esteem'], 'selfesteem').correct);
  assert.ok(ok(['south african human rights commission'], 'south african human rights comission').correct);
  assert.ok(ok(['human dignity'], 'humman dignity').correct); // short word inside a phrase
  assert.ok(ok(['admission point score'], 'admission poinnt score').correct);
  assert.ok(!ok(['human dignity'], 'humman dignnityy').correct); // 3 slips in 12 letters: over the phrase budget
  // Exact answers are not reported as spelling slips.
  assert.strictEqual(ok(['resilience'], 'resilience').spelling, null);
  // A real, different word is a different answer, never a typo.
  assert.ok(!ok(['exothermic'], 'endothermic').correct);
  assert.ok(!ok(['alkane'], 'alkene').correct);
  assert.ok(!ok(['hypotonic'], 'hypertonic').correct);
  assert.ok(!ok(['mitosis'], 'meiosis').correct);
  // Short words, acronyms and anything with digits must be exact.
  assert.ok(!ok(['cell'], 'cel').correct);
  assert.ok(!ok(['sahrc'], 'sarhc').correct);
  assert.ok(!ok(['0800 428 428'], '0800 428 429').correct);
  // Too many slips.
  assert.ok(ok(['resilience'], 'reslince').correct); // two slips in a 10-letter word: fine
  assert.ok(!ok(['resilience'], 'reselyunce').correct); // three: too many
  // Off unless the app switches it on (language subjects stay strict).
  assert.ok(!K.mark({ type: 'short', answers: ['resilience'] }, 'Resillience').correct);
  assert.ok(K.isLanguageSubject('english-home-language'));
  assert.ok(K.isLanguageSubject('afrikaans-first-additional-language'));
  assert.ok(!K.isLanguageSubject('life-orientation'));
  assert.ok(!K.isLanguageSubject('natural-sciences'));
});
t('hints never give the answer away (all content)', () => {
  const H = require('../src/js/hints.js');
  const fs = require('fs');
  const path = require('path');
  const root = path.join(__dirname, '..', 'content');
  const strip = (h) => h.replace(/<[^>]+>/g, '').replace(/&amp;/g, '&').replace(/&#39;/g, "'").replace(/&quot;/g, '"').toLowerCase();
  const reEsc = (a) => a.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  let questions = 0;
  for (const g of fs.readdirSync(root)) for (const s of fs.readdirSync(path.join(root, g))) {
    const d = path.join(root, g, s, 'topics');
    if (!fs.existsSync(d)) continue;
    for (const f of fs.readdirSync(d)) {
      const topic = JSON.parse(fs.readFileSync(path.join(d, f), 'utf8'));
      for (const q of topic.questions || []) {
        questions++;
        const hints = H.forQuestion(q, topic);
        assert.ok(hints.length >= 1, `${g}/${s}/${topic.id}#${q.id} has no hints`);
        const answers = H._answerTexts(q).map((a) => a.trim().toLowerCase()).filter((a) => a.length >= 3);
        for (const h of hints) {
          if (h.kind === 'pair' || h.kind === 'point') continue; // these reveal part of the answer on purpose, as the last hints
          const text = strip(h.html);
          for (const a of answers) {
            const re = /^\d/.test(a) ? new RegExp(`(?<![\\d,.])${reEsc(a)}(?![\\d])`) : new RegExp(`\\b${reEsc(a)}\\b`);
            assert.ok(!re.test(text), `${g}/${s}/${topic.id}#${q.id}: ${h.kind} hint gives away "${a}"`);
          }
        }
      }
    }
  }
  assert.ok(questions > 100);
});
t('hints: multiple choice rules out wrong options only', () => {
  const H = require('../src/js/hints.js');
  const q = { id: 'x', type: 'mcq', prompt: 'Pick one', options: ['a', 'b', 'c', 'd'], answer: 2 };
  const hs = H.forQuestion(q, { notes: [] });
  const out = hs.filter((h) => h.eliminate).pop().eliminate;
  assert.strictEqual(out.length, 2);
  assert.ok(!out.includes(2));
});
console.log(`${n} marking/markup test groups passed.`);
