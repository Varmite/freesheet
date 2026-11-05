#!/usr/bin/env node
'use strict';
// Validates FreeSheet content against docs/CONTENT_FORMAT.md.
// Usage: node scripts/validate-content.js [path ...]
//   path may be content/, content/grade-10, or content/grade-10/mathematics (default: content/)
// Exit code 1 if any error is found. Warnings do not fail the run.

const fs = require('fs');
const path = require('path');
const katex = require('katex');
const Markup = require('../src/js/markup.js');

const ROOT = path.join(__dirname, '..', 'content');
const ID_RE = /^[a-z0-9]+(-[a-z0-9]+)*$/;
const BLOCK_TYPES = ['heading', 'paragraph', 'list', 'definition', 'example', 'callout', 'table', 'formula', 'passage', 'diagram'];
const Q_TYPES = ['mcq', 'truefalse', 'short', 'numeric', 'match', 'open'];

let errors = 0;
let warnings = 0;
const stats = { subjects: 0, topics: 0, exams: 0, questions: 0, examQuestions: 0 };

function err(file, msg) { errors++; console.log(`ERROR  ${rel(file)}: ${msg}`); }
function warn(file, msg) { warnings++; console.log(`warn   ${rel(file)}: ${msg}`); }
function rel(f) { return path.relative(path.join(__dirname, '..'), f).replace(/\\/g, '/'); }

function readJson(file) {
  let raw;
  try { raw = fs.readFileSync(file, 'utf8'); } catch (e) { err(file, 'cannot read file'); return null; }
  if (raw.charCodeAt(0) === 0xfeff) { err(file, 'file starts with a BOM; save as UTF-8 without BOM'); raw = raw.slice(1); }
  try { return JSON.parse(raw); } catch (e) { err(file, `invalid JSON: ${e.message}`); return null; }
}

const isStr = (v) => typeof v === 'string' && v.trim() !== '';

function checkText(file, where, v, required = true) {
  if (v == null || v === '') { if (required) err(file, `${where}: missing text`); return; }
  if (typeof v !== 'string') { err(file, `${where}: must be a string`); return; }
  if (/<\/?(b|i|br|p|div|span|sup|sub|strong|em|ul|li)\b[^>]*>/i.test(v)) err(file, `${where}: contains HTML tags; use the documented markup instead`);
  const segs = Markup.segments(v);
  for (const s of segs) {
    if (s.unclosed) err(file, `${where}: unclosed $ maths (use \\$ for a literal dollar sign): "${v.slice(0, 80)}"`);
    if (s.math) checkLatex(file, where, s.text, false);
  }
}

function checkLatex(file, where, tex, display) {
  if (!isStr(tex)) { err(file, `${where}: empty maths`); return; }
  try {
    katex.renderToString(tex, { displayMode: display, throwOnError: true, strict: 'ignore' });
  } catch (e) {
    err(file, `${where}: KaTeX error "${e.message.replace(/\s+/g, ' ').slice(0, 160)}" in $${tex}$`);
  }
}

function checkBlocks(file, where, blocks) {
  if (!Array.isArray(blocks)) { err(file, `${where}: must be an array`); return; }
  blocks.forEach((b, i) => {
    const w = `${where}[${i}]`;
    if (!b || typeof b !== 'object') { err(file, `${w}: not an object`); return; }
    if (!BLOCK_TYPES.includes(b.type)) { err(file, `${w}: unknown block type "${b.type}"`); return; }
    switch (b.type) {
      case 'heading': case 'paragraph': checkText(file, `${w}.text`, b.text); break;
      case 'list':
        if (!Array.isArray(b.items) || !b.items.length) err(file, `${w}.items: needs at least one item`);
        else b.items.forEach((it, j) => checkText(file, `${w}.items[${j}]`, it));
        break;
      case 'definition': checkText(file, `${w}.term`, b.term); checkText(file, `${w}.text`, b.text); break;
      case 'example':
        checkText(file, `${w}.title`, b.title, false);
        checkText(file, `${w}.problem`, b.problem);
        if (b.steps != null) {
          if (!Array.isArray(b.steps)) err(file, `${w}.steps: must be an array`);
          else b.steps.forEach((s, j) => checkText(file, `${w}.steps[${j}]`, s));
        }
        checkText(file, `${w}.answer`, b.answer, false);
        if (!(b.steps && b.steps.length) && !b.answer) err(file, `${w}: example needs steps or an answer`);
        break;
      case 'callout': checkText(file, `${w}.label`, b.label); checkText(file, `${w}.text`, b.text); break;
      case 'table':
        checkText(file, `${w}.caption`, b.caption, false);
        if (!Array.isArray(b.headers) || !b.headers.length) { err(file, `${w}.headers: required`); break; }
        b.headers.forEach((h, j) => checkText(file, `${w}.headers[${j}]`, h, false));
        if (!Array.isArray(b.rows) || !b.rows.length) { err(file, `${w}.rows: required`); break; }
        b.rows.forEach((r, j) => {
          if (!Array.isArray(r)) { err(file, `${w}.rows[${j}]: must be an array`); return; }
          if (r.length !== b.headers.length) err(file, `${w}.rows[${j}]: has ${r.length} cells but there are ${b.headers.length} headers`);
          r.forEach((c, k) => {
            if (typeof c === 'number') return;
            checkText(file, `${w}.rows[${j}][${k}]`, c, false);
          });
        });
        break;
      case 'formula': checkLatex(file, `${w}.latex`, b.latex, true); checkText(file, `${w}.caption`, b.caption, false); break;
      case 'passage': checkText(file, `${w}.title`, b.title, false); checkText(file, `${w}.text`, b.text); break;
      case 'diagram':
        if (!isStr(b.svg) || !/^\s*<svg[\s>]/.test(b.svg) || !/<\/svg>\s*$/.test(b.svg)) err(file, `${w}.svg: must be a complete <svg>...</svg> string`);
        else {
          if (!/viewBox\s*=/.test(b.svg)) err(file, `${w}.svg: needs a viewBox`);
          if (/<script|on[a-z]+\s*=|href\s*=\s*["']?(https?:|javascript:)/i.test(b.svg)) err(file, `${w}.svg: scripts, event handlers and external links are not allowed`);
          if (!/xmlns\s*=\s*["']http:\/\/www\.w3\.org\/2000\/svg["']/.test(b.svg)) err(file, `${w}.svg: needs xmlns="http://www.w3.org/2000/svg" (required when shown as an image)`);
        }
        checkText(file, `${w}.caption`, b.caption);
        break;
    }
  });
}

function checkQuestion(file, where, q, ids) {
  if (!q || typeof q !== 'object') { err(file, `${where}: not an object`); return 0; }
  if (!isStr(q.id) || !ID_RE.test(q.id)) err(file, `${where}.id: missing or not kebab-case ("${q.id}")`);
  else if (ids.has(q.id)) err(file, `${where}.id: duplicate id "${q.id}"`);
  else ids.add(q.id);
  const w = `${where}(${q.id})`;
  if (!Q_TYPES.includes(q.type)) { err(file, `${w}.type: unknown "${q.type}"`); return 0; }
  checkText(file, `${w}.prompt`, q.prompt);
  checkText(file, `${w}.explanation`, q.explanation, false);
  let marks = 1;
  if (q.marks != null) {
    if (!Number.isInteger(q.marks) || q.marks < 1) err(file, `${w}.marks: must be a positive integer`);
    else marks = q.marks;
  }
  switch (q.type) {
    case 'mcq':
      if (!Array.isArray(q.options) || q.options.length < 2 || q.options.length > 6) { err(file, `${w}.options: need 2-6 options`); break; }
      q.options.forEach((o, i) => checkText(file, `${w}.options[${i}]`, o));
      if (new Set(q.options.map((o) => String(o).trim().toLowerCase())).size !== q.options.length) err(file, `${w}.options: duplicate options`);
      if (!Number.isInteger(q.answer) || q.answer < 0 || q.answer >= q.options.length) err(file, `${w}.answer: must be a valid 0-based option index`);
      break;
    case 'truefalse':
      if (typeof q.answer !== 'boolean') err(file, `${w}.answer: must be true or false`);
      break;
    case 'short':
      if (!Array.isArray(q.answers) || !q.answers.length || !q.answers.every(isStr)) err(file, `${w}.answers: need at least one accepted answer string`);
      else if (q.answers.some((a) => a.split(/\s+/).length > 6)) warn(file, `${w}.answers: long answers are hard to auto-mark; consider type "open"`);
      break;
    case 'numeric':
      if (typeof q.answer !== 'number' || !Number.isFinite(q.answer)) err(file, `${w}.answer: must be a number`);
      if (q.tolerance != null && (typeof q.tolerance !== 'number' || q.tolerance < 0)) err(file, `${w}.tolerance: must be a non-negative number`);
      if (q.unit != null) checkText(file, `${w}.unit`, q.unit);
      break;
    case 'match':
      if (!Array.isArray(q.pairs) || q.pairs.length < 2 || q.pairs.length > 8) { err(file, `${w}.pairs: need 2-8 pairs`); break; }
      q.pairs.forEach((p, i) => {
        if (!p || typeof p !== 'object') { err(file, `${w}.pairs[${i}]: not an object`); return; }
        checkText(file, `${w}.pairs[${i}].left`, p.left); checkText(file, `${w}.pairs[${i}].right`, p.right);
        if (typeof p.right === 'string' && /[$*]/.test(p.right)) err(file, `${w}.pairs[${i}].right: must be plain text (no $maths$ or * markup); it is shown in a drop-down`);
      });
      if (new Set(q.pairs.map((p) => p && p.right)).size !== q.pairs.length) err(file, `${w}.pairs: right-hand values must be distinct`);
      if (marks !== q.pairs.length) warn(file, `${w}: match marks (${marks}) should equal the number of pairs (${q.pairs.length})`);
      break;
    case 'open':
      checkText(file, `${w}.memo`, q.memo);
      break;
  }
  return marks;
}

function checkTopic(file, meta) {
  const t = readJson(file);
  if (!t) return;
  stats.topics++;
  if (t.id !== meta.id) err(file, `id "${t.id}" does not match file name / subject.json ("${meta.id}")`);
  if (t.title !== meta.title) err(file, `title "${t.title}" does not match subject.json ("${meta.title}")`);
  if (t.term !== meta.term) err(file, `term ${t.term} does not match subject.json (${meta.term})`);
  checkText(file, 'summary', t.summary);
  checkBlocks(file, 'notes', t.notes);
  if (Array.isArray(t.notes) && t.notes.length < 6) warn(file, `notes are very short (${t.notes.length} blocks)`);
  if (!Array.isArray(t.questions) || !t.questions.length) { err(file, 'questions: need at least one question'); return; }
  if (t.questions.length < 6) warn(file, `only ${t.questions.length} practice questions`);
  const ids = new Set();
  t.questions.forEach((q, i) => { checkQuestion(file, `questions[${i}]`, q, ids); stats.questions++; });
}

function checkExam(file, meta) {
  const x = readJson(file);
  if (!x) return;
  stats.exams++;
  if (x.id !== meta.id) err(file, `id "${x.id}" does not match ("${meta.id}")`);
  if (x.title !== meta.title) err(file, `title does not match subject.json`);
  if (!Number.isInteger(x.durationMinutes) || x.durationMinutes < 10) err(file, 'durationMinutes: must be an integer >= 10');
  if (x.durationMinutes !== meta.durationMinutes) err(file, 'durationMinutes does not match subject.json');
  if (x.totalMarks !== meta.totalMarks) err(file, 'totalMarks does not match subject.json');
  if (!Array.isArray(x.instructions)) err(file, 'instructions: must be an array of strings');
  else x.instructions.forEach((s, i) => checkText(file, `instructions[${i}]`, s));
  if (!Array.isArray(x.sections) || !x.sections.length) { err(file, 'sections: required'); return; }
  const ids = new Set();
  let sum = 0;
  x.sections.forEach((s, i) => {
    const w = `sections[${i}]`;
    checkText(file, `${w}.title`, s.title);
    checkText(file, `${w}.instructions`, s.instructions, false);
    if (s.context != null) checkBlocks(file, `${w}.context`, s.context);
    if (!Array.isArray(s.questions) || !s.questions.length) { err(file, `${w}.questions: required`); return; }
    s.questions.forEach((q, j) => { sum += checkQuestion(file, `${w}.questions[${j}]`, q, ids); stats.examQuestions++; });
  });
  if (sum !== x.totalMarks) err(file, `totalMarks is ${x.totalMarks} but questions add up to ${sum}`);
}

function checkSubject(dir) {
  const file = path.join(dir, 'subject.json');
  const s = readJson(file);
  if (!s) return;
  stats.subjects++;
  const gradeMatch = /grade-(\d+)$/.exec(path.dirname(dir));
  if (s.schemaVersion !== 1) err(file, 'schemaVersion must be 1');
  if (!gradeMatch || s.grade !== Number(gradeMatch[1])) err(file, `grade ${s.grade} does not match folder`);
  if (s.id !== path.basename(dir)) err(file, `id "${s.id}" does not match folder name "${path.basename(dir)}"`);
  checkText(file, 'name', s.name);
  checkText(file, 'description', s.description);

  const topicIds = new Set();
  if (!Array.isArray(s.topics) || !s.topics.length) err(file, 'topics: required');
  else s.topics.forEach((t, i) => {
    if (!t || !isStr(t.id) || !ID_RE.test(t.id)) { err(file, `topics[${i}].id invalid`); return; }
    if (topicIds.has(t.id)) err(file, `topics[${i}]: duplicate id ${t.id}`);
    topicIds.add(t.id);
    checkText(file, `topics[${i}].title`, t.title);
    if (![1, 2, 3, 4].includes(t.term)) err(file, `topics[${i}].term must be 1-4`);
    const tf = path.join(dir, 'topics', `${t.id}.json`);
    if (!fs.existsSync(tf)) err(file, `topics[${i}]: missing file topics/${t.id}.json`);
    else checkTopic(tf, t);
  });
  for (const f of listJson(path.join(dir, 'topics'))) {
    if (!topicIds.has(path.basename(f, '.json'))) err(f, 'topic file is not listed in subject.json');
  }

  const examIds = new Set();
  if (!Array.isArray(s.exams)) err(file, 'exams: must be an array');
  else s.exams.forEach((x, i) => {
    if (!x || !isStr(x.id) || !ID_RE.test(x.id)) { err(file, `exams[${i}].id invalid`); return; }
    if (examIds.has(x.id)) err(file, `exams[${i}]: duplicate id ${x.id}`);
    examIds.add(x.id);
    checkText(file, `exams[${i}].title`, x.title);
    const xf = path.join(dir, 'exams', `${x.id}.json`);
    if (!fs.existsSync(xf)) err(file, `exams[${i}]: missing file exams/${x.id}.json`);
    else checkExam(xf, x);
  });
  for (const f of listJson(path.join(dir, 'exams'))) {
    if (!examIds.has(path.basename(f, '.json'))) err(f, 'exam file is not listed in subject.json');
  }
}

function listJson(dir) {
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir).filter((f) => f.endsWith('.json')).map((f) => path.join(dir, f));
}

function subjectDirsUnder(p) {
  const abs = path.resolve(p);
  if (fs.existsSync(path.join(abs, 'subject.json'))) return [abs];
  const out = [];
  if (!fs.existsSync(abs)) { err(abs, 'path does not exist'); return out; }
  for (const d of fs.readdirSync(abs, { withFileTypes: true })) {
    if (d.isDirectory()) out.push(...subjectDirsUnder(path.join(abs, d.name)));
  }
  return out;
}

// A single topic/exam file can be validated on its own (used while content is being written):
//   node scripts/validate-content.js content/grade-10/mathematics/topics/exponents.json
function checkSingleFile(file) {
  const abs = path.resolve(file);
  const kind = path.basename(path.dirname(abs));
  const subjectFile = path.join(path.dirname(path.dirname(abs)), 'subject.json');
  const id = path.basename(abs, '.json');
  const s = fs.existsSync(subjectFile) ? readJson(subjectFile) : null;
  if (!s) { err(abs, 'cannot find subject.json for this file'); return; }
  const list = kind === 'topics' ? s.topics : kind === 'exams' ? s.exams : null;
  if (!list) { err(abs, 'file must be inside a topics/ or exams/ folder'); return; }
  const meta = list.find((m) => m && m.id === id);
  if (!meta) { err(abs, `"${id}" is not listed in subject.json ${kind}`); return; }
  if (kind === 'topics') checkTopic(abs, meta); else checkExam(abs, meta);
}

const targets = process.argv.slice(2);
const files = targets.filter((t) => t.endsWith('.json'));
const dirTargets = targets.filter((t) => !t.endsWith('.json'));
files.forEach(checkSingleFile);
const dirs = (dirTargets.length || files.length ? dirTargets : [ROOT]).flatMap(subjectDirsUnder);
dirs.forEach(checkSubject);
console.log(`\n${stats.subjects} subjects, ${stats.topics} topics, ${stats.questions} practice questions, ` +
  `${stats.exams} exams (${stats.examQuestions} questions). ${errors} errors, ${warnings} warnings.`);
process.exit(errors ? 1 : 0);
