//! What the study buddy says and offers, decided from where you are and how you're doing
//! (1.1's app.js "study buddy"). Never shown while an exam is written or marked, and
//! hints are never offered in exams.

use super::app::{pct, App, Mode, Tab, View};
use super::buddy::{Action, Go, Priority};
use super::buddy_art::Mood;
use crate::marking;
use crate::streak;

fn pick<'a>(a: &[&'a str]) -> &'a str {
    a[fastrand::usize(0..a.len())]
}

const PRAISE: [&str; 7] = ["Yes! That's it.", "Spot on!", "Nailed it!", "Lekker, that's right!", "Correct. Nice thinking!", "Sharp!", "That's the one!"];
const ENCOURAGE_NOTES: [&str; 3] = ["Close one! Want to see where it's explained?", "No stress. Let's look at it together in the notes.", "Not quite, but that's how we learn. I can show you the part of the notes that explains it."];
const ENCOURAGE: [&str; 3] = ["Not quite, but that's how we learn. Have a look at the explanation.", "Mistakes help your brain grow. Read through the explanation, then try the next one.", "No stress. The explanation shows how it's done."];
const PARTIAL: [&str; 2] = ["Nearly there! Check the ones marked with a cross.", "Some right, some not. Look at the corrections."];
pub const HINT_LINES: [&str; 2] = ["Here's a clue! Each time you ask, the hints get a bit stronger.", "Thinking cap on! Ask again if you need a bigger clue."];

pub struct Advice {
    pub text: String,
    pub label: Option<String>,
    pub go: Option<Go>,
}

impl Advice {
    fn text(t: impl Into<String>) -> Advice {
        Advice { text: t.into(), label: None, go: None }
    }
    fn with(t: impl Into<String>, label: impl Into<String>, go: Go) -> Advice {
        Advice { text: t.into(), label: Some(label.into()), go: Some(go) }
    }
    fn actions(&self) -> Vec<Action> {
        match (&self.label, &self.go) {
            (Some(l), Some(g)) => vec![Action::new(l.clone(), g.clone())],
            _ => vec![],
        }
    }
}

impl App {
    fn now_say(&mut self, text: &str, pr: Priority, mood: Option<Mood>, actions: Vec<Action>) -> bool {
        let now = self.now;
        self.buddy.say(text, pr, mood, actions, now, &mut self.sound)
    }

    pub fn tell(&mut self, key: &str, text: &str, pr: Priority, mood: Option<Mood>, actions: Vec<Action>) -> bool {
        let now = self.now;
        self.buddy.tell(key, text, pr, mood, actions, now, &mut self.sound)
    }

    fn open_topic(&self, s: &str, t: &str, questions: bool) -> Go {
        Go::OpenTopic { subject: s.into(), topic: t.into(), questions }
    }

    pub fn study_recommendation(&self) -> Option<Advice> {
        let g = self.grade_data();
        if let Some(last) = &self.p.settings.last_topic {
            if last.grade == self.grade() {
                if let Some(s) = self.subject(&last.subject_id) {
                    if let Some(t) = s.topics.iter().find(|x| x.id == last.topic_id) {
                        let done = self.topic_prog(&self.tkey(&s.id, &t.id)).is_some_and(|tp| tp.best.is_some());
                        if !done {
                            return Some(Advice::with(format!("Want to pick up **{}** where you left off?", t.title), "Carry on", self.open_topic(&s.id, &t.id, last.mode.as_deref() == Some("questions"))));
                        }
                    }
                }
            }
        }
        let mut weakest: Option<(&str, &str, &str, i64)> = None;
        for s in &g.subjects {
            for t in &s.topics {
                if let Some(tp) = self.topic_prog(&self.tkey(&s.id, &t.id)) {
                    if let (Some(b), Some(bm)) = (tp.best, tp.best_max) {
                        if bm.0 > 0.0 {
                            let p = pct(b.0, bm.0);
                            if p < 60 && weakest.is_none_or(|w| p < w.3) {
                                weakest = Some((&s.id, &t.id, &t.title, p));
                            }
                        }
                    }
                }
            }
        }
        if let Some((s, t, title, p)) = weakest {
            return Some(Advice::with(format!("Your best on **{title}** is {p}%. Want to give it another go?"), format!("Practise {title}"), self.open_topic(s, t, true)));
        }
        let mut subs: Vec<_> = g.subjects.iter().collect();
        subs.sort_by_key(|s| std::cmp::Reverse(self.subject_stats(s).practised));
        let next = subs.iter().find_map(|s| s.topics.iter().find(|x| self.topic_prog(&self.tkey(&s.id, &x.id)).is_none()).map(|t| (*s, t)));
        let st = streak::status(&self.p, streak::today());
        if st.current > 0 && !st.today_met {
            let left = (streak::GOAL_QUESTIONS - st.questions_today).max(0.0) as i64;
            let text = format!("{left} more question{} today keeps your **{}-day streak** going.", if left == 1 { "" } else { "s" }, st.current);
            return Some(match next {
                Some((s, t)) => Advice::with(text, format!("Start {}", t.title), self.open_topic(&s.id, &t.id, false)),
                None => Advice::text(text),
            });
        }
        next.map(|(s, t)| Advice::with(format!("Why not start **{}** in {}?", t.title, s.name), format!("Start {}", t.title), self.open_topic(&s.id, &t.id, false)))
    }

    pub fn subject_advice(&self, sid: &str) -> Option<Advice> {
        let s = self.subject(sid)?;
        if s.topics.is_empty() {
            return None;
        }
        if let Some(todo) = s.topics.iter().find(|t| !self.topic_prog(&self.tkey(&s.id, &t.id)).is_some_and(|tp| tp.best.is_some())) {
            let tp = self.topic_prog(&self.tkey(&s.id, &todo.id));
            let started = tp.is_some_and(|tp| tp.notes_read || !tp.checked.is_empty());
            let text = if started { format!("You've started **{}**. Let's finish it!", todo.title) } else { format!("Next up: **{}**.", todo.title) };
            return Some(Advice::with(text, format!("Open {}", todo.title), self.open_topic(&s.id, &todo.id, false)));
        }
        let mut low: Option<(&str, &str, i64)> = None;
        for t in &s.topics {
            let tp = self.topic_prog(&self.tkey(&s.id, &t.id));
            let p = tp.map_or(0, |tp| pct(tp.best.map_or(0.0, |b| b.0), tp.best_max.map_or(0.0, |b| b.0)));
            if low.is_none_or(|l| p < l.2) {
                low = Some((&t.id, &t.title, p));
            }
        }
        if let Some((tid, title, p)) = low {
            if p < 80 {
                return Some(Advice::with(format!("You've practised every topic here! **{title}** ({p}%) could use another look."), format!("Practise {title}"), self.open_topic(&s.id, tid, true)));
            }
        }
        Some(if !s.exams.is_empty() {
            Advice::with(format!("80% or more on every topic in {}. Impressive! Ready for an exam?", s.name), "See the exams", Go::ExamList(s.id.clone()))
        } else {
            Advice::text(format!("80% or more on every topic in {}. Impressive!", s.name))
        })
    }

    pub fn exam_advice(&self, sid: &str) -> Option<Advice> {
        let s = self.subject(sid)?;
        if s.exams.is_empty() {
            return None;
        }
        let intro = |x: &crate::model::ExamMeta| Go::ExamIntro { subject: s.id.clone(), exam: x.id.clone() };
        if let Some(fresh) = s.exams.iter().find(|x| self.p.exams.get(&self.tkey(&s.id, &x.id)).is_none_or(|h| h.is_empty())) {
            return Some(Advice::with(format!("You haven't written **{}** yet. Fancy a go?", fresh.title), format!("Open {}", fresh.title), intro(fresh)));
        }
        let mut low: Option<(&crate::model::ExamMeta, i64)> = None;
        for x in &s.exams {
            let best = self.p.exams.get(&self.tkey(&s.id, &x.id)).map(|h| h.iter().map(|a| a.percent).max().unwrap_or(i64::MIN)).unwrap_or(i64::MIN);
            if low.is_none_or(|l| best < l.1) {
                low = Some((x, best));
            }
        }
        let (x, best) = low?;
        Some(Advice::with(format!("Your best on **{}** is {best}%. Want to beat it?", x.title), format!("Open {}", x.title), intro(x)))
    }

    pub fn progress_advice(&self) -> Advice {
        let mut stats: Vec<(&crate::model::SubjectMeta, i64)> = self.grade_data().subjects.iter().filter_map(|s| self.subject_stats(s).avg.map(|a| (s, a))).collect();
        stats.sort_by_key(|x| std::cmp::Reverse(x.1));
        if stats.is_empty() {
            return Advice::text("Practise a few topics and I'll tell you how you're doing.");
        }
        let best = stats[0];
        let worst = *stats.last().unwrap();
        if stats.len() == 1 {
            return Advice::text(format!("So far you're averaging **{}%** in {}. Practise another subject and I'll compare them.", best.1, best.0.name));
        }
        Advice::with(
            format!("You're strongest in **{}** ({}%). **{}** ({}%) needs the most work.", best.0.name, best.1, worst.0.name, worst.1),
            format!("Go to {}", worst.0.name),
            Go::OpenSubject(worst.0.id.clone()),
        )
    }

    pub fn streak_advice(&self) -> Advice {
        let st = streak::status(&self.p, streak::today());
        let left = (streak::GOAL_QUESTIONS - st.questions_today).max(0.0) as i64;
        if st.today_met {
            Advice::text(format!("You're on a **{}-day streak** and today's goal is done. See you tomorrow!", st.current))
        } else if st.current > 0 {
            Advice::text(format!("**{}-day streak!** {left} more question{} or one exam today keeps it going.", st.current, if left == 1 { "" } else { "s" }))
        } else {
            Advice::text(format!("No streak yet. Answer {} practice questions today to start one!", streak::GOAL_QUESTIONS))
        }
    }

    pub fn result_advice(&self, subject: &str, exam: &str, attempt: usize) -> Option<Advice> {
        let a = self.p.exams.get(&self.tkey(subject, exam))?.get(attempt)?;
        let (lvl, _) = marking::caps_level(a.percent as f64);
        let mut worst: Option<&crate::model::SectionScore> = None;
        for sec in &a.sections {
            if sec.max.0 > 0.0 && worst.is_none_or(|w| sec.score.0 / sec.max.0 < w.score.0 / w.max.0) {
                worst = Some(sec);
            }
        }
        let head = if a.percent >= 80 {
            format!("**{}%**: outstanding! That's level {lvl}.", a.percent)
        } else if a.percent >= 50 {
            format!("**{}%**: well done!", a.percent)
        } else {
            format!("**{}%**. That was a tough paper.", a.percent)
        };
        let tail = match worst {
            Some(w) if w.score.0 < w.max.0 => format!(" {} cost you the most marks ({}%).", crate::markup::strip_tags(&crate::markup::inline(&w.title)), pct(w.score.0, w.max.0)),
            _ => String::new(),
        };
        Some(Advice::with(head + &tail, "Review my answers", Go::Review { subject: subject.into(), exam: exam.into(), attempt }))
    }

    fn say_advice(&mut self, r: Option<Advice>) {
        match r {
            None => {
                self.now_say("I don't have a suggestion right now. You're doing great!", Priority::User, None, vec![]);
            }
            Some(a) => {
                let actions = a.actions();
                self.now_say(&a.text, Priority::User, None, actions);
            }
        }
    }

    /// After every page change: step out during exams, otherwise maybe offer a tip.
    pub fn buddy_on_page(&mut self) {
        if self.p.settings.buddy.is_none() {
            return;
        }
        let now = self.now;
        self.buddy.close_bubble(now);
        let v = self.view();
        let in_exam = self.tab == Tab::Exams && matches!(v, View::Run { .. } | View::Mark { .. });
        if in_exam {
            if !self.buddy_away_flag() {
                self.set_buddy_away(true);
                if matches!(v, View::Run { .. }) {
                    self.buddy.react("wave", now, &mut self.sound);
                    let line = pick(&["Good luck! No help in exams, so I'll wait outside.", "You've got this! See you when you're done."]);
                    self.now_say(line, Priority::User, Some(Mood::Happy), vec![]);
                    self.hide_buddy_later();
                } else {
                    self.buddy.hide("exam", true);
                }
            }
            return;
        }
        if self.buddy_away_flag() {
            self.set_buddy_away(false);
            self.buddy.hide("exam", false);
        }
        let course = self.tab == Tab::Study && matches!(v, View::Course { .. } | View::Lesson { .. });
        self.buddy.hide("course", course);
        if course {
            return;
        }
        self.schedule_tip();
    }

    pub fn buddy_tip(&mut self) {
        let v = self.view();
        match (self.tab, &v) {
            (Tab::Study, View::Home) => {
                let r = self.study_recommendation();
                use chrono::Timelike;
                let h = chrono::Local::now().hour();
                let hi = if h < 12 { "Good morning" } else if h < 17 { "Good afternoon" } else { "Good evening" };
                let (text, actions) = match &r {
                    Some(a) => (format!("{hi}! {}", a.text), a.actions()),
                    None => (format!("{hi}! Pick a subject and let's get started."), vec![]),
                };
                self.tell("home", &text, Priority::Tip, Some(Mood::Happy), actions);
            }
            (Tab::Study, View::Subject { subject }) => {
                if let Some(a) = self.subject_advice(subject) {
                    let actions = a.actions();
                    self.tell(&format!("subject:{subject}"), &a.text, Priority::Tip, None, actions);
                }
            }
            (Tab::Study, View::Topic { subject, topic, mode }) => {
                let Some(td) = self.topics.get(&self.tkey(subject, topic)).cloned() else { return };
                let key = self.tkey(subject, topic);
                if *mode == Mode::Questions {
                    let sc = self.topic_score(&td.topic, &key);
                    if sc.done == 0 && sc.pending == 0 {
                        self.tell(&format!("qs:{key}"), &format!("{} questions. Stuck on one? Tap **Hint** and I'll help.", sc.total), Priority::Tip, None, vec![]);
                    } else if !sc.complete {
                        self.tell(&format!("qs:{key}"), &format!("Welcome back! {} of {} done. Let's finish them.", sc.done, sc.total), Priority::Tip, None, vec![]);
                    }
                } else {
                    let n = td.sections.len();
                    let listen = if td.sections.iter().any(|s| td.audio.contains(&s.id)) { " Or tap **Listen** and I'll read along with you." } else { "" };
                    let text = if n > 1 { format!("This topic has {n} sections. Read one, then tap **Continue** for the next.{listen}") } else { format!("Read through this, then test yourself with the questions.{listen}") };
                    self.tell(&format!("notes:{key}"), &text, Priority::Tip, None, vec![]);
                }
            }
            (Tab::Exams, View::Home) => {
                self.tell("exams", "Exams here work like the real thing: timed, then marked with the memo. I can't help during an exam, but I'll help you review afterwards.", Priority::Tip, None, vec![]);
            }
            (Tab::Exams, View::List { subject }) => {
                if let Some(a) = self.exam_advice(subject) {
                    let actions = a.actions();
                    self.tell(&format!("examlist:{subject}"), &a.text, Priority::Tip, None, actions);
                }
            }
            (Tab::Exams, View::Intro { exam, .. }) => {
                self.tell(&format!("intro:{exam}"), "Read every question carefully and keep an eye on the timer. You've got this!", Priority::Tip, Some(Mood::Happy), vec![]);
            }
            (Tab::Exams, View::Result { subject, exam, attempt }) => {
                let key = format!("result:{exam}:{attempt}");
                if !self.buddy.told.contains(&key) {
                    if let Some(a) = self.result_advice(subject, exam, *attempt) {
                        let pc = self.p.exams.get(&self.tkey(subject, exam)).and_then(|h| h.get(*attempt)).map_or(0, |a| a.percent);
                        let now = self.now;
                        self.buddy.react(if pc >= 50 { "celebrate" } else { "think" }, now, &mut self.sound);
                        let actions = a.actions();
                        self.tell(&key, &a.text, Priority::User, None, actions);
                    }
                }
            }
            (Tab::Exams, View::Review { .. }) => {
                self.tell("review", "Here's the memo for every question. Look closely at where you lost marks.", Priority::Tip, None, vec![]);
            }
            (Tab::Progress, _) => {
                let a = self.progress_advice();
                let actions = a.actions();
                self.tell("progress", &a.text, Priority::Tip, None, actions);
            }
            (Tab::Settings, _) => {
                self.tell("settings", "This is where you can change how I look, or turn me off if you need some quiet.", Priority::Tip, None, vec![]);
            }
            _ => {}
        }
    }

    /// Tap the buddy: a menu that fits the page.
    pub fn buddy_menu(&mut self) {
        let v = self.view();
        let mut items: Vec<Action> = Vec::new();
        match (self.tab, &v) {
            (Tab::Study, View::Home) => {
                items.push(Action::menu("What should I study?", Go::StudyRecommendation));
                items.push(Action::menu("How's my streak?", Go::StreakAdvice));
            }
            (Tab::Study, View::Subject { subject }) => items.push(Action::menu("Which topic should I do next?", Go::SubjectAdvice(subject.clone()))),
            (Tab::Study, View::Topic { mode: Mode::Questions, .. }) => {
                items.push(Action::menu("Give me a hint", Go::Hint));
                if self.first_wrong.is_some() {
                    items.push(Action::menu("Show me what I got wrong", Go::ShowFirstWrong));
                }
                items.push(Action::menu("Take me to the notes", Go::Notes));
            }
            (Tab::Study, View::Topic { subject, topic, .. }) => {
                let has_audio = self.topics.get(&self.tkey(subject, topic)).is_some_and(|td| td.sections.iter().any(|s| td.audio.contains(&s.id)));
                if has_audio && self.notes.reader.index.is_none() {
                    items.push(Action::menu("Read these notes to me", Go::ReadNotes));
                }
                items.push(Action::menu("Test me on this topic", Go::QuizMe));
            }
            (Tab::Exams, View::Home) | (Tab::Exams, View::List { .. }) | (Tab::Exams, View::Intro { .. }) => {
                let sid = match &v {
                    View::List { subject } | View::Intro { subject, .. } => Some(subject.clone()),
                    _ => self.grade_data().subjects.iter().find(|s| !s.exams.is_empty()).map(|s| s.id.clone()),
                };
                if let Some(sid) = sid {
                    items.push(Action::menu("Which exam should I write?", Go::ExamAdvice(sid)));
                }
            }
            (Tab::Exams, View::Result { .. }) | (Tab::Exams, View::Review { .. }) => items.push(Action::menu("How did I do?", Go::ResultAdvice)),
            (Tab::Progress, _) => {
                items.push(Action::menu("How am I doing?", Go::ProgressAdvice));
                items.push(Action::menu("How's my streak?", Go::StreakAdvice));
            }
            _ => {}
        }
        items.push(Action::menu("Change how I look", Go::Look));
        items.push(Action::menu("Hide me for now", Go::Min));
        let greet = pick(&["How can I help?", "What do you need?", "Hi! What can I do for you?", "Need a hand?"]);
        if self.now_say(greet, Priority::User, Some(Mood::Happy), items) {
            self.buddy.mark_menu();
        }
    }

    pub fn buddy_go(&mut self, go: Go) {
        match go {
            Go::OpenTopic { subject, topic, questions } => self.go(Tab::Study, View::Topic { subject, topic, mode: if questions { Mode::Questions } else { Mode::Notes } }),
            Go::OpenSubject(s) => self.go(Tab::Study, View::Subject { subject: s }),
            Go::ExamList(s) => self.go(Tab::Exams, View::List { subject: s }),
            Go::ExamIntro { subject, exam } => self.go(Tab::Exams, View::Intro { subject, exam }),
            Go::Review { subject, exam, attempt } => self.go(Tab::Exams, View::Review { subject, exam, attempt }),
            Go::ShowFirstWrong => self.show_first_wrong(),
            Go::ShowInNotes(sec) => self.show_in_notes(sec),
            Go::StartQuestions | Go::QuizMe => self.set_topic_mode(Mode::Questions),
            Go::Notes => self.set_topic_mode(Mode::Notes),
            Go::ReadNotes => self.start_reading(0),
            Go::Hint => self.buddy_hint(),
            Go::StudyRecommendation => {
                let r = self.study_recommendation();
                self.say_advice(r)
            }
            Go::StreakAdvice => {
                let r = self.streak_advice();
                self.say_advice(Some(r))
            }
            Go::SubjectAdvice(s) => {
                let r = self.subject_advice(&s);
                self.say_advice(r)
            }
            Go::ExamAdvice(s) => {
                let r = self.exam_advice(&s);
                self.say_advice(r)
            }
            Go::ResultAdvice => {
                if let View::Result { subject, exam, attempt } | View::Review { subject, exam, attempt } = self.view() {
                    let r = self.result_advice(&subject, &exam, attempt);
                    self.say_advice(r)
                }
            }
            Go::ProgressAdvice => {
                let r = self.progress_advice();
                self.say_advice(Some(r))
            }
            Go::Look => self.open_setup(false, ""),
            Go::Min => {}
        }
    }

    /// React to one checked answer.
    pub fn buddy_answer(&mut self, correct: Option<bool>, partial: bool, related: Option<usize>) {
        let now = self.now;
        if correct == Some(true) {
            self.buddy_run += 1;
            self.buddy.react("correct", now, &mut self.sound);
            let n = self.buddy_run;
            if n == 3 || n == 5 || n % 10 == 0 {
                let text = if n == 3 { "Three in a row!".to_string() } else if n == 5 { "Five in a row! You're flying.".to_string() } else { format!("{n} in a row! Unstoppable.") };
                self.now_say(&text, Priority::Reaction, Some(Mood::Happy), vec![]);
            } else if fastrand::f32() < 0.35 {
                self.now_say(pick(&PRAISE), Priority::Reaction, Some(Mood::Happy), vec![]);
            }
        } else if correct == Some(false) {
            self.buddy_run = 0;
            self.buddy.react(if partial { "partial" } else { "wrong" }, now, &mut self.sound);
            let line = if partial { pick(&PARTIAL) } else if related.is_some() { pick(&ENCOURAGE_NOTES) } else { pick(&ENCOURAGE) };
            let actions = related.map(|s| vec![Action::new("Show me in the notes", Go::ShowInNotes(s))]).unwrap_or_default();
            self.now_say(line, Priority::Tip, Some(if partial { Mood::Think } else { Mood::Sad }), actions);
        }
    }

    fn buddy_away_flag(&self) -> bool {
        self.buddy_away_get()
    }
}
