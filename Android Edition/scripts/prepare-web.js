#!/usr/bin/env node
'use strict';
// Builds Android Edition/www from the desktop app, so both editions always have the
// same features and content:
//   ../src      -> www/            (the app itself, with phone tweaks injected)
//   ../content  -> www/content/    (notes, questions, exams, courses)
//   ../audio    -> www/audio/      (read-aloud .ogg files)
//   katex       -> www/katex/
// On a desktop the Electron main process lists folders and reads files; a phone app
// cannot list its own bundled files, so the lists it needs are made here instead:
//   www/data/catalog.json, words.json, audio-index.json, app.json

const fs = require('fs');
const path = require('path');

const HERE = path.join(__dirname, '..');
const ROOT = path.join(HERE, '..');
const WWW = path.join(HERE, 'www');
const ID_RE = /^[a-z0-9]+(-[a-z0-9]+)*$/;

function readJson(file) {
  let text = fs.readFileSync(file, 'utf8');
  if (text.charCodeAt(0) === 0xfeff) text = text.slice(1);
  return JSON.parse(text);
}
function copyDir(from, to, filter) {
  fs.mkdirSync(to, { recursive: true });
  for (const e of fs.readdirSync(from, { withFileTypes: true })) {
    const a = path.join(from, e.name);
    const b = path.join(to, e.name);
    if (e.isDirectory()) copyDir(a, b, filter);
    else if (!filter || filter(e.name)) fs.copyFileSync(a, b);
  }
}
function writeJson(rel, data) {
  const file = path.join(WWW, rel);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(data));
}

// ---------- same logic as main.js (catalog, courses, word list) ----------

function buildCatalog(root) {
  const grades = [];
  for (const dirent of fs.readdirSync(root, { withFileTypes: true })) {
    const m = dirent.isDirectory() && /^grade-(\d+)$/.exec(dirent.name);
    if (!m) continue;
    const subjects = [];
    const gradeDir = path.join(root, dirent.name);
    for (const sub of fs.readdirSync(gradeDir, { withFileTypes: true })) {
      if (!sub.isDirectory()) continue;
      const file = path.join(gradeDir, sub.name, 'subject.json');
      if (!fs.existsSync(file)) continue;
      const s = readJson(file);
      subjects.push({
        id: s.id,
        name: s.name,
        description: s.description || '',
        topics: (s.topics || []).map((t) => ({ id: t.id, title: t.title, term: t.term })),
        exams: (s.exams || []).map((e) => ({ id: e.id, title: e.title, durationMinutes: e.durationMinutes, totalMarks: e.totalMarks })),
      });
    }
    subjects.sort((a, b) => a.name.localeCompare(b.name));
    grades.push({ grade: Number(m[1]), subjects });
  }
  grades.sort((a, b) => a.grade - b.grade);
  return { grades };
}

function listCourses(dir) {
  const out = [];
  if (!fs.existsSync(dir)) return out;
  for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
    const file = path.join(dir, d.name, 'course.json');
    if (!d.isDirectory() || !ID_RE.test(d.name) || !fs.existsSync(file)) continue;
    const c = readJson(file);
    out.push({ id: d.name, name: c.name, about: c.about, grade: c.grade, level: c.level, units: c.units.length, lessons: c.units.reduce((n, u) => n + u.lessons.length, 0) });
  }
  return out;
}

function buildWordList(root) {
  const words = new Set();
  const addText = (t) => {
    for (const w of String(t).normalize('NFKC').toLowerCase().match(/\p{L}[\p{L}']*/gu) || []) words.add(w.replace(/'+$/, ''));
  };
  const walkValue = (v) => {
    if (typeof v === 'string') addText(v);
    else if (Array.isArray(v)) v.forEach(walkValue);
    else if (v && typeof v === 'object') Object.values(v).forEach(walkValue);
  };
  const walkDir = (d) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) walkDir(p);
      else if (e.name.endsWith('.json')) { try { walkValue(JSON.parse(fs.readFileSync(p, 'utf8'))); } catch (err) { /* skip */ } }
    }
  };
  walkDir(root);
  return [...words];
}

// Every folder of audio/ that holds .ogg files -> the 8-character ids in it.
function buildAudioIndex(root) {
  const index = {};
  const walk = (d, rel) => {
    const ids = [];
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      if (e.isDirectory()) walk(path.join(d, e.name), rel ? `${rel}/${e.name}` : e.name);
      else if (/^[0-9a-f]{8}\.ogg$/.test(e.name)) ids.push(e.name.slice(0, 8));
    }
    if (ids.length) index[rel] = ids;
  };
  if (fs.existsSync(root)) walk(root, '');
  return index;
}

// ---------- index.html ----------

function patchIndex(html) {
  const out = html
    .replace(/\.\.\/node_modules\/katex\/dist\//g, 'katex/')
    // Content is read with fetch() from the app's own files, and audio is played from them.
    .replace("connect-src 'none'", "connect-src 'self'")
    .replace('media-src blob:', "media-src 'self' blob:")
    .replace('<meta charset="utf-8">', '<meta charset="utf-8">\n  <meta name="viewport" content="width=device-width, initial-scale=1, interactive-widget=resizes-content">')
    .replace('<link rel="stylesheet" href="styles.css">', '<link rel="stylesheet" href="styles.css">\n  <link rel="stylesheet" href="mobile.css">')
    // The phone back-end (replaces Electron's preload.js) must exist before app.js runs.
    .replace('<script src="js/motion.js"></script>', '<script src="android-api.js"></script>\n  <script src="js/motion.js"></script>');
  for (const must of ['katex/katex.min.js', "connect-src 'self'", 'mobile.css', 'android-api.js', 'name="viewport"']) {
    if (!out.includes(must)) throw new Error(`index.html changed: could not add ${must}. Update scripts/prepare-web.js.`);
  }
  return out;
}

// Wording that only makes sense on a computer.
function patchAppText(js) {
  return js.replace(/this computer/g, 'this phone');
}

// ---------- build ----------

function main() {
  const t0 = Date.now();
  fs.rmSync(WWW, { recursive: true, force: true });
  fs.mkdirSync(WWW, { recursive: true });

  copyDir(path.join(ROOT, 'src'), WWW);
  fs.writeFileSync(path.join(WWW, 'index.html'), patchIndex(fs.readFileSync(path.join(ROOT, 'src', 'index.html'), 'utf8')));
  for (const f of ['app.js', 'buddy.js', 'course-ui.js']) {
    const p = path.join(WWW, 'js', f);
    fs.writeFileSync(p, patchAppText(fs.readFileSync(p, 'utf8')));
  }
  copyDir(path.join(HERE, 'web'), WWW);

  const katexDir = path.dirname(require.resolve('katex/package.json', { paths: [ROOT] }));
  copyDir(path.join(katexDir, 'dist'), path.join(WWW, 'katex'), (n) => n === 'katex.min.js' || n === 'katex.min.css' || /\.(woff2?|ttf)$/.test(n));

  const content = path.join(ROOT, 'content');
  copyDir(content, path.join(WWW, 'content'), (n) => n.endsWith('.json'));
  copyDir(path.join(ROOT, 'audio'), path.join(WWW, 'audio'), (n) => n.endsWith('.ogg'));

  const catalog = buildCatalog(content);
  catalog.courses = listCourses(path.join(content, 'courses'));
  writeJson('data/catalog.json', catalog);
  writeJson('data/words.json', buildWordList(content));
  writeJson('data/audio-index.json', buildAudioIndex(path.join(ROOT, 'audio')));
  writeJson('data/app.json', { version: require(path.join(ROOT, 'package.json')).version });

  const subjects = catalog.grades.reduce((n, g) => n + g.subjects.length, 0);
  console.log(`www ready: ${catalog.grades.length} grades, ${subjects} subjects, ${catalog.courses.length} courses (${((Date.now() - t0) / 1000).toFixed(1)}s)`);
}

main();
