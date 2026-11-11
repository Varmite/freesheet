// FreeSheet renderer: navigation, study notes, practice worksheets, exams and progress.
(function () {
  'use strict';
  const M = window.Markup;
  const K = window.Marking;
  const Q = window.Questions;
  const api = window.api;
  const esc = M.escapeHtml;
  // api.web: the website (server/), where this depends on whether the learner has signed in.
  const device = () => api.deviceName || (api.platform === 'android' ? 'this phone' : 'this computer');

  const main = document.getElementById('main');
  const gradeSelect = document.getElementById('grade-select');
  const dialogRoot = document.getElementById('dialog-root');

  const state = {
    catalog: null,
    progress: null,
    tab: 'study',
    views: { study: { name: 'home' }, exams: { name: 'home' }, progress: { name: 'home' }, settings: { name: 'home' } },
    returnTab: 'study', // the tab to go back to when leaving Settings
    cache: new Map(),
    exam: null,       // active exam session
    timer: null,
    dirty: false,
    renderToken: 0,
  };

  // ---------------------------------------------------------------- persistence

  let saveTimer = null;
  function save() {
    state.dirty = true;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(flush, 400);
  }
  function flush() {
    clearTimeout(saveTimer);
    saveTimer = null;
    state.dirty = false;
    return api.saveProgress(state.progress).catch(() => toast('Could not save your progress.'));
  }
  window.addEventListener('beforeunload', () => {
    if (state.exam) writeExamDraft();
    if (state.dirty || state.exam) {
      clearTimeout(saveTimer);
      api.saveProgressSync(state.progress);
    }
  });

  function P() { return state.progress; }

  // ---------------------------------------------------------------- daily streak

  const streakBtn = document.getElementById('streak-btn');
  const settingsBtn = document.getElementById('settings-btn');
  function updateStreakBadge(pop) {
    if (!streakBtn || !P()) return;
    const st = Streak.status(P());
    streakBtn.classList.toggle('lit', st.todayMet);
    // Only touched when the number changes: this runs on every page and every minute.
    if (streakBtn.dataset.n !== String(st.current)) {
      streakBtn.dataset.n = st.current;
      streakBtn.innerHTML = `${Icons.ui('flame', 'streak-flame')}<span>${st.current}</span>`;
    }
    const left = Math.max(0, st.goalQuestions - st.questionsToday);
    streakBtn.title = st.todayMet
      ? `${st.current}-day streak. Today's goal is done.`
      : `${st.current}-day streak. ${left} more question${left === 1 ? '' : 's'} or 1 exam today to ${st.current ? 'keep it going' : 'start a streak'}.`;
    if (pop) {
      streakBtn.classList.remove('pop');
      void streakBtn.offsetWidth;
      streakBtn.classList.add('pop');
    }
  }
  function recordStreak(kind, count) {
    if (!count) return;
    const r = Streak.record(P(), kind, count);
    updateStreakBadge(r.justMet);
    if (r.justMet) celebrateStreak(r.current);
  }
  function celebrateStreak(n) {
    setTimeout(() => {
      Buddy.react('celebrate');
      Buddy.say(n === 1 ? `You started a **streak**! Come back tomorrow to keep it going.` : `**${n}-day streak!** Today's goal is done. Keep it going!`, { priority: 'user', mood: 'happy' });
    }, 1300);
    setTimeout(() => Sound.play('streak'), 450); // after the answer's own chime
    const t = document.createElement('div');
    t.className = 'streak-toast';
    t.setAttribute('role', 'status');
    t.innerHTML = `${Icons.ui('flame', 'streak-toast-flame')}<div><strong>${n}-day streak!</strong>` +
      `<span>${n === 1 ? 'You started a streak. Come back tomorrow to keep it going.' : "Today's goal is done. See you tomorrow."}</span></div>`;
    document.body.appendChild(t);
    setTimeout(() => t.classList.add('show'), 10);
    setTimeout(() => { t.classList.remove('show'); setTimeout(() => t.remove(), 400); }, 4200);
  }
  function showStreak() {
    const st = Streak.status(P());
    const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    const left = Math.max(0, st.goalQuestions - st.questionsToday);
    const goal = st.todayMet
      ? `<p class="streak-goal done">Today's goal is done. Come back tomorrow to keep your streak.</p>`
      : `<p class="streak-goal">Check <strong>${left} more practice question${left === 1 ? '' : 's'}</strong> or finish <strong>1 exam</strong> today to ${st.current ? 'keep your streak' : 'start a streak'}.</p>
         <div class="streak-meter">${bar(Math.round(Math.min(st.questionsToday, st.goalQuestions) / st.goalQuestions * 100))}<span>${Math.min(st.questionsToday, st.goalQuestions)} / ${st.goalQuestions} questions</span></div>`;
    dialog({
      title: 'Daily streak',
      cancel: null,
      ok: 'Close',
      body: `<div class="streak-hero${st.todayMet ? ' lit' : ''}">${Icons.ui('flame', 'streak-hero-flame')}
          <div><div class="streak-num">${st.current}</div><div class="muted">day${st.current === 1 ? '' : 's'} in a row</div></div></div>
        <div class="streak-week">${st.week.map((d) => `<div class="streak-day${d.met ? ' met' : ''}${d.today ? ' today' : ''}">
          <span class="streak-dot">${d.met ? Icons.ui('flame') : ''}</span><span>${days[d.date.getDay()]}</span></div>`).join('')}</div>
        ${goal}
        <p class="muted">Longest streak: ${st.longest} day${st.longest === 1 ? '' : 's'}</p>`,
    });
  }
  function grade() { return P().settings.grade; }
  function gradeData() {
    return state.catalog.grades.find((g) => g.grade === grade()) || { grade: grade(), subjects: [] };
  }
  function subjectMeta(id) { return gradeData().subjects.find((s) => s.id === id); }
  function tKey(subjectId, topicId) { return `${grade()}/${subjectId}/${topicId}`; }
  function xKey(subjectId, examId) { return `${grade()}/${subjectId}/${examId}`; }

  function topicProg(key, create) {
    let tp = P().topics[key];
    if (!tp && create) {
      tp = P().topics[key] = { notesRead: false, responses: {}, checked: {}, selfMarks: {}, attempts: 0 };
    }
    return tp;
  }

  // Recently opened topics and exams stay parsed, so going back is instant. Only the
  // most recent few are kept: a long session must not hold every topic in memory
  // (old phones have little to spare).
  const CACHE_MAX = 40;
  async function cached(k, load) {
    if (state.cache.has(k)) {
      const v = state.cache.get(k);
      state.cache.delete(k);
      state.cache.set(k, v); // now the most recent
      return v;
    }
    const v = await load();
    state.cache.set(k, v);
    while (state.cache.size > CACHE_MAX) state.cache.delete(state.cache.keys().next().value);
    return v;
  }
  function loadTopic(subjectId, topicId) {
    const g = grade();
    return cached(`t:${g}/${subjectId}/${topicId}`, async () => {
      const topic = await api.getTopic(g, subjectId, topicId);
      await forgiveSpelling(subjectId, topic.questions || []);
      return topic;
    });
  }
  function loadExam(subjectId, examId) {
    const g = grade();
    return cached(`x:${g}/${subjectId}/${examId}`, async () => {
      const exam = await api.getExam(g, subjectId, examId);
      await forgiveSpelling(subjectId, (exam.sections || []).flatMap((sec) => sec.questions || []));
      return exam;
    });
  }

  // Short typed answers accept small spelling slips, except in language subjects
  // where spelling is part of what's being tested (see typoOf in marking.js).
  let contentWords = null;
  async function forgiveSpelling(subjectId, questions) {
    if (K.isLanguageSubject(subjectId) || !questions.some((q) => q.type === 'short')) return;
    if (!contentWords) contentWords = api.contentWords().then((w) => new Set(w)).catch(() => null);
    const known = await contentWords;
    if (!known) return;
    // Not enumerable, so it never ends up in saved progress or exports.
    for (const q of questions) if (q.type === 'short') Object.defineProperty(q, 'typoWords', { value: known, configurable: true });
  }

  // ---------------------------------------------------------------- helpers

  // The sound for a just-checked answer card.
  function answerSound(card) {
    if (card.classList.contains('is-correct')) Sound.play('correct');
    else if (card.classList.contains('is-wrong')) Sound.play('wrong');
    else if (card.classList.contains('is-partial')) Sound.play('partial');
    else Sound.play('reveal'); // a written answer: the memo appears for self-marking
  }

  function pct(score, max) { return max > 0 ? Math.round((score / max) * 100) : 0; }

  function toast(msg) {
    Sound.play('toast');
    const t = document.createElement('div');
    t.className = 'toast';
    t.textContent = msg;
    document.body.appendChild(t);
    setTimeout(() => t.classList.add('show'), 10);
    setTimeout(() => { t.classList.remove('show'); setTimeout(() => t.remove(), 300); }, 2600);
  }

  let closeOpenDialog = null;
  function dialog({ title, body, ok = 'OK', cancel = 'Cancel', danger = false }) {
    // Only one dialog at a time: one opening over another (time running out while
    // "Submit exam?" is up) answers the old one with "cancel" first.
    if (closeOpenDialog) closeOpenDialog(false);
    return new Promise((resolve) => {
      const prevFocus = document.activeElement;
      dialogRoot.innerHTML = `<div class="modal-back"><div class="modal" role="dialog" aria-modal="true" aria-labelledby="dlg-title">
        <h2 id="dlg-title">${esc(title)}</h2><div class="modal-body">${body || ''}</div>
        <div class="modal-actions">${cancel ? `<button class="btn" data-r="0">${esc(cancel)}</button>` : ''}
        <button class="btn ${danger ? 'danger' : 'primary'}" data-r="1">${esc(ok)}</button></div></div></div>`;
      const back = dialogRoot.firstElementChild;
      let closed = false;
      const close = (v) => {
        if (closed) return;
        closed = true;
        if (closeOpenDialog === close) closeOpenDialog = null;
        Sound.play('close');
        Motion.dialogOut(back).then(() => back.remove());
        document.removeEventListener('keydown', onKey, true);
        if (prevFocus && prevFocus.focus) prevFocus.focus();
        resolve(v);
      };
      const onKey = (e) => {
        if (e.key === 'Escape') { e.preventDefault(); close(false); }
      };
      document.addEventListener('keydown', onKey, true);
      back.querySelectorAll('[data-r]').forEach((b) => b.addEventListener('click', () => close(b.dataset.r === '1')));
      back.addEventListener('click', (e) => { if (e.target === back && cancel) close(false); });
      closeOpenDialog = close;
      back.querySelector('[data-r="1"]').focus();
      Motion.dialogIn(back);
      Sound.play('open');
    });
  }

  function crumbs(items) {
    return `<nav class="crumbs" aria-label="Breadcrumb">` + items.map((c, i) => {
      const last = i === items.length - 1;
      return last ? `<span aria-current="page">${esc(c.label)}</span>`
        : `<button class="link" data-crumb="${i}">${esc(c.label)}</button><span class="sep">›</span>`;
    }).join('') + `</nav>`;
  }
  function wireCrumbs(items) {
    main.querySelectorAll('[data-crumb]').forEach((b) => b.addEventListener('click', () => items[Number(b.dataset.crumb)].go()));
  }

  function bar(percent) {
    return `<div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${percent}"><span style="width:${percent}%"></span></div>`;
  }

  function formatDuration(sec) {
    sec = Math.max(0, Math.round(sec));
    const h = Math.floor(sec / 3600);
    const m = Math.floor((sec % 3600) / 60);
    const s = sec % 60;
    const two = (n) => String(n).padStart(2, '0');
    return h ? `${h}:${two(m)}:${two(s)}` : `${two(m)}:${two(s)}`;
  }

  function formatDate(iso) {
    try {
      return new Date(iso).toLocaleDateString('en-ZA', { day: 'numeric', month: 'short', year: 'numeric' });
    } catch (e) { return iso; }
  }

  function applySound() {
    Sound.setEnabled(P().settings.sounds !== false);
  }

  // Text rendering: Inter, with Apple-style letter spacing; on Windows strokes are
  // thickened a touch, as macOS does (see "smooth text" in styles.css). ClearType is
  // switched off in main.js at start-up.
  function applyTextRendering() {
    const root = document.documentElement;
    root.classList.toggle('smooth-text', P().settings.textRendering !== 'system');
    root.classList.toggle('os-windows', api.platform === 'win32');
  }

  // Animations are on unless turned off; on a slow phone they start off (Motion.lowEnd)
  // until the learner chooses.
  function animationsOn() {
    const a = P().settings.animations;
    return a === undefined ? !Motion.lowEnd : a !== false;
  }
  function applyMotion() {
    Motion.setEnabled(animationsOn());
  }

  // Theme, accent and reading comfort. Changing them while the app is open cross-fades.
  function applyAppearance(animate) {
    const s = P().settings;
    const run = () => Theme.apply(s);
    if (animate && Motion.enabled() && document.startViewTransition) document.startViewTransition(run);
    else run();
  }

  function applyTextSize() {
    const sizes = { small: '15px', normal: '16.5px', large: '18.5px', xlarge: '20.5px' };
    document.documentElement.style.fontSize = sizes[P().settings.textSize] || sizes.normal;
  }

  // ---------------------------------------------------------------- practice scoring

  function topicScore(topic, tp) {
    let score = 0, max = 0, done = 0, pending = 0;
    for (const q of topic.questions) {
      max += K.marksOf(q);
      if (tp && tp.checked && tp.checked[q.id]) {
        const res = K.mark(q, tp.responses[q.id], tp.selfMarks[q.id]);
        if (res.needsSelfMark) pending++;
        else { done++; score += res.score; }
      }
    }
    return { score, max, done, pending, total: topic.questions.length, complete: done === topic.questions.length };
  }

  function topicStatus(subjectId, topicMeta) {
    const tp = topicProg(tKey(subjectId, topicMeta.id));
    if (!tp) return { label: 'Not started', cls: '' };
    if (tp.best != null && tp.bestMax) {
      return { label: `Best ${pct(tp.best, tp.bestMax)}%`, cls: 'done', percent: pct(tp.best, tp.bestMax) };
    }
    const checked = Object.keys(tp.checked || {}).length;
    if (checked) return { label: `${checked} answered`, cls: 'progress' };
    if (tp.notesRead) return { label: 'Notes read', cls: 'progress' };
    return { label: 'Not started', cls: '' };
  }

  function subjectStats(subject) {
    let practised = 0, notes = 0, sumPct = 0;
    for (const t of subject.topics) {
      const tp = topicProg(tKey(subject.id, t.id));
      if (!tp) continue;
      if (tp.notesRead) notes++;
      if (tp.best != null && tp.bestMax) { practised++; sumPct += pct(tp.best, tp.bestMax); }
    }
    let examsWritten = 0, bestExam = null;
    for (const x of subject.exams) {
      const hist = P().exams[xKey(subject.id, x.id)] || [];
      examsWritten += hist.length;
      for (const a of hist) bestExam = Math.max(bestExam == null ? 0 : bestExam, a.percent);
    }
    return {
      practised, notes, total: subject.topics.length,
      avg: practised ? Math.round(sumPct / practised) : null,
      examsWritten, bestExam,
    };
  }

  // ---------------------------------------------------------------- rendering

  function setTab(tab) {
    state.tab = tab;
    render();
  }

  function go(tab, view) {
    state.tab = tab;
    state.views[tab] = view;
    render();
  }

  // How a page change animates (see js/motion.js): going deeper into a tab is a
  // push (tapped cards zoom open), going back up is a pop, tabs cross-fade.
  const DEPTH = { home: 0, subject: 1, list: 1, course: 1, topic: 2, intro: 2, lesson: 2, run: 3, mark: 4, result: 5, review: 6 };
  let lastNav = null;
  function navInfo() {
    const v = state.views[state.tab] || {};
    return {
      tab: state.tab,
      depth: DEPTH[v.name] || 0,
      key: [state.tab, grade(), v.name, v.subjectId, v.topicId, v.examId, v.attempt, v.mode, v.courseId, v.lessonId, v.run].join('|'),
    };
  }
  function navKind(from, to) {
    if (!from) return 'first';
    // Settings sits on top of whichever tab it was opened from, like an iOS push.
    if (to.tab === 'settings' && from.tab !== 'settings') return 'push';
    if (from.tab === 'settings' && to.tab !== 'settings') return 'pop';
    if (from.tab !== to.tab) return 'tab';
    if (to.depth > from.depth) return 'push';
    if (to.depth < from.depth) return 'pop';
    return 'swap';
  }

  // Re-renders of the same page (text size, retry) do not animate; moving to a new page does.
  // A page change can name its own sound (the topic pager sounds like forward/back).
  let pendingNavSound = null;

  // Scroll position of each page, so going back returns to where you were (as on iOS).
  const scrollMemory = new Map();

  async function render() {
    const nav = navInfo();
    const moved = !lastNav || lastNav.key !== nav.key;
    const kind = navKind(lastNav, nav);
    const fromKey = lastNav && lastNav.key;
    if (moved && fromKey) scrollMemory.set(fromKey, window.scrollY);
    lastNav = nav;
    const token = state.renderToken + 1;
    // Freeze the old page on screen so it can animate out once the new one is ready.
    const tr = moved && fromKey ? Motion.capture(main) : null;
    await renderView();
    syncUpdateCard();
    if (token !== state.renderToken) { Motion.discard(tr); return; }
    Motion.decorate(main);
    if (!moved) { Motion.discard(tr); return; }
    // Going back, or to another tab, returns to where that page was left; a tab
    // seen for the first time starts at the top, not at the old tab's scroll position.
    if (kind === 'pop' && scrollMemory.has(nav.key)) window.scrollTo(0, scrollMemory.get(nav.key));
    else if (kind === 'tab') window.scrollTo(0, scrollMemory.get(nav.key) || 0);
    Motion.navigate(main, tr, { kind, fromKey, toKey: nav.key });
    const navSound = pendingNavSound || { push: 'push', pop: 'pop', tab: 'tab', swap: 'swap' }[kind];
    pendingNavSound = null;
    if (navSound) Sound.play(navSound);
    scheduleWarmUp();
    buddyOnPage();
  }

  // ---------- Transition warm-up
  // The first card zoom in a session drops ~35 frames at 180 Hz; once a real zoom
  // has run, they drop 1-2 (measured with Chromium frame tracing). So when the app
  // is idle on the Study home, it runs one real zoom into a subject and back,
  // hidden under a frozen copy of the page (see the warm-up notes in motion.js).
  // It takes ~1.4 s and runs at full speed (running it faster was measured to warm
  // less). A press during it ends it at once and the click is passed on to
  // whatever is under the pointer once the page is back.
  let warmedUp = false, warmTimer = 0;
  function scheduleWarmUp() {
    if (warmedUp || !Motion.enabled()) return;
    clearTimeout(warmTimer);
    warmTimer = setTimeout(() => requestIdleCallback(() => rehearseTransitions(), { timeout: 2000 }), 400);
  }

  async function rehearseTransitions() {
    if (warmedUp || !Motion.enabled()) return;
    const card = state.tab === 'study' && state.views.study.name === 'home' && main.querySelector('.grid .card[data-subject]');
    if (!card || document.querySelector('.modal-back, .buddy-setup-back')) return; // tried again after the next page change
    // Wait for the page to be at rest and the pointer to be off anything with a
    // hover effect (the frozen copy can't show one).
    if (!Motion.quiet() || main.querySelector('.card:hover, .row:hover, .btn:hover, .continue:hover, .link:hover, .seg:hover')) { scheduleWarmUp(); return; }
    warmedUp = true;
    const y = window.scrollY;
    const cover = Motion.cover(main);
    let cut, pressAt = null, released = false;
    const interrupted = new Promise((r) => { cut = r; });
    const hold = (e) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.type === 'pointerdown') pressAt = { x: e.clientX, y: e.clientY };
      if (e.type === 'pointerup' && pressAt) released = true;
      cut();
    };
    const kinds = ['pointerdown', 'mousedown', 'mouseup', 'pointerup', 'click', 'keydown', 'wheel'];
    for (const k of kinds) window.addEventListener(k, hold, { capture: true, passive: false });
    const pause = () => Promise.race([new Promise((r) => setTimeout(r, Motion.rehearsalStep)), interrupted]);
    // The rehearsed page may be longer or shorter than this one: keep the scrollbar
    // exactly as it is. Its track is always there (styles.css), so on a page that
    // doesn't scroll it only has to stay invisible.
    const root = document.documentElement;
    if (root.scrollHeight <= root.clientHeight) root.style.scrollbarColor = 'transparent transparent';
    Motion.setRehearsing(true);
    try {
      Motion.noteOrigin(card);
      state.views.study = { name: 'subject', subjectId: card.dataset.subject };
      await render();
      await pause();
      state.views.study = { name: 'home' };
      await render();
      await pause();
    } finally {
      Motion.settle();
      window.scrollTo(0, y);
      root.style.scrollbarColor = '';
      Motion.setRehearsing(false);
      cover.layer.remove();
      for (const k of kinds) window.removeEventListener(k, hold, { capture: true });
      document.documentElement.dataset.warmedUp = '';
    }
    // A press that interrupted it was aimed at the cover, which is gone by the time
    // the button comes up, so the browser won't produce a click. Act on the release
    // instead: click whatever is really under the pointer now, and drop any native
    // click that does still follow so it can't happen twice.
    if (pressAt) {
      const at = pressAt;
      const fire = () => { const el = document.elementFromPoint(at.x, at.y); if (el) el.click(); };
      const dropNative = (e) => { if (e.isTrusted) { e.preventDefault(); e.stopPropagation(); } };
      window.addEventListener('click', dropNative, true);
      setTimeout(() => window.removeEventListener('click', dropNative, true), 600);
      if (released) fire();
      else {
        const up = () => { window.removeEventListener('pointerup', up, true); fire(); };
        window.addEventListener('pointerup', up, true);
      }
    }
  }

  async function renderView() {
    const token = ++state.renderToken;
    stopReader();
    modeFloat.detach();
    updateStreakBadge();
    document.querySelectorAll('.tab').forEach((b) => {
      const on = b.dataset.tab === state.tab;
      b.classList.toggle('active', on);
      b.setAttribute('aria-selected', on);
    });
    Motion.moveTabPill(document.querySelector('.tabs'), true);
    settingsBtn.classList.toggle('active', state.tab === 'settings');
    settingsBtn.setAttribute('aria-pressed', state.tab === 'settings');
    const v = state.views[state.tab];
    try {
      let html;
      if (!state.catalog.grades.length) {
        main.innerHTML = `<div class="page"><h1>No content found</h1><p>The <code>content</code> folder is empty or missing.</p></div>`;
        return;
      }
      if (!(state.tab === 'study' && v.name === 'lesson')) courseUI.leave();
      if (state.tab === 'study') {
        if (v.name === 'course') return await courseUI.viewCourse(v, token);
        if (v.name === 'lesson') return await courseUI.viewLesson(v, token);
        if (v.name === 'home') return viewStudyHome();
        if (v.name === 'subject') return viewSubject(v);
        if (v.name === 'topic') return await viewTopic(v, token);
      } else if (state.tab === 'exams') {
        if (v.name === 'home') return viewExamHome();
        if (v.name === 'list') return viewExamList(v);
        if (v.name === 'intro') return await viewExamIntro(v, token);
        if (v.name === 'run') return viewExamRun(v);
        if (v.name === 'mark') return viewExamMark(v);
        if (v.name === 'result') return viewExamResult(v);
        if (v.name === 'review') return await viewExamReview(v, token);
      } else if (state.tab === 'progress') {
        return viewProgress();
      } else if (state.tab === 'settings') {
        return viewSettings();
      }
      main.innerHTML = html || '';
    } catch (err) {
      console.error(err);
      main.innerHTML = `<div class="page"><h1>Something went wrong</h1><p>${esc(err && err.message || err)}</p>
        <button class="btn" id="err-home">Go back</button></div>`;
      main.querySelector('#err-home').addEventListener('click', () => go(state.tab, { name: 'home' }));
    }
  }

  function scrollTop() { window.scrollTo(0, 0); }

  // ---------- Study: home

  function viewStudyHome() {
    const g = gradeData();
    const last = P().settings.lastTopic;
    let cont = '';
    if (last && last.grade === grade()) {
      const s = subjectMeta(last.subjectId);
      const t = s && s.topics.find((x) => x.id === last.topicId);
      if (t) {
        cont = `<button class="continue" id="continue"><span class="muted small">Continue where you left off</span>
          <span class="continue-title">${esc(t.title)}</span><span class="muted">${esc(s.name)}</span></button>`;
      }
    }
    main.innerHTML = `<div class="page">
      <h1>Grade ${g.grade} subjects</h1>
      <p class="lead">Choose a subject to read the notes and practise questions on each topic.</p>
      ${cont}
      <div class="grid">${g.subjects.map((s) => {
        const st = subjectStats(s);
        const p = st.total ? Math.round((st.practised / st.total) * 100) : 0;
        return `<button class="card" data-subject="${esc(s.id)}">
          <span class="card-head">${Icons.subject(s.id)}<span class="card-title">${esc(s.name)}</span></span>
          <span class="card-sub">${st.total} topic${st.total === 1 ? '' : 's'}</span>
          ${bar(p)}
          <span class="card-meta">${st.practised} of ${st.total} practised${st.avg != null ? ` · avg ${st.avg}%` : ''}</span>
        </button>`;
      }).join('')}${(state.catalog.courses || []).filter((c) => c.grade == null || c.grade === grade()).map(courseUI.homeCard).join('')}</div>
      ${g.subjects.length ? '' : '<p class="muted">No subjects are available for this grade yet.</p>'}
    </div>`;
    main.querySelectorAll('[data-subject]').forEach((b) => b.addEventListener('click', () => {
      go('study', { name: 'subject', subjectId: b.dataset.subject }); scrollTop();
    }));
    main.querySelectorAll('[data-course]').forEach((b) => b.addEventListener('click', () => {
      go('study', { name: 'course', courseId: b.dataset.course }); scrollTop();
    }));
    const c = main.querySelector('#continue');
    if (c) c.addEventListener('click', () => { go('study', { name: 'topic', subjectId: last.subjectId, topicId: last.topicId, mode: last.mode || 'notes' }); scrollTop(); });
  }

  // ---------- Study: subject

  function viewSubject(v) {
    const s = subjectMeta(v.subjectId);
    if (!s) return go('study', { name: 'home' });
    const items = [
      { label: `Grade ${grade()}`, go: () => go('study', { name: 'home' }) },
      { label: s.name },
    ];
    const terms = [1, 2, 3, 4].map((term) => ({ term, topics: s.topics.filter((t) => t.term === term) })).filter((x) => x.topics.length);
    const st = subjectStats(s);
    main.innerHTML = `<div class="page">
      ${crumbs(items)}
      <div class="page-head">
        <div><h1 class="with-icon">${Icons.subject(s.id, "lg")}<span>${esc(s.name)}</span></h1><p class="lead">${M.inline(s.description)}</p></div>
        <div class="head-stats"><div><strong>${st.practised}/${st.total}</strong><span>practised</span></div>
          <div><strong>${st.avg != null ? st.avg + '%' : '–'}</strong><span>average</span></div></div>
      </div>
      <input type="search" class="filter" id="filter" placeholder="Find a topic" aria-label="Find a topic" value="${esc(v.filter || '')}">
      <div id="topic-groups">${terms.map((g) => `
        <section class="term" data-term="${g.term}"><h2>Term ${g.term}</h2>
        <ul class="rows">${g.topics.map((t) => {
          const status = topicStatus(s.id, t);
          return `<li data-title="${esc(t.title.toLowerCase())}"><button class="row" data-topic="${esc(t.id)}">
            <span class="row-title">${esc(t.title)}</span>
            <span class="row-status ${status.cls}">${esc(status.label)}</span></button></li>`;
        }).join('')}</ul></section>`).join('')}</div>
      <p class="muted" id="no-match" hidden>No topics match your search.</p>
      ${s.exams.length ? `<div class="aside-link"><button class="btn" id="to-exams">Practise ${esc(s.name)} exams →</button></div>` : ''}
    </div>`;
    wireCrumbs(items);
    main.querySelectorAll('[data-topic]').forEach((b) => b.addEventListener('click', () => {
      go('study', { name: 'topic', subjectId: s.id, topicId: b.dataset.topic, mode: 'notes' }); scrollTop();
    }));
    const f = main.querySelector('#filter');
    const applyFilter = () => {
      const q = f.value.trim().toLowerCase();
      v.filter = f.value;
      let any = false;
      main.querySelectorAll('.term').forEach((sec) => {
        let shown = 0;
        sec.querySelectorAll('li').forEach((li) => {
          const ok = !q || li.dataset.title.includes(q);
          li.hidden = !ok;
          if (ok) shown++;
        });
        sec.hidden = !shown;
        if (shown) any = true;
      });
      main.querySelector('#no-match').hidden = any;
    };
    f.addEventListener('input', () => Motion.flip(main.querySelectorAll('.term, .term li, #no-match, .aside-link'), applyFilter));
    applyFilter();
    const te = main.querySelector('#to-exams');
    if (te) te.addEventListener('click', () => { go('exams', { name: 'list', subjectId: s.id }); scrollTop(); });
  }

  // ---------- Study: topic (notes + questions)

  // Notes | Questions stays within reach: once the page's own switch scrolls out of
  // sight, a copy slides in beside the page (wide windows) or above the bottom edge
  // (narrow windows and phones). It lives on <body> so page transitions don't move it.
  const modeFloat = (() => {
    let el = null, seg = null, pick = null, raf = 0;
    function make() {
      el = document.createElement('div');
      el.className = 'mode-float';
      el.setAttribute('role', 'tablist');
      el.setAttribute('aria-label', 'Notes or questions');
      document.body.appendChild(el);
      el.addEventListener('click', (e) => {
        const b = e.target.closest('[data-mode]');
        if (b && pick && !b.classList.contains('active')) pick(b.dataset.mode);
      });
      window.addEventListener('scroll', queue, { passive: true });
      window.addEventListener('resize', () => { if (seg) place(); queue(); });
    }
    function queue() { if (!raf) raf = requestAnimationFrame(update); }
    // Beside the page if there is room on its left, otherwise at the bottom.
    function place() {
      const page = seg.closest('.page');
      if (!page) return;
      el.classList.add('side');
      el.classList.remove('bottom');
      const left = page.getBoundingClientRect().left + 8 - el.offsetWidth - 20;
      const side = left >= 12 && window.innerWidth > 720;
      el.classList.toggle('side', side);
      el.classList.toggle('bottom', !side);
      el.style.left = side ? Math.round(left) + 'px' : '';
    }
    function update() {
      raf = 0;
      if (!el) return;
      const bar = document.querySelector('.topbar');
      const limit = bar ? bar.getBoundingClientRect().bottom : 56;
      const show = !!(seg && seg.isConnected && seg.getBoundingClientRect().bottom < limit + 4);
      el.classList.toggle('show', show);
      el.setAttribute('aria-hidden', String(!show));
      el.querySelectorAll('button').forEach((b) => { b.tabIndex = show ? 0 : -1; });
    }
    return {
      attach(segEl, mode, count, onPick) {
        if (!segEl) return this.detach();
        if (!el) make();
        seg = segEl; pick = onPick;
        el.innerHTML = [['notes', 'Notes'], ['questions', `Questions (${count})`]].map(([m, label]) =>
          `<button type="button" role="tab" class="mf-btn ${m === mode ? 'active' : ''}" data-mode="${m}" aria-selected="${m === mode}">${label}</button>`).join('');
        place();
        update();
      },
      detach() { seg = null; pick = null; if (el) update(); },
    };
  })();

  async function viewTopic(v, token) {
    const s = subjectMeta(v.subjectId);
    const meta = s && s.topics.find((t) => t.id === v.topicId);
    if (!meta) return go('study', { name: 'home' });
    main.innerHTML = `<div class="page"><div class="loading">Loading…</div></div>`;
    const topic = await loadTopic(s.id, meta.id);
    if (token !== state.renderToken) return;
    const key = tKey(s.id, meta.id);
    P().settings.lastTopic = { grade: grade(), subjectId: s.id, topicId: meta.id, mode: v.mode };
    save();
    const idx = s.topics.findIndex((t) => t.id === meta.id);
    const prev = s.topics[idx - 1];
    const next = s.topics[idx + 1];
    const items = [
      { label: `Grade ${grade()}`, go: () => go('study', { name: 'home' }) },
      { label: s.name, go: () => go('study', { name: 'subject', subjectId: s.id }) },
      { label: meta.title },
    ];
    const mode = v.mode === 'questions' ? 'questions' : 'notes';
    main.innerHTML = `<div class="page page-reading page-topic">
      ${crumbs(items)}
      <h1>${esc(topic.title)}</h1>
      <p class="lead">${M.inline(topic.summary || '')}</p>
      <div class="segmented" role="tablist">
        <button role="tab" class="seg ${mode === 'notes' ? 'active' : ''}" data-mode="notes" aria-selected="${mode === 'notes'}">Notes</button>
        <button role="tab" class="seg ${mode === 'questions' ? 'active' : ''}" data-mode="questions" aria-selected="${mode === 'questions'}">Questions (${topic.questions.length})</button>
      </div>
      <div id="topic-body"></div>
      <nav class="pager">
        ${prev ? `<button class="btn" data-goto="${esc(prev.id)}" data-dir="back">← ${esc(prev.title)}</button>` : '<span></span>'}
        ${next ? `<button class="btn" data-goto="${esc(next.id)}" data-dir="forward">${esc(next.title)} →</button>` : '<span></span>'}
      </nav>
    </div>`;
    wireCrumbs(items);
    main.querySelectorAll('.seg').forEach((b) => b.addEventListener('click', () => {
      v.mode = b.dataset.mode; render(); scrollTop();
    }));
    modeFloat.attach(main.querySelector('.page-topic > .segmented'), mode, topic.questions.length, (m) => {
      v.mode = m; render(); scrollTop();
    });
    main.querySelectorAll('[data-goto]').forEach((b) => b.addEventListener('click', () => {
      Motion.setHint(b.dataset.dir);
      pendingNavSound = b.dataset.dir === 'forward' ? 'push' : 'pop';
      go('study', { name: 'topic', subjectId: s.id, topicId: b.dataset.goto, mode: 'notes' }); scrollTop();
    }));
    const body = main.querySelector('#topic-body');
    if (mode === 'notes') renderNotes(body, topic, key, v);
    else renderPractice(body, topic, key);
  }

  // ---------- Notes: revealed one section at a time, with optional read-aloud audio

  const reader = { audio: null, url: null, index: -1, paused: false, follow: null };

  function stopReader() {
    Buddy.listen(false);
    if (reader.follow) reader.follow.stop();
    reader.follow = null;
    if (reader.audio) { reader.audio.onended = null; reader.audio.pause(); reader.audio.removeAttribute('src'); }
    if (reader.url) URL.revokeObjectURL(reader.url);
    reader.audio = null; reader.url = null; reader.index = -1; reader.paused = false;
  }

  function renderNotes(body, topic, key, v) {
    const secs = Sections.split(topic.notes);
    const total = secs.length;
    const tpOld = topicProg(key);
    let shown = Math.min(total, Math.max(1, v.shown || (tpOld && tpOld.sectionsSeen) || 1));
    let audioIds = new Set();
    let speed = Number(P().settings.readSpeed) || 1;

    body.innerHTML = `<div class="reader-bar" id="reader-bar" hidden>
        <button class="btn" id="read-btn"></button>
        <button class="btn icon-btn" id="read-stop" title="Stop reading" aria-label="Stop reading" hidden>${Icons.ui('stop')}</button>
        <label class="read-speed">Speed <select id="read-speed">${[0.75, 1, 1.25, 1.5].map((x) =>
          `<option value="${x}" ${x === speed ? 'selected' : ''}>${x}×</option>`).join('')}</select></label>
        <span class="muted small" id="read-status"></span>
      </div>
      <article class="notes" id="notes"></article>
      <div id="notes-more"></div>`;
    const article = body.querySelector('#notes');
    const more = body.querySelector('#notes-more');
    const bar = body.querySelector('#reader-bar');
    const readBtn = body.querySelector('#read-btn');
    const stopBtn = body.querySelector('#read-stop');
    const status = body.querySelector('#read-status');

    function hasAudio(i) { return audioIds.has(secs[i].id); }

    function sectionHtml(sec, i) {
      const play = hasAudio(i)
        ? `<button class="section-play" data-play="${i}" title="Listen to this section" aria-label="Listen to this section">${Icons.ui('speaker')}</button>` : '';
      return `<section class="note-section${reader.index === i ? ' speaking' : ''}" data-i="${i}">${play}${M.blocks(sec.blocks)}</section>`;
    }

    function drawSections() {
      article.innerHTML = secs.slice(0, shown).map(sectionHtml).join('');
      article.querySelectorAll('[data-play]').forEach((b) => b.addEventListener('click', () => { Sound.play('playStart'); play(Number(b.dataset.play)); }));
      if (shown < total) {
        const nextTitle = secs[shown].title ? `: ${M.inline(secs[shown].title)}` : '';
        more.innerHTML = `<div class="notes-next">
            <div class="section-count">Section ${shown} of ${total}</div>
            <div class="bar"><span style="width:${Math.round((shown / total) * 100)}%"></span></div>
            <button class="btn primary" id="more-btn">Continue${nextTitle} →</button>
          </div>`;
        more.querySelector('#more-btn').addEventListener('click', () => { Sound.play('reveal'); reveal(shown + 1); scrollToSection(shown - 1); });
      } else {
        const tp = topicProg(key);
        more.innerHTML = `<div class="notes-end">
            <p>${tp && tp.notesRead ? 'You have marked these notes as read.' : 'You have reached the end of the notes. Test yourself on what you have just studied.'}</p>
            <button class="btn primary" id="to-questions">Practise ${topic.questions.length} questions →</button>
          </div>`;
        more.querySelector('#to-questions').addEventListener('click', () => {
          stopReader();
          topicProg(key, true).notesRead = true;
          save();
          v.mode = 'questions';
          render();
          scrollTop();
        });
      }
    }

    function reveal(n) {
      if (n <= shown) return;
      shown = Math.min(total, n);
      v.shown = shown;
      const tp = topicProg(key, true);
      tp.sectionsSeen = Math.max(tp.sectionsSeen || 0, shown);
      save();
      drawSections();
      Motion.reveal(article.querySelector(`[data-i="${shown - 1}"]`));
      Motion.reveal(more.firstElementChild, 80);
      if (shown === total && reader.index < 0) {
        tell(`notesdone:${key}`, `That's all the notes for **${topic.title}**! Ready to test yourself?`, {
          mood: 'happy',
          actions: [{ id: 'q', label: 'Start the questions', primary: true, run: () => { const b = more.querySelector('#to-questions'); if (b) b.click(); } }],
        });
      }
    }

    function scrollToSection(i) {
      const el = article.querySelector(`[data-i="${i}"]`);
      if (el) el.scrollIntoView({ behavior: Motion.enabled() ? 'smooth' : 'auto', block: 'start' });
    }

    function drawControls() {
      const active = reader.index >= 0;
      readBtn.innerHTML = !active ? `${Icons.ui('speaker')}<span>Listen to these notes</span>`
        : reader.paused ? `${Icons.ui('play')}<span>Resume</span>` : `${Icons.ui('pause')}<span>Pause</span>`;
      stopBtn.hidden = !active;
      status.textContent = active ? `Reading section ${reader.index + 1} of ${total}` : '';
      article.querySelectorAll('.note-section').forEach((el) => el.classList.toggle('speaking', Number(el.dataset.i) === reader.index));
      Buddy.listen(active && !reader.paused); // the buddy closes its eyes and sways along
    }

    async function play(i) {
      stopReader();
      while (i < total && !hasAudio(i)) i++;
      if (i >= total || !body.isConnected) { drawControls(); return; }
      reveal(i + 1);
      reader.index = i;
      drawControls();
      scrollToSection(i);
      let audio = null;
      try {
        const data = await api.getAudio(grade(), v.subjectId, topic.id, secs[i].id);
        if (reader.index !== i || reader.audio || !body.isConnected) return;
        reader.url = URL.createObjectURL(new Blob([data], { type: 'audio/ogg' }));
        audio = reader.audio = new Audio(reader.url);
        audio.playbackRate = speed;
        audio.onended = () => { if (i + 1 < total) play(i + 1); else { stopReader(); drawControls(); } };
        // Work out word timings while the audio starts; highlighting joins in when ready.
        ReadAlong.analyse(data).then((timing) => {
          if (reader.audio !== audio) return;
          reader.follow = ReadAlong.follow(audio, () => article.querySelector(`.note-section[data-i="${i}"]`), secs[i], timing);
        }).catch((err) => console.warn('Read-along timing unavailable:', err));
        await audio.play();
      } catch (err) {
        // Stopped, or another section started, while this one was loading: its
        // play() rejects, and that must not stop the reading that replaced it.
        if (reader.index !== i || (audio && reader.audio !== audio) || !body.isConnected) return;
        console.error('Audio failed:', err);
        stopReader();
        drawControls();
        status.textContent = 'This audio could not be played.';
      }
    }

    readBtn.addEventListener('click', () => {
      if (reader.index < 0) { Sound.play('playStart'); return play(0); }
      if (!reader.audio) return;
      if (reader.paused) { Sound.play('playStart'); reader.audio.play(); reader.paused = false; } else { Sound.play('playPause'); reader.audio.pause(); reader.paused = true; }
      drawControls();
    });
    stopBtn.addEventListener('click', () => { Sound.play('playStop'); stopReader(); drawControls(); });
    body.querySelector('#read-speed').addEventListener('change', (e) => {
      speed = Number(e.target.value) || 1;
      P().settings.readSpeed = speed;
      save();
      if (reader.audio) reader.audio.playbackRate = speed;
    });

    drawSections();
    drawControls();
    // Arrived from a hint or the buddy's "show me in the notes": open that section and flash it.
    if (v.focus != null) {
      const f = Math.max(0, Math.min(total - 1, Number(v.focus) || 0));
      delete v.focus;
      if (f + 1 > shown) reveal(f + 1);
      setTimeout(() => {
        const el = article.querySelector(`[data-i="${f}"]`);
        if (!el) return;
        el.scrollIntoView({ behavior: Motion.enabled() ? 'smooth' : 'auto', block: 'start' });
        el.classList.remove('focus-flash');
        void el.offsetWidth;
        el.classList.add('focus-flash');
      }, 350);
    }
    api.listAudio(grade(), v.subjectId, topic.id).then((ids) => {
      if (!body.isConnected) return;
      audioIds = new Set(ids);
      if (secs.some((sec, i) => hasAudio(i))) { bar.hidden = false; drawSections(); drawControls(); Motion.reveal(bar); }
    }).catch(() => {});
  }

  function renderPractice(body, topic, key) {
    const tp = topicProg(key, true);
    tp.notesRead = true;
    // Drop answers to questions that no longer exist (content updated).
    const qids = new Set(topic.questions.map((q) => q.id));
    for (const field of ['responses', 'checked', 'selfMarks', 'hints']) {
      for (const id of Object.keys(tp[field] || {})) if (!qids.has(id)) delete tp[field][id];
    }
    body.innerHTML = `<div class="score-bar" id="score-bar"></div><div class="q-list" id="q-list"></div>
      <div class="practice-end" id="practice-end"></div>`;
    const list = body.querySelector('#q-list');
    const cards = [];

    function recordIfComplete() {
      const sc = topicScore(topic, tp);
      if (sc.complete && !tp.recorded) {
        tp.recorded = true;
        tp.attempts = (tp.attempts || 0) + 1;
        tp.last = sc.score; tp.lastMax = sc.max;
        tp.completedAt = new Date().toISOString();
        if (tp.best == null || pct(sc.score, sc.max) > pct(tp.best, tp.bestMax)) { tp.best = sc.score; tp.bestMax = sc.max; }
      }
    }

    // The buddy's view of this worksheet.
    function firstWrongCard() { return list.querySelector('.q-card.is-wrong, .q-card.is-partial'); }
    function showCard(card) {
      if (!card) return;
      card.scrollIntoView({ behavior: Motion.enabled() ? 'smooth' : 'auto', block: 'center' });
      setTimeout(() => Motion.verdict(card), 350);
    }
    function buddyBatch() {
      const right = list.querySelectorAll('.q-card.is-correct').length;
      const wrong = list.querySelectorAll('.q-card.is-wrong, .q-card.is-partial').length;
      if (!right && !wrong) return;
      Buddy.react(!wrong ? 'celebrate' : right >= wrong ? 'correct' : 'wrong');
      Buddy.say(!wrong ? `All ${right} right! Brilliant.` : `${right} right and ${wrong} to look at again.`, {
        priority: 'tip',
        actions: wrong ? [{ id: 'w', label: 'Show me the first one', primary: true, run: () => showCard(firstWrongCard()) }] : [],
      });
    }
    function buddyTopicDone(p, lvl) {
      Buddy.react(p >= 50 ? 'celebrate' : 'wrong');
      const wrongQ = topic.questions.find((q) => tp.checked[q.id] && !K.mark(q, tp.responses[q.id], tp.selfMarks[q.id]).correct);
      const sec = wrongQ ? Hints.relatedSection(wrongQ, topic) : null;
      if (p >= 80) Buddy.say(`**${p}%**: outstanding! That's level ${lvl.level}.`, { priority: 'user', mood: 'happy' });
      else if (p >= 50) Buddy.say(`**${p}%**: good work! Let's look at the ones you missed.`, {
        priority: 'user', mood: 'happy', actions: [{ id: 'w', label: 'Show me the first one', primary: true, run: () => showCard(firstWrongCard()) }],
      });
      else Buddy.say(`**${p}%**. This topic is a tricky one. Going over the notes again will help.`, {
        priority: 'user', mood: 'sad',
        actions: [{ id: 'n', label: sec != null ? 'Show me what to read' : 'Back to the notes', primary: true, run: () => (sec != null ? showInNotes(sec) : showInNotes(0)) }],
      });
    }
    buddyWorksheet = { firstWrongCard, showCard };

    let wasComplete = topicScore(topic, tp).complete;
    function updateSummary() {
      const sc = topicScore(topic, tp);
      const justDone = sc.complete && !wasComplete;
      wasComplete = sc.complete;
      const p = pct(sc.score, sc.max);
      body.querySelector('#score-bar').innerHTML =
        `<span><strong>${K.formatMarks(sc.score)}</strong> / ${sc.max} marks</span>` +
        `<span class="muted">${sc.done} of ${sc.total} marked${sc.pending ? ` · ${sc.pending} to self-mark` : ''}</span>` +
        (tp.best != null ? `<span class="muted">Best: ${pct(tp.best, tp.bestMax)}%</span>` : '');
      const end = body.querySelector('#practice-end');
      if (sc.complete) {
        const lvl = K.capsLevel(p);
        end.innerHTML = `<div class="result-box">
          <div class="result-big">${p}%</div>
          <div>${K.formatMarks(sc.score)} / ${sc.max} marks · Level ${lvl.level}: ${esc(lvl.label)}</div>
          <p class="muted">${p >= 80 ? 'Excellent work.' : p >= 50 ? 'Good effort. Review the notes for the questions you got wrong, then try again.' : 'Go back over the notes and try again.'}</p>
          <div class="btn-row"><button class="btn" id="re-notes">Back to notes</button><button class="btn primary" id="retry">Try again</button></div></div>`;
        if (justDone) {
          Motion.pop(end.firstElementChild);
          setTimeout(() => Sound.play(p >= 50 ? 'complete' : 'finishSoft'), 380);
          setTimeout(() => buddyTopicDone(p, lvl), 450);
        }
        end.querySelector('#retry').addEventListener('click', () => {
          tp.responses = {}; tp.checked = {}; tp.selfMarks = {}; tp.hints = {}; tp.recorded = false;
          save(); render(); scrollTop();
        });
        end.querySelector('#re-notes').addEventListener('click', () => { state.views.study.mode = 'notes'; render(); scrollTop(); });
      } else {
        const unchecked = topic.questions.filter((q) => !tp.checked[q.id]).length;
        end.innerHTML = unchecked
          ? `<div class="btn-row end"><button class="btn primary" id="check-all">Check all answers</button></div>`
          : (sc.pending ? `<p class="muted center">Award yourself marks for the written answers above to finish.</p>` : '');
        const ca = end.querySelector('#check-all');
        if (ca) ca.addEventListener('click', async () => {
          const unanswered = topic.questions.filter((q) => !tp.checked[q.id] && !K.isAnswered(q, tp.responses[q.id])).length;
          if (unanswered) {
            const ok = await dialog({ title: 'Check all answers?', body: `<p>${unanswered} question${unanswered === 1 ? ' is' : 's are'} not answered yet and will be marked wrong.</p>`, ok: 'Check all' });
            if (!ok) return;
          }
          let answeredNow = 0, n = 0;
          topic.questions.forEach((q, i) => {
            if (!tp.checked[q.id]) {
              if (K.isAnswered(q, tp.responses[q.id])) answeredNow++;
              tp.checked[q.id] = true;
              cards[i].ctx.phase = 'feedback';
              cards[i].el.refresh();
              const k = n++, d = Math.min(k, 12) * 30;
              Motion.reveal(cards[i].el.querySelector('.q-feedback'), d);
              setTimeout(() => Motion.verdict(cards[i].el), d);
              // One chime per answer, spaced out so they ripple rather than pile up.
              if (k < 8) setTimeout(() => answerSound(cards[i].el), k * 90);
            }
          });
          recordStreak('question', answeredNow);
          recordIfComplete(); save(); updateSummary();
          setTimeout(buddyBatch, 500);
        });
      }
    }

    topic.questions.forEach((q, i) => {
      const ctx = {
        number: `Question ${i + 1}`,
        response: tp.responses[q.id],
        phase: tp.checked[q.id] ? 'feedback' : 'answer',
        selfMark: tp.selfMarks[q.id],
        allowCheck: true,
        onResponse: (r) => { tp.responses[q.id] = r; save(); },
        onCheck: () => {
          if (!tp.checked[q.id] && K.isAnswered(q, tp.responses[q.id])) recordStreak('question', 1);
          tp.checked[q.id] = true;
          ctx.phase = 'feedback';
          el.refresh();
          Motion.reveal(el.querySelector('.q-feedback'));
          Motion.verdict(el);
          answerSound(el);
          buddyAnswer(el, q, topic);
          recordIfComplete(); save(); updateSummary();
        },
        onSelfMark: (n) => {
          tp.selfMarks[q.id] = n;
          recordIfComplete(); save(); updateSummary();
        },
        // Hints: practice only (exams never pass this).
        hint: {
          list: Hints.forQuestion(q, topic),
          level: (tp.hints && tp.hints[q.id]) || 0,
          onUse: (level) => {
            tp.hints = tp.hints || {};
            tp.hints[q.id] = level;
            save();
            Sound.play('hint');
            Buddy.react('hint');
            if (level === 1) tell('hint-howto', pickOne(BUDDY_HINT_LINES), { priority: 'reaction', mood: 'think' });
          },
          onShowNotes: (sec) => showInNotes(sec),
        },
      };
      const el = Q.mount(q, ctx);
      cards.push({ el, ctx });
      list.appendChild(el);
    });
    updateSummary();
  }

  // ---------- Exams: home and list

  function viewExamHome() {
    const g = gradeData();
    const subs = g.subjects.filter((s) => s.exams.length);
    const draftHtml = activeDraftsHtml();
    main.innerHTML = `<div class="page">
      <h1>Grade ${g.grade} exams</h1>
      <p class="lead">Write term and November exam papers under timed conditions, then mark them with the memo.</p>
      ${draftHtml}
      <div class="grid">${subs.map((s) => {
        const st = subjectStats(s);
        return `<button class="card" data-subject="${esc(s.id)}">
          <span class="card-head">${Icons.subject(s.id)}<span class="card-title">${esc(s.name)}</span></span>
          <span class="card-sub">${s.exams.length} paper${s.exams.length === 1 ? '' : 's'}</span>
          <span class="card-meta">${st.examsWritten ? `${st.examsWritten} written · best ${st.bestExam}%` : 'Not written yet'}</span>
        </button>`;
      }).join('')}</div>
      ${subs.length ? '' : '<p class="muted">No exams are available for this grade yet.</p>'}
    </div>`;
    main.querySelectorAll('[data-subject]').forEach((b) => b.addEventListener('click', () => {
      go('exams', { name: 'list', subjectId: b.dataset.subject }); scrollTop();
    }));
    wireDrafts();
  }

  function activeDraftsHtml() {
    const drafts = Object.keys(P().examDrafts || {}).filter((k) => k.startsWith(`${grade()}/`));
    if (!drafts.length) return '';
    return `<div class="drafts"><h2>Unfinished exams</h2><ul class="rows">${drafts.map((k) => {
      const [, sid, xid] = k.split('/');
      const s = subjectMeta(sid);
      const x = s && s.exams.find((e) => e.id === xid);
      if (!x) return '';
      return `<li><button class="row" data-draft="${esc(k)}"><span class="row-title">${esc(s.name)}: ${esc(x.title)}</span>
        <span class="row-status progress">Resume</span></button></li>`;
    }).join('')}</ul></div>`;
  }
  function wireDrafts() {
    main.querySelectorAll('[data-draft]').forEach((b) => b.addEventListener('click', () => {
      const [, sid, xid] = b.dataset.draft.split('/');
      go('exams', { name: 'intro', subjectId: sid, examId: xid }); scrollTop();
    }));
  }

  function viewExamList(v) {
    const s = subjectMeta(v.subjectId);
    if (!s) return go('exams', { name: 'home' });
    const items = [
      { label: `Grade ${grade()} exams`, go: () => go('exams', { name: 'home' }) },
      { label: s.name },
    ];
    main.innerHTML = `<div class="page">
      ${crumbs(items)}
      <h1 class="with-icon">${Icons.subject(s.id, "lg")}<span>${esc(s.name)} exams</span></h1>
      <ul class="rows exam-rows">${s.exams.map((x) => {
        const hist = P().exams[xKey(s.id, x.id)] || [];
        const draft = P().examDrafts[xKey(s.id, x.id)];
        const best = hist.length ? Math.max(...hist.map((a) => a.percent)) : null;
        return `<li><button class="row" data-exam="${esc(x.id)}">
          <span class="row-title">${esc(x.title)}<span class="row-sub">${x.durationMinutes} minutes · ${x.totalMarks} marks</span></span>
          <span class="row-status ${draft ? 'progress' : best != null ? 'done' : ''}">${draft ? 'In progress' : best != null ? `Best ${best}% · ${hist.length} attempt${hist.length === 1 ? '' : 's'}` : 'Not written'}</span>
        </button></li>`;
      }).join('')}</ul>
    </div>`;
    wireCrumbs(items);
    main.querySelectorAll('[data-exam]').forEach((b) => b.addEventListener('click', () => {
      go('exams', { name: 'intro', subjectId: s.id, examId: b.dataset.exam }); scrollTop();
    }));
  }

  // ---------- Exams: intro

  function examCrumbs(s, x, last) {
    return [
      { label: `Grade ${grade()} exams`, go: () => go('exams', { name: 'home' }) },
      { label: s.name, go: () => go('exams', { name: 'list', subjectId: s.id }) },
      { label: last || x.title },
    ];
  }

  function allExamQuestions(exam) {
    const out = [];
    exam.sections.forEach((sec, si) => sec.questions.forEach((q, qi) => out.push({ q, si, qi, number: `${si + 1}.${qi + 1}` })));
    return out;
  }

  async function viewExamIntro(v, token) {
    const s = subjectMeta(v.subjectId);
    const meta = s && s.exams.find((x) => x.id === v.examId);
    if (!meta) return go('exams', { name: 'home' });
    main.innerHTML = `<div class="page"><div class="loading">Loading…</div></div>`;
    const exam = await loadExam(s.id, meta.id);
    if (token !== state.renderToken) return;
    const key = xKey(s.id, meta.id);
    const draft = P().examDrafts[key];
    const hist = P().exams[key] || [];
    const items = examCrumbs(s, meta);
    // Time left: from the running session if there is one (the draft is only saved every 15 s).
    const liveExam = state.exam && state.exam.key === key && state.exam.grade === grade() ? state.exam : null;
    const left = liveExam ? liveExam.remainingSec : draft && draft.remainingSec;
    main.innerHTML = `<div class="page page-reading">
      ${crumbs(items)}
      <h1>${esc(exam.title)}</h1>
      <p class="lead">${esc(s.name)} · Grade ${grade()}</p>
      <div class="facts"><div><strong>${exam.durationMinutes}</strong><span>minutes</span></div>
        <div><strong>${exam.totalMarks}</strong><span>marks</span></div>
        <div><strong>${allExamQuestions(exam).length}</strong><span>questions</span></div></div>
      <h2>Instructions</h2>
      <ol class="n-list">${exam.instructions.map((i) => `<li>${M.inline(i)}</li>`).join('')}</ol>
      <p class="muted">Multiple-choice, one-word and calculation answers are marked automatically. For written answers you will compare your work with the memo and award your own marks after you submit.</p>
      ${draft ? `<div class="notice">You have an unfinished attempt${draft.timerOn ? ` with ${formatDuration(left)} left` : ''}.</div>` : ''}
      <label class="check"><input type="checkbox" id="timed" ${draft ? (draft.timerOn ? 'checked' : '') : 'checked'} ${draft ? 'disabled' : ''}> Use a countdown timer</label>
      <div class="btn-row">
        ${draft ? `<button class="btn" id="restart">Start over</button><button class="btn primary" id="resume">Resume exam</button>`
          : `<button class="btn primary" id="start">Start exam</button>`}
      </div>
      ${hist.length ? `<h2>Previous attempts</h2><table class="n-table"><thead><tr><th>Date</th><th>Mark</th><th>%</th><th>Time</th><th></th></tr></thead><tbody>
        ${hist.slice().reverse().map((a, i) => `<tr><td>${esc(formatDate(a.date))}</td><td>${K.formatMarks(a.score)} / ${a.max}</td><td>${a.percent}%</td>
        <td>${formatDuration(a.timeUsedSec || 0)}</td><td><button class="link" data-attempt="${hist.length - 1 - i}">Review</button></td></tr>`).join('')}
      </tbody></table>` : ''}
    </div>`;
    wireCrumbs(items);
    const start = (fresh) => {
      // Still running from earlier in this session (the learner looked at another
      // page): carry on with it. Restarting from the draft would wind the clock back
      // to the last autosave.
      const live = state.exam;
      if (!fresh && live && live.key === key && live.grade === grade()) {
        go('exams', { name: live.phase === 'mark' ? 'mark' : 'run', subjectId: s.id, examId: meta.id });
        scrollTop();
        return;
      }
      const timerOn = fresh ? main.querySelector('#timed').checked : !!draft.timerOn;
      startExamSession(s, meta, exam, fresh ? null : draft, timerOn);
    };
    const st = main.querySelector('#start');
    if (st) st.addEventListener('click', () => start(true));
    const rs = main.querySelector('#resume');
    if (rs) rs.addEventListener('click', () => start(false));
    const rst = main.querySelector('#restart');
    if (rst) rst.addEventListener('click', async () => {
      if (!(await dialog({ title: 'Start over?', body: '<p>Your answers in the unfinished attempt will be deleted.</p>', ok: 'Start over', danger: true }))) return;
      delete P().examDrafts[key];
      if (state.exam && state.exam.key === key) stopExam();
      save(); render();
    });
    main.querySelectorAll('[data-attempt]').forEach((b) => b.addEventListener('click', () => {
      go('exams', { name: 'review', subjectId: s.id, examId: meta.id, attempt: Number(b.dataset.attempt) }); scrollTop();
    }));
  }

  // ---------- Exams: session

  function startExamSession(s, meta, exam, draft, timerOn) {
    stopExam();
    const key = xKey(s.id, meta.id);
    state.exam = {
      key, subjectId: s.id, examId: meta.id, grade: grade(), exam,
      responses: draft ? Object.assign({}, draft.responses) : {},
      selfMarks: draft ? Object.assign({}, draft.selfMarks) : {},
      remainingSec: draft ? draft.remainingSec : exam.durationMinutes * 60,
      elapsedSec: draft ? (draft.elapsedSec || 0) : 0,
      timerOn,
      phase: draft ? draft.phase : 'run',
    };
    writeExamDraft(); flush();
    Sound.play('examStart');
    if (state.exam.phase === 'run') startTimer();
    go('exams', { name: state.exam.phase === 'mark' ? 'mark' : 'run', subjectId: s.id, examId: meta.id });
    scrollTop();
  }

  function writeExamDraft() {
    const e = state.exam;
    if (!e) return;
    P().examDrafts[e.key] = {
      phase: e.phase, responses: e.responses, selfMarks: e.selfMarks,
      remainingSec: e.remainingSec, elapsedSec: e.elapsedSec, timerOn: e.timerOn,
      savedAt: new Date().toISOString(),
    };
  }

  function startTimer() {
    clearInterval(state.timer);
    state.timer = setInterval(() => {
      const e = state.exam;
      if (!e || e.phase !== 'run') { clearInterval(state.timer); return; }
      e.elapsedSec++;
      if (e.timerOn) e.remainingSec = Math.max(0, e.remainingSec - 1);
      const t = document.getElementById('exam-timer');
      if (t) {
        t.textContent = e.timerOn ? formatDuration(e.remainingSec) : formatDuration(e.elapsedSec);
        t.classList.toggle('low', e.timerOn && e.remainingSec <= 300);
      }
      if (e.elapsedSec % 15 === 0) { writeExamDraft(); save(); }
      if (e.timerOn && (e.remainingSec === 300 || e.remainingSec === 60)) Sound.play('timeWarning');
      if (e.timerOn && e.remainingSec <= 0) {
        clearInterval(state.timer);
        submitExam(true);
      }
    }, 1000);
  }

  function stopExam() {
    clearInterval(state.timer);
    state.timer = null;
    state.exam = null;
  }

  function ensureSession(v) {
    const e = state.exam;
    if (e && e.key === xKey(v.subjectId, v.examId) && e.grade === grade()) return e;
    go('exams', { name: 'intro', subjectId: v.subjectId, examId: v.examId });
    return null;
  }

  function viewExamRun(v) {
    const e = ensureSession(v);
    if (!e) return;
    const s = subjectMeta(e.subjectId);
    const exam = e.exam;
    const all = allExamQuestions(exam);
    const items = examCrumbs(s, exam);
    main.innerHTML = `<div class="exam-bar">
        <div class="exam-bar-title">${esc(exam.title)}</div>
        <div class="exam-bar-mid"><span id="answered"></span></div>
        <div class="exam-timer ${e.timerOn ? '' : 'up'}" id="exam-timer" title="${e.timerOn ? 'Time remaining' : 'Time spent'}">${formatDuration(e.timerOn ? e.remainingSec : e.elapsedSec)}</div>
        <button class="btn primary" id="submit-top">Submit</button>
      </div>
      <div class="page page-reading">
      ${crumbs(items)}
      <h1>${esc(exam.title)}</h1>
      <p class="muted">Your answers are saved automatically. You can leave and come back to this exam later.</p>
      <div id="sections"></div>
      <div class="btn-row end"><button class="btn primary" id="submit">Submit exam</button></div>
    </div>`;
    wireCrumbs(items);
    const timerEl = main.querySelector('#exam-timer');
    timerEl.classList.toggle('low', e.timerOn && e.remainingSec <= 300);
    const secWrap = main.querySelector('#sections');
    const updateAnswered = () => {
      const n = all.filter((x) => K.isAnswered(x.q, e.responses[x.q.id])).length;
      main.querySelector('#answered').textContent = `${n} of ${all.length} answered`;
    };
    exam.sections.forEach((sec, si) => {
      const sub = sec.questions.reduce((a, q) => a + K.marksOf(q), 0);
      const wrap = document.createElement('section');
      wrap.className = 'exam-section';
      wrap.innerHTML = `<h2>${M.inline(sec.title)} <span class="muted small">(${sub} marks)</span></h2>` +
        (sec.instructions ? `<div class="sec-instr">${M.paragraphs(sec.instructions)}</div>` : '') +
        (sec.context ? `<div class="notes">${M.blocks(sec.context)}</div>` : '');
      sec.questions.forEach((q, qi) => {
        const ctx = {
          number: `${si + 1}.${qi + 1}`,
          response: e.responses[q.id],
          phase: 'answer',
          allowCheck: false,
          onResponse: (r) => { e.responses[q.id] = r; writeExamDraft(); save(); updateAnswered(); },
        };
        wrap.appendChild(Q.mount(q, ctx));
      });
      secWrap.appendChild(wrap);
    });
    updateAnswered();
    const onSubmit = async () => {
      const unanswered = all.filter((x) => !K.isAnswered(x.q, e.responses[x.q.id])).length;
      const ok = await dialog({
        title: 'Submit exam?',
        body: unanswered ? `<p>You have <strong>${unanswered}</strong> unanswered question${unanswered === 1 ? '' : 's'}. Unanswered questions get 0 marks.</p>` : '<p>You have answered every question.</p>',
        ok: 'Submit',
      });
      if (ok) submitExam(false);
    };
    main.querySelector('#submit').addEventListener('click', onSubmit);
    main.querySelector('#submit-top').addEventListener('click', onSubmit);
  }

  async function submitExam(timeUp) {
    const e = state.exam;
    if (!e || e.phase !== 'run') return;
    clearInterval(state.timer);
    e.phase = 'mark';
    writeExamDraft(); flush();
    Sound.play(timeUp ? 'timeWarning' : 'submit');
    const hasOpen = allExamQuestions(e.exam).some((x) => x.q.type === 'open');
    if (timeUp) {
      // Show the marking/result screen behind the notice so the exam cannot continue.
      if (!hasOpen) return finishExam();
      if (state.tab === 'exams') go('exams', { name: 'mark', subjectId: e.subjectId, examId: e.examId });
      await dialog({ title: 'Time is up', body: '<p>Your exam has been submitted. Now mark your written answers using the memo.</p>', cancel: null, ok: 'Start marking' });
      go('exams', { name: 'mark', subjectId: e.subjectId, examId: e.examId });
      scrollTop();
      return;
    }
    if (!hasOpen) return finishExam();
    go('exams', { name: 'mark', subjectId: e.subjectId, examId: e.examId });
    scrollTop();
  }

  function viewExamMark(v) {
    const e = ensureSession(v);
    if (!e) return;
    if (e.phase === 'run') return go('exams', { name: 'run', subjectId: v.subjectId, examId: v.examId });
    const s = subjectMeta(e.subjectId);
    const all = allExamQuestions(e.exam);
    const open = all.filter((x) => x.q.type === 'open');
    const items = examCrumbs(s, e.exam, 'Marking');
    main.innerHTML = `<div class="page page-reading">
      ${crumbs(items)}
      <h1>Mark your written answers</h1>
      <p class="lead">The other questions have been marked automatically. For each written answer below, read the memo and award yourself marks honestly.</p>
      <div class="score-bar" id="mark-bar"></div>
      <div id="mark-list"></div>
      <div class="btn-row end"><button class="btn primary" id="finish">See my results</button></div>
    </div>`;
    wireCrumbs(items);
    const list = main.querySelector('#mark-list');
    const updateBar = () => {
      const n = open.filter((x) => e.selfMarks[x.q.id] != null).length;
      main.querySelector('#mark-bar').innerHTML = `<span><strong>${n}</strong> of ${open.length} written answers marked</span>`;
      main.querySelector('#finish').disabled = n < open.length;
    };
    let lastSi = -1;
    open.forEach((x) => {
      if (x.si !== lastSi) {
        lastSi = x.si;
        const sec = e.exam.sections[x.si];
        const h = document.createElement('div');
        h.innerHTML = `<h2>${M.inline(sec.title)}</h2>` + (sec.context ? `<details class="ctx"><summary>Show the source material</summary><div class="notes">${M.blocks(sec.context)}</div></details>` : '');
        list.appendChild(h);
      }
      const ctx = {
        number: x.number,
        response: e.responses[x.q.id],
        phase: 'feedback',
        selfMark: e.selfMarks[x.q.id],
        onSelfMark: (n) => { e.selfMarks[x.q.id] = n; writeExamDraft(); save(); updateBar(); },
      };
      if (!K.isAnswered(x.q, ctx.response) && ctx.selfMark == null) {
        // Unanswered written questions automatically get 0.
        e.selfMarks[x.q.id] = 0;
        ctx.selfMark = 0;
      }
      list.appendChild(Q.mount(x.q, ctx));
    });
    writeExamDraft(); save();
    updateBar();
    main.querySelector('#finish').addEventListener('click', finishExam);
  }

  function finishExam() {
    const e = state.exam;
    if (!e) return;
    const all = allExamQuestions(e.exam);
    let score = 0, max = 0;
    const perSection = e.exam.sections.map((sec) => ({ title: sec.title, score: 0, max: 0 }));
    for (const x of all) {
      const res = K.mark(x.q, e.responses[x.q.id], e.selfMarks[x.q.id]);
      score += res.score; max += res.max;
      perSection[x.si].score += res.score; perSection[x.si].max += res.max;
    }
    const attempt = {
      date: new Date().toISOString(),
      score: Math.round(score * 10) / 10, max, percent: pct(score, max),
      timeUsedSec: e.elapsedSec, timed: e.timerOn,
      sections: perSection,
      responses: e.responses, selfMarks: e.selfMarks,
    };
    const hist = P().exams[e.key] || (P().exams[e.key] = []);
    hist.push(attempt);
    if (hist.length > 30) hist.splice(0, hist.length - 30);
    delete P().examDrafts[e.key];
    recordStreak('exam', 1);
    const { subjectId, examId } = e;
    stopExam();
    flush();
    go('exams', { name: 'result', subjectId, examId, attempt: hist.length - 1 });
    setTimeout(() => Sound.play(attempt.percent >= 50 ? 'complete' : 'finishSoft'), 300);
    scrollTop();
  }

  function viewExamResult(v) {
    const s = subjectMeta(v.subjectId);
    const meta = s && s.exams.find((x) => x.id === v.examId);
    const hist = P().exams[xKey(v.subjectId, v.examId)] || [];
    const a = hist[v.attempt];
    if (!meta || !a) return go('exams', { name: 'home' });
    const lvl = K.capsLevel(a.percent);
    const items = examCrumbs(s, meta, 'Results');
    main.innerHTML = `<div class="page page-reading">
      ${crumbs(items)}
      <h1>${esc(meta.title)}: results</h1>
      <div class="result-box">
        <div class="result-big">${a.percent}%</div>
        <div>${K.formatMarks(a.score)} / ${a.max} marks · Level ${lvl.level}: ${esc(lvl.label)}</div>
        <div class="muted">Time used: ${formatDuration(a.timeUsedSec)} of ${meta.durationMinutes} minutes</div>
      </div>
      <h2>Marks per question</h2>
      <table class="n-table"><thead><tr><th>Question</th><th>Mark</th><th>%</th></tr></thead><tbody>
        ${(a.sections || []).map((sec) => `<tr><td>${M.inline(sec.title)}</td><td>${K.formatMarks(sec.score)} / ${sec.max}</td><td>${pct(sec.score, sec.max)}%</td></tr>`).join('')}
      </tbody></table>
      <div class="btn-row">
        <button class="btn" id="back">Back to ${esc(s.name)} exams</button>
        <button class="btn" id="retake">Write again</button>
        <button class="btn primary" id="review">Review answers and memo</button>
      </div>
    </div>`;
    wireCrumbs(items);
    main.querySelector('#back').addEventListener('click', () => go('exams', { name: 'list', subjectId: s.id }));
    main.querySelector('#retake').addEventListener('click', () => go('exams', { name: 'intro', subjectId: s.id, examId: meta.id }));
    main.querySelector('#review').addEventListener('click', () => { go('exams', { name: 'review', subjectId: s.id, examId: meta.id, attempt: v.attempt }); scrollTop(); });
  }

  async function viewExamReview(v, token) {
    const s = subjectMeta(v.subjectId);
    const meta = s && s.exams.find((x) => x.id === v.examId);
    const hist = P().exams[xKey(v.subjectId, v.examId)] || [];
    const a = hist[v.attempt];
    if (!meta || !a) return go('exams', { name: 'home' });
    main.innerHTML = `<div class="page"><div class="loading">Loading…</div></div>`;
    const exam = await loadExam(s.id, meta.id);
    if (token !== state.renderToken) return;
    const items = examCrumbs(s, meta, 'Review');
    main.innerHTML = `<div class="page page-reading">
      ${crumbs(items)}
      <h1>${esc(meta.title)}: review</h1>
      <p class="lead">${esc(formatDate(a.date))} · ${K.formatMarks(a.score)} / ${a.max} (${a.percent}%)</p>
      <div id="sections"></div>
      <div class="btn-row end"><button class="btn" id="back">Back to results</button></div>
    </div>`;
    wireCrumbs(items);
    const wrap = main.querySelector('#sections');
    exam.sections.forEach((sec, si) => {
      const el = document.createElement('section');
      el.className = 'exam-section';
      el.innerHTML = `<h2>${M.inline(sec.title)}</h2>` + (sec.context ? `<details class="ctx"><summary>Show the source material</summary><div class="notes">${M.blocks(sec.context)}</div></details>` : '');
      sec.questions.forEach((q, qi) => {
        el.appendChild(Q.mount(q, {
          number: `${si + 1}.${qi + 1}`,
          response: a.responses ? a.responses[q.id] : undefined,
          phase: 'feedback',
          selfMark: a.selfMarks ? a.selfMarks[q.id] : undefined,
        }));
      });
      wrap.appendChild(el);
    });
    main.querySelector('#back').addEventListener('click', () => go('exams', { name: 'result', subjectId: s.id, examId: meta.id, attempt: v.attempt }));
  }

  // ---------- Progress

  function viewProgress() {
    const g = gradeData();
    let practised = 0, total = 0, sumPct = 0, examsWritten = 0, examPctSum = 0;
    const rows = g.subjects.map((s) => {
      const st = subjectStats(s);
      practised += st.practised; total += st.total;
      if (st.avg != null) sumPct += st.avg * st.practised;
      for (const x of s.exams) {
        for (const a of P().exams[xKey(s.id, x.id)] || []) { examsWritten++; examPctSum += a.percent; }
      }
      return { s, st };
    });
    const recent = [];
    for (const s of g.subjects) for (const x of s.exams) {
      for (const [i, a] of (P().exams[xKey(s.id, x.id)] || []).entries()) recent.push({ s, x, a, i });
    }
    recent.sort((p, q) => (p.a.date < q.a.date ? 1 : -1));
    const streakNow = Streak.status(P());
    main.innerHTML = `<div class="page">
      <h1>Grade ${g.grade} progress</h1>
      <div class="facts">
        <div><strong>${practised}/${total}</strong><span>topics practised</span></div>
        <div><strong>${practised ? Math.round(sumPct / practised) + '%' : '–'}</strong><span>average practice score</span></div>
        <div><strong>${examsWritten}</strong><span>exams written</span></div>
        <div><strong>${examsWritten ? Math.round(examPctSum / examsWritten) + '%' : '–'}</strong><span>average exam score</span></div>
        <div><strong>${streakNow.current} day${streakNow.current === 1 ? '' : 's'}</strong><span>current streak (best ${streakNow.longest})</span></div>
      </div>
      <h2>By subject</h2>
      <div class="n-table-wrap"><table class="n-table progress-table"><thead><tr><th>Subject</th><th>Notes read</th><th>Topics practised</th><th>Average</th><th>Best exam</th></tr></thead><tbody>
        ${rows.map(({ s, st }) => `<tr><td><button class="link subject-link" data-subject="${esc(s.id)}">${Icons.subject(s.id, "sm")}${esc(s.name)}</button></td>
          <td>${st.notes}/${st.total}</td><td><div class="cell-bar">${bar(st.total ? Math.round(st.practised / st.total * 100) : 0)}<span>${st.practised}/${st.total}</span></div></td>
          <td>${st.avg != null ? st.avg + '%' : '–'}</td><td>${st.bestExam != null ? st.bestExam + '%' : '–'}</td></tr>`).join('')}
      </tbody></table></div>
      <h2>Recent exams</h2>
      ${recent.length ? `<div class="n-table-wrap"><table class="n-table"><thead><tr><th>Date</th><th>Subject</th><th>Paper</th><th>Mark</th></tr></thead><tbody>
        ${recent.slice(0, 15).map((r) => `<tr><td>${esc(formatDate(r.a.date))}</td><td>${esc(r.s.name)}</td>
          <td><button class="link" data-res="${esc(r.s.id)}|${esc(r.x.id)}|${r.i}">${esc(r.x.title)}</button></td><td>${r.a.percent}%</td></tr>`).join('')}
      </tbody></table></div>` : '<p class="muted">You have not written any exams in this grade yet.</p>'}

    </div>`;
    main.querySelectorAll('[data-subject]').forEach((b) => b.addEventListener('click', () => go('study', { name: 'subject', subjectId: b.dataset.subject })));
    main.querySelectorAll('[data-res]').forEach((b) => b.addEventListener('click', () => {
      const [sid, xid, i] = b.dataset.res.split('|');
      go('exams', { name: 'result', subjectId: sid, examId: xid, attempt: Number(i) });
    }));
  }

  // ---------------------------------------------------------------- settings
  // Opened from the gear in the top bar; Back returns to the tab you came from.

  // A row with a segmented control; bindSeg3 answers its buttons.
  function seg3(label, name, value, options, note) {
    return `<div class="setting"><span>${label}</span><div class="segmented small">${options.map(([v, t]) => `<button class="seg ${value === v ? 'active' : ''}" data-${name}="${v}">${t}</button>`).join('')}</div>${note ? `<span class="muted small">${note}</span>` : ''}</div>`;
  }
  function bindSeg3(name, set) {
    main.querySelectorAll(`[data-${name}]`).forEach((b) => b.addEventListener('click', () => {
      Sound.play('select');
      Motion.selectSegment(b);
      set(b.dataset[name]);
      save();
    }));
  }

  function renderAccents() {
    const row = main.querySelector('#accent-row');
    if (!row) return;
    const s = P().settings;
    const look = Theme.resolve(s.theme || 'system');
    const locked = look.id === 'contrast';
    const cur = s.accent || 'default';
    const dots = [{ id: 'default', label: 'Theme colour', c: look.p.accent }].concat(Theme.ACCENTS.map((a) => ({ id: a.id, label: a.label, c: look.dark ? a.dark : a.light })));
    row.innerHTML = dots.map((d) => `<button class="accent-dot ${d.id === 'default' ? 'default' : ''} ${cur === d.id && !locked ? 'on' : ''}" style="--c:${d.c}" data-accent-id="${d.id}" title="${d.label}" aria-label="${d.label}" aria-pressed="${cur === d.id}" ${locked ? 'disabled' : ''}></button>`).join('');
    main.querySelector('#accent-note').hidden = !locked;
    row.querySelectorAll('[data-accent-id]').forEach((b) => b.addEventListener('click', () => {
      if (s.accent === b.dataset.accentId) return;
      Sound.play('select');
      s.accent = b.dataset.accentId;
      applyAppearance(true);
      renderAccents();
      save();
    }));
  }

  // The website only: signing in keeps progress with the learner rather than the browser.
  function accountSection() {
    const a = api.account;
    if (!a.signedIn && !a.googleAvailable) return '';
    return `<h2>Account</h2>
      <div class="settings">${a.signedIn
        ? `<div class="setting"><span>Signed in</span><span>${esc(a.email)}</span><button class="btn" id="sign-out">Sign out</button></div>`
        : `<div class="setting"><span>Google</span><button class="btn" id="sign-in">Sign in with Google</button><span class="muted small">Keep your progress on any phone or computer.</span></div>`}
      </div>`;
  }
  function bindAccount() {
    const on = (sel, fn) => { const b = main.querySelector(sel); if (b) b.addEventListener('click', async () => { await flush(); fn(); }); };
    on('#sign-in', () => api.signIn());
    on('#sign-out', () => api.signOut());
  }

  function viewSettings() {
    const s = P().settings;
    const ts = P().settings.textSize || 'normal';
    const anim = Motion.supported && animationsOn();
    const snd = P().settings.sounds !== false;
    const smooth = P().settings.textRendering !== 'system';
    const bud = P().settings.buddy;
    const budOn = !!bud && bud.on !== false;
    const budQuiet = !!(bud && bud.quiet);
    const lookLabel = (list, id) => (list.find((x) => x.id === id) || {}).label || '';
    main.innerHTML = `<div class="page page-settings">
      <button class="link back-link" id="settings-back">‹ Back</button>
      <h1>Settings</h1>
      <h2>Appearance</h2>
      <div class="settings">
        <div><span class="set-label">Theme</span><div class="theme-grid">${Theme.THEMES.map((x) => `<button class="theme-tile ${(s.theme || 'system') === x.id ? 'on' : ''}" data-theme-id="${x.id}" aria-pressed="${(s.theme || 'system') === x.id}">
          <span class="theme-prev ${x.p ? '' : 'auto'}" ${x.p ? `style="--t-bg:${x.p.bg};--t-card:${x.p.card};--t-line:${x.p.line};--t-btn:${x.p.btn}"` : ''}><i class="tp-card"></i><i class="tp-line"></i><i class="tp-btn"></i></span>${x.label}</button>`).join('')}</div></div>
        <div><span class="set-label">Accent colour</span><div class="accent-row" id="accent-row"></div>
          <p class="muted small" id="accent-note" hidden>High contrast keeps its own colours so everything stays easy to read.</p></div>
      </div>
      <h2>Display</h2>
      <div class="settings">
        <div class="setting"><span>Text size</span><div class="segmented small">
          ${['small', 'normal', 'large', 'xlarge'].map((k) => `<button class="seg ${ts === k ? 'active' : ''}" data-size="${k}">${{ small: 'Small', normal: 'Normal', large: 'Large', xlarge: 'Extra large' }[k]}</button>`).join('')}
        </div></div>
        <div class="setting"><span>Text</span><div class="segmented small">
          <button class="seg ${smooth ? 'active' : ''}" data-text="smooth">Smooth</button>
          <button class="seg ${smooth ? '' : 'active'}" data-text="system">System</button>
        </div><span class="muted small">Smooth draws text like macOS: greyscale smoothing (no colour fringes), the Inter typeface and fuller strokes.</span>
        <span class="text-restart" id="text-restart" hidden><button class="btn small-btn" id="restart">Restart now</button><span class="muted small">to finish switching</span></span></div>
        <div class="setting"><span>Animations</span><div class="segmented small">
          <button class="seg ${anim ? 'active' : ''}" data-anim="1" ${Motion.supported ? '' : 'disabled'}>On</button>
          <button class="seg ${anim ? '' : 'active'}" data-anim="0">Off</button>
        </div><span class="muted small">${Motion.supported ? `Smooth transitions at your screen's full refresh rate, with a touch of motion blur.${Motion.lowEnd ? ' They start off on this phone to keep it quick.' : ''}` : 'Animations need a newer Android System WebView: update it in the Play Store to turn them on.'}</span></div>
        ${seg3('Line spacing', 'spacing', s.lineSpacing || 'normal', [['compact', 'Compact'], ['normal', 'Normal'], ['relaxed', 'Relaxed']], 'How much room there is between lines in the notes.')}
        ${seg3('Reading width', 'width', s.readingWidth || 'normal', [['narrow', 'Narrow'], ['normal', 'Normal'], ['wide', 'Wide']])}
        ${seg3('Open on', 'start', s.startTab || 'study', [['study', 'Study'], ['exams', 'Exams'], ['progress', 'Progress']])}
      </div>
      <h2>Sound</h2>
      <div class="settings">
        <div class="setting"><span>Sound effects</span><div class="segmented small">
          <button class="seg ${snd ? 'active' : ''}" data-snd="1">On</button>
          <button class="seg ${snd ? '' : 'active'}" data-snd="0">Off</button>
        </div><span class="muted small">Soft sounds for taps, page changes and answers.</span></div>
      </div>
      <h2>Study buddy</h2>
      <div class="settings">
        <div class="setting"><span>Your buddy</span>${bud
          ? `<div class="buddy-card"><div class="buddy-mini">${Buddy.svg(bud, 'static')}</div><div><strong>${esc(bud.name)}</strong>` +
            `<div class="muted small">${esc(lookLabel(Buddy.KINDS, bud.kind))} · ${esc(lookLabel(Buddy.COLOURS, bud.color))}` +
            `${bud.accessory && bud.accessory !== 'none' ? ' · ' + esc(lookLabel(Buddy.ACCESSORIES, bud.accessory)) : ''}</div></div>` +
            `<button class="btn" id="buddy-change">Change look and name</button></div>`
          : `<button class="btn" id="buddy-change">Set up your buddy</button>`}</div>
        <div class="setting"><span>Buddy</span><div class="segmented small">
          <button class="seg ${budOn ? 'active' : ''}" data-bud="1">On</button>
          <button class="seg ${budOn ? '' : 'active'}" data-bud="0">Off</button>
        </div><span class="muted small">Hints on practice questions work either way.</span></div>
        <div class="setting"><span>Tips</span><div class="segmented small">
          <button class="seg ${budQuiet ? '' : 'active'}" data-bquiet="0">As I go</button>
          <button class="seg ${budQuiet ? 'active' : ''}" data-bquiet="1">Only when I ask</button>
        </div></div>
      </div>
      <h2>Your data</h2>
      <div class="settings">
        <div class="setting"><span>Progress</span><div class="btn-row">
          <button class="btn" id="export">Export progress</button>
          <button class="btn" id="import">Import progress</button>
          <button class="btn" id="open-folder">Open data folder</button>
          <button class="btn danger-outline" id="reset">Reset all progress</button></div></div>
        <p class="muted small" id="location"></p>
      </div>
      ${api.web ? accountSection() : `<h2>Updates</h2>
      <div class="settings">
        ${seg3('Check for updates', 'updates', s.checkUpdates === false ? 'off' : 'on', [['on', 'On'], ['off', 'Off']], 'Look for a new version each time FreeSheet opens.')}
        <div class="update-status" id="update-status"></div>
      </div>`}
    </div>`;
    if (api.web) bindAccount();
    renderAccents();
    renderUpdateStatus();
    main.querySelectorAll('[data-theme-id]').forEach((b) => b.addEventListener('click', () => {
      if (b.classList.contains('on')) return;
      Sound.play('select');
      main.querySelectorAll('[data-theme-id]').forEach((x) => { x.classList.toggle('on', x === b); x.setAttribute('aria-pressed', x === b); });
      s.theme = b.dataset.themeId;
      applyAppearance(true);
      renderAccents();
      save();
    }));
    bindSeg3('spacing', (v) => { s.lineSpacing = v; applyAppearance(); });
    bindSeg3('width', (v) => { s.readingWidth = v; applyAppearance(); });
    bindSeg3('start', (v) => { s.startTab = v; });
    bindSeg3('updates', (v) => { s.checkUpdates = v === 'on'; });
    main.querySelector('#settings-back').addEventListener('click', leaveSettings);
    main.querySelectorAll('[data-snd]').forEach((b) => b.addEventListener('click', () => {
      const on = b.dataset.snd === '1';
      P().settings.sounds = on;
      applySound();
      Sound.play('toggleOn'); // only heard when turning sound on: a preview of it
      Motion.selectSegment(b);
      save();
    }));
    // The font changes at once; ClearType on/off needs a restart.
    const restartNote = main.querySelector('#text-restart');
    const syncRestart = () => api.smoothTextActive().then((on) => {
      if (api.platform !== 'win32') return; // only Windows has ClearType to switch off
      restartNote.hidden = on === (P().settings.textRendering !== 'system');
    }).catch(() => {});
    syncRestart();
    main.querySelectorAll('[data-text]').forEach((b) => b.addEventListener('click', () => {
      Sound.play('select');
      Motion.selectSegment(b);
      P().settings.textRendering = b.dataset.text;
      applyTextRendering();
      save();
      syncRestart();
    }));
    main.querySelector('#restart').addEventListener('click', async () => { await flush(); api.relaunch(); });
    main.querySelector('#buddy-change').addEventListener('click', () => setupBuddy(!P().settings.buddy));
    main.querySelectorAll('[data-bud]').forEach((b) => b.addEventListener('click', async () => {
      const on = b.dataset.bud === '1';
      Sound.play(on ? 'toggleOn' : 'toggleOff');
      Motion.selectSegment(b);
      if (!P().settings.buddy) { if (on && !(await setupBuddy(true))) render(); return; }
      Object.assign(P().settings.buddy, { on, min: false });
      save();
      applyBuddy();
    }));
    main.querySelectorAll('[data-bquiet]').forEach((b) => b.addEventListener('click', () => {
      Sound.play('select');
      Motion.selectSegment(b);
      if (!P().settings.buddy) return;
      P().settings.buddy.quiet = b.dataset.bquiet === '1';
      save();
      applyBuddy();
    }));
    main.querySelectorAll('[data-size]').forEach((b) => b.addEventListener('click', () => {
      Sound.size(['small', 'normal', 'large', 'xlarge'].indexOf(b.dataset.size));
      Motion.selectSegment(b);
      P().settings.textSize = b.dataset.size; applyTextSize(); save();
    }));
    main.querySelectorAll('[data-anim]').forEach((b) => b.addEventListener('click', () => {
      // Turning animations on: enable first so the thumb itself animates.
      if (b.dataset.anim === '1') { P().settings.animations = true; applyMotion(); }
      Sound.play(b.dataset.anim === '1' ? 'toggleOn' : 'toggleOff');
      Motion.selectSegment(b);
      P().settings.animations = b.dataset.anim === '1';
      applyMotion(); save();
    }));
    main.querySelector('#export').addEventListener('click', async () => {
      await flush();
      const r = await api.exportProgress(P());
      if (r && r.ok) toast('Progress exported.');
    });
    main.querySelector('#import').addEventListener('click', async () => {
      const r = await api.importProgress();
      if (!r || !r.ok) { if (r && r.error) toast(r.error); return; }
      if (!(await dialog({ title: 'Replace your progress?', body: `<p>Importing replaces all progress on ${device()} with the progress in the file.</p>`, ok: 'Import', danger: true }))) return;
      stopExam();
      // Keep this computer's buddy if the file doesn't have one.
      if (P().settings.buddy && !(r.data.settings && r.data.settings.buddy)) {
        r.data.settings = r.data.settings || {};
        r.data.settings.buddy = P().settings.buddy;
      }
      state.progress = normaliseProgress(r.data, grade());
      applyAppearance(); applyTextSize(); applyTextRendering(); applyMotion(); applySound(); applyBuddy(); syncGradeSelect(); await flush(); toast('Progress imported.'); render();
    });
    main.querySelector('#open-folder').addEventListener('click', () => api.openDataFolder());
    main.querySelector('#reset').addEventListener('click', async () => {
      if (!(await dialog({ title: 'Reset all progress?', body: '<p>This deletes all notes read, practice answers and exam results for every grade. This cannot be undone. Consider exporting first.</p>', ok: 'Reset everything', danger: true }))) return;
      stopExam();
      const settings = P().settings;
      state.progress = { version: 1, settings: { grade: settings.grade, textSize: settings.textSize, animations: settings.animations, sounds: settings.sounds, buddy: settings.buddy, textRendering: settings.textRendering, theme: settings.theme, accent: settings.accent, lineSpacing: settings.lineSpacing, readingWidth: settings.readingWidth, startTab: settings.startTab, checkUpdates: settings.checkUpdates, updateDismissed: settings.updateDismissed }, topics: {}, exams: {}, examDrafts: {} };
      await flush(); toast('Progress reset.'); render();
    });
    api.progressLocation().then((loc) => {
      const el = main.querySelector('#location');
      if (el) el.textContent = api.web ? loc : `Progress is saved on ${device()} only, in ${loc}`;
    });
  }

  function openSettings() {
    if (document.body.classList.contains('welcome-mode')) return;
    if (state.tab === 'settings') return leaveSettings();
    state.returnTab = state.tab;
    setTab('settings');
    scrollTop();
  }
  function leaveSettings() { setTab(state.returnTab || 'study'); }

  // ---------------------------------------------------------------- updates
  // Each time the app opens it asks GitHub, quietly, once the first page is on screen. A
  // new version shows a small card (never during an exam); offline, nothing happens.

  // Pressing Update downloads the new version inside the app. When it has been checked, the
  // learner chooses when to install, because the app has to close for it.

  const update = { status: 'idle', version: null, url: null, current: '', sizeMB: 0, job: null, received: 0, total: 0 };

  api.onUpdateProgress((p) => {
    if (update.job !== 'downloading') return;
    update.received = p.received;
    update.total = p.total;
    document.querySelectorAll('.update-note').forEach((n) => { n.textContent = updateDownloadText(); });
    document.querySelectorAll('.update-bar i').forEach((b) => { b.style.width = `${updatePercent()}%`; });
  });

  const updatePercent = () => (update.total ? Math.min(100, Math.floor((update.received * 100) / update.total)) : 0);
  function updateDownloadText() {
    if (!update.total) return 'Downloading the update…';
    const mb = (n) => Math.round(n / 1048576);
    return `Downloading the update: ${updatePercent()}% (${mb(update.received)} of ${mb(update.total)} MB)`;
  }

  async function checkForUpdates() {
    if (update.status === 'checking' || update.job) return;
    update.status = 'checking';
    renderUpdateStatus();
    try {
      Object.assign(update, { version: null, url: null, sizeMB: 0 }, await Updates.check(update.current, api.platform === 'android'));
    } catch (e) {
      update.status = 'failed';
    }
    renderUpdateStatus();
    syncUpdateCard();
  }

  function setUpdateJob(job) {
    update.job = job;
    renderUpdateStatus();
    syncUpdateCard();
  }

  async function downloadUpdate() {
    if (update.job === 'downloading' || update.job === 'installing') return;
    Object.assign(update, { received: 0, total: 0 });
    setUpdateJob('downloading');
    try {
      await api.downloadUpdate(update.version);
      setUpdateJob('ready');
    } catch (e) {
      setUpdateJob(String(e) === 'cancelled' ? null : 'error');
    }
  }

  async function installUpdate() {
    setUpdateJob('installing');
    try {
      const r = await api.applyUpdate(update.version);
      if (r === 'permission' || r === 'opened') setUpdateJob(r);
      else if (r === 'started') setUpdateJob('ready');
    } catch (e) {
      setUpdateJob('error');
    }
  }

  function updateAction(what) {
    if (what === 'get') downloadUpdate();
    else if (what === 'install') installUpdate();
    else if (what === 'cancel') api.cancelUpdate();
    else if (what === 'page') api.openReleasePage(update.url);
    else if (what === 'later') {
      P().settings.updateDismissed = update.version;
      save();
      setUpdateJob(null);
    }
  }

  // What to say and offer while an update is under way; null when none is.
  function updateJobView() {
    const phone = api.platform === 'android';
    const install = phone ? 'Install' : 'Restart and update';
    const later = ['later', 'Later', false];
    switch (update.job) {
      case 'downloading': return { text: updateDownloadText(), bar: true, buttons: [['cancel', 'Cancel', false]] };
      case 'ready': return {
        text: phone ? `FreeSheet ${esc(update.version)} is ready. Android will ask you to confirm the install. Your progress stays.`
          : `FreeSheet ${esc(update.version)} is ready. FreeSheet will close and reopen. Your progress stays.`,
        buttons: [['install', install, true], later],
      };
      case 'installing': return { text: 'Starting the installer…', buttons: [] };
      case 'permission': return { text: 'Allow FreeSheet to install apps on the screen that just opened, then come back and press Install.', buttons: [['install', 'Install', true]] };
      case 'opened': return { text: 'Your software installer has opened. Follow it to finish updating.', buttons: [later] };
      case 'error': return { text: 'The update could not be finished. Check your connection and try again.', buttons: [['get', 'Try again', true], ['page', 'Open download page', false], later] };
      default: return null;
    }
  }

  function updateJobHtml(view) {
    return `<span class="update-note">${view.text}</span>` + (view.bar ? `<div class="update-bar"><i style="width:${updatePercent()}%"></i></div>` : '') +
      `<div class="btn-row">${view.buttons.map(([id, label, primary]) => `<button class="btn small-btn ${primary ? 'primary' : ''}" data-u="${id}">${label}</button>`).join('')}</div>`;
  }

  function renderUpdateStatus() {
    const el = main.querySelector('#update-status');
    if (!el) return;
    const job = updateJobView();
    if (job) { el.innerHTML = updateJobHtml(job); }
    else {
      const v = update.current;
      const text = {
        idle: `FreeSheet ${esc(v)} is installed.`,
        checking: 'Checking for updates…',
        current: `FreeSheet ${esc(v)} is up to date.`,
        available: `FreeSheet ${esc(update.version)} is available. You have ${esc(v)}.`,
        failed: 'Could not check for updates. Are you online?',
      }[update.status];
      const available = update.status === 'available';
      el.innerHTML = `<span>${text}</span><button class="btn small-btn ${available ? 'primary' : ''}" data-u="${available ? 'get' : 'check'}" ${update.status === 'checking' ? 'disabled' : ''}>${available ? 'Update now' : 'Check now'}</button>`;
    }
    el.querySelectorAll('[data-u]').forEach((b) => b.addEventListener('click', () => (b.dataset.u === 'check' ? checkForUpdates() : updateAction(b.dataset.u))));
  }

  let updateCard = null;
  function syncUpdateCard() {
    const v = state.views[state.tab];
    const writing = state.tab === 'exams' && v && (v.name === 'run' || v.name === 'mark');
    const show = update.status === 'available' && !writing && P() && (update.job || P().settings.updateDismissed !== update.version)
      && !document.body.classList.contains('welcome-mode') && !document.querySelector('.buddy-setup-back');
    if (!show) { if (updateCard) { updateCard.remove(); updateCard = null; } return; }
    if (!updateCard) {
      updateCard = document.createElement('aside');
      updateCard.className = 'update-card';
      updateCard.setAttribute('role', 'status');
      updateCard.addEventListener('click', (e) => {
        const b = e.target.closest('[data-u]');
        if (b) updateAction(b.dataset.u);
      });
      document.body.appendChild(updateCard);
    }
    const job = updateJobView();
    const size = update.sizeMB ? ` The download is about ${update.sizeMB} MB.` : '';
    updateCard.innerHTML = `<strong>FreeSheet ${esc(update.version)} is available</strong>` + (job
      ? `<div class="update-job">${updateJobHtml(job)}</div>`
      : `<p>You have ${esc(update.current)}. Your progress stays when you update.${size}</p>
      <div class="btn-row"><button class="btn primary small-btn" data-u="get">${Icons.ui('download')}Update now</button><button class="btn small-btn" data-u="later">Later</button></div>`);
  }

  // ---------------------------------------------------------------- study buddy
  //
  // The buddy is drawn and animated in js/buddy.js; what it says and offers is
  // decided here, from where you are and how you're doing. It is never shown while an
  // exam is being written or marked, and hints are never offered in exams.

  const pickOne = (a) => a[Math.floor(Math.random() * a.length)];
  const BUDDY_PRAISE = ["Yes! That's it.", 'Spot on!', 'Nailed it!', "Lekker, that's right!", 'Correct. Nice thinking!', 'Sharp!', "That's the one!"];
  // After a wrong answer: offering the notes section when there is one, else the explanation on the card.
  const BUDDY_ENCOURAGE_NOTES = ["Close one! Want to see where it's explained?", "No stress. Let's look at it together in the notes.", "Not quite, but that's how we learn. I can show you the part of the notes that explains it."];
  const BUDDY_ENCOURAGE = ["Not quite, but that's how we learn. Have a look at the explanation.", 'Mistakes help your brain grow. Read through the explanation, then try the next one.', "No stress. The explanation shows how it's done."];
  const BUDDY_PARTIAL = ['Nearly there! Check the ones marked with a cross.', 'Some right, some not. Look at the corrections.'];
  const BUDDY_HINT_LINES = ['Here\'s a clue! Each time you ask, the hints get a bit stronger.', 'Thinking cap on! Ask again if you need a bigger clue.'];
  const buddyTold = new Set(); // things already said this session
  let buddyRun = 0;            // correct answers in a row
  let buddyPageKey = '';
  let buddyAway = false;       // stepped out for an exam
  let buddyWorksheet = null;   // the open practice worksheet's helpers

  function buddyPrefs() { return P().settings.buddy || null; }

  // ---------------------------------------------------------------- language courses
  // Screens in js/course-ui.js, engine in js/course.js.
  const courseUI = CourseUI.create({
    main, api, M, esc, Icons, Course,
    P: () => P(), save: () => save(), go: (t, v) => go(t, v), grade: () => grade(), token: () => state.renderToken,
    crumbs: (i) => crumbs(i), wireCrumbs: (i) => wireCrumbs(i), bar: (p) => bar(p), scrollTop: () => scrollTop(),
    toast: (m) => toast(m), dialog: (o) => dialog(o), recordStreak: (k, n) => recordStreak(k, n),
    Sound, Motion,
    // The buddy plays the character in lessons (unless it's switched off).
    buddyLook: () => { const b = buddyPrefs(); return b && b.on !== false ? b : null; },
  });

  function applyBuddy() {
    const b = buddyPrefs();
    Buddy.configure({
      look: b || Buddy.randomLook(),
      on: !!b && b.on !== false,
      quiet: !!(b && b.quiet),
      min: !!(b && b.min),
      hooks: {
        menu: buddyMenu,
        onPrefs: (prefs) => { if (P().settings.buddy) { Object.assign(P().settings.buddy, prefs); save(); } },
        onAction: (id) => { if (id === '_look') setupBuddy(false); },
      },
    });
  }

  // "Meet your study buddy" (first time) or "Change your buddy".
  async function setupBuddy(first, intro) {
    const cur = buddyPrefs();
    const look = await Buddy.openSetup({ first: !cur || first, look: cur, intro });
    if (!look) return false;
    P().settings.buddy = Object.assign({ on: true, quiet: false, min: false }, cur || {}, look, cur ? {} : { on: true });
    save();
    applyBuddy();
    if (state.tab === 'settings') render();
    setTimeout(() => {
      Buddy.react('wave');
      Buddy.say(cur ? 'Looking good! Thanks for the new look.'
        : `Hi, I'm **${look.name}**! I'll be around while you study, with tips, hints and a cheer when you get things right. Tap me any time.`,
      { priority: 'user', mood: 'happy' });
    }, 800);
    return true;
  }

  // Say something once per session (key), if the buddy is free to talk.
  function tell(key, text, opts) {
    if (key && buddyTold.has(key)) return false;
    const ok = Buddy.say(text, opts);
    if (ok && key) buddyTold.add(key);
    return ok;
  }

  function openTopic(s, t, mode) { return () => { go('study', { name: 'topic', subjectId: s.id, topicId: t.id, mode: mode || 'notes' }); scrollTop(); }; }

  // What to study next, across the grade.
  function studyRecommendation() {
    const g = gradeData();
    const last = P().settings.lastTopic;
    if (last && last.grade === grade()) {
      const s = subjectMeta(last.subjectId);
      const t = s && s.topics.find((x) => x.id === last.topicId);
      const tp = t && topicProg(tKey(s.id, t.id));
      if (t && !(tp && tp.best != null)) return { text: `Want to pick up **${t.title}** where you left off?`, label: 'Carry on', go: openTopic(s, t, last.mode) };
    }
    let weakest = null;
    for (const s of g.subjects) for (const t of s.topics) {
      const tp = topicProg(tKey(s.id, t.id));
      if (tp && tp.best != null && tp.bestMax) {
        const p = pct(tp.best, tp.bestMax);
        if (p < 60 && (!weakest || p < weakest.p)) weakest = { s, t, p };
      }
    }
    if (weakest) return { text: `Your best on **${weakest.t.title}** is ${weakest.p}%. Want to give it another go?`, label: `Practise ${weakest.t.title}`, go: openTopic(weakest.s, weakest.t, 'questions') };
    let next = null;
    const subs = g.subjects.slice().sort((a, b) => subjectStats(b).practised - subjectStats(a).practised);
    for (const s of subs) {
      const t = s.topics.find((x) => !topicProg(tKey(s.id, x.id)));
      if (t) { next = { s, t }; break; }
    }
    const st = Streak.status(P());
    if (st.current > 0 && !st.todayMet) {
      const left = Math.max(0, st.goalQuestions - st.questionsToday);
      return { text: `${left} more question${left === 1 ? '' : 's'} today keeps your **${st.current}-day streak** going.`, label: next ? `Start ${next.t.title}` : null, go: next ? openTopic(next.s, next.t) : null };
    }
    if (next) return { text: `Why not start **${next.t.title}** in ${next.s.name}?`, label: `Start ${next.t.title}`, go: openTopic(next.s, next.t) };
    return null;
  }

  function subjectAdvice(subjectId) {
    const s = subjectMeta(subjectId);
    if (!s || !s.topics.length) return null;
    const todo = s.topics.find((t) => { const tp = topicProg(tKey(s.id, t.id)); return !(tp && tp.best != null); });
    if (todo) {
      const tp = topicProg(tKey(s.id, todo.id));
      const started = tp && (tp.notesRead || Object.keys(tp.checked || {}).length);
      return { text: started ? `You've started **${todo.title}**. Let's finish it!` : `Next up: **${todo.title}**.`, label: `Open ${todo.title}`, go: openTopic(s, todo) };
    }
    let low = null;
    for (const t of s.topics) {
      const tp = topicProg(tKey(s.id, t.id));
      const p = pct(tp.best, tp.bestMax);
      if (!low || p < low.p) low = { t, p };
    }
    if (low && low.p < 80) return { text: `You've practised every topic here! **${low.t.title}** (${low.p}%) could use another look.`, label: `Practise ${low.t.title}`, go: openTopic(s, low.t, 'questions') };
    return s.exams.length
      ? { text: `80% or more on every topic in ${s.name}. Impressive! Ready for an exam?`, label: 'See the exams', go: () => { go('exams', { name: 'list', subjectId: s.id }); scrollTop(); } }
      : { text: `80% or more on every topic in ${s.name}. Impressive!` };
  }

  function examAdvice(subjectId) {
    const s = subjectMeta(subjectId);
    if (!s || !s.exams.length) return null;
    const intro = (x) => () => { go('exams', { name: 'intro', subjectId: s.id, examId: x.id }); scrollTop(); };
    const fresh = s.exams.find((x) => !(P().exams[xKey(s.id, x.id)] || []).length);
    if (fresh) return { text: `You haven't written **${fresh.title}** yet. Fancy a go?`, label: `Open ${fresh.title}`, go: intro(fresh) };
    let low = null;
    for (const x of s.exams) {
      const best = Math.max(...(P().exams[xKey(s.id, x.id)] || []).map((a) => a.percent));
      if (!low || best < low.best) low = { x, best };
    }
    return { text: `Your best on **${low.x.title}** is ${low.best}%. Want to beat it?`, label: `Open ${low.x.title}`, go: intro(low.x) };
  }

  function progressAdvice() {
    const stats = gradeData().subjects.map((s) => ({ s, st: subjectStats(s) })).filter((x) => x.st.avg != null).sort((a, b) => b.st.avg - a.st.avg);
    if (!stats.length) return { text: "Practise a few topics and I'll tell you how you're doing." };
    const best = stats[0], worst = stats[stats.length - 1];
    if (stats.length === 1) return { text: `So far you're averaging **${best.st.avg}%** in ${best.s.name}. Practise another subject and I'll compare them.` };
    return {
      text: `You're strongest in **${best.s.name}** (${best.st.avg}%). **${worst.s.name}** (${worst.st.avg}%) needs the most work.`,
      label: `Go to ${worst.s.name}`, go: () => { go('study', { name: 'subject', subjectId: worst.s.id }); scrollTop(); },
    };
  }

  function streakAdvice() {
    const st = Streak.status(P());
    const left = Math.max(0, st.goalQuestions - st.questionsToday);
    if (st.todayMet) return { text: `You're on a **${st.current}-day streak** and today's goal is done. See you tomorrow!` };
    if (st.current) return { text: `**${st.current}-day streak!** ${left} more question${left === 1 ? '' : 's'} or one exam today keeps it going.` };
    return { text: `No streak yet. Answer ${st.goalQuestions} practice questions today to start one!` };
  }

  function resultAdvice(v) {
    const a = (P().exams[xKey(v.subjectId, v.examId)] || [])[v.attempt];
    if (!a) return null;
    const lvl = K.capsLevel(a.percent);
    let worst = null;
    for (const sec of a.sections || []) if (sec.max > 0 && (!worst || sec.score / sec.max < worst.score / worst.max)) worst = sec;
    const head = a.percent >= 80 ? `**${a.percent}%**: outstanding! That's level ${lvl.level}.` : a.percent >= 50 ? `**${a.percent}%**: well done!` : `**${a.percent}%**. That was a tough paper.`;
    const tail = worst && worst.score < worst.max ? ` ${M.inline(worst.title).replace(/<[^>]+>/g, '')} cost you the most marks (${pct(worst.score, worst.max)}%).` : '';
    return { text: head + tail, label: 'Review my answers', go: () => { go('exams', { name: 'review', subjectId: v.subjectId, examId: v.examId, attempt: v.attempt }); scrollTop(); } };
  }

  function sayAdvice(r, opts) {
    if (!r) return Buddy.say("I don't have a suggestion right now. You're doing great!", { priority: 'user' });
    return Buddy.say(r.text, Object.assign({ priority: 'user', actions: r.go && r.label ? [{ id: 'go', label: r.label, primary: true, run: r.go }] : [] }, opts));
  }

  // After every page change: step out during exams, otherwise maybe offer a tip.
  function buddyOnPage() {
    if (document.documentElement.classList.contains('rehearsing') || !buddyPrefs()) return;
    Buddy.closeBubble(); // whatever was said was about the page you just left
    const v = state.views[state.tab] || {};
    const inExam = state.tab === 'exams' && (v.name === 'run' || v.name === 'mark');
    if (inExam) {
      if (!buddyAway) {
        buddyAway = true;
        if (v.name === 'run') {
          Buddy.react('wave');
          Buddy.say(pickOne(["Good luck! No help in exams, so I'll wait outside.", "You've got this! See you when you're done."]), { priority: 'user', mood: 'happy' });
          setTimeout(() => { if (buddyAway) Buddy.hide('exam', true); }, 2300);
        } else Buddy.hide('exam', true);
      }
      return;
    }
    if (buddyAway) { buddyAway = false; Buddy.hide('exam', false); }
    Buddy.hide('course', state.tab === 'study' && (v.name === 'course' || v.name === 'lesson'));
    if (state.tab === 'study' && (v.name === 'course' || v.name === 'lesson')) return;
    const key = navInfo().key;
    buddyPageKey = key;
    setTimeout(() => { if (buddyPageKey === key) buddyTip(v); }, 950);
  }

  function buddyTip(v) {
    const tab = state.tab;
    const withAction = (r) => (r && r.go && r.label ? [{ id: 'go', label: r.label, primary: true, run: r.go }] : []);
    if (tab === 'study' && v.name === 'home') {
      const r = studyRecommendation();
      const h = new Date().getHours();
      const hi = h < 12 ? 'Good morning' : h < 17 ? 'Good afternoon' : 'Good evening';
      tell('home', `${hi}! ${r ? r.text : "Pick a subject and let's get started."}`, { mood: 'happy', actions: withAction(r) });
    } else if (tab === 'study' && v.name === 'subject') {
      const r = subjectAdvice(v.subjectId);
      if (r) tell(`subject:${v.subjectId}`, r.text, { actions: withAction(r) });
    } else if (tab === 'study' && v.name === 'topic') {
      const topic = state.cache.get(`t:${grade()}/${v.subjectId}/${v.topicId}`);
      if (!topic) return;
      const key = tKey(v.subjectId, v.topicId);
      if (v.mode === 'questions') {
        const sc = topicScore(topic, topicProg(key) || {});
        if (sc.done === 0 && !sc.pending) tell(`qs:${key}`, `${sc.total} questions. Stuck on one? Tap **Hint** and I'll help.`);
        else if (!sc.complete) tell(`qs:${key}`, `Welcome back! ${sc.done} of ${sc.total} done. Let's finish them.`);
      } else {
        const n = Sections.split(topic.notes).length;
        const listen = document.querySelector('#reader-bar:not([hidden])') ? ' Or tap **Listen** and I\'ll read along with you.' : '';
        tell(`notes:${key}`, n > 1 ? `This topic has ${n} sections. Read one, then tap **Continue** for the next.${listen}` : `Read through this, then test yourself with the questions.${listen}`);
      }
    } else if (tab === 'exams' && v.name === 'home') {
      tell('exams', "Exams here work like the real thing: timed, then marked with the memo. I can't help during an exam, but I'll help you review afterwards.");
    } else if (tab === 'exams' && v.name === 'list') {
      const r = examAdvice(v.subjectId);
      if (r) tell(`examlist:${v.subjectId}`, r.text, { actions: withAction(r) });
    } else if (tab === 'exams' && v.name === 'intro') {
      tell(`intro:${v.examId}`, 'Read every question carefully and keep an eye on the timer. You\'ve got this!', { mood: 'happy' });
    } else if (tab === 'exams' && v.name === 'result') {
      const r = resultAdvice(v);
      const a = (P().exams[xKey(v.subjectId, v.examId)] || [])[v.attempt];
      if (r && !buddyTold.has(`result:${v.examId}:${v.attempt}`)) {
        Buddy.react(a && a.percent >= 50 ? 'celebrate' : 'think');
        tell(`result:${v.examId}:${v.attempt}`, r.text, { priority: 'user', actions: withAction(r) });
      }
    } else if (tab === 'exams' && v.name === 'review') {
      tell('review', "Here's the memo for every question. Look closely at where you lost marks.");
    } else if (tab === 'progress') {
      const r = progressAdvice();
      tell('progress', r.text, { actions: withAction(r) });
    } else if (tab === 'settings') {
      tell('settings', 'This is where you can change how I look, or turn me off if you need some quiet.');
    }
  }

  // Tap the buddy: a menu that fits the page.
  function buddyMenu() {
    const v = state.views[state.tab] || {};
    const items = [];
    const add = (id, label, run) => items.push({ id, label, run });
    if (state.tab === 'study' && v.name === 'home') {
      add('rec', 'What should I study?', () => sayAdvice(studyRecommendation()));
      add('streak', "How's my streak?", () => sayAdvice(streakAdvice()));
    } else if (state.tab === 'study' && v.name === 'subject') {
      add('next', 'Which topic should I do next?', () => sayAdvice(subjectAdvice(v.subjectId)));
    } else if (state.tab === 'study' && v.name === 'topic') {
      if (v.mode === 'questions') {
        add('hint', 'Give me a hint', () => {
          const card = [...main.querySelectorAll('.q-card')].find((c) => c.querySelector('.hint-btn'));
          if (!card) return Buddy.say("No more hints on these. Give it your best shot. You can do it!", { priority: 'user' });
          card.scrollIntoView({ behavior: Motion.enabled() ? 'smooth' : 'auto', block: 'center' });
          setTimeout(() => { const b = card.querySelector('.hint-btn'); if (b) b.click(); }, 300);
        });
        if (buddyWorksheet && buddyWorksheet.firstWrongCard()) add('wrong', 'Show me what I got wrong', () => buddyWorksheet.showCard(buddyWorksheet.firstWrongCard()));
        add('notes', 'Take me to the notes', () => { state.views.study.mode = 'notes'; render(); scrollTop(); });
      } else {
        if (document.querySelector('#reader-bar:not([hidden])') && reader.index < 0) add('read', 'Read these notes to me', () => { const b = document.querySelector('#read-btn'); if (b) b.click(); });
        add('quiz', 'Test me on this topic', () => { state.views.study.mode = 'questions'; render(); scrollTop(); });
      }
    } else if (state.tab === 'exams' && (v.name === 'home' || v.name === 'list' || v.name === 'intro')) {
      const sid = v.subjectId || (gradeData().subjects.find((s) => s.exams.length) || {}).id;
      if (sid) add('paper', 'Which exam should I write?', () => sayAdvice(examAdvice(sid)));
    } else if (state.tab === 'exams' && (v.name === 'result' || v.name === 'review')) {
      add('revise', 'How did I do?', () => sayAdvice(resultAdvice(v)));
    } else if (state.tab === 'progress') {
      add('how', 'How am I doing?', () => sayAdvice(progressAdvice()));
      add('streak', "How's my streak?", () => sayAdvice(streakAdvice()));
    }
    return items;
  }

  // React to one checked answer.
  function buddyAnswer(card, q, topic) {
    if (card.classList.contains('is-correct')) {
      buddyRun++;
      Buddy.react('correct');
      const n = buddyRun;
      if (n === 3 || n === 5 || n % 10 === 0) Buddy.say(n === 3 ? 'Three in a row!' : n === 5 ? "Five in a row! You're flying." : `${n} in a row! Unstoppable.`, { priority: 'reaction', mood: 'happy' });
      else if (Math.random() < 0.35) Buddy.say(pickOne(BUDDY_PRAISE), { priority: 'reaction', mood: 'happy' });
    } else if (card.classList.contains('is-wrong') || card.classList.contains('is-partial')) {
      buddyRun = 0;
      const partial = card.classList.contains('is-partial');
      Buddy.react(partial ? 'partial' : 'wrong');
      const sec = Hints.relatedSection(q, topic);
      Buddy.say(pickOne(partial ? BUDDY_PARTIAL : sec != null ? BUDDY_ENCOURAGE_NOTES : BUDDY_ENCOURAGE), {
        priority: 'tip', mood: partial ? 'think' : 'sad',
        actions: sec != null ? [{ id: 'notes', label: 'Show me in the notes', primary: true, run: () => showInNotes(sec) }] : [],
      });
    }
  }

  // Jump from a question to the notes section that explains it.
  function showInNotes(sec) {
    const v = state.views.study;
    if (state.tab !== 'study' || v.name !== 'topic') return;
    v.mode = 'notes';
    v.focus = sec;
    render();
  }

  // ---------------------------------------------------------------- welcome / grade

  function syncGradeSelect() {
    gradeSelect.innerHTML = state.catalog.grades.map((g) => `<option value="${g.grade}">Grade ${g.grade}</option>`).join('');
    gradeSelect.value = String(grade());
  }

  function showWelcome() {
    document.body.classList.add('welcome-mode');
    main.innerHTML = `<div class="page welcome">
      <h1>Welcome to FreeSheet</h1>
      <p class="lead">Notes, practice questions and full exams for the CAPS curriculum.${api.web ? '' : ` Everything works offline and your progress is saved on ${device()}.`}</p>
      <h2>Which grade are you in?</h2>
      <div class="grade-buttons">${state.catalog.grades.map((g) => `<button class="btn big" data-grade="${g.grade}">Grade ${g.grade}</button>`).join('')}</div>
      <p class="muted">You can change the grade at any time from the top bar.</p>
    </div>`;
    Motion.navigate(main, null, { kind: 'first' });
    main.querySelectorAll('[data-grade]').forEach((b) => b.addEventListener('click', () => {
      P().settings.grade = Number(b.dataset.grade);
      save();
      document.body.classList.remove('welcome-mode');
      syncGradeSelect();
      render();
      if (!P().settings.buddy) setTimeout(() => setupBuddy(true), 600);
    }));
  }

  // ---------------------------------------------------------------- boot

  // Saved or imported progress may come from an older version or another device:
  // make sure every part the app writes to exists and is the right shape.
  function normaliseProgress(progress, fallbackGrade) {
    for (const f of ['settings', 'topics', 'exams', 'examDrafts', 'courses']) {
      if (!progress[f] || typeof progress[f] !== 'object' || Array.isArray(progress[f])) progress[f] = {};
    }
    for (const k of Object.keys(progress.exams)) if (!Array.isArray(progress.exams[k])) delete progress.exams[k];
    for (const k of Object.keys(progress.topics)) if (!progress.topics[k] || typeof progress.topics[k] !== 'object') delete progress.topics[k];
    if (fallbackGrade != null && !state.catalog.grades.some((g) => g.grade === progress.settings.grade)) progress.settings.grade = fallbackGrade;
    return progress;
  }

  async function boot() {
    const [catalog, progress] = await Promise.all([api.getCatalog(), api.loadProgress()]);
    state.catalog = catalog;
    state.progress = progress;
    normaliseProgress(progress);
    streakBtn.addEventListener('click', showStreak);
    document.addEventListener('pointerdown', (e) => {
      if (e.button !== 0) return;
      const t = e.target.closest('button, .card, .row, select, summary, a, label.check, input[type="checkbox"]');
      if (t && !t.disabled) Sound.play('tap');
    }, true);
    document.addEventListener('keydown', (e) => {
      if ((e.key === 'Enter' || e.key === ' ') && e.target.closest && e.target.closest('button, summary') && !e.repeat) Sound.play('tap');
    }, true);
    // Ctrl+, (Cmd+, on a Mac) opens Settings, as in most desktop apps.
    document.addEventListener('keydown', (e) => {
      if (e.key === ',' && (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && api.platform !== 'android') { e.preventDefault(); openSettings(); }
    });
    document.addEventListener('toggle', (e) => {
      if (e.target instanceof HTMLDetailsElement) Sound.play(e.target.open ? 'reveal' : 'close');
    }, true);
    // On phones a long introduction shows three lines (mobile.css); a tap shows it all.
    main.addEventListener('click', (e) => {
      const lead = e.target.closest('.page-head .lead, .page-topic > .lead');
      if (lead && !e.target.closest('a, button')) lead.classList.toggle('open');
    });
    settingsBtn.innerHTML = Icons.ui('settings');
    settingsBtn.addEventListener('click', openSettings);
    // Remember which card or row was tapped, so the page it opens can zoom out of it.
    main.addEventListener('click', (e) => {
      const el = e.target.closest('.card, .row, .continue');
      if (el && main.contains(el)) Motion.noteOrigin(el);
    }, true);
    // Keep the badge right if the app stays open past midnight.
    window.addEventListener('focus', () => updateStreakBadge());
    setInterval(() => updateStreakBadge(), 60 * 1000);
    applyAppearance();
    applyTextSize();
    applyTextRendering();
    applyMotion();
    applySound();
    applyBuddy();

    document.getElementById('brand-logo').innerHTML = Icons.ui('logo', 'brand-logo');
    document.querySelectorAll('[data-icon]').forEach((el) => { el.innerHTML = Icons.ui(el.dataset.icon); });

    document.querySelectorAll('.tab').forEach((b) => b.addEventListener('click', () => {
      if (document.body.classList.contains('welcome-mode')) return;
      if (b.dataset.tab === state.tab) {
        // Clicking the active tab returns to its home screen (except mid-exam).
        const v = state.views[state.tab];
        if (!(state.tab === 'exams' && (v.name === 'run' || v.name === 'mark'))) state.views[state.tab] = { name: 'home' };
      }
      setTab(b.dataset.tab);
      scrollTop();
    }));
    gradeSelect.addEventListener('change', async () => {
      if (state.exam && state.exam.phase === 'run') {
        writeExamDraft();
        toast('Your exam has been saved. Resume it from the Exams tab.');
      }
      stopExam();
      P().settings.grade = Number(gradeSelect.value);
      save();
      state.views = { study: { name: 'home' }, exams: { name: 'home' }, progress: { name: 'home' }, settings: { name: 'home' } };
      render();
      scrollTop();
    });

    // Quietly, after the first page is up: the window never waits for the network.
    api.version().then((v) => {
      update.current = v;
      renderUpdateStatus();
      if (!api.web && progress.settings.checkUpdates !== false) setTimeout(checkForUpdates, 1200);
    }).catch(() => {});

    const grades = catalog.grades.map((g) => g.grade);
    if (!grades.includes(progress.settings.grade)) {
      progress.settings.grade = undefined;
      if (grades.length) {
        gradeSelect.innerHTML = '';
        return showWelcome();
      }
    }
    syncGradeSelect();
    if (['exams', 'progress'].includes(progress.settings.startTab)) state.tab = progress.settings.startTab;
    render();
    // New in this version for existing users: meet the study buddy.
    if (!progress.settings.buddy) setTimeout(() => setupBuddy(true, 'New in this update!'), 700);
  }

  boot().catch((err) => {
    console.error(err);
    main.innerHTML = `<div class="page"><h1>FreeSheet could not start</h1><p>${esc(err.message || err)}</p></div>`;
  });
})();
