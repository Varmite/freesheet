'use strict';
// Notes, questions, exams, language courses and read-aloud audio, read from the repo's
// content/ and audio/ folders. This is a Node copy of src-tauri/src/content.rs and the
// audio commands in src-tauri/src/lib.rs, and must give the app the same JSON shapes.
//
//   content/grade-<n>/<subject>/subject.json
//   content/grade-<n>/<subject>/topics/<topic>.json
//   content/grade-<n>/<subject>/exams/<exam>.json
//   content/courses/<course>/course.json
//   audio/grade-<n>/<subject>/<topic>/<id>.ogg
//   audio/courses/<course>/<id>.ogg

const fs = require('fs');
const path = require('path');

// Ids used in file names: lower-case words joined by single hyphens. Anything else
// (dots, slashes, %2e%2e, backslashes) is refused before it can touch a path.
const ID = /^[a-z0-9]+(-[a-z0-9]+)*$/;
const AUDIO_ID = /^[0-9a-f]{8}$/;

const validId = (id) => typeof id === 'string' && ID.test(id);
const validAudioId = (id) => typeof id === 'string' && AUDIO_ID.test(id);
function validGrade(g) {
  const s = String(g);
  if (!/^\d{1,2}$/.test(s)) return null;
  const n = Number(s);
  return n >= 1 && n <= 12 ? n : null;
}

const parse = (text) => JSON.parse(text.replace(/^﻿/, ''));
const readJson = (file) => parse(fs.readFileSync(file, 'utf8'));
const dirsIn = (dir) => (fs.existsSync(dir)
  ? fs.readdirSync(dir, { withFileTypes: true }).filter((e) => e.isDirectory()).map((e) => e.name)
  : []);

function createContent(root) {
  const CONTENT = path.join(root, 'content');
  const AUDIO = path.join(root, 'audio');

  // The subject list entry, as subject_summary() in content.rs.
  function subjectSummary(s) {
    const list = (k) => (Array.isArray(s[k]) ? s[k] : []);
    const v = (x) => (x === undefined ? null : x);
    return {
      id: v(s.id),
      name: v(s.name),
      description: typeof s.description === 'string' ? s.description : '',
      topics: list('topics').map((t) => ({ id: v(t.id), title: v(t.title), term: v(t.term) })),
      exams: list('exams').map((e) => ({ id: v(e.id), title: v(e.title), durationMinutes: v(e.durationMinutes), totalMarks: v(e.totalMarks) })),
    };
  }

  // Language courses are shown as a subject in every grade.
  function listCourses() {
    const out = [];
    for (const id of dirsIn(path.join(CONTENT, 'courses')).sort()) {
      const file = path.join(CONTENT, 'courses', id, 'course.json');
      if (!validId(id) || !fs.existsSync(file)) continue;
      try {
        const c = readJson(file);
        const units = Array.isArray(c.units) ? c.units : [];
        const v = (x) => (x === undefined ? null : x);
        out.push({
          id, name: v(c.name), about: v(c.about), grade: v(c.grade), level: v(c.level),
          units: units.length,
          lessons: units.reduce((n, u) => n + (Array.isArray(u.lessons) ? u.lessons.length : 0), 0),
        });
      } catch (e) {
        console.error(`Bad course file ${file}: ${e.message}`);
      }
    }
    return out;
  }

  function buildCatalog() {
    const grades = [];
    for (const name of dirsIn(CONTENT)) {
      const m = /^grade-(\d+)$/.exec(name);
      if (!m) continue;
      const subjects = [];
      for (const sub of dirsIn(path.join(CONTENT, name))) {
        const file = path.join(CONTENT, name, sub, 'subject.json');
        if (!fs.existsSync(file)) continue;
        try { subjects.push(subjectSummary(readJson(file))); } catch (e) { console.error(`Could not read ${file}: ${e.message}`); }
      }
      const key = (s) => String(s.name || '').toLowerCase();
      subjects.sort((a, b) => (key(a) < key(b) ? -1 : key(a) > key(b) ? 1 : 0));
      grades.push({ grade: Number(m[1]), subjects });
    }
    grades.sort((a, b) => a.grade - b.grade);
    return { grades, courses: listCourses() };
  }

  // Every word in the content, for telling spelling slips from wrong answers
  // (see typoOf in src/js/marking.js). Order doesn't matter: the app makes a Set.
  function buildWords() {
    const seen = new Set();
    const add = (text) => {
      for (const m of text.normalize('NFKC').toLowerCase().matchAll(/\p{L}[\p{L}']*/gu)) {
        seen.add(m[0].replace(/'+$/, ''));
      }
    };
    const walk = (v) => {
      if (typeof v === 'string') add(v);
      else if (Array.isArray(v)) v.forEach(walk);
      else if (v && typeof v === 'object') Object.values(v).forEach(walk);
    };
    const files = (dir) => {
      for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
        const p = path.join(dir, e.name);
        if (e.isDirectory()) files(p);
        else if (e.name.endsWith('.json')) { try { walk(readJson(p)); } catch { /* skipped, as in Rust */ } }
      }
    };
    files(CONTENT);
    return [...seen];
  }

  // Built once at start-up: the content only changes when the server is updated.
  const catalog = buildCatalog();
  const words = buildWords();

  // A topic or exam file, or null if the ids are bad or it doesn't exist.
  function item(grade, subject, kind, id) {
    const g = validGrade(grade);
    if (!g || !validId(subject) || !validId(id)) return null;
    const file = path.join(CONTENT, `grade-${g}`, subject, kind, `${id}.json`);
    return fs.existsSync(file) ? readJson(file) : null;
  }

  function course(id) {
    if (!validId(id)) return null;
    const file = path.join(CONTENT, 'courses', id, 'course.json');
    return fs.existsSync(file) ? readJson(file) : null;
  }

  // Audio folders, relative to audio/; null for bad ids.
  function topicAudioFolder(grade, subject, topic) {
    const g = validGrade(grade);
    return g && validId(subject) && validId(topic) ? path.join(AUDIO, `grade-${g}`, subject, topic) : null;
  }
  const courseAudioFolder = (id) => (validId(id) ? path.join(AUDIO, 'courses', id) : null);

  // The ids of the .ogg files in a folder, sorted (ids_from_names in audio.rs).
  function audioList(folder) {
    if (!folder) return null;
    let names = [];
    try { names = fs.readdirSync(folder); } catch { /* no audio for this topic */ }
    return names.filter((n) => n.endsWith('.ogg')).map((n) => n.slice(0, -4)).filter(validAudioId).sort();
  }
  // The path of one audio file, or null.
  function audioFile(folder, id) {
    if (!folder || !validAudioId(id)) return null;
    const file = path.join(folder, `${id}.ogg`);
    return fs.existsSync(file) ? file : null;
  }

  return { catalog, words, item, course, topicAudioFolder, courseAudioFolder, audioList, audioFile };
}

module.exports = { createContent, validId, validAudioId, validGrade };
