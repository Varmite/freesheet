// Language course screens: the course path (units and lessons) and the lesson player.
// The lesson engine (exercises, checking) is in js/course.js; this file draws it.
// Created by app.js with the app's helpers: CourseUI.create(deps).
(function () {
  'use strict';

  const DAY = () => (window.Streak ? Streak.dayKey() : new Date().toISOString().slice(0, 10));

  function create(d) {
    const { main, api, M, esc, Icons, Course } = d;
    const cache = new Map();          // course id -> course JSON
    const audioIds = new Map();       // course id -> Set of audio ids that exist
    let session = null;               // the lesson being played
    let keyHandler = null;

    // ---------------------------------------------------------------- data

    async function load(id) {
      if (!cache.has(id)) {
        const [course, ids] = await Promise.all([api.getCourse(id), api.courseAudioList(id).catch(() => [])]);
        cache.set(id, course);
        audioIds.set(id, new Set(ids));
      }
      return cache.get(id);
    }
    const hasAudio = (id) => (aid) => !!aid && audioIds.has(id) && audioIds.get(id).has(aid);

    // Progress for a course: { done: { lessonId: { times, best, xp } }, xp, days: { day: xp }, seen: [words], practice }
    function prog(id) {
      const all = d.P().courses || (d.P().courses = {});
      const p = all[id] || (all[id] = { done: {}, xp: 0, days: {}, seen: [], practice: 0 });
      for (const [f, v] of [['done', {}], ['days', {}], ['seen', []]]) if (!p[f]) p[f] = v;
      return p;
    }
    function lessonState(m, p) {
      // Lessons unlock in order: one is open once the one before it is done.
      return m.lessons.map((x, i) => ({
        ...x,
        done: !!p.done[x.lesson.id],
        open: i === 0 || !!p.done[m.lessons[i - 1].lesson.id] || !!p.done[x.lesson.id],
      }));
    }

    // A card for the Study home grid (the course shows up as a subject in every grade).
    function homeCard(meta) {
      const p = (d.P().courses || {})[meta.id];
      const done = p ? Object.keys(p.done || {}).length : 0;
      const pc = meta.lessons ? Math.round((done / meta.lessons) * 100) : 0;
      return `<button class="card" data-course="${esc(meta.id)}">
          <span class="card-head">${Icons.subject(meta.id)}<span class="card-title">${esc(meta.name)}</span></span>
          <span class="card-sub">${meta.units} units · ${meta.lessons} lessons</span>
          ${d.bar(pc)}
          <span class="card-meta">${done} of ${meta.lessons} lessons${p && p.xp ? ` · ${p.xp} XP` : ''}</span>
        </button>`;
    }

    // ---------------------------------------------------------------- course path

    async function viewCourse(v, token) {
      main.innerHTML = `<div class="page"><div class="loading">Loading…</div></div>`;
      const course = await load(v.courseId);
      if (token !== d.token()) return;
      const m = Course.model(course);
      const p = prog(course.id);
      const ls = lessonState(m, p);
      const next = ls.find((x) => x.open && !x.done) || null;
      const doneCount = ls.filter((x) => x.done).length;
      const items = [{ label: `Grade ${d.grade()}`, go: () => d.go('study', { name: 'home' }) }, { label: course.name }];
      const today = p.days[DAY()] || 0;
      main.innerHTML = `<div class="page page-reading course-page">
        ${d.crumbs(items)}
        <div class="course-hero">
          <div class="course-hero-main">
            <h1 class="with-icon">${Icons.subject(course.id, 'lg')}${esc(course.name)}</h1>
            <p class="lead">${esc(course.about || '')}</p>
            <div class="btn-row">
              ${next ? `<button class="btn primary" id="go-next">${doneCount ? 'Continue' : 'Start'}: ${esc(next.lesson.title)}</button>` : ''}
              <button class="btn" id="practise" ${doneCount ? '' : 'disabled title="Finish a lesson first"'}>Practise what I've learnt</button>
            </div>
          </div>
          <div class="course-stats">
            <div><strong>${p.xp || 0}</strong><span>XP</span></div>
            <div><strong>${today}</strong><span>XP today</span></div>
            <div><strong>${doneCount}/${ls.length}</strong><span>lessons</span></div>
          </div>
        </div>
        ${course.units.map((u, ui) => {
          const lessons = ls.filter((x) => x.unitIndex === ui);
          const udone = lessons.filter((x) => x.done).length;
          return `<section class="unit" style="--h:${u.hue || 200}">
            <div class="unit-head"><div><div class="unit-label">Unit ${ui + 1}</div><h2>${esc(u.title)}</h2><div class="unit-en">${esc(u.english || '')}</div></div>
              <div class="unit-count">${udone}/${lessons.length}</div></div>
            <ol class="trail">${lessons.map((x) => {
              const cur = next && x.lesson.id === next.lesson.id;
              const cls = x.done ? 'done' : cur ? 'current' : x.open ? 'open' : 'locked';
              const times = x.done ? p.done[x.lesson.id].times : 0;
              const icon = x.done ? Icons.ui('check') : x.open ? Icons.ui('star') : Icons.ui('lock');
              return `<li class="stop ${cls}">
                <button class="stop-btn" data-lesson="${esc(x.lesson.id)}" aria-label="${esc(x.lesson.title)}${x.done ? ' (done)' : x.open ? '' : ' (locked)'}">${icon}</button>
                <div class="stop-text"><strong>${esc(x.lesson.title)}</strong><span>Lesson ${x.indexInUnit + 1}${x.done ? ` · done${times > 1 ? ` ×${times}` : ''} · best ${p.done[x.lesson.id].best}%` : x.open ? '' : ' · locked'}</span></div>
                ${cur ? `<div class="stop-buddy" aria-hidden="true"></div><div class="stop-call">Start here!</div>` : ''}
              </li>`;
            }).join('')}</ol>
          </section>`;
        }).join('')}
      </div>`;
      d.wireCrumbs(items);
      const start = (id) => { d.go('study', { name: 'lesson', courseId: course.id, lessonId: id, run: Date.now() }); d.scrollTop(); };
      const nb = main.querySelector('#go-next');
      if (nb) nb.addEventListener('click', () => start(next.lesson.id));
      main.querySelector('#practise').addEventListener('click', () => { if (doneCount) start('_practice'); });
      main.querySelectorAll('[data-lesson]').forEach((b) => b.addEventListener('click', () => {
        const x = ls.find((y) => y.lesson.id === b.dataset.lesson);
        if (!x.open) { d.Sound.play('error'); d.Motion.verdict(b.closest('.stop')); return d.toast('Finish the lessons before this one first.'); }
        start(x.lesson.id);
      }));
      const spot = main.querySelector('.stop-buddy');
      if (spot && d.buddyLook()) {
        const pup = Buddy.puppet(spot, { look: d.buddyLook() });
        setTimeout(() => pup.react('wave'), 600);
      }
      const cur = main.querySelector('.stop.current');
      if (cur && doneCount) setTimeout(() => cur.scrollIntoView({ behavior: d.Motion.enabled() ? 'smooth' : 'auto', block: 'center' }), 500);
    }

    // ---------------------------------------------------------------- lesson player

    const TITLES = {
      meaning: (ex) => (ex.isNew ? 'New word! What does it mean?' : 'What does this word mean?'),
      pick: () => 'What does this mean?',
      build: (ex) => (ex.dir === 'af-en' ? 'Translate this sentence' : 'Write this in Afrikaans'),
      type: (ex) => (ex.dir === 'af-en' ? 'Type this in English' : 'Type this in Afrikaans'),
      blank: () => 'Fill in the missing word',
      listen: () => 'Tap what you hear',
      pairs: () => 'Tap the matching pairs',
    };

    async function viewLesson(v, token) {
      main.innerHTML = `<div class="page"><div class="loading">Loading…</div></div>`;
      const course = await load(v.courseId);
      if (token !== d.token()) return;
      const m = Course.model(course);
      const p = prog(course.id);
      const practice = v.lessonId === '_practice';
      const at = m.lessons.find((x) => x.lesson.id === v.lessonId);
      if (!practice && !at) return d.go('study', { name: 'course', courseId: course.id });
      const attempt = practice ? (p.practice || 0) : ((p.done[v.lessonId] && p.done[v.lessonId].times) || 0);
      const opts = { attempt, seen: new Set(p.seen), hasAudio: hasAudio(course.id) };
      const list = practice ? Course.practiceExercises(course, Object.keys(p.done), opts) : Course.lessonExercises(course, v.lessonId, opts);
      session = {
        course, practice, lesson: at && at.lesson, unit: at && at.unit,
        queue: list.map((ex) => ({ ex, tries: 0 })), done: 0, total: list.length,
        mistakes: 0, firstTry: 0, answered: 0, started: Date.now(), current: null, phase: 'answer', response: null, puppet: null,
      };
      main.innerHTML = `<div class="lesson" role="region" aria-label="Lesson">
        <div class="lesson-top">
          <button class="lesson-x" id="quit" aria-label="Quit the lesson" title="Quit">${Icons.ui('close')}</button>
          <div class="lesson-bar" role="progressbar" aria-valuemin="0" aria-valuemax="100"><span></span></div>
        </div>
        <div class="lesson-stage" id="stage"></div>
        <div class="lesson-foot" id="foot"></div>
      </div>`;
      main.querySelector('#quit').addEventListener('click', quit);
      bindKeys();
      next();
    }

    function bindKeys() {
      if (keyHandler) document.removeEventListener('keydown', keyHandler, true);
      keyHandler = (e) => {
        if (!session || !main.querySelector('.lesson')) { document.removeEventListener('keydown', keyHandler, true); keyHandler = null; return; }
        if (document.querySelector('.modal-back')) return;
        const typing = e.target.closest && e.target.closest('textarea, input');
        if (e.key === 'Enter' && !e.shiftKey) {
          const b = main.querySelector('#foot .btn.primary:not([disabled])');
          if (b) { e.preventDefault(); b.click(); }
          return;
        }
        if (typing || session.phase !== 'answer') return;
        const n = Number(e.key);
        if (n >= 1 && n <= 6) {
          const opt = main.querySelectorAll('.lesson-stage [data-opt]')[n - 1];
          if (opt) { e.preventDefault(); opt.click(); }
        } else if (e.key === 'Backspace') {
          const placed = main.querySelectorAll('.answer-line .tile');
          if (placed.length) { e.preventDefault(); placed[placed.length - 1].click(); }
        }
      };
      document.addEventListener('keydown', keyHandler, true);
    }

    async function quit() {
      const left = session ? session.queue.length : 0;
      if (left && !(await d.dialog({ title: 'Quit this lesson?', body: "<p>You'll lose your progress in this lesson.</p>", ok: 'Quit', cancel: 'Keep going' }))) return;
      const id = session && session.course.id;
      session = null;
      d.go('study', { name: 'course', courseId: id });
    }

    function setBar() {
      const bar = main.querySelector('.lesson-bar');
      if (!bar || !session) return;
      const pc = Math.round((session.done / Math.max(1, session.done + session.queue.length)) * 100);
      bar.querySelector('span').style.width = pc + '%';
      bar.setAttribute('aria-valuenow', pc);
    }

    // A speech bubble from the buddy, with a speaker button if there's audio.
    function say(text, lang, audio) {
      const speaker = audio && hasAudio(session.course.id)(audio)
        ? `<button class="say-play" data-audio="${audio}" aria-label="Listen">${Icons.ui('speaker')}</button>` : '';
      return `<div class="say"><div class="say-buddy" aria-hidden="true"></div><div class="say-bubble" lang="${lang}">${speaker}<span>${esc(text)}</span></div></div>`;
    }

    function next() {
      if (!session) return;
      setBar();
      if (!session.queue.length) return finish();
      const item = session.queue[0];
      session.current = item;
      session.phase = 'answer';
      session.response = null;
      const ex = item.ex;
      const stage = main.querySelector('#stage');
      let body = '';
      switch (ex.type) {
        case 'meaning':
          body = `${ex.isNew ? '<div class="new-badge">New word</div>' : ''}${say(ex.af, 'af', ex.audio)}` +
            `<div class="opt-grid">${ex.options.map((o, i) => `<button class="lopt" data-opt="${i}"><kbd>${i + 1}</kbd>${esc(o)}</button>`).join('')}</div>`;
          break;
        case 'pick':
          body = `${say(ex.prompt, 'af', ex.audio)}<div class="opt-list">${ex.options.map((o, i) => `<button class="lopt" data-opt="${i}"><kbd>${i + 1}</kbd>${esc(o)}</button>`).join('')}</div>`;
          break;
        case 'blank':
          body = `<p class="blank-sentence" lang="af">${esc(ex.sentence).replace('___', '<span class="blank-gap">&nbsp;</span>')}</p><p class="muted blank-en">${esc(ex.en)}</p>` +
            `<div class="opt-row">${ex.options.map((o, i) => `<button class="lopt" data-opt="${i}" lang="af"><kbd>${i + 1}</kbd>${esc(o)}</button>`).join('')}</div>`;
          break;
        case 'build': case 'listen':
          body = (ex.type === 'listen'
            ? `<div class="listen-row"><button class="listen-big" data-audio="${ex.audio}" aria-label="Play the sentence">${Icons.ui('speaker')}</button><div class="say-buddy listen-buddy" aria-hidden="true"></div></div>`
            : say(ex.prompt, ex.dir === 'af-en' ? 'af' : 'en', ex.audio)) +
            `<div class="answer-line" aria-label="Your answer"></div>` +
            `<div class="bank" lang="${ex.dir === 'af-en' ? 'en' : 'af'}">${ex.tiles.map((t, i) => `<button class="tile" data-tile="${i}">${esc(t)}</button>`).join('')}</div>`;
          break;
        case 'type':
          body = say(ex.prompt, ex.dir === 'af-en' ? 'af' : 'en', ex.audio) +
            `<textarea class="lesson-input" rows="2" spellcheck="false" autocomplete="off" lang="${ex.dir === 'af-en' ? 'en' : 'af'}" placeholder="${ex.dir === 'af-en' ? 'Type in English' : 'Type in Afrikaans'}"></textarea>` +
            (ex.dir === 'en-af' ? `<div class="accent-keys">${['ê', 'ë', 'ô', 'é', 'û', "'n"].map((k) => `<button class="tile small" data-key="${k}">${k}</button>`).join('')}</div>` : '');
          break;
        case 'pairs':
          body = `<div class="pairs"><div class="pairs-col" lang="af">${ex.left.map((i) => `<button class="lopt pair" data-side="af" data-p="${i}">${esc(ex.pairs[i].af)}</button>`).join('')}</div>` +
            `<div class="pairs-col" lang="en">${ex.right.map((i) => `<button class="lopt pair" data-side="en" data-p="${i}">${esc(ex.pairs[i].en)}</button>`).join('')}</div></div>`;
          break;
        default: break;
      }
      stage.innerHTML = `<h2 class="ex-title">${esc(TITLES[ex.type](ex))}</h2><div class="ex-body ex-${ex.type}">${body}</div>`;
      d.Motion.reveal(stage.firstElementChild);
      d.Motion.reveal(stage.lastElementChild, 60);
      // The buddy says the sentence.
      const spot = stage.querySelector('.say-buddy');
      if (session.puppet) { session.puppet.destroy(); session.puppet = null; }
      if (spot && d.buddyLook()) { session.puppet = Buddy.puppet(spot, { look: d.buddyLook() }); session.puppet.talk(700); }
      else if (spot) spot.remove();
      stage.querySelectorAll('[data-audio]').forEach((b) => b.addEventListener('click', () => play(b.dataset.audio)));
      if (ex.audio && (ex.type === 'listen' || ex.type === 'meaning')) setTimeout(() => play(ex.audio), 350);
      wireExercise(ex, stage);
      foot('answer');
    }

    let audioEl = null;
    async function play(id) {
      if (!id || !hasAudio(session.course.id)(id)) return;
      try {
        const data = await api.courseAudio(session.course.id, id);
        if (audioEl) { audioEl.pause(); URL.revokeObjectURL(audioEl.src); }
        audioEl = new Audio(URL.createObjectURL(new Blob([data], { type: 'audio/ogg' })));
        if (session && session.puppet) session.puppet.talk(1200);
        await audioEl.play();
      } catch (e) { /* no audio, no problem */ }
    }

    function wireExercise(ex, stage) {
      const ready = (ok) => { const b = main.querySelector('#check'); if (b) b.disabled = !ok; };
      if (ex.type === 'meaning' || ex.type === 'pick' || ex.type === 'blank') {
        stage.querySelectorAll('[data-opt]').forEach((b) => b.addEventListener('click', () => {
          if (session.phase !== 'answer') return;
          stage.querySelectorAll('[data-opt]').forEach((x) => x.classList.toggle('selected', x === b));
          session.response = Number(b.dataset.opt);
          d.Sound.play('select');
          d.Motion.press(b);
          ready(true);
        }));
      } else if (ex.type === 'build' || ex.type === 'listen') {
        const line = stage.querySelector('.answer-line');
        const bank = stage.querySelector('.bank');
        const sync = () => {
          session.response = [...line.querySelectorAll('.tile')].map((t) => t.textContent).join(' ');
          ready(line.children.length > 0);
        };
        bank.querySelectorAll('.tile').forEach((t) => t.addEventListener('click', () => {
          if (session.phase !== 'answer' || t.classList.contains('used')) return;
          const placed = document.createElement('button');
          placed.className = 'tile';
          placed.textContent = t.textContent;
          placed.dataset.from = t.dataset.tile;
          placed.addEventListener('click', () => {
            if (session.phase !== 'answer') return;
            placed.remove();
            t.classList.remove('used');
            d.Motion.reveal(t);
            d.Sound.play('tap');
            sync();
          });
          t.classList.add('used');
          line.appendChild(placed);
          d.Motion.press(placed);
          d.Sound.play('select');
          sync();
        }));
      } else if (ex.type === 'type') {
        const input = stage.querySelector('.lesson-input');
        input.addEventListener('input', () => { session.response = input.value; ready(input.value.trim().length > 0); });
        stage.querySelectorAll('[data-key]').forEach((k) => k.addEventListener('click', () => {
          const s = input.selectionStart, e2 = input.selectionEnd;
          input.setRangeText(k.dataset.key, s, e2, 'end');
          input.focus();
          input.dispatchEvent(new Event('input'));
        }));
        setTimeout(() => input.focus({ preventScroll: true }), 250);
      } else if (ex.type === 'pairs') {
        let picked = null, matched = 0;
        stage.querySelectorAll('.pair').forEach((b) => b.addEventListener('click', () => {
          if (b.classList.contains('matched')) return;
          if (!picked || picked.dataset.side === b.dataset.side) {
            if (picked) picked.classList.remove('selected');
            picked = b;
            b.classList.add('selected');
            d.Sound.play('select');
            return;
          }
          const a = picked;
          picked = null;
          a.classList.remove('selected');
          if (a.dataset.p === b.dataset.p) {
            [a, b].forEach((x) => { x.classList.add('matched'); x.disabled = true; d.Motion.press(x); });
            d.Sound.play('select');
            matched++;
            if (matched === ex.pairs.length) {
              session.response = 'done';
              setTimeout(() => { if (session && session.current && session.current.ex === ex) answer({ correct: true }); }, 350);
            }
          } else {
            session.pairMisses = (session.pairMisses || 0) + 1;
            [a, b].forEach((x) => { x.classList.add('miss'); setTimeout(() => x.classList.remove('miss'), 600); d.Motion.verdict(x); });
            d.Sound.play('wrong');
          }
        }));
      }
    }

    // The bottom bar: Check, then the verdict with Continue.
    function foot(phase, result, ex) {
      const f = main.querySelector('#foot');
      if (!f) return;
      f.className = 'lesson-foot';
      if (phase === 'answer') {
        const pairs = session.current && session.current.ex.type === 'pairs';
        f.innerHTML = pairs ? '<span class="muted">Tap a word, then its match.</span>'
          : `<button class="btn" id="skip">Skip</button><button class="btn primary big" id="check" disabled>Check</button>`;
        const skip = f.querySelector('#skip');
        if (skip) skip.addEventListener('click', () => answer({ correct: false, skipped: true }));
        const check = f.querySelector('#check');
        if (check) check.addEventListener('click', () => {
          if (session.phase !== 'answer') return;
          answer(Course.checkExercise(session.course, session.current.ex, session.response));
        });
        return;
      }
      const right = result.correct;
      f.classList.add(right ? 'is-right' : 'is-wrong');
      const solution = solutionText(ex, result);
      const praise = ['Nice!', 'Great!', 'Correct!', 'Lekker!', 'Spot on!', 'Excellent!'];
      let msg = right ? praise[Math.floor(Math.random() * praise.length)] : result.skipped ? 'Skipped. Here\'s the answer:' : 'Not quite. The answer is:';
      let extra = '';
      if (right && result.note === 'typo') extra = `<div class="fb-note">You have a typo: <strong>${esc(result.match)}</strong></div>`;
      else if (right && result.note === 'accent') extra = `<div class="fb-note">Watch the accents: <strong>${esc(result.match)}</strong></div>`;
      else if (right && result.match && solution && Course.norm(result.match, 'x') !== Course.norm(solution, 'x')) extra = `<div class="fb-note">Also correct: <strong>${esc(solution)}</strong></div>`;
      else if (!right) extra = `<div class="fb-solution">${esc(solution)}</div>`;
      f.innerHTML = `<div class="fb"><div class="fb-icon">${Icons.ui(right ? 'check' : 'close')}</div><div><div class="fb-msg">${msg}</div>${extra}</div></div>` +
        `<button class="btn primary big" id="continue">Continue</button>`;
      d.Motion.reveal(f.firstElementChild);
      f.querySelector('#continue').addEventListener('click', () => { if (session) next(); });
      setTimeout(() => { const c = f.querySelector('#continue'); if (c) c.focus({ preventScroll: true }); }, 50);
    }

    function solutionText(ex, result) {
      switch (ex.type) {
        case 'meaning': case 'pick': return ex.options[ex.answer];
        case 'blank': return ex.full;
        case 'build': case 'type': case 'listen': return ex.answer;
        default: return '';
      }
    }

    function answer(result) {
      const item = session.current;
      const ex = item.ex;
      session.phase = 'feedback';
      session.answered++;
      main.querySelectorAll('.lesson-stage button, .lesson-stage textarea').forEach((b) => { if (!b.classList.contains('say-play') && !b.classList.contains('listen-big')) b.disabled = true; });
      // Show which option was right/wrong.
      if (ex.type === 'meaning' || ex.type === 'pick' || ex.type === 'blank') {
        main.querySelectorAll('.lesson-stage [data-opt]').forEach((b) => {
          const i = Number(b.dataset.opt);
          if (i === ex.answer) b.classList.add('right');
          else if (i === session.response) b.classList.add('wrong');
        });
      }
      session.queue.shift();
      if (result.correct) {
        session.done++;
        if (item.tries === 0) session.firstTry++;
        d.Sound.play('correct');
        if (session.puppet) session.puppet.react('correct');
      } else {
        session.mistakes++;
        item.tries++;
        // Mistakes come back at the end of the lesson (up to twice each).
        if (item.tries <= 2) session.queue.push(item);
        else session.done++;
        d.Sound.play('wrong');
        if (session.puppet) session.puppet.react('wrong');
      }
      if (ex.type !== 'pairs' && ex.answerAudio && result.correct) setTimeout(() => play(ex.answerAudio), 250);
      setBar();
      if (ex.type === 'pairs') { session.done += 0; next(); return; }
      foot('feedback', result, ex);
    }

    function finish() {
      const s = session;
      const course = s.course;
      const p = prog(course.id);
      const seconds = Math.round((Date.now() - s.started) / 1000);
      const graded = s.answered;
      const accuracy = graded ? Math.max(0, Math.round(((graded - s.mistakes - (s.pairMisses || 0) * 0.5) / graded) * 100)) : 100;
      const xp = Course.xpFor(s.practice ? 'practice' : 'lesson', s.mistakes);
      p.xp = (p.xp || 0) + xp;
      p.days[DAY()] = (p.days[DAY()] || 0) + xp;
      let unitDone = false;
      if (s.practice) p.practice = (p.practice || 0) + 1;
      else {
        const r = p.done[s.lesson.id] || { times: 0, best: 0, xp: 0 };
        r.times++;
        r.best = Math.max(r.best || 0, accuracy);
        r.xp = (r.xp || 0) + xp;
        p.done[s.lesson.id] = r;
        const seen = new Set(p.seen);
        s.lesson.words.forEach((w) => seen.add(Course.norm(w.af, 'af')));
        p.seen = [...seen];
        unitDone = s.unit.lessons.every((l) => p.done[l.id]);
      }
      d.save();
      d.recordStreak('question', Math.max(1, s.firstTry));
      session = null;
      if (audioEl) { audioEl.pause(); audioEl = null; }
      const mm = Math.floor(seconds / 60), ss = String(seconds % 60).padStart(2, '0');
      const headline = s.practice ? 'Practice complete!' : unitDone && r0(p, s) ? `Unit complete: ${esc(s.unit.title)}!` : 'Lesson complete!';
      main.querySelector('.lesson').innerHTML = `<div class="lesson-done">
        <div class="done-buddy" aria-hidden="true"></div>
        <h1>${headline}</h1>
        <p class="lead">${accuracy === 100 ? 'Perfect, not a single mistake!' : accuracy >= 80 ? 'Great work!' : 'Nice effort. Practice makes perfect.'}</p>
        <div class="done-stats">
          <div class="ds xp"><span>XP</span><strong>+${xp}</strong></div>
          <div class="ds acc"><span>Accuracy</span><strong>${accuracy}%</strong></div>
          <div class="ds time"><span>Time</span><strong>${mm}:${ss}</strong></div>
        </div>
        <button class="btn primary big" id="done-continue">Continue</button>
      </div>`;
      const box = main.querySelector('.lesson-done');
      d.Motion.pop(box);
      box.querySelectorAll('.ds').forEach((x, i) => d.Motion.reveal(x, 250 + i * 120));
      d.Sound.play(accuracy >= 50 ? 'complete' : 'finishSoft');
      const spot = box.querySelector('.done-buddy');
      if (d.buddyLook()) { const pup = Buddy.puppet(spot, { look: d.buddyLook() }); setTimeout(() => pup.react('celebrate'), 200); }
      else spot.remove();
      box.querySelector('#done-continue').addEventListener('click', () => d.go('study', { name: 'course', courseId: course.id }));
      setTimeout(() => { const c = box.querySelector('#done-continue'); if (c) c.focus({ preventScroll: true }); }, 400);
    }
    // (unit-complete headline only on the first time through)
    function r0(p, s) { return p.done[s.lesson.id] && p.done[s.lesson.id].times === 1; }

    // Leaving the lesson page any other way ends the session.
    function leave() { session = null; if (audioEl) { audioEl.pause(); audioEl = null; } }

    return { load, homeCard, viewCourse, viewLesson, leave, inLesson: () => !!session };
  }

  window.CourseUI = { create };
})();
