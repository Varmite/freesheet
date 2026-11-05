'use strict';
// Tests for language courses (node scripts/test-course.js): every generated exercise
// in every lesson must be solvable, and answer checking must be fair.
const assert = require('assert');
const fs = require('fs');
const path = require('path');
const C = require('../src/js/course.js');

const dir = path.join(__dirname, '..', 'content', 'courses');
let groups = 0, exercises = 0;
function t(name, fn) { fn(); groups++; }

for (const id of fs.existsSync(dir) ? fs.readdirSync(dir) : []) {
  const course = JSON.parse(fs.readFileSync(path.join(dir, id, 'course.json'), 'utf8'));
  const m = C.model(course);
  const ctx = { syn: m.syn, known: m.known };

  t(`${id}: structure`, () => {
    const ids = new Set();
    for (const u of course.units) for (const l of u.lessons) {
      assert.ok(!ids.has(l.id), `duplicate lesson id ${l.id}`);
      ids.add(l.id);
      assert.ok(l.words.length >= 3, `${l.id}: needs at least 3 words`);
      assert.ok(l.sentences.length >= 3, `${l.id}: needs at least 3 sentences`);
      for (const s of l.sentences) assert.ok(s.af.length && s.en.length, `${l.id}: sentence missing a translation`);
      for (const d of l.drills || []) assert.ok(d.task && d.from && d.to && d.to.length, `${l.id}: incomplete drill`);
      assert.ok(Number.isInteger(course.grade), `${id}: needs a grade`);
    }
  });

  t(`${id}: every exercise in every lesson is solvable (5 versions each, new and returning learner)`, () => {
    const seen = new Set();
    for (const { lesson } of m.lessons) {
      for (let attempt = 0; attempt < 5; attempt++) {
        for (const seenNow of [new Set(seen), new Set(m.words.map((w) => C.norm(w.af, 'af')))]) {
          const list = C.lessonExercises(course, lesson.id, { attempt, seen: seenNow, hasAudio: () => attempt % 2 === 0 });
          assert.ok(list.length >= 8, `${lesson.id}: only ${list.length} exercises`);
          for (const ex of list) {
            exercises++;
            const where = `${lesson.id}#${attempt} ${ex.type}`;
            switch (ex.type) {
              case 'meaning': case 'pick': case 'blank': {
                assert.ok(ex.answer >= 0, `${where}: answer missing from options`);
                const norm = ex.options.map((o) => C.norm(o, ex.type === 'blank' ? 'af' : 'en', m.syn));
                assert.strictEqual(new Set(norm).size, norm.length, `${where}: repeated option ${ex.options}`);
                assert.ok(C.checkExercise(course, ex, ex.answer).correct, `${where}: right option marked wrong`);
                if (ex.type === 'meaning') {
                  const own = m.meaningsOf.get(C.norm(ex.af, 'af'));
                  ex.options.forEach((o, i) => { if (i !== ex.answer) assert.ok(!own.has(C.norm(o, 'en', m.syn)), `${where}: distractor "${o}" is also a meaning of ${ex.af}`); });
                }
                break;
              }
              case 'pairs': {
                assert.ok(ex.pairs.length >= 3, `${where}: too few pairs`);
                const en = ex.pairs.map((p) => C.norm(p.en, 'en', m.syn));
                assert.strictEqual(new Set(en).size, en.length, `${where}: two pairs share a meaning`);
                break;
              }
              case 'build': case 'listen': {
                const need = C.tilesOf(ex.answer);
                const bag = ex.tiles.slice();
                for (const w of need) {
                  const i = bag.indexOf(w);
                  assert.ok(i >= 0, `${where}: tile "${w}" missing for "${ex.answer}"`);
                  bag.splice(i, 1);
                }
                assert.ok(C.checkExercise(course, ex, need.join(' ')).correct, `${where}: the tiles in order are marked wrong: ${need.join(' ')}`);
                break;
              }
              case 'type':
                for (const a of ex.accepted) assert.ok(C.checkExercise(course, ex, a).correct, `${where}: accepted answer "${a}" marked wrong`);
                break;
              case 'transform': {
                for (const a of ex.accepted) assert.ok(C.checkExercise(course, ex, a).correct, `${where}: accepted answer "${a}" marked wrong`);
                assert.ok(!C.checkExercise(course, ex, ex.prompt).correct, `${where}: leaving the sentence unchanged is marked right: ${ex.prompt}`);
                const bag = ex.tiles.slice();
                for (const w of C.tilesOf(ex.answer)) { const i = bag.indexOf(w); assert.ok(i >= 0, `${where}: tile "${w}" missing`); bag.splice(i, 1); }
                break;
              }
              default: assert.fail(`${where}: unknown type`);
            }
          }
        }
      }
      for (const w of lesson.words) seen.add(C.norm(w.af, 'af'));
    }
  });

  t(`${id}: practice sessions are solvable`, () => {
    const done = m.lessons.map((x) => x.lesson.id);
    for (let n = 1; n <= done.length; n += 5) for (let attempt = 0; attempt < 3; attempt++) {
      const list = C.practiceExercises(course, done.slice(0, n), { attempt });
      assert.ok(list.length >= 6);
      for (const ex of list) if (ex.type === 'type') assert.ok(C.checkExercise(course, ex, ex.answer).correct);
    }
  });
}

t('answer checking', () => {
  const syn = { mom: 'mother', thanks: 'thank you', hi: 'hello' };
  const known = new Set(['honde', 'katte', 'ek', 'het', 'drie', 'dogs', 'cats', 'goeiemore']);
  const ctx = { syn, known };
  const ok = (g, acc, lang) => C.check(g, acc, lang, ctx);
  assert.ok(ok("i'm hungry", ['I am hungry.'], 'en').correct, 'contractions');
  assert.ok(ok('This is my mom', ['This is my mother.'], 'en').correct, 'synonyms');
  assert.ok(ok('  hello   THANK you!! ', ['Hello, thank you!'], 'en').correct, 'case and punctuation');
  assert.ok(ok('Thanks!', ['Thank you!'], 'en').correct);
  const accent = ok('goeiemore', ['Goeiemôre'], 'af');
  assert.ok(accent.correct && accent.note === 'accent', 'missing accent counts, with a note');
  // As in the rest of FreeSheet: a one-word answer of 5 letters or fewer must be exact,
  // but inside a sentence a short word may have one slip (the other words pin it down).
  assert.ok(!ok('hondde', ['honde'], 'af').correct, 'a short one-word answer is exact');
  const inSentence = ok('ek het drie hondde', ['Ek het drie honde.'], 'af');
  assert.ok(inSentence.correct && inSentence.note === 'typo', 'one slip in a sentence counts, with a note');
  const exact = ok('I have three dogs', ['I have three dogs.'], 'en');
  assert.ok(exact.correct && exact.note === null, 'no note on an exact answer');
  const longTypo = ok('die onderwyzer skryf', ['Die onderwyser skryf.'], 'af');
  assert.ok(longTypo.correct && longTypo.note === 'typo', 'a slip in a long word counts, with a note');
  assert.ok(!ok('ek het drie katte', ['Ek het drie honde.'], 'af').correct, 'a different real word is wrong');
  assert.ok(ok("ek het 'n kat", ["Ek het 'n kat."], 'af').correct);
  assert.ok(ok('ek het n kat', ["Ek het 'n kat."], 'af').correct, "'n without the apostrophe");
  assert.ok(ok('aangename kenis', ['Aangename kennis!'], 'af').correct, 'one slip in a long word');
  assert.ok(!ok('', ['Ja'], 'af').correct);
});

console.log(`${groups} course test groups passed (${exercises} generated exercises checked).`);
