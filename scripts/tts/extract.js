#!/usr/bin/env node
'use strict';
// Lists every notes section that needs audio and writes tts/jobs.json for tts/generate.py.
// Audio files are named by a hash of the spoken text, so only new or changed sections are
// regenerated, and files for sections that no longer exist are removed (unless --keep).
//
//   node scripts/tts/extract.js [content/grade-8 ...] [--keep]

const fs = require('fs');
const path = require('path');
const Sections = require('../../src/js/sections.js');

const ROOT = path.join(__dirname, '..', '..');
const CONTENT = path.join(ROOT, 'content');
const AUDIO = path.join(ROOT, 'audio');
const argv = process.argv.slice(2);
const keep = argv.includes('--keep');
const only = argv.filter((a) => !a.startsWith('--')).map((p) => path.resolve(p));

const jobs = [];
const wanted = new Set();
let sections = 0;

for (const g of fs.readdirSync(CONTENT).filter((d) => /^grade-\d+$/.test(d))) {
  for (const subj of fs.readdirSync(path.join(CONTENT, g))) {
    const sfile = path.join(CONTENT, g, subj, 'subject.json');
    if (!fs.existsSync(sfile)) continue;
    const subject = JSON.parse(fs.readFileSync(sfile, 'utf8'));
    for (const t of subject.topics || []) {
      const tfile = path.join(CONTENT, g, subj, 'topics', `${t.id}.json`);
      if (!fs.existsSync(tfile)) continue;
      const dir = path.join(AUDIO, g, subj, t.id);
      const inScope = !only.length || only.some((p) => tfile.startsWith(p));
      const topic = JSON.parse(fs.readFileSync(tfile, 'utf8'));
      for (const s of Sections.split(topic.notes)) {
        const out = path.join(dir, `${s.id}.ogg`);
        wanted.add(out);
        sections++;
        if (inScope && !fs.existsSync(out)) jobs.push({ out, text: s.text, label: `${g}/${subj}/${t.id}#${s.index + 1}` });
      }
    }
  }
}

let removed = 0;
function prune(dir) {
  if (!fs.existsSync(dir)) return;
  for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, d.name);
    if (d.isDirectory()) { prune(p); if (!fs.readdirSync(p).length) fs.rmdirSync(p); }
    else if (d.name.endsWith('.ogg') && !wanted.has(p)) { fs.unlinkSync(p); removed++; }
  }
}
if (!keep && !only.length) prune(AUDIO);

fs.mkdirSync(path.join(ROOT, 'tts'), { recursive: true });
fs.writeFileSync(path.join(ROOT, 'tts', 'jobs.json'), JSON.stringify(jobs));
const chars = jobs.reduce((n, j) => n + j.text.length, 0);
console.log(`${sections} sections in total; ${jobs.length} need audio (${Math.round(chars / 1000)}k characters); ${removed} stale files removed.`);
