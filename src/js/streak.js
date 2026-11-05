'use strict';
// Daily streak: a day counts when the learner checks at least GOAL_QUESTIONS
// practice questions or finishes at least one exam. The streak is the number of
// consecutive qualifying days ending today, or ending yesterday if today's goal
// isn't met yet (the streak is still alive until midnight).
(function () {
  const GOAL_QUESTIONS = 3;
  const GOAL_EXAMS = 1;

  function dayKey(d) {
    const date = d || new Date();
    const two = (n) => String(n).padStart(2, '0');
    return `${date.getFullYear()}-${two(date.getMonth() + 1)}-${two(date.getDate())}`;
  }
  function addDays(d, n) {
    const x = new Date(d.getFullYear(), d.getMonth(), d.getDate());
    x.setDate(x.getDate() + n);
    return x;
  }

  function ensure(progress) {
    const s = progress.streak && typeof progress.streak === 'object' ? progress.streak : {};
    if (!s.days || typeof s.days !== 'object') s.days = {};
    if (typeof s.longest !== 'number') s.longest = 0;
    progress.streak = s;
    return s;
  }

  function met(entry) {
    return !!entry && ((entry.q || 0) >= GOAL_QUESTIONS || (entry.x || 0) >= GOAL_EXAMS);
  }

  function current(s, now) {
    const today = now || new Date();
    let d = met(s.days[dayKey(today)]) ? today : addDays(today, -1);
    let n = 0;
    while (met(s.days[dayKey(d)])) { n++; d = addDays(d, -1); }
    return n;
  }

  // kind: 'question' | 'exam'. Returns { justMet, current }.
  function record(progress, kind, count, now) {
    const s = ensure(progress);
    const k = dayKey(now);
    const e = s.days[k] || (s.days[k] = { q: 0, x: 0 });
    const before = met(e);
    if (kind === 'exam') e.x = (e.x || 0) + (count || 1);
    else e.q = (e.q || 0) + (count || 1);
    const justMet = !before && met(e);
    const cur = current(s, now);
    if (cur > s.longest) s.longest = cur;
    return { justMet, current: cur };
  }

  function status(progress, now) {
    const s = ensure(progress);
    const today = now || new Date();
    const e = s.days[dayKey(today)] || { q: 0, x: 0 };
    const week = [];
    for (let i = 6; i >= 0; i--) {
      const d = addDays(today, -i);
      week.push({ date: d, key: dayKey(d), met: met(s.days[dayKey(d)]), today: i === 0 });
    }
    const cur = current(s, today);
    return {
      current: cur,
      longest: Math.max(s.longest, cur),
      todayMet: met(e),
      questionsToday: e.q || 0,
      examsToday: e.x || 0,
      goalQuestions: GOAL_QUESTIONS,
      week,
    };
  }

  window.Streak = { record, status, dayKey, GOAL_QUESTIONS, GOAL_EXAMS };
})();
