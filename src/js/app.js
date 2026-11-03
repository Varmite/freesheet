// FreeSheet renderer: navigation, study notes, practice worksheets, exams and progress.
(function () {
  'use strict';
  const M = window.Markup;
  const K = window.Marking;
  const Q = window.Questions;
  const api = window.api;
  const esc = M.escapeHtml;

  const main = document.getElementById('main');
  const gradeSelect = document.getElementById('grade-select');
  const dialogRoot = document.getElementById('dialog-root');

  const state = {
    catalog: null,
    progress: null,
    tab: 'study',
    views: { study: { name: 'home' }, exams: { name: 'home' }, progress: { name: 'home' } },
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

  async function loadTopic(subjectId, topicId) {
    const k = `t:${grade()}/${subjectId}/${topicId}`;
    if (!state.cache.has(k)) state.cache.set(k, await api.getTopic(grade(), subjectId, topicId));
    return state.cache.get(k);
  }
  async function loadExam(subjectId, examId) {
    const k = `x:${grade()}/${subjectId}/${examId}`;
    if (!state.cache.has(k)) state.cache.set(k, await api.getExam(grade(), subjectId, examId));
    return state.cache.get(k);
  }

  // ---------------------------------------------------------------- helpers

  function pct(score, max) { return max > 0 ? Math.round((score / max) * 100) : 0; }

  function toast(msg) {
    const t = document.createElement('div');
    t.className = 'toast';
    t.textContent = msg;
    document.body.appendChild(t);
    setTimeout(() => t.classList.add('show'), 10);
    setTimeout(() => { t.classList.remove('show'); setTimeout(() => t.remove(), 300); }, 2600);
  }

  function dialog({ title, body, ok = 'OK', cancel = 'Cancel', danger = false }) {
    return new Promise((resolve) => {
      const prevFocus = document.activeElement;
      dialogRoot.innerHTML = `<div class="modal-back"><div class="modal" role="dialog" aria-modal="true" aria-labelledby="dlg-title">
        <h2 id="dlg-title">${esc(title)}</h2><div class="modal-body">${body || ''}</div>
        <div class="modal-actions">${cancel ? `<button class="btn" data-r="0">${esc(cancel)}</button>` : ''}
        <button class="btn ${danger ? 'danger' : 'primary'}" data-r="1">${esc(ok)}</button></div></div></div>`;
      const close = (v) => {
        dialogRoot.innerHTML = '';
        document.removeEventListener('keydown', onKey, true);
        if (prevFocus && prevFocus.focus) prevFocus.focus();
        resolve(v);
      };
      const onKey = (e) => {
        if (e.key === 'Escape') { e.preventDefault(); close(false); }
      };
      document.addEventListener('keydown', onKey, true);
      dialogRoot.querySelectorAll('[data-r]').forEach((b) => b.addEventListener('click', () => close(b.dataset.r === '1')));
      dialogRoot.querySelector('.modal-back').addEventListener('click', (e) => { if (e.target.classList.contains('modal-back') && cancel) close(false); });
      dialogRoot.querySelector('[data-r="1"]').focus();
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

  async function render() {
    const token = ++state.renderToken;
    document.querySelectorAll('.tab').forEach((b) => {
      const on = b.dataset.tab === state.tab;
      b.classList.toggle('active', on);
      b.setAttribute('aria-selected', on);
    });
    const v = state.views[state.tab];
    try {
      let html;
      if (!state.catalog.grades.length) {
        main.innerHTML = `<div class="page"><h1>No content found</h1><p>The <code>content</code> folder is empty or missing.</p></div>`;
        return;
      }
      if (state.tab === 'study') {
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
          <span class="card-title">${esc(s.name)}</span>
          <span class="card-sub">${st.total} topic${st.total === 1 ? '' : 's'}</span>
          ${bar(p)}
          <span class="card-meta">${st.practised} of ${st.total} practised${st.avg != null ? ` · avg ${st.avg}%` : ''}</span>
        </button>`;
      }).join('')}</div>
      ${g.subjects.length ? '' : '<p class="muted">No subjects are available for this grade yet.</p>'}
    </div>`;
    main.querySelectorAll('[data-subject]').forEach((b) => b.addEventListener('click', () => {
      go('study', { name: 'subject', subjectId: b.dataset.subject }); scrollTop();
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
        <div><h1>${esc(s.name)}</h1><p class="lead">${M.inline(s.description)}</p></div>
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
      ${s.exams.length ? `<div class="aside-link"><button class="btn" id="to-exams">Practise full ${esc(s.name)} exams →</button></div>` : ''}
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
    f.addEventListener('input', applyFilter);
    applyFilter();
    const te = main.querySelector('#to-exams');
    if (te) te.addEventListener('click', () => { go('exams', { name: 'list', subjectId: s.id }); scrollTop(); });
  }

  // ---------- Study: topic (notes + questions)

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
    main.innerHTML = `<div class="page page-reading">
      ${crumbs(items)}
      <h1>${esc(topic.title)}</h1>
      <p class="lead">${M.inline(topic.summary || '')}</p>
      <div class="segmented" role="tablist">
        <button role="tab" class="seg ${mode === 'notes' ? 'active' : ''}" data-mode="notes" aria-selected="${mode === 'notes'}">Notes</button>
        <button role="tab" class="seg ${mode === 'questions' ? 'active' : ''}" data-mode="questions" aria-selected="${mode === 'questions'}">Questions (${topic.questions.length})</button>
      </div>
      <div id="topic-body"></div>
      <nav class="pager">
        ${prev ? `<button class="btn" data-goto="${esc(prev.id)}">← ${esc(prev.title)}</button>` : '<span></span>'}
        ${next ? `<button class="btn" data-goto="${esc(next.id)}">${esc(next.title)} →</button>` : '<span></span>'}
      </nav>
    </div>`;
    wireCrumbs(items);
    main.querySelectorAll('.seg').forEach((b) => b.addEventListener('click', () => {
      v.mode = b.dataset.mode; render(); scrollTop();
    }));
    main.querySelectorAll('[data-goto]').forEach((b) => b.addEventListener('click', () => {
      go('study', { name: 'topic', subjectId: s.id, topicId: b.dataset.goto, mode: 'notes' }); scrollTop();
    }));
    const body = main.querySelector('#topic-body');
    if (mode === 'notes') renderNotes(body, topic, key, v);
    else renderPractice(body, topic, key);
  }

  function renderNotes(body, topic, key, v) {
    const tp = topicProg(key);
    body.innerHTML = `<article class="notes">${M.blocks(topic.notes)}</article>
      <div class="notes-end">
        <p>${tp && tp.notesRead ? 'You have marked these notes as read.' : 'Finished reading? Test yourself on what you have just studied.'}</p>
        <button class="btn primary" id="to-questions">Practise ${topic.questions.length} questions →</button>
      </div>`;
    body.querySelector('#to-questions').addEventListener('click', () => {
      topicProg(key, true).notesRead = true;
      save();
      v.mode = 'questions';
      render();
      scrollTop();
    });
  }

  function renderPractice(body, topic, key) {
    const tp = topicProg(key, true);
    tp.notesRead = true;
    // Drop answers to questions that no longer exist (content updated).
    const qids = new Set(topic.questions.map((q) => q.id));
    for (const field of ['responses', 'checked', 'selfMarks']) {
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

    function updateSummary() {
      const sc = topicScore(topic, tp);
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
        end.querySelector('#retry').addEventListener('click', () => {
          tp.responses = {}; tp.checked = {}; tp.selfMarks = {}; tp.recorded = false;
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
          topic.questions.forEach((q, i) => {
            if (!tp.checked[q.id]) {
              tp.checked[q.id] = true;
              cards[i].ctx.phase = 'feedback';
              cards[i].el.refresh();
            }
          });
          recordIfComplete(); save(); updateSummary();
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
          tp.checked[q.id] = true;
          ctx.phase = 'feedback';
          el.refresh();
          recordIfComplete(); save(); updateSummary();
        },
        onSelfMark: (n) => {
          tp.selfMarks[q.id] = n;
          recordIfComplete(); save(); updateSummary();
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
      <p class="lead">Write full exam papers under timed conditions, then mark them with the memo.</p>
      ${draftHtml}
      <div class="grid">${subs.map((s) => {
        const st = subjectStats(s);
        return `<button class="card" data-subject="${esc(s.id)}">
          <span class="card-title">${esc(s.name)}</span>
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
      <h1>${esc(s.name)} exams</h1>
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
      ${draft ? `<div class="notice">You have an unfinished attempt${draft.timerOn ? ` with ${formatDuration(draft.remainingSec)} left` : ''}.</div>` : ''}
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
    const { subjectId, examId } = e;
    stopExam();
    flush();
    go('exams', { name: 'result', subjectId, examId, attempt: hist.length - 1 });
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
    const ts = P().settings.textSize || 'normal';
    main.innerHTML = `<div class="page">
      <h1>Grade ${g.grade} progress</h1>
      <div class="facts">
        <div><strong>${practised}/${total}</strong><span>topics practised</span></div>
        <div><strong>${practised ? Math.round(sumPct / practised) + '%' : '–'}</strong><span>average practice score</span></div>
        <div><strong>${examsWritten}</strong><span>exams written</span></div>
        <div><strong>${examsWritten ? Math.round(examPctSum / examsWritten) + '%' : '–'}</strong><span>average exam score</span></div>
      </div>
      <h2>By subject</h2>
      <div class="n-table-wrap"><table class="n-table progress-table"><thead><tr><th>Subject</th><th>Notes read</th><th>Topics practised</th><th>Average</th><th>Best exam</th></tr></thead><tbody>
        ${rows.map(({ s, st }) => `<tr><td><button class="link" data-subject="${esc(s.id)}">${esc(s.name)}</button></td>
          <td>${st.notes}/${st.total}</td><td><div class="cell-bar">${bar(st.total ? Math.round(st.practised / st.total * 100) : 0)}<span>${st.practised}/${st.total}</span></div></td>
          <td>${st.avg != null ? st.avg + '%' : '–'}</td><td>${st.bestExam != null ? st.bestExam + '%' : '–'}</td></tr>`).join('')}
      </tbody></table></div>
      <h2>Recent exams</h2>
      ${recent.length ? `<div class="n-table-wrap"><table class="n-table"><thead><tr><th>Date</th><th>Subject</th><th>Paper</th><th>Mark</th></tr></thead><tbody>
        ${recent.slice(0, 15).map((r) => `<tr><td>${esc(formatDate(r.a.date))}</td><td>${esc(r.s.name)}</td>
          <td><button class="link" data-res="${esc(r.s.id)}|${esc(r.x.id)}|${r.i}">${esc(r.x.title)}</button></td><td>${r.a.percent}%</td></tr>`).join('')}
      </tbody></table></div>` : '<p class="muted">You have not written any exams in this grade yet.</p>'}

      <h2>Settings</h2>
      <div class="settings">
        <div class="setting"><span>Text size</span><div class="segmented small">
          ${['small', 'normal', 'large', 'xlarge'].map((k) => `<button class="seg ${ts === k ? 'active' : ''}" data-size="${k}">${{ small: 'Small', normal: 'Normal', large: 'Large', xlarge: 'Extra large' }[k]}</button>`).join('')}
        </div></div>
        <div class="setting"><span>Your data</span><div class="btn-row">
          <button class="btn" id="export">Export progress</button>
          <button class="btn" id="import">Import progress</button>
          <button class="btn" id="open-folder">Open data folder</button>
          <button class="btn danger-outline" id="reset">Reset all progress</button></div></div>
        <p class="muted small" id="location"></p>
      </div>
    </div>`;
    main.querySelectorAll('[data-subject]').forEach((b) => b.addEventListener('click', () => go('study', { name: 'subject', subjectId: b.dataset.subject })));
    main.querySelectorAll('[data-res]').forEach((b) => b.addEventListener('click', () => {
      const [sid, xid, i] = b.dataset.res.split('|');
      go('exams', { name: 'result', subjectId: sid, examId: xid, attempt: Number(i) });
    }));
    main.querySelectorAll('[data-size]').forEach((b) => b.addEventListener('click', () => {
      P().settings.textSize = b.dataset.size; applyTextSize(); save(); render();
    }));
    main.querySelector('#export').addEventListener('click', async () => {
      await flush();
      const r = await api.exportProgress(P());
      if (r && r.ok) toast('Progress exported.');
    });
    main.querySelector('#import').addEventListener('click', async () => {
      const r = await api.importProgress();
      if (!r || !r.ok) { if (r && r.error) toast(r.error); return; }
      if (!(await dialog({ title: 'Replace your progress?', body: '<p>Importing replaces all progress on this computer with the progress in the file.</p>', ok: 'Import', danger: true }))) return;
      stopExam();
      state.progress = r.data;
      applyTextSize(); syncGradeSelect(); await flush(); toast('Progress imported.'); render();
    });
    main.querySelector('#open-folder').addEventListener('click', () => api.openDataFolder());
    main.querySelector('#reset').addEventListener('click', async () => {
      if (!(await dialog({ title: 'Reset all progress?', body: '<p>This deletes all notes read, practice answers and exam results for every grade. This cannot be undone. Consider exporting first.</p>', ok: 'Reset everything', danger: true }))) return;
      stopExam();
      const settings = P().settings;
      state.progress = { version: 1, settings: { grade: settings.grade, textSize: settings.textSize }, topics: {}, exams: {}, examDrafts: {} };
      await flush(); toast('Progress reset.'); render();
    });
    api.progressLocation().then((loc) => {
      const el = main.querySelector('#location');
      if (el) el.textContent = `Progress is saved on this computer only, in ${loc}`;
    });
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
      <p class="lead">Notes, practice questions and full exams for the CAPS curriculum. Everything works offline and your progress is saved on this computer.</p>
      <h2>Which grade are you in?</h2>
      <div class="grade-buttons">${state.catalog.grades.map((g) => `<button class="btn big" data-grade="${g.grade}">Grade ${g.grade}</button>`).join('')}</div>
      <p class="muted">You can change the grade at any time from the top bar.</p>
    </div>`;
    main.querySelectorAll('[data-grade]').forEach((b) => b.addEventListener('click', () => {
      P().settings.grade = Number(b.dataset.grade);
      save();
      document.body.classList.remove('welcome-mode');
      syncGradeSelect();
      render();
    }));
  }

  // ---------------------------------------------------------------- boot

  async function boot() {
    const [catalog, progress] = await Promise.all([api.getCatalog(), api.loadProgress()]);
    state.catalog = catalog;
    state.progress = progress;
    for (const f of ['settings', 'topics', 'exams', 'examDrafts']) if (!progress[f] || typeof progress[f] !== 'object') progress[f] = {};
    applyTextSize();

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
      state.views = { study: { name: 'home' }, exams: { name: 'home' }, progress: { name: 'home' } };
      render();
      scrollTop();
    });

    const grades = catalog.grades.map((g) => g.grade);
    if (!grades.includes(progress.settings.grade)) {
      progress.settings.grade = undefined;
      if (grades.length) {
        gradeSelect.innerHTML = '';
        return showWelcome();
      }
    }
    syncGradeSelect();
    render();
  }

  boot().catch((err) => {
    console.error(err);
    main.innerHTML = `<div class="page"><h1>FreeSheet could not start</h1><p>${esc(err.message || err)}</p></div>`;
  });
})();
