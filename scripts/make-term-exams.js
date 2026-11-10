#!/usr/bin/env node
/*
 * Builds the two term papers of every subject out of its practice questions:
 *   exams/term-1-and-2.json   work taught in Terms 1 and 2
 *   exams/term-3-and-4.json   work taught in Terms 3 and 4
 *
 * The papers are ordinary exam files, so both front ends list them without any
 * code. The same input always gives the same papers. Run it again after
 * changing practice questions, then `npm run validate`:
 *
 *   node scripts/make-term-exams.js            write the papers
 *   node scripts/make-term-exams.js --report   print what would be left out
 */
'use strict';
const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..', 'content');
const REPORT = process.argv.includes('--report');
const PAPERS = [
  { id: 'term-1-and-2', title: 'Term 1 and 2 Examination', terms: [1, 2] },
  { id: 'term-3-and-4', title: 'Term 3 and 4 Examination', terms: [3, 4] },
];

// One question may not be most of a short paper.
const MAX_QUESTION_MARKS = 15;
// Papers leave at least a fifth of the practice marks out, so that no paper is
// simply the whole practice set again.
const POOL_SHARE = 0.8;
const MIN_MARKS = 25;

// A practice question sits under its topic's notes and after the questions
// before it. A paper has neither, so anything leaning on them stays out.
const NEEDS_NOTES = [
  /\b(?:the|this|that|these|those|following)\s+(?:\w+\s+)?(?:image|picture|photo|photograph|diagram|figure|sketch|map|floor plan|plan)\b/i,
  /\bgraph\b(?!\s+(?:of|has|is best|would|that|for|paper))/i,
  /\btable\b(?!\s+of\s+values)/i,
  /\b(?:above|below)\b(?!\s+(?:sea|zero|the\s+(?:x|y|horizontal|line|poverty)|\$|average|freezing|\d))/i,
  /\b(?:figure|diagram)\s+\d/i,
  /\b(?:shown|pictured|illustrated|labelled)\b/i,
  /\b(?:in|from)\s+the\s+notes\b|\bworked\s+example\b/i,
  /\b(?:previous|earlier|preceding|same|last)\s+(?:question|answer|part|example|function|triangle|circle|diagram|graph|data|set|sequence|scenario|case|learner|business|family|figures?|bag|box)\b/i,
  /\bthat\s+(?:summary|essay|letter|answer)\b/i,
  /\byour\s+(?:answer|result|calculation|graph|sketch)\b/i,
  /\bhence\b|\brefer\s+to\b|\bquestion\s+\d/i,
  /^(?:he|she|they|it|its|his|her|their|this|these|that|those)\b/i,
];
// These lean on a text. Real papers print the text above the questions, so
// they can stay when the topic's notes have that text to print.
const NEEDS_TEXT = [
  /\b(?:the|this|that|these|those|following)\s+(?:\w+\s+)?(?:passage|extract|poem|article|cartoon|advert|advertisement|comprehension|stanza|speech|report|scene|source|text|dialogue|notice|poster|letter|paragraph)\b/i,
  /\b(?:the|this)\s+(?:writer|author|poet|speaker|narrator)\b/i,
  /\b(?:paragraph|line|lines|stanza)\s+\d/i,
  /\beach\s+stanza\b/i,
  /\b(?:the\s+)?(?:bottle|designer|cartoonist)\b|\bcartoonist's\b|\bframe\s+\d/i,
  /\b(?:Sources?|Texts?)\s+[A-Z]\b/,
];
// "The novel" means nothing without the book's name; "In *Macbeth*" stands alone.
const UNNAMED_WORK = /\b(?:the|this)\s+(?:novel|play|drama|short\s+story|story|film)\b/i;

// Capitalised words that turn up in any letter or article and name nobody in it.
const NOT_A_NAME = /^(?:Dear|Yours|Sincerely|Faithfully|Regards|Street|Road|Avenue|Grade|South|Africa|African|Durban|Johannesburg|Pretoria|Cape|Town|Monday|Tuesday|Wednesday|Thursday|Friday|Saturday|Sunday|January|February|March|April|June|July|August|September|October|November|December|THREE|FOUR|FIVE|ONLY|LEFT|TRUE|FALSE|BOTH|EACH|MUST)$/;

const plain = (s) => s.toLowerCase().replace(/[‘’]/g, "'").replace(/[“”]/g, '"').replace(/\s+/g, ' ');

// The passages a question talks about: by label ("Source K", "Text A"), by the
// work's title, by a phrase it quotes, or by the name of someone in it.
function passagesNamedBy(prompt, passages) {
  const text = plain(prompt);
  const quotes = (prompt.replace(/[‘’]/g, "'").replace(/[“”]/g, '"').match(/(?:^|[\s(])['"]([^'"]{6,})['"]/g) || [])
    .flatMap((m) => m.replace(/^[\s(]*['"]|['"]$/g, '').split('/'))
    .map((q) => plain(q).trim())
    .filter((q) => q.split(' ').length >= 3);
  // Names are the weakest sign, and a long prompt is full of its own.
  const names = prompt.length > 240 ? [] : [...prompt.matchAll(/(?:^|\s)([A-Z][a-z]{3,})(?='s\b)|(?<=[a-z,] )([A-Z][a-z]{3,})\b|\b([A-Z]{4,})\b/g)].map((m) => m[1] || m[2] || m[3]).filter((n) => !NOT_A_NAME.test(n));
  return passages.filter((p) => {
    const title = p.title || '';
    const label = title.match(/^(source|text)\s+([A-Z])\b/i);
    if (label && new RegExp(`\\b${label[1]}s?\\s+(?:[A-Z]\\s*(?:,|and|&)\\s*)*${label[2]}\\b`, 'i').test(prompt)) return true;
    const head = plain(title.split(/\s[–-]\s|:|\(/)[0]).trim();
    if (head.length > 5 && text.includes(head)) return true;
    const body = plain(p.text);
    return quotes.some((q) => body.includes(q)) || names.some((n) => new RegExp(`\\b${n}\\b`).test(p.text));
  });
}

// Which passages a question needs printed with it, or null if it cannot stand
// on a paper at all.
function contextFor(q, passages) {
  if (NEEDS_NOTES.some((re) => re.test(q.prompt))) return null;
  const named = passagesNamedBy(q.prompt, passages);
  if (named.length) return named;
  const named_work = UNNAMED_WORK.test(q.prompt) && !/\*[^*]+\*/.test(q.prompt);
  if (!named_work && !NEEDS_TEXT.some((re) => re.test(q.prompt))) return [];
  // A long prompt has brought its own letter or paragraph to work on.
  if (q.prompt.includes('\n') || q.prompt.length > 240) return [];
  // It needs a text but doesn't say which, so there must be only one it can mean.
  // A model letter or essay only counts if the question asks about the model.
  const texts = passages.filter((p) => !/^(?:model|example)\b/i.test(p.title || '') || /\b(?:model|example)\b/i.test(q.prompt));
  return texts.length === 1 ? texts : null;
}
const marksOf = (q) => q.marks || 1;
const sum = (qs) => qs.reduce((a, q) => a + marksOf(q), 0);
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const writeJson = (f, data) => fs.writeFileSync(f, JSON.stringify(data, null, 2) + '\n');

function random(seedText) {
  let h = 2166136261;
  for (let i = 0; i < seedText.length; i++) { h ^= seedText.charCodeAt(i); h = Math.imul(h, 16777619); }
  return () => {
    h = (h + 0x6d2b79f5) | 0;
    let t = Math.imul(h ^ (h >>> 15), 1 | h);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function shuffled(list, rnd) {
  const a = list.slice();
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(rnd() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

// Takes questions from one topic up to `quota` marks, cycling through the
// question types so a topic is not all one kind.
function pickFromTopic(pool, quota, rnd) {
  const byType = new Map();
  for (const q of shuffled(pool, rnd)) {
    if (!byType.has(q.type)) byType.set(q.type, []);
    byType.get(q.type).push(q);
  }
  const picked = [];
  let got = 0;
  for (let more = true; more && got < quota;) {
    more = false;
    for (const list of byType.values()) {
      const at = list.findIndex((q) => got + marksOf(q) <= quota);
      if (at < 0) continue;
      const [q] = list.splice(at, 1);
      picked.push(q);
      got += marksOf(q);
      more = true;
      if (got >= quota) break;
    }
  }
  return picked;
}

function paperMarks(november, poolMarks) {
  const wanted = november.totalMarks >= 75 && poolMarks >= 94 ? 75 : 50;
  return Math.max(MIN_MARKS, Math.floor(Math.min(wanted, poolMarks * POOL_SHARE) / 5) * 5);
}

function buildPaper(gradeDir, sub, topics, paper) {
  const inTerm = topics.filter((t) => paper.terms.includes(t.term));
  const seen = new Set();
  const groups = [];
  let left = 0;
  for (const t of inTerm) {
    const passages = t.notes.filter((b) => b.type === 'passage');
    const needs = new Map();
    const pool = t.questions.filter((q) => {
      const key = q.prompt.toLowerCase().replace(/\s+/g, ' ').trim();
      if (seen.has(key) || marksOf(q) > MAX_QUESTION_MARKS) return false;
      const context = contextFor(q, passages);
      if (!context) { left++; return false; }
      seen.add(key);
      needs.set(q, context);
      return true;
    });
    if (pool.length) groups.push({ topic: t, pool, passages, needs });
  }
  const poolMarks = groups.reduce((a, g) => a + sum(g.pool), 0);
  const target = paperMarks(sub.exams[sub.exams.length - 1], poolMarks);
  if (poolMarks < target) throw new Error(`${gradeDir}/${sub.id}/${paper.id}: only ${poolMarks} marks to choose from`);

  const rnd = random(`${gradeDir}/${sub.id}/${paper.id}`);
  for (const g of groups) {
    const quota = Math.max(3, Math.round((target * sum(g.pool)) / poolMarks));
    g.picked = pickFromTopic(g.pool, Math.min(quota, sum(g.pool)), rnd);
  }

  // Bring the rounded topic shares to exactly the paper's total.
  const total = () => groups.reduce((a, g) => a + sum(g.picked), 0);
  for (let guard = 0; total() !== target; guard++) {
    if (guard > 2000) throw new Error(`${gradeDir}/${sub.id}/${paper.id}: could not reach ${target} marks`);
    const diff = target - total();
    if (diff > 0) {
      const spare = groups.map((g) => ({ g, q: shuffled(g.pool.filter((q) => !g.picked.includes(q) && marksOf(q) <= diff), rnd)[0] }))
        .filter((x) => x.q)
        .sort((a, b) => sum(a.g.picked) / sum(a.g.pool) - sum(b.g.picked) / sum(b.g.pool))[0];
      if (spare) { spare.g.picked.push(spare.q); continue; }
    }
    // Nothing small enough to add, or too many marks: take one out and try again.
    const biggest = groups.filter((g) => g.picked.length > 1).sort((a, b) => sum(b.picked) - sum(a.picked))[0];
    const fits = biggest.picked.filter((q) => diff < 0 ? marksOf(q) <= -diff : marksOf(q) === 1);
    const out = shuffled(fits.length ? fits : biggest.picked, rnd)[0];
    biggest.picked.splice(biggest.picked.indexOf(out), 1);
  }

  const used = groups.filter((g) => g.picked.length);
  const novem = sub.exams[sub.exams.length - 1];
  const taught = paper.terms.filter((n) => inTerm.some((t) => t.term === n));
  // Other subjects have the odd question answered with a number, like a year.
  const hasSums = /^(?:mathemat|natural-sciences|technology|economic)/.test(sub.id) && used.some((g) => g.picked.some((q) => q.type === 'numeric'));
  const covers = taught.length === paper.terms.length
    ? `This paper covers the work of Terms ${paper.terms.join(' and ')}.`
    : `This paper covers the work of Term ${taught[0]}. Term ${paper.terms.find((n) => !taught.includes(n))} is set aside for revision and the final examinations.`;
  const exam = {
    id: paper.id,
    title: paper.title,
    durationMinutes: Math.max(30, Math.ceil((target * novem.durationMinutes) / novem.totalMarks / 5) * 5),
    totalMarks: target,
    instructions: [
      covers,
      `This question paper consists of ${used.length} question${used.length === 1 ? '' : 's'}. Answer ALL the questions.`,
      'Read each question carefully before you answer it.',
      ...(hasSums ? ['Show your working, and round your answers as each question asks.'] : []),
    ],
    sections: used.map((g, i) => {
      // Back into teaching order: the shuffle was only for choosing.
      const chosen = g.pool.filter((q) => g.picked.includes(q));
      const wanted = new Set(chosen.flatMap((q) => g.needs.get(q)));
      const context = g.passages.filter((p) => wanted.has(p));
      return Object.assign(
        { title: `QUESTION ${i + 1}: ${g.topic.title}` },
        context.length ? { instructions: 'Read the text below, then answer the questions.', context } : {},
        { questions: chosen.map((q) => Object.assign({}, q, { id: `${g.topic.id}-${q.id}` })) },
      );
    }),
  };
  return { exam, left, poolMarks, count: exam.sections.reduce((a, s) => a + s.questions.length, 0) };
}

let papers = 0;
let leftOut = 0;
for (const gradeName of fs.readdirSync(ROOT).filter((d) => /^grade-\d+$/.test(d))) {
  const gradeDir = path.join(ROOT, gradeName);
  for (const subjectName of fs.readdirSync(gradeDir)) {
    const dir = path.join(gradeDir, subjectName);
    const subjectFile = path.join(dir, 'subject.json');
    if (!fs.existsSync(subjectFile)) continue;
    const sub = readJson(subjectFile);
    const ids = new Set(PAPERS.map((p) => p.id));
    sub.exams = sub.exams.filter((x) => !ids.has(x.id));
    const topics = sub.topics.map((t) => readJson(path.join(dir, 'topics', `${t.id}.json`)));
    const metas = [];
    for (const paper of PAPERS) {
      const { exam, left, poolMarks, count } = buildPaper(gradeName, sub, topics, paper);
      papers++;
      leftOut += left;
      console.log(`${gradeName}/${subjectName}/${paper.id}: ${exam.totalMarks} marks, ${exam.durationMinutes} min, ${count} questions, ${exam.sections.length} topics (pool ${poolMarks} marks, ${left} need context)`);
      metas.push({ id: exam.id, title: exam.title, durationMinutes: exam.durationMinutes, totalMarks: exam.totalMarks });
      if (!REPORT) writeJson(path.join(dir, 'exams', `${exam.id}.json`), exam);
    }
    // Earlier in the year comes first: the term papers, then the November ones.
    sub.exams = metas.concat(sub.exams);
    // Only the exam list is rewritten, so the file keeps its one-line entries.
    if (!REPORT) {
      const lines = sub.exams.map((x) => `    ${JSON.stringify(x).replace(/^\{/, '{ ').replace(/\}$/, ' }').replace(/":/g, '": ').replace(/,"/g, ', "')}`);
      fs.writeFileSync(subjectFile, fs.readFileSync(subjectFile, 'utf8').replace(/"exams": \[[\s\S]*?\n {2}\]/, () => `"exams": [\n${lines.join(',\n')}\n  ]`));
    }
  }
}
console.log(`${papers} papers; ${leftOut} questions left out because they need notes or an earlier question.`);
