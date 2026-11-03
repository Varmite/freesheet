// Question cards: rendering, answering, checking and self-marking.
// Used by both practice worksheets and exams.
(function (root) {
  'use strict';
  const M = root.Markup;
  const K = root.Marking;
  const esc = M.escapeHtml;
  const LETTERS = 'ABCDEFGH';

  function hash(str) {
    let h = 2166136261;
    for (let i = 0; i < str.length; i++) { h ^= str.charCodeAt(i); h = Math.imul(h, 16777619); }
    return h >>> 0;
  }

  // Deterministic shuffle so a question always shows the same order.
  function seededShuffle(arr, seed) {
    const a = arr.slice();
    let s = hash(seed) || 1;
    for (let i = a.length - 1; i > 0; i--) {
      s = (Math.imul(s, 1103515245) + 12345) >>> 0;
      const j = s % (i + 1);
      [a[i], a[j]] = [a[j], a[i]];
    }
    // Avoid showing the answers in their original order when possible.
    if (a.length > 1 && a.every((v, i) => v === arr[i])) a.push(a.shift());
    return a;
  }

  function formatNumber(n) {
    return String(n).replace('.', ',');
  }

  function correctAnswerHtml(q) {
    switch (q.type) {
      case 'mcq': return `${LETTERS[q.answer]}. ${M.inline(q.options[q.answer])}`;
      case 'truefalse': return q.answer ? 'True' : 'False';
      case 'short': return M.inline(q.answers[0]) +
        (q.answers.length > 1 ? ` <span class="muted">(also accepted: ${q.answers.slice(1).map(M.inline).join('; ')})</span>` : '');
      case 'numeric': return esc(formatNumber(q.answer)) + (q.unit ? ' ' + M.inline(q.unit) : '');
      default: return '';
    }
  }

  // ctx: { number, response, phase: 'answer'|'feedback', selfMark, allowCheck, showMarks,
  //        onResponse(r), onCheck(), onSelfMark(n), lockedText }
  function mount(q, ctx) {
    const el = document.createElement('article');
    el.className = 'q-card';
    el.dataset.qid = q.id;

    function locked() { return ctx.phase !== 'answer'; }

    function inputHtml() {
      const r = ctx.response;
      const dis = locked() ? 'disabled' : '';
      const fb = ctx.phase === 'feedback';
      switch (q.type) {
        case 'mcq':
          return `<div class="opts" role="radiogroup">` + q.options.map((o, i) => {
            let cls = 'opt';
            if (r === i) cls += ' selected';
            if (fb && i === q.answer) cls += ' opt-correct';
            if (fb && r === i && i !== q.answer) cls += ' opt-wrong';
            return `<button type="button" class="${cls}" role="radio" aria-checked="${r === i}" data-i="${i}" ${dis}>` +
              `<span class="opt-letter">${LETTERS[i]}</span><span class="opt-text">${M.inline(o)}</span></button>`;
          }).join('') + `</div>`;
        case 'truefalse':
          return `<div class="opts opts-row" role="radiogroup">` + [true, false].map((v) => {
            let cls = 'opt';
            if (r === v) cls += ' selected';
            if (fb && v === q.answer) cls += ' opt-correct';
            if (fb && r === v && v !== q.answer) cls += ' opt-wrong';
            return `<button type="button" class="${cls}" role="radio" aria-checked="${r === v}" data-v="${v}" ${dis}>` +
              `<span class="opt-text">${v ? 'True' : 'False'}</span></button>`;
          }).join('') + `</div>`;
        case 'short':
          return `<input type="text" class="text-in" spellcheck="false" autocomplete="off" placeholder="Type your answer" value="${esc(r || '')}" ${dis}>`;
        case 'numeric':
          return `<div class="num-row"><input type="text" class="text-in num-in" inputmode="decimal" autocomplete="off" placeholder="Answer" value="${esc(r || '')}" ${dis}>` +
            (q.unit ? `<span class="unit">${M.inline(q.unit)}</span>` : '') + `</div>`;
        case 'match': {
          const rights = seededShuffle(q.pairs.map((p) => p.right), q.id);
          const given = Array.isArray(r) ? r : [];
          return `<table class="match"><tbody>` + q.pairs.map((p, i) => {
            const sel = given[i];
            let mark = '';
            if (fb) {
              mark = sel === p.right
                ? `<span class="tick" aria-label="correct">✓</span>`
                : `<span class="cross" aria-label="incorrect">✗</span><div class="match-correct">${M.inline(p.right)}</div>`;
            }
            return `<tr><td class="match-left">${M.inline(p.left)}</td><td class="match-right">` +
              `<select data-i="${i}" ${dis}><option value="">Choose…</option>` +
              rights.map((rt, k) => `<option value="${k}" ${sel === rt ? 'selected' : ''}>${esc(stripMarkup(rt))}</option>`).join('') +
              `</select>${mark}</td></tr>`;
          }).join('') + `</tbody></table>`;
        }
        case 'open':
          return `<textarea class="text-area" rows="${Math.min(14, 3 + Math.ceil(K.marksOf(q) / 2))}" placeholder="Write your answer" ${dis}>${esc((r && r.text) || '')}</textarea>`;
        default:
          return `<p class="muted">Unsupported question type.</p>`;
      }
    }

    function feedbackHtml() {
      if (ctx.phase !== 'feedback') return '';
      const expl = q.explanation ? `<div class="q-expl">${M.paragraphs(q.explanation)}</div>` : '';
      if (q.type === 'open') {
        const max = K.marksOf(q);
        const sm = ctx.selfMark;
        let picker = '';
        if (ctx.onSelfMark) {
          if (max <= 12) {
            picker = `<div class="self-mark"><span>Your mark:</span>` +
              Array.from({ length: max + 1 }, (_, n) =>
                `<button type="button" class="mark-btn ${sm === n ? 'selected' : ''}" data-mark="${n}">${n}</button>`).join('') +
              `<span class="muted">/ ${max}</span></div>`;
          } else {
            picker = `<div class="self-mark"><span>Your mark:</span><input type="number" class="mark-in" min="0" max="${max}" step="1" value="${sm == null ? '' : sm}"><span class="muted">/ ${max}</span></div>`;
          }
        } else if (sm != null) {
          picker = `<div class="self-mark"><span>Your mark: <strong>${K.formatMarks(sm)} / ${max}</strong></span></div>`;
        }
        return `<div class="q-feedback memo"><div class="n-label">Memo</div>${M.paragraphs(q.memo)}${expl}` +
          `${ctx.onSelfMark ? '<p class="muted small">Compare your answer with the memo and award yourself marks honestly.</p>' : ''}${picker}</div>`;
      }
      const res = K.mark(q, ctx.response);
      if (q.type === 'match') {
        const cls = res.correct ? 'good' : 'bad';
        return `<div class="q-feedback ${cls}"><strong>${res.correct ? 'All correct' : `${res.pairsRight} of ${q.pairs.length} correct`}</strong>` +
          ` <span class="muted">(${K.formatMarks(res.score)} / ${res.max})</span>${expl}</div>`;
      }
      if (res.correct) {
        return `<div class="q-feedback good"><strong>Correct</strong>${expl}</div>`;
      }
      const unanswered = !K.isAnswered(q, ctx.response);
      return `<div class="q-feedback bad"><strong>${unanswered ? 'Not answered' : 'Incorrect'}</strong>` +
        `<div>Correct answer: ${correctAnswerHtml(q)}</div>${expl}</div>`;
    }

    function headHtml() {
      const max = K.marksOf(q);
      let status = '';
      if (ctx.phase === 'feedback') {
        const res = q.type === 'open' ? K.mark(q, ctx.response, ctx.selfMark) : K.mark(q, ctx.response);
        status = res.needsSelfMark
          ? `<span class="q-status">Mark yourself</span>`
          : `<span class="q-status">${K.formatMarks(res.score)} / ${max}</span>`;
      }
      return `<header class="q-head"><span class="q-num">${esc(ctx.number)}</span>` +
        `<span class="q-marks">${max} mark${max === 1 ? '' : 's'}</span>${status}</header>`;
    }

    function actionsHtml() {
      if (ctx.phase !== 'answer' || !ctx.allowCheck) return '';
      const label = q.type === 'open' ? 'Show memo' : 'Check answer';
      return `<div class="q-actions"><button type="button" class="btn primary check-btn">${label}</button></div>`;
    }

    function draw() {
      let cls = 'q-card';
      if (ctx.phase === 'feedback') {
        const res = q.type === 'open' ? K.mark(q, ctx.response, ctx.selfMark) : K.mark(q, ctx.response);
        if (res.needsSelfMark) cls += ' is-pending';
        else cls += res.correct ? ' is-correct' : (res.score > 0 ? ' is-partial' : ' is-wrong');
      }
      el.className = cls;
      el.innerHTML = headHtml() +
        `<div class="q-prompt">${M.paragraphs(q.prompt)}</div>` +
        `<div class="q-input">${inputHtml()}</div>` +
        actionsHtml() + feedbackHtml();
      wire();
    }

    function setResp(r, redraw) {
      ctx.response = r;
      if (ctx.onResponse) ctx.onResponse(r);
      if (redraw) draw();
    }

    function wire() {
      if (!locked()) {
        el.querySelectorAll('.opt').forEach((b) => b.addEventListener('click', () => {
          if (q.type === 'mcq') setResp(Number(b.dataset.i), true);
          else setResp(b.dataset.v === 'true', true);
          const again = el.querySelector(`.opt[data-${q.type === 'mcq' ? 'i' : 'v'}="${q.type === 'mcq' ? b.dataset.i : b.dataset.v}"]`);
          if (again) again.focus();
        }));
        const txt = el.querySelector('.text-in');
        if (txt) {
          txt.addEventListener('input', () => setResp(txt.value, false));
          txt.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && ctx.allowCheck && ctx.onCheck) { e.preventDefault(); ctx.onCheck(); }
          });
        }
        const ta = el.querySelector('.text-area');
        if (ta) ta.addEventListener('input', () => setResp({ text: ta.value }, false));
        el.querySelectorAll('.match select').forEach((s) => s.addEventListener('change', () => {
          const rights = seededShuffle(q.pairs.map((p) => p.right), q.id);
          const cur = Array.isArray(ctx.response) ? ctx.response.slice() : new Array(q.pairs.length).fill(null);
          cur[Number(s.dataset.i)] = s.value === '' ? null : rights[Number(s.value)];
          setResp(cur, false);
        }));
      }
      const check = el.querySelector('.check-btn');
      if (check) check.addEventListener('click', () => ctx.onCheck && ctx.onCheck());
      el.querySelectorAll('.mark-btn').forEach((b) => b.addEventListener('click', () => {
        ctx.selfMark = Number(b.dataset.mark);
        if (ctx.onSelfMark) ctx.onSelfMark(ctx.selfMark);
        draw();
      }));
      const mi = el.querySelector('.mark-in');
      if (mi) mi.addEventListener('change', () => {
        const max = K.marksOf(q);
        const v = Math.max(0, Math.min(max, Math.round(Number(mi.value))));
        ctx.selfMark = Number.isFinite(v) ? v : null;
        if (ctx.onSelfMark) ctx.onSelfMark(ctx.selfMark);
        draw();
      });
    }

    el.refresh = draw;
    draw();
    return el;
  }

  function stripMarkup(s) {
    // For <option> labels, which cannot contain HTML: drop ** and *, keep $maths$ source minus $.
    return String(s).replace(/\*\*|\*/g, '').replace(/\\\$/g, '\u0000').replace(/\$/g, '').replace(/\u0000/g, '$');
  }

  root.Questions = { mount, seededShuffle, correctAnswerHtml };
})(typeof self !== 'undefined' ? self : this);
