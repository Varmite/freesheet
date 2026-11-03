'use strict';
// FreeSheet - Electron main process.
// Responsibilities: create the window, serve content from the local content/ folder,
// and persist learner progress to a JSON file in the OS user-data directory.

const { app, BrowserWindow, ipcMain, dialog, Menu, shell } = require('electron');
const path = require('path');
const fs = require('fs');

const ID_RE = /^[a-z0-9]+(-[a-z0-9]+)*$/;

function contentDir() {
  return app.isPackaged
    ? path.join(process.resourcesPath, 'content')
    : path.join(__dirname, 'content');
}

function progressFile() {
  return path.join(app.getPath('userData'), 'progress.json');
}

function readJson(file) {
  let text = fs.readFileSync(file, 'utf8');
  if (text.charCodeAt(0) === 0xfeff) text = text.slice(1);
  return JSON.parse(text);
}

function assertId(id, what) {
  if (typeof id !== 'string' || !ID_RE.test(id)) throw new Error(`Invalid ${what}`);
}

function assertGrade(grade) {
  const g = Number(grade);
  if (!Number.isInteger(g) || g < 1 || g > 12) throw new Error('Invalid grade');
  return g;
}

// ---------- Content ----------

let catalogCache = null;

function buildCatalog() {
  const root = contentDir();
  const grades = [];
  if (!fs.existsSync(root)) return { grades };
  for (const dirent of fs.readdirSync(root, { withFileTypes: true })) {
    const m = dirent.isDirectory() && /^grade-(\d+)$/.exec(dirent.name);
    if (!m) continue;
    const grade = Number(m[1]);
    const subjects = [];
    const gradeDir = path.join(root, dirent.name);
    for (const sub of fs.readdirSync(gradeDir, { withFileTypes: true })) {
      if (!sub.isDirectory()) continue;
      const file = path.join(gradeDir, sub.name, 'subject.json');
      if (!fs.existsSync(file)) continue;
      try {
        const s = readJson(file);
        subjects.push({
          id: s.id,
          name: s.name,
          description: s.description || '',
          topics: (s.topics || []).map((t) => ({ id: t.id, title: t.title, term: t.term })),
          exams: (s.exams || []).map((e) => ({
            id: e.id, title: e.title, durationMinutes: e.durationMinutes, totalMarks: e.totalMarks,
          })),
        });
      } catch (err) {
        console.error(`Could not read ${file}:`, err.message);
      }
    }
    subjects.sort((a, b) => a.name.localeCompare(b.name));
    grades.push({ grade, subjects });
  }
  grades.sort((a, b) => a.grade - b.grade);
  return { grades };
}

function readContentFile(grade, subjectId, kind, id) {
  const g = assertGrade(grade);
  assertId(subjectId, 'subject');
  assertId(id, kind === 'topics' ? 'topic' : 'exam');
  const file = path.join(contentDir(), `grade-${g}`, subjectId, kind, `${id}.json`);
  return readJson(file);
}

// ---------- Progress ----------

function emptyProgress() {
  return { version: 1, settings: {}, topics: {}, exams: {}, examDrafts: {} };
}

function loadProgress() {
  const file = progressFile();
  for (const candidate of [file, `${file}.bak`]) {
    try {
      if (fs.existsSync(candidate)) {
        const data = readJson(candidate);
        if (data && typeof data === 'object') return Object.assign(emptyProgress(), data);
      }
    } catch (err) {
      console.error(`Progress file ${candidate} unreadable:`, err.message);
    }
  }
  return emptyProgress();
}

// Serialise writes so two saves never interleave.
let writeChain = Promise.resolve();
function saveProgress(data) {
  const json = JSON.stringify(data);
  writeChain = writeChain.then(async () => {
    const file = progressFile();
    await fs.promises.mkdir(path.dirname(file), { recursive: true });
    const tmp = `${file}.tmp`;
    await fs.promises.writeFile(tmp, json, 'utf8');
    if (fs.existsSync(file)) {
      await fs.promises.copyFile(file, `${file}.bak`).catch(() => {});
    }
    await fs.promises.rename(tmp, file);
  }).catch((err) => {
    console.error('Saving progress failed:', err);
    throw err;
  });
  return writeChain.then(() => true);
}

function saveProgressSync(data) {
  const file = progressFile();
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const tmp = `${file}.tmp`;
  fs.writeFileSync(tmp, JSON.stringify(data), 'utf8');
  fs.renameSync(tmp, file);
}

// ---------- IPC ----------

function registerIpc() {
  ipcMain.handle('catalog:get', () => {
    if (!catalogCache) catalogCache = buildCatalog();
    return catalogCache;
  });
  ipcMain.handle('topic:get', (_e, grade, subjectId, topicId) =>
    readContentFile(grade, subjectId, 'topics', topicId));
  ipcMain.handle('exam:get', (_e, grade, subjectId, examId) =>
    readContentFile(grade, subjectId, 'exams', examId));

  ipcMain.handle('progress:load', () => loadProgress());
  ipcMain.handle('progress:save', (_e, data) => saveProgress(data));
  ipcMain.on('progress:save-sync', (e, data) => {
    try { saveProgressSync(data); e.returnValue = true; } catch (err) { e.returnValue = false; }
  });
  ipcMain.handle('progress:location', () => progressFile());

  ipcMain.handle('progress:export', async (e, data) => {
    const win = BrowserWindow.fromWebContents(e.sender);
    const stamp = new Date().toISOString().slice(0, 10);
    const res = await dialog.showSaveDialog(win, {
      title: 'Export progress',
      defaultPath: path.join(app.getPath('documents'), `freesheet-progress-${stamp}.json`),
      filters: [{ name: 'FreeSheet progress', extensions: ['json'] }],
    });
    if (res.canceled || !res.filePath) return { ok: false };
    await fs.promises.writeFile(res.filePath, JSON.stringify(data, null, 2), 'utf8');
    return { ok: true, path: res.filePath };
  });

  ipcMain.handle('progress:import', async (e) => {
    const win = BrowserWindow.fromWebContents(e.sender);
    const res = await dialog.showOpenDialog(win, {
      title: 'Import progress',
      properties: ['openFile'],
      filters: [{ name: 'FreeSheet progress', extensions: ['json'] }],
    });
    if (res.canceled || !res.filePaths.length) return { ok: false };
    try {
      const data = readJson(res.filePaths[0]);
      if (!data || typeof data !== 'object' || !data.topics || !data.exams) {
        return { ok: false, error: 'That file is not a FreeSheet progress file.' };
      }
      return { ok: true, data: Object.assign(emptyProgress(), data) };
    } catch (err) {
      return { ok: false, error: 'That file could not be read.' };
    }
  });

  ipcMain.handle('app:open-data-folder', () => shell.openPath(app.getPath('userData')));
  ipcMain.handle('app:version', () => app.getVersion());
}

// ---------- Window ----------

function createWindow() {
  const win = new BrowserWindow({
    width: 1200,
    height: 820,
    minWidth: 760,
    minHeight: 560,
    title: 'FreeSheet',
    backgroundColor: '#ffffff',
    show: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      spellcheck: true,
    },
  });
  win.once('ready-to-show', () => win.show());
  // Never navigate away from the app or open new windows inside it.
  win.webContents.on('will-navigate', (ev) => ev.preventDefault());
  win.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  win.loadFile(path.join(__dirname, 'src', 'index.html'));
}

function buildMenu() {
  if (process.platform === 'darwin') {
    // macOS needs an Edit menu for copy/paste shortcuts to work in text fields.
    Menu.setApplicationMenu(Menu.buildFromTemplate([
      { role: 'appMenu' },
      { role: 'editMenu' },
      { label: 'View', submenu: [{ role: 'resetZoom' }, { role: 'zoomIn' }, { role: 'zoomOut' }, { type: 'separator' }, { role: 'togglefullscreen' }] },
      { role: 'windowMenu' },
    ]));
  } else {
    Menu.setApplicationMenu(null);
  }
}

if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  app.on('second-instance', () => {
    const [win] = BrowserWindow.getAllWindows();
    if (win) { if (win.isMinimized()) win.restore(); win.focus(); }
  });
  app.whenReady().then(() => {
    registerIpc();
    buildMenu();
    createWindow();
    app.on('activate', () => {
      if (BrowserWindow.getAllWindows().length === 0) createWindow();
    });
  });
  app.on('window-all-closed', () => {
    if (process.platform !== 'darwin') app.quit();
  });
}
