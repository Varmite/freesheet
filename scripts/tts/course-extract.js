#!/usr/bin/env node
'use strict';
// Lists every Afrikaans word and sentence in the language courses that needs audio,
// with the file name the app looks for. Record them (a native speaker is best; there
// is no good offline Afrikaans text-to-speech voice yet), save each as Ogg
// (Opus or Vorbis) at audio/courses/<course>/<id>.ogg, and the app picks them up:
// speaker buttons appear and "Tap what you hear" listening exercises switch on.
//
//   node scripts/tts/course-extract.js            -> tts/course-phrases.csv (missing ones only)
//   node scripts/tts/course-extract.js --all      -> every phrase

const fs = require('fs');
const path = require('path');
const Course = require('../../src/js/course.js');

const ROOT = path.join(__dirname, '..', '..');
const all = process.argv.includes('--all');
const rows = [['course', 'id', 'file', 'text']];
let total = 0, have = 0;
const dir = path.join(ROOT, 'content', 'courses');
for (const id of fs.existsSync(dir) ? fs.readdirSync(dir) : []) {
  const course = JSON.parse(fs.readFileSync(path.join(dir, id, 'course.json'), 'utf8'));
  for (const item of Course.speechItems(course)) {
    total++;
    const file = path.join('audio', 'courses', id, `${item.id}.ogg`);
    const exists = fs.existsSync(path.join(ROOT, file));
    if (exists) have++;
    if (all || !exists) rows.push([id, item.id, file.replace(/\\/g, '/'), item.text]);
  }
}
const csv = rows.map((r) => r.map((c) => (/[",\n]/.test(c) ? `"${String(c).replace(/"/g, '""')}"` : c)).join(',')).join('\n') + '\n';
fs.mkdirSync(path.join(ROOT, 'tts'), { recursive: true });
fs.writeFileSync(path.join(ROOT, 'tts', 'course-phrases.csv'), csv);
console.log(`${total} phrases; ${have} recorded; wrote ${rows.length - 1} to tts/course-phrases.csv`);
