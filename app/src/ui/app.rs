//! The app: state, navigation, the top bar, saving, dialogs and toasts. Pages are in the
//! other ui modules as `impl App` blocks.

use super::buddy::{self, Action, Buddy, Go, Priority};
use super::buddy_art::Mood;
use super::course_ui::{CourseData, LessonSession};
use super::rich::{self, Style};
use super::theme::{self, Theme, W};
use super::widgets::{self, btn, radius, Kind};
use crate::audio;
use crate::content;
use crate::hints::{self, Hint, NotesIndex};
use crate::marking;
use crate::model::*;
use crate::sections::{self, Section};
use crate::store::{Saver, Store};
use crate::streak;
use egui::{pos2, vec2, Align, Align2, Color32, Context, Id, Layout, Order, Rect, Sense, Stroke, Ui};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tab {
    Study,
    Exams,
    Progress,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Notes,
    Questions,
}

#[derive(Clone, Debug, PartialEq)]
pub enum View {
    Home,
    Subject { subject: String },
    Topic { subject: String, topic: String, mode: Mode },
    Course { course: String },
    Lesson { course: String, lesson: String, run: u64 },
    List { subject: String },
    Intro { subject: String, exam: String },
    Run { subject: String, exam: String },
    Mark { subject: String, exam: String },
    Result { subject: String, exam: String, attempt: usize },
    Review { subject: String, exam: String, attempt: usize },
}

impl View {
    pub fn depth(&self) -> u8 {
        match self {
            View::Home => 0,
            View::Subject { .. } | View::List { .. } | View::Course { .. } => 1,
            View::Topic { .. } | View::Intro { .. } | View::Lesson { .. } => 2,
            View::Run { .. } => 3,
            View::Mark { .. } => 4,
            View::Result { .. } => 5,
            View::Review { .. } => 6,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NavKind {
    First,
    Push,
    Pop,
    Tab,
    Swap,
}

/// Something to do when a dialog's main button is pressed.
pub enum DialogAction {
    None,
    CheckAll { key: String },
    SubmitExam,
    RestartExam { key: String },
    StartMarking,
    QuitLesson,
    Import(Box<Progress>),
    Reset,
}

pub enum DialogBody {
    Html(Vec<String>),
    Streak,
}

pub struct Dialog {
    pub title: String,
    pub body: DialogBody,
    pub ok: String,
    pub cancel: Option<String>,
    pub danger: bool,
    pub action: DialogAction,
    pub opened: f64,
    pub closing: Option<(f64, bool)>,
}

pub struct Toast {
    pub text: String,
    pub at: f64,
}

pub struct TopicData {
    pub topic: Topic,
    pub sections: Vec<Section>,
    pub index: NotesIndex,
    pub hints: Vec<Vec<Hint>>,
    pub related: Vec<Option<usize>>,
    pub audio: HashSet<String>,
}

/// A running exam.
pub struct ExamSession {
    pub key: String,
    pub subject: String,
    pub exam_id: String,
    pub grade: i64,
    pub exam: Arc<Exam>,
    pub responses: indexmap::IndexMap<String, serde_json::Value>,
    pub self_marks: indexmap::IndexMap<String, Option<Num>>,
    pub remaining: f64,
    pub elapsed: f64,
    pub timer_on: bool,
    pub marking: bool,
    pub tick_from: f64,
}

/// The notes reader of the open topic.
#[derive(Default)]
pub struct Reader {
    /// The section being read (index), or None.
    pub index: Option<usize>,
    pub paused: bool,
    pub clip: Option<u64>,
    pub pcm: Option<Arc<Vec<f32>>>,
    pub segs: Option<Vec<audio::readalong::Seg>>,
    pub plan: Option<Vec<audio::readalong::Tok>>,
    pub status: String,
    pub user_scrolled: f64,
}

#[derive(Default)]
pub struct NotesState {
    pub key: String,
    pub shown: usize,
    pub reader: Reader,
    pub focus: Option<(usize, f64)>,
    pub scroll_to: Option<usize>,
    pub revealed: Option<(usize, f64)>,
}

pub struct App {
    pub t: Theme,
    pub p: Progress,
    pub store: Store,
    saver: Saver,
    save_due: Option<f64>,
    pub dirty: bool,
    pub tab: Tab,
    pub views: HashMap<Tab, View>,
    pub return_tab: Tab,
    pub topics: HashMap<String, Arc<TopicData>>,
    pub exam_cache: HashMap<String, Arc<Exam>>,
    pub courses: HashMap<String, Arc<CourseData>>,
    pub inputs: HashMap<String, String>,
    pub flashes: HashMap<String, (f64, bool)>,
    pub dialog: Option<Dialog>,
    pub toasts: Vec<Toast>,
    pub streak_toast: Option<(u32, f64)>,
    pub streak_pop: Option<f64>,
    pub session: Option<ExamSession>,
    pub lesson: Option<LessonSession>,
    pub notes: NotesState,
    pub sound: audio::Engine,
    pub buddy: Buddy,
    pub setup: Option<buddy::Setup>,
    nav_kind: NavKind,
    nav_start: f64,
    nav_key: String,
    scroll_reset: bool,
    pub scroll_rect: Option<(Rect, Align)>,
    pub welcome: bool,
    buddy_tip_at: Option<(String, f64)>,
    pub buddy_run: u32,
    buddy_away: bool,
    pub first_wrong: Option<String>,
    pub now: f64,
    started: bool,
    pub filter: String,
    pub file_pending: Option<bool>,
    pub pending_setup: Option<(bool, String, f64)>,
    pub delayed: Vec<(f64, Delayed)>,
    pub viewport_top: f32,
    pub card_rects: HashMap<String, Rect>,
    pub completion_seen: Option<String>,
    pub completion_at: Option<f64>,
    pub course_fig: Option<super::buddy::Figure>,
    /// Testing: pages to open and a screenshot to take (see automation.rs).
    pub auto: Option<super::automation::Script>,
}

/// Things that happen a moment after something else.
pub enum Delayed {
    Sound(&'static str),
    BuddyReact(&'static str),
    BuddySay(String, Priority, Option<Mood>, Vec<Action>),
    Setup(bool, String),
    ScrollToSection(usize),
    HideBuddyForExam,
}

pub fn pct(score: f64, max: f64) -> i64 {
    marking::pct(score, max)
}

impl App {
    pub fn new(ctx: &Context) -> App {
        // Testing flags first: --data-dir must apply before progress is read.
        let auto = super::automation::Script::from_args();
        let (dir, legacy) = crate::platform::data_dirs();
        let store = Store::new(dir, legacy);
        let mut p = store.load();
        let saver = Saver::new(store.clone());
        let catalog = content::catalog();
        let grades: Vec<i64> = catalog.grades.iter().map(|g| g.grade).collect();
        let welcome = !p.settings.grade.is_some_and(|g| grades.contains(&g));
        if welcome {
            p.settings.grade = None;
        }
        let mut views = HashMap::new();
        for t in [Tab::Study, Tab::Exams, Tab::Progress, Tab::Settings] {
            views.insert(t, View::Home);
        }
        let t = Theme { p: theme::LIGHT, dark: false, rem: 16.5, motion: true, narrow: false };
        ctx.set_fonts(theme::fonts());
        let mut app = App {
            t,
            p,
            store,
            saver,
            save_due: None,
            dirty: false,
            tab: Tab::Study,
            views,
            return_tab: Tab::Study,
            topics: HashMap::new(),
            exam_cache: HashMap::new(),
            courses: HashMap::new(),
            inputs: HashMap::new(),
            flashes: HashMap::new(),
            dialog: None,
            toasts: Vec::new(),
            streak_toast: None,
            streak_pop: None,
            session: None,
            lesson: None,
            notes: NotesState::default(),
            sound: audio::Engine::new(),
            buddy: Buddy::new(),
            setup: None,
            nav_kind: NavKind::First,
            nav_start: 0.0,
            nav_key: String::new(),
            scroll_reset: false,
            scroll_rect: None,
            welcome,
            buddy_tip_at: None,
            buddy_run: 0,
            buddy_away: false,
            first_wrong: None,
            now: 0.0,
            started: false,
            filter: String::new(),
            file_pending: None,
            pending_setup: None,
            delayed: Vec::new(),
            viewport_top: 0.0,
            card_rects: HashMap::new(),
            completion_seen: None,
            completion_at: None,
            course_fig: None,
            auto,
        };
        app.apply_buddy();
        app.sound.enabled = app.p.settings.sounds != Some(false);
        app
    }

    // ---------------------------------------------------------------- persistence

    pub fn save(&mut self) {
        self.dirty = true;
        self.save_due = Some(self.now + 0.4);
    }

    pub fn flush(&mut self) {
        self.save_due = None;
        self.dirty = false;
        self.saver.save(&self.p);
    }

    /// Everything that must be kept before the app closes or goes to the background.
    pub fn on_leave(&mut self) {
        if self.session.is_some() {
            self.write_draft();
        }
        self.flush();
        self.saver.flush();
    }

    pub fn grade(&self) -> i64 {
        self.p.settings.grade.unwrap_or(0)
    }

    pub fn grade_data(&self) -> &'static GradeData {
        static EMPTY: std::sync::OnceLock<GradeData> = std::sync::OnceLock::new();
        let g = self.grade();
        content::catalog().grades.iter().find(|x| x.grade == g).unwrap_or_else(|| EMPTY.get_or_init(GradeData::default))
    }

    pub fn subject(&self, id: &str) -> Option<&'static SubjectMeta> {
        self.grade_data().subjects.iter().find(|s| s.id == id)
    }

    pub fn tkey(&self, subject: &str, topic: &str) -> String {
        format!("{}/{subject}/{topic}", self.grade())
    }

    pub fn topic_prog(&self, key: &str) -> Option<&TopicProg> {
        self.p.topics.get(key)
    }

    pub fn topic_prog_mut(&mut self, key: &str) -> &mut TopicProg {
        self.p.topics.entry(key.to_string()).or_default()
    }

    pub fn load_topic(&mut self, subject: &str, topic: &str) -> Option<Arc<TopicData>> {
        let k = self.tkey(subject, topic);
        if let Some(t) = self.topics.get(&k) {
            return Some(t.clone());
        }
        let mut tp = match content::topic(self.grade(), subject, topic) {
            Ok(t) => t,
            Err(e) => {
                log::error!("topic {k}: {e}");
                return None;
            }
        };
        // Short typed answers forgive spelling slips, except in language subjects.
        if !marking::is_language_subject(subject) && tp.questions.iter().any(|q| q.kind == "short") {
            let words = content::words();
            for q in tp.questions.iter_mut().filter(|q| q.kind == "short") {
                q.typo_words = Some(words.clone());
            }
        }
        let index = NotesIndex::new(&tp);
        let hints: Vec<Vec<Hint>> = tp.questions.iter().map(|q| hints::build(q, &index)).collect();
        let related = tp.questions.iter().map(|q| hints::related_section(q, &index)).collect();
        let sections = sections::split(&tp.notes);
        let audio: HashSet<String> = crate::platform::list_audio(&format!("grade-{}/{subject}/{topic}", self.grade())).into_iter().collect();
        let d = Arc::new(TopicData { topic: tp, sections, index, hints, related, audio });
        self.topics.insert(k, d.clone());
        Some(d)
    }

    pub fn load_exam(&mut self, subject: &str, exam: &str) -> Option<Arc<Exam>> {
        let k = self.tkey(subject, exam);
        if let Some(x) = self.exam_cache.get(&k) {
            return Some(x.clone());
        }
        let mut x = content::exam(self.grade(), subject, exam).ok()?;
        if !marking::is_language_subject(subject) {
            let words = content::words();
            for sec in x.sections.iter_mut() {
                for q in sec.questions.iter_mut().filter(|q| q.kind == "short") {
                    q.typo_words = Some(words.clone());
                }
            }
        }
        let x = Arc::new(x);
        self.exam_cache.insert(k, x.clone());
        Some(x)
    }

    // ---------------------------------------------------------------- navigation

    pub fn view(&self) -> View {
        self.views.get(&self.tab).cloned().unwrap_or(View::Home)
    }

    fn nav_key_of(&self, tab: Tab, v: &View) -> String {
        format!("{tab:?}|{}|{v:?}", self.grade())
    }

    pub fn go(&mut self, tab: Tab, view: View) {
        let from_key = self.nav_key.clone();
        let from_tab = self.tab;
        let from_depth = self.view().depth();
        let to_depth = view.depth();
        self.tab = tab;
        self.views.insert(tab, view.clone());
        let key = self.nav_key_of(tab, &view);
        if key == from_key {
            return;
        }
        let kind = if from_key.is_empty() {
            NavKind::First
        } else if tab == Tab::Settings && from_tab != Tab::Settings {
            NavKind::Push
        } else if from_tab == Tab::Settings && tab != Tab::Settings {
            NavKind::Pop
        } else if from_tab != tab {
            NavKind::Tab
        } else if to_depth > from_depth {
            NavKind::Push
        } else if to_depth < from_depth {
            NavKind::Pop
        } else {
            NavKind::Swap
        };
        self.nav_kind = kind;
        self.nav_start = self.now;
        self.nav_key = key;
        self.scroll_reset = kind != NavKind::Pop;
        self.on_page_change(kind);
    }

    pub fn set_tab(&mut self, tab: Tab) {
        let v = self.views.get(&tab).cloned().unwrap_or(View::Home);
        self.go(tab, v);
    }

    fn on_page_change(&mut self, kind: NavKind) {
        self.stop_reader();
        let sound = match kind {
            NavKind::Push => Some("push"),
            NavKind::Pop => Some("pop"),
            NavKind::Tab => Some("tab"),
            NavKind::Swap => Some("swap"),
            NavKind::First => None,
        };
        if let Some(s) = sound {
            self.sound.play(s);
        }
        // Leaving a lesson ends it.
        if !matches!(self.view(), View::Lesson { .. }) {
            self.lesson = None;
        }
        self.first_wrong = None;
        self.buddy_on_page();
    }

    pub fn open_settings(&mut self) {
        if self.welcome {
            return;
        }
        if self.tab == Tab::Settings {
            return self.leave_settings();
        }
        self.return_tab = self.tab;
        self.set_tab(Tab::Settings);
    }

    pub fn leave_settings(&mut self) {
        let t = self.return_tab;
        self.set_tab(if t == Tab::Settings { Tab::Study } else { t });
    }

    /// The Android back button: one step back, like the on-screen links.
    pub fn back(&mut self) {
        if self.setup.is_some() {
            if let Some(s) = &self.setup {
                if s.first {
                    let look = s.look.clone();
                    self.finish_setup(Some(look));
                } else {
                    self.finish_setup(None);
                }
            }
            return;
        }
        if self.dialog.is_some() {
            self.close_dialog(false);
            return;
        }
        if self.buddy.has_bubble() {
            self.buddy.close_bubble(self.now);
            return;
        }
        if self.tab == Tab::Settings {
            return self.leave_settings();
        }
        let v = self.view();
        match v {
            View::Lesson { .. } => self.quit_lesson(),
            View::Subject { .. } | View::Course { .. } => self.go(Tab::Study, View::Home),
            View::Topic { subject, .. } => self.go(Tab::Study, View::Subject { subject }),
            View::List { .. } => self.go(Tab::Exams, View::Home),
            View::Intro { subject, .. } => self.go(Tab::Exams, View::List { subject }),
            View::Run { .. } | View::Mark { .. } => {
                // Leaving mid-exam keeps it as an unfinished attempt.
                if let Some(s) = &self.session {
                    let subject = s.subject.clone();
                    let exam = s.exam_id.clone();
                    self.write_draft();
                    self.flush();
                    self.go(Tab::Exams, View::Intro { subject, exam });
                }
            }
            View::Result { subject, .. } => self.go(Tab::Exams, View::List { subject }),
            View::Review { subject, exam, attempt } => self.go(Tab::Exams, View::Result { subject, exam, attempt }),
            View::Home => {
                if self.tab != Tab::Study && !self.welcome {
                    self.set_tab(Tab::Study);
                } else {
                    self.on_leave();
                    crate::platform::minimise();
                }
            }
        }
    }

    pub fn buddy_away_get(&self) -> bool {
        self.buddy_away
    }
    pub fn set_buddy_away(&mut self, v: bool) {
        self.buddy_away = v;
    }
    pub fn hide_buddy_later(&mut self) {
        self.later(2.3, Delayed::HideBuddyForExam);
    }
    pub fn schedule_tip(&mut self) {
        self.buddy_tip_at = Some((self.nav_key.clone(), self.now + 0.95));
    }

    // ---------------------------------------------------------------- feedback

    pub fn toast(&mut self, text: impl Into<String>) {
        self.sound.play("toast");
        self.toasts.push(Toast { text: text.into(), at: self.now });
    }

    pub fn dialog(&mut self, title: &str, body: Vec<String>, ok: &str, cancel: Option<&str>, danger: bool, action: DialogAction) {
        self.sound.play("open");
        self.dialog = Some(Dialog { title: title.into(), body: DialogBody::Html(body), ok: ok.into(), cancel: cancel.map(Into::into), danger, action, opened: self.now, closing: None });
    }

    pub fn close_dialog(&mut self, ok: bool) {
        if let Some(d) = &mut self.dialog {
            if d.closing.is_none() {
                d.closing = Some((self.now, ok));
                self.sound.play("close");
            }
        }
    }

    pub fn later(&mut self, secs: f64, d: Delayed) {
        self.delayed.push((self.now + secs, d));
    }

    // ---------------------------------------------------------------- streak

    pub fn record_streak(&mut self, kind: &str, count: f64) {
        if count <= 0.0 {
            return;
        }
        let r = streak::record(&mut self.p, kind, count, streak::today());
        if r.just_met {
            self.streak_pop = Some(self.now);
            let n = r.current;
            self.streak_toast = Some((n, self.now));
            self.later(0.45, Delayed::Sound("streak"));
            self.later(1.3, Delayed::BuddyReact("celebrate"));
            let text = if n == 1 { "You started a **streak**! Come back tomorrow to keep it going.".to_string() } else { format!("**{n}-day streak!** Today's goal is done. Keep it going!") };
            self.later(1.3, Delayed::BuddySay(text, Priority::User, Some(Mood::Happy), vec![]));
        }
    }

    pub fn show_streak(&mut self) {
        self.sound.play("open");
        self.dialog = Some(Dialog { title: "Daily streak".into(), body: DialogBody::Streak, ok: "Close".into(), cancel: None, danger: false, action: DialogAction::None, opened: self.now, closing: None });
    }

    // ---------------------------------------------------------------- settings applied

    pub fn apply_buddy(&mut self) {
        let b = self.p.settings.buddy.clone();
        match &b {
            Some(b) => self.buddy.configure(Some(b), b.on, b.quiet, b.min),
            None => self.buddy.configure(None, false, false, false),
        }
    }

    fn theme_for(&self, ctx: &Context) -> Theme {
        let dark = match ctx.system_theme() {
            Some(egui::Theme::Dark) => true,
            Some(egui::Theme::Light) => false,
            None => ctx.theme() == egui::Theme::Dark,
        };
        let rem = match self.p.settings.text_size.as_deref() {
            Some("small") => 15.0,
            Some("large") => 18.5,
            Some("xlarge") => 20.5,
            _ => 16.5,
        };
        let narrow = ctx.content_rect().width() < 720.0;
        Theme { p: if dark { theme::DARK } else { theme::LIGHT }, dark, rem, motion: self.p.settings.animations != Some(false), narrow }
    }

    // ---------------------------------------------------------------- the frame

    pub fn frame(&mut self, ctx: &Context) {
        self.now = ctx.input(|i| i.time);
        let t = self.theme_for(ctx);
        let changed = t.dark != self.t.dark || t.rem != self.t.rem || t.narrow != self.t.narrow || t.motion != self.t.motion || !self.started;
        if t.rem != self.t.rem {
            rich::clear_cache();
        }
        self.t = t;
        if changed {
            widgets::apply_visuals(ctx, &self.t);
        }
        if !self.started {
            self.started = true;
            self.nav_key = self.nav_key_of(self.tab, &self.view());
            self.nav_start = self.now;
            if !self.welcome && self.p.settings.buddy.is_none() {
                self.later(0.7, Delayed::Setup(true, "New in this update!".into()));
            } else if !self.welcome {
                self.buddy_on_page();
            }
        }
        // Saving.
        if let Some(due) = self.save_due {
            if self.now >= due {
                self.flush();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(due - self.now));
            }
        }
        if self.saver.last_error().is_some() {
            self.toast("Could not save your progress.");
        }
        // Delayed things.
        let now = self.now;
        let due: Vec<Delayed> = {
            let mut out = Vec::new();
            let mut i = 0;
            while i < self.delayed.len() {
                if self.delayed[i].0 <= now {
                    out.push(self.delayed.remove(i).1);
                } else {
                    i += 1;
                }
            }
            out
        };
        for d in due {
            self.run_delayed(d);
        }
        if let Some(next) = self.delayed.iter().map(|d| d.0).fold(None, |a: Option<f64>, b| Some(a.map_or(b, |a| a.min(b)))) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((next - now).max(0.01)));
        }
        self.tick_exam(ctx);
        self.tick_reader(ctx);
        self.run_script(ctx);
        self.sound.tick();
        // Phone file dialogs answer later.
        if let Some((is_export, r)) = crate::platform::poll_file() {
            self.file_answer(is_export, r);
        }
        // Input wakes the buddy.
        if ctx.input(|i| !i.events.is_empty() || i.pointer.delta() != egui::Vec2::ZERO) {
            self.buddy.activity(self.now);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::BrowserBack)) || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) && self.dialog.is_none() && self.setup.is_none() && crate::platform::is_android() {
            self.back();
        }
        if let Some((tip_key, at)) = self.buddy_tip_at.clone() {
            if now >= at {
                self.buddy_tip_at = None;
                if tip_key == self.nav_key {
                    self.buddy_tip();
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(at - now));
            }
        }
    }

    fn run_delayed(&mut self, d: Delayed) {
        match d {
            Delayed::Sound(s) => self.sound.play(s),
            Delayed::BuddyReact(k) => self.buddy.react(k, self.now, &mut self.sound),
            Delayed::BuddySay(text, pr, mood, actions) => {
                self.buddy.say(&text, pr, mood, actions, self.now, &mut self.sound);
            }
            Delayed::Setup(first, intro) => self.open_setup(first, &intro),
            Delayed::ScrollToSection(i) => self.notes.scroll_to = Some(i),
            Delayed::HideBuddyForExam => {
                if self.buddy_away {
                    self.buddy.hide("exam", true);
                }
            }
        }
    }

    pub fn open_setup(&mut self, first: bool, intro: &str) {
        let cur = self.p.settings.buddy.clone();
        self.setup = Some(buddy::Setup::new(first || cur.is_none(), cur.as_ref(), intro, self.now));
        self.buddy.hide("setup", true);
        self.sound.play("open");
    }

    pub fn finish_setup(&mut self, look: Option<BuddyPrefs>) {
        self.setup = None;
        self.buddy.hide("setup", false);
        let Some(look) = look else { return };
        let cur = self.p.settings.buddy.clone();
        let mut b = cur.clone().unwrap_or_default();
        b.kind = look.kind;
        b.color = look.color;
        b.accessory = look.accessory;
        b.name = look.name.clone();
        if cur.is_none() {
            b.on = true;
        }
        self.p.settings.buddy = Some(b);
        self.save();
        self.apply_buddy();
        let text = if cur.is_some() {
            "Looking good! Thanks for the new look.".to_string()
        } else {
            format!("Hi, I'm **{}**! I'll be around while you study, with tips, hints and a cheer when you get things right. Tap me any time.", look.name)
        };
        self.later(0.8, Delayed::BuddyReact("wave"));
        self.later(0.8, Delayed::BuddySay(text, Priority::User, Some(Mood::Happy), vec![]));
    }

    /// The whole window.
    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.frame(&ctx);
        let t = self.t;
        let p = t.p;
        let in_lesson = matches!(self.view(), View::Lesson { .. }) && self.tab == Tab::Study;
        // Top bar.
        egui::Panel::top("topbar").exact_size(56.0).frame(egui::Frame::new().fill(p.bg).inner_margin(egui::Margin::symmetric(if t.narrow { 12 } else { 20 }, 0))).show_separator_line(false).show(ui, |ui| {
            self.top_bar(ui);
            let r = ui.max_rect();
            ui.painter().hline(r.x_range().expand(30.0), r.bottom(), Stroke::new(1.0, p.border));
        });
        // Phone layout: the tabs sit in a bar at the bottom.
        let tabbar = t.narrow && !self.welcome && !in_lesson;
        if tabbar {
            egui::Panel::bottom("tabbar").exact_size(62.0).frame(egui::Frame::new().fill(p.bg)).show_separator_line(false).show(ui, |ui| {
                let r = ui.max_rect();
                ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, p.border));
                self.bottom_tabs(ui);
            });
        }
        // Page header/footer that stay put (exam bar, reader, lesson footer).
        self.fixed_parts(ui);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.bg)).show(ui, |ui| {
            let nav_t = ((self.now - self.nav_start) / 0.28) as f32;
            let (mut dx, mut op) = (0.0, 1.0);
            if t.motion && nav_t < 1.0 && self.nav_kind != NavKind::First {
                let e = super::anim::Ease::Bezier(0.22, 1.0, 0.36, 1.0).at(nav_t);
                op = 0.25 + 0.75 * e;
                dx = match self.nav_kind {
                    NavKind::Push => 36.0 * (1.0 - e),
                    NavKind::Pop => -36.0 * (1.0 - e),
                    _ => 0.0,
                };
                ctx.request_repaint();
            }
            let mut sa = egui::ScrollArea::vertical().id_salt(self.nav_key.clone()).auto_shrink([false, false]);
            if self.scroll_reset {
                sa = sa.vertical_scroll_offset(0.0);
                self.scroll_reset = false;
            }
            self.viewport_top = ui.max_rect().top();
            let out = sa.show(ui, |ui| {
                ui.set_opacity(op);
                let rect = ui.max_rect().translate(vec2(dx, 0.0));
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    if self.welcome {
                        self.view_welcome(ui);
                    } else {
                        self.page(ui);
                    }
                });
                if let Some((r, a)) = self.scroll_rect.take() {
                    ui.scroll_to_rect(r, Some(a));
                }
            });
            let _ = out;
            if ctx.input(|i| i.smooth_scroll_delta.y != 0.0) {
                self.notes.reader.user_scrolled = self.now;
            }
        });
        self.overlays(&ctx, tabbar);
    }

    fn page(&mut self, ui: &mut Ui) {
        let v = self.view();
        match (self.tab, v) {
            (Tab::Study, View::Home) => self.view_study_home(ui),
            (Tab::Study, View::Subject { subject }) => self.view_subject(ui, &subject),
            (Tab::Study, View::Topic { subject, topic, mode }) => self.view_topic(ui, &subject, &topic, mode),
            (Tab::Study, View::Course { course }) => self.view_course(ui, &course),
            (Tab::Study, View::Lesson { course, lesson, run }) => self.view_lesson(ui, &course, &lesson, run),
            (Tab::Exams, View::Home) => self.view_exam_home(ui),
            (Tab::Exams, View::List { subject }) => self.view_exam_list(ui, &subject),
            (Tab::Exams, View::Intro { subject, exam }) => self.view_exam_intro(ui, &subject, &exam),
            (Tab::Exams, View::Run { subject, exam }) => self.view_exam_run(ui, &subject, &exam),
            (Tab::Exams, View::Mark { subject, exam }) => self.view_exam_mark(ui, &subject, &exam),
            (Tab::Exams, View::Result { subject, exam, attempt }) => self.view_exam_result(ui, &subject, &exam, attempt),
            (Tab::Exams, View::Review { subject, exam, attempt }) => self.view_exam_review(ui, &subject, &exam, attempt),
            (Tab::Progress, _) => self.view_progress(ui),
            (Tab::Settings, _) => self.view_settings(ui),
            _ => self.go(self.tab, View::Home),
        }
    }

    fn fixed_parts(&mut self, ui: &mut Ui) {
        let v = self.view();
        let p = self.t.p;
        match (self.tab, &v) {
            (Tab::Exams, View::Run { .. }) if self.session.as_ref().is_some_and(|s| !s.marking) => {
                egui::Panel::top("exam-bar").frame(egui::Frame::new().fill(p.bg).inner_margin(egui::Margin::symmetric(if self.t.narrow { 16 } else { 24 }, 9))).show_separator_line(false).show(ui, |ui| {
                    self.exam_bar(ui);
                    let r = ui.max_rect();
                    ui.painter().hline(r.x_range().expand(30.0), r.bottom() + 9.0, Stroke::new(1.0, p.border));
                });
            }
            (Tab::Study, View::Topic { mode: Mode::Questions, subject, topic }) => {
                let (s, tp) = (subject.clone(), topic.clone());
                egui::Panel::top("score-bar").frame(egui::Frame::new().fill(p.bg)).show_separator_line(false).show(ui, |ui| {
                    self.score_bar(ui, &s, &tp);
                });
            }
            (Tab::Study, View::Lesson { .. }) => {
                egui::Panel::bottom("lesson-foot").frame(egui::Frame::new().fill(p.bg)).show_separator_line(false).show(ui, |ui| {
                    self.lesson_foot(ui);
                });
            }
            _ => {}
        }
    }

    // ---------------------------------------------------------------- top bar

    fn top_bar(&mut self, ui: &mut Ui) {
        let t = self.t;
        let p = t.p;
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = if t.narrow { 8.0 } else { 24.0 };
            // Brand.
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 7.0;
                widgets::icon(ui, "logo", t.px(1.35), p.accent);
                if !t.narrow || self.welcome {
                    widgets::plain(ui, &t, "FreeSheet", 1.05, W::Bold, p.text);
                }
            });
            if self.welcome {
                return;
            }
            if !t.narrow {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (tab, name, icon) in [(Tab::Study, "Study", "study"), (Tab::Exams, "Exams", "exams"), (Tab::Progress, "Progress", "progress")] {
                        if self.tab_button(ui, tab, name, icon, false).clicked() {
                            self.tab_clicked(tab);
                        }
                    }
                });
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = if t.narrow { 8.0 } else { 16.0 };
                // Settings cog.
                let on = self.tab == Tab::Settings;
                let (rect, resp) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::click());
                let hover = resp.hovered();
                ui.painter().rect(rect, radius(13.0), if on { p.surface2 } else if hover { p.surface } else { p.bg }, Stroke::new(1.0, if on { p.border_strong } else { p.border }), egui::StrokeKind::Inside);
                let rot = ui.ctx().animate_value_with_time(Id::new("cog"), if on { 90.0 } else if hover { 30.0 } else { 0.0 }, if t.motion { 0.4 } else { 0.0 });
                let tex = super::images::icon(ui.ctx(), "settings", 20.0, 1.8);
                let mut mesh = egui::Mesh::with_texture(tex.id());
                mesh.add_rect_with_uv(Rect::from_center_size(rect.center(), vec2(20.0, 20.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), if on || hover { p.text } else { p.muted });
                mesh.rotate(egui::emath::Rot2::from_angle(rot.to_radians()), rect.center());
                ui.painter().add(mesh);
                if resp.on_hover_text("Settings").on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    self.open_settings();
                }
                // Grade.
                let grades: Vec<i64> = content::catalog().grades.iter().map(|g| g.grade).collect();
                let labels: Vec<String> = grades.iter().map(|g| format!("Grade {g}")).collect();
                let cur = grades.iter().position(|g| *g == self.grade()).unwrap_or(0);
                if let Some(i) = widgets::select(ui, &t, Id::new("grade-select"), cur, &labels, if t.narrow { 92.0 } else { 100.0 }, true) {
                    self.change_grade(grades[i]);
                }
                if !t.narrow {
                    widgets::plain(ui, &t, "Grade", 0.9, W::Regular, p.muted);
                }
                // Streak.
                self.streak_button(ui);
            });
        });
    }

    fn tab_button(&mut self, ui: &mut Ui, tab: Tab, name: &str, icon: &str, vertical: bool) -> egui::Response {
        let t = self.t;
        let p = t.p;
        let active = self.tab == tab;
        if vertical {
            let w = ui.available_width();
            let (rect, resp) = ui.allocate_exact_size(vec2(w, 54.0), Sense::click());
            let hover = resp.hovered();
            let color = if active { p.accent } else if hover { p.text } else { p.muted };
            let pill = Rect::from_center_size(pos2(rect.center().x, rect.top() + 18.0), vec2(56.0, 30.0));
            if active {
                ui.painter().rect_filled(pill, radius(15.0), p.accent_soft);
            }
            widgets::paint_icon(ui, icon, Rect::from_center_size(pill.center(), vec2(22.0, 22.0)), color, 1.8);
            let st = Style::new(t.px(0.72), color, p.muted).weight(if active { W::Semi } else { W::Medium }).center();
            let l = rich::layout(ui.ctx(), name, &st, w);
            rich::paint(ui.painter(), &l, pos2(rect.left(), rect.top() + 35.0), &st);
            return resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
        let st = Style::new(t.px(1.0), if active { p.text } else { p.muted }, p.muted).weight(if active { W::Semi } else { W::Regular }).line(1.3);
        let l = rich::layout(ui.ctx(), name, &st, 300.0);
        let size = vec2(l.size.x + t.px(1.05) + 6.0 + 2.0 * t.px(0.85), l.size.y + 2.0 * t.px(0.4));
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let hover = resp.hovered();
        if active {
            ui.painter().rect_filled(rect, radius(13.0), p.surface2);
        } else if hover {
            ui.painter().rect_filled(rect, radius(13.0), p.surface);
        }
        let mut st2 = st;
        if hover {
            st2.color = p.text;
        }
        let ix = rect.left() + t.px(0.85);
        widgets::paint_icon(ui, icon, Rect::from_min_size(pos2(ix, rect.center().y - t.px(1.05) / 2.0), vec2(t.px(1.05), t.px(1.05))), st2.color, 1.8);
        rich::paint(ui.painter(), &l, pos2(ix + t.px(1.05) + 6.0, rect.center().y - l.size.y / 2.0), &st2);
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    fn tab_clicked(&mut self, tab: Tab) {
        if self.welcome {
            return;
        }
        if tab == self.tab {
            // Clicking the active tab returns to its home screen (except mid-exam).
            let v = self.view();
            if !(tab == Tab::Exams && matches!(v, View::Run { .. } | View::Mark { .. })) {
                self.go(tab, View::Home);
            }
            return;
        }
        self.set_tab(tab);
    }

    fn bottom_tabs(&mut self, ui: &mut Ui) {
        let w = ui.available_width() / 3.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (tab, name, icon) in [(Tab::Study, "Study", "study"), (Tab::Exams, "Exams", "exams"), (Tab::Progress, "Progress", "progress")] {
                ui.allocate_ui_with_layout(vec2(w, 62.0), Layout::top_down(Align::Center), |ui| {
                    ui.add_space(4.0);
                    if self.tab_button(ui, tab, name, icon, true).clicked() {
                        self.tab_clicked(tab);
                    }
                });
            }
        });
    }

    fn streak_button(&mut self, ui: &mut Ui) {
        let t = self.t;
        let p = t.p;
        let st = streak::status(&self.p, streak::today());
        let lit = st.today_met;
        let color = if lit { p.flame } else { p.muted };
        let num = st.current.to_string();
        let ts = Style::new(t.px(0.95), color, p.muted).weight(W::Bold).line(1.2);
        let l = rich::layout(ui.ctx(), &num, &ts, 100.0);
        let size = vec2(20.0 + 5.0 + l.size.x + t.px(0.5) + t.px(0.65), 20.0f32.max(l.size.y) + 2.0 * t.px(0.3));
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        // Pop when today's goal is reached.
        let mut scale = 1.0;
        if let Some(s) = self.streak_pop {
            let k = ((self.now - s) / 0.7) as f32;
            if k < 1.0 && t.motion {
                scale = super::anim::keyframes(&[(0.0, 1.0), (0.35, 1.3), (0.7, 0.95), (1.0, 1.0)], super::anim::Ease::Ease, k);
                ui.ctx().request_repaint();
            } else {
                self.streak_pop = None;
            }
        }
        let r = Rect::from_center_size(rect.center(), rect.size() * scale);
        ui.painter().rect(r, radius(99.0), if resp.hovered() { p.surface } else { p.bg }, Stroke::new(1.0, if lit { theme::mix(p.flame, p.border, 0.45) } else { p.border }), egui::StrokeKind::Inside);
        let ir = Rect::from_min_size(pos2(r.left() + t.px(0.5), r.center().y - 10.0 * scale), vec2(20.0, 20.0) * scale);
        if lit {
            widgets::paint_icon(ui, "flame-fill", ir, theme::with_alpha(p.flame, 0.3), 1.8);
        }
        widgets::paint_icon(ui, "flame", ir, color, 1.8);
        rich::paint(ui.painter(), &l, pos2(ir.right() + 5.0, r.center().y - l.size.y / 2.0), &ts);
        let left = (streak::GOAL_QUESTIONS - st.questions_today).max(0.0) as i64;
        let tip = if lit {
            format!("{}-day streak. Today's goal is done.", st.current)
        } else {
            format!("{}-day streak. {left} more question{} or 1 exam today to {}.", st.current, if left == 1 { "" } else { "s" }, if st.current > 0 { "keep it going" } else { "start a streak" })
        };
        if resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            self.show_streak();
        }
    }

    pub fn change_grade(&mut self, g: i64) {
        if self.session.as_ref().is_some_and(|s| !s.marking) {
            self.write_draft();
            self.toast("Your exam has been saved. Resume it from the Exams tab.");
        }
        self.stop_exam();
        self.p.settings.grade = Some(g);
        self.save();
        for t in [Tab::Study, Tab::Exams, Tab::Progress, Tab::Settings] {
            self.views.insert(t, View::Home);
        }
        self.nav_key.clear();
        let tab = self.tab;
        self.go(tab, View::Home);
    }

    // ---------------------------------------------------------------- overlays

    fn overlays(&mut self, ctx: &Context, tabbar: bool) {
        let t = self.t;
        let p = t.p;
        let screen = ctx.content_rect();
        // The buddy.
        let gap = if tabbar { 62.0 } else if matches!(self.view(), View::Lesson { .. }) { 90.0 } else { 0.0 };
        let events = self.buddy.show(ctx, &t, self.now, gap, &mut self.sound);
        for e in events {
            match e {
                buddy::Event::Tapped => self.buddy_menu(),
                buddy::Event::Go(go) => self.buddy_go(go),
                buddy::Event::Prefs { min } => {
                    if let Some(b) = &mut self.p.settings.buddy {
                        b.min = min;
                        self.save();
                    }
                }
            }
        }
        // Toasts.
        self.toasts.retain(|x| self.now - x.at < 2.9);
        for (i, toast) in self.toasts.iter().enumerate() {
            let age = (self.now - toast.at) as f32;
            let k_in = if t.motion { super::anim::Ease::Spring.at((age / 0.4).min(1.0)) } else { 1.0 };
            let k_out = ((age - 2.6) / 0.3).clamp(0.0, 1.0);
            let op = (age / 0.25).min(1.0) * (1.0 - k_out);
            let y = screen.bottom() - 24.0 - gap - i as f32 * 48.0 + 24.0 * (1.0 - k_in) + 20.0 * k_out;
            egui::Area::new(Id::new(("toast", i))).order(Order::Tooltip).pivot(Align2::CENTER_BOTTOM).fixed_pos(pos2(screen.center().x, y)).interactable(false).show(ctx, |ui| {
                ui.set_opacity(op);
                egui::Frame::new().fill(p.text).corner_radius(radius(13.0)).inner_margin(egui::Margin::symmetric(16, 10)).show(ui, |ui| {
                    ui.set_max_width((screen.width() - 32.0).min(480.0));
                    widgets::plain(ui, &t, &toast.text, 0.92, W::Regular, p.bg);
                });
            });
            ctx.request_repaint();
        }
        // The streak celebration.
        if let Some((n, at)) = self.streak_toast {
            let age = (self.now - at) as f32;
            if age > 4.6 {
                self.streak_toast = None;
            } else {
                let k_in = if t.motion { super::anim::Ease::Spring.at((age / 0.4).min(1.0)) } else { 1.0 };
                let k_out = ((age - 4.2) / 0.4).clamp(0.0, 1.0);
                egui::Area::new(Id::new("streak-toast")).order(Order::Tooltip).pivot(Align2::CENTER_TOP).fixed_pos(pos2(screen.center().x, 70.0 - 24.0 * (1.0 - k_in) - 12.0 * k_out)).interactable(false).show(ctx, |ui| {
                    ui.set_opacity((age / 0.3).min(1.0) * (1.0 - k_out));
                    egui::Frame::new()
                        .fill(p.flame)
                        .corner_radius(radius(22.0))
                        .inner_margin(egui::Margin::symmetric(19, 13))
                        .shadow(egui::epaint::Shadow { offset: [0, 8], blur: 30, spread: 0, color: Color32::from_black_alpha(64) })
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
                                widgets::paint_icon(ui, "flame-fill", r, Color32::from_white_alpha(77), 1.8);
                                widgets::paint_icon(ui, "flame", r, Color32::WHITE, 1.8);
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    widgets::plain(ui, &t, &format!("{n}-day streak!"), 1.05, W::Bold, Color32::WHITE);
                                    widgets::plain(ui, &t, if n == 1 { "You started a streak. Come back tomorrow to keep it going." } else { "Today's goal is done. See you tomorrow." }, 0.88, W::Regular, Color32::WHITE);
                                });
                            });
                        });
                });
                ctx.request_repaint();
            }
        }
        self.show_dialog(ctx);
        if let Some(mut s) = self.setup.take() {
            match s.show(ctx, &t, self.now, &mut self.sound) {
                buddy::SetupResult::Open => self.setup = Some(s),
                buddy::SetupResult::Done(look) => {
                    self.setup = Some(s);
                    self.finish_setup(Some(look));
                }
                buddy::SetupResult::Cancelled => {
                    self.setup = Some(s);
                    self.finish_setup(None);
                }
            }
        }
    }

    fn show_dialog(&mut self, ctx: &Context) {
        let t = self.t;
        let p = t.p;
        let Some(d) = &self.dialog else { return };
        let screen = ctx.content_rect();
        let age = (self.now - d.opened) as f32;
        let mut k = if t.motion { super::anim::Ease::Spring.at((age / 0.32).min(1.0)) } else { 1.0 };
        let mut fade = if t.motion { (age / 0.2).min(1.0) } else { 1.0 };
        if let Some((c, ok)) = d.closing {
            let ck = ((self.now - c) / 0.18) as f32;
            if ck >= 1.0 || !t.motion {
                let d = self.dialog.take().unwrap();
                if ok {
                    self.dialog_ok(d.action);
                }
                return;
            }
            k = 1.0 - 0.06 * ck;
            fade = 1.0 - ck;
        }
        if t.motion {
            ctx.request_repaint();
        }
        let mut close: Option<bool> = None;
        egui::Area::new(Id::new("modal-back")).order(Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(r, 0.0, Color32::from_black_alpha((90.0 * fade) as u8));
            if resp.clicked() && d.cancel.is_some() {
                close = Some(false);
            }
        });
        let sheet = t.narrow;
        let w = if sheet { screen.width() } else { (screen.width() - 32.0).min(440.0) };
        let (pivot, pos) = if sheet {
            (Align2::CENTER_BOTTOM, pos2(screen.center().x, screen.bottom() + (1.0 - k) * 60.0))
        } else {
            (Align2::CENTER_CENTER, pos2(screen.center().x, screen.center().y + (1.0 - k) * 16.0))
        };
        let title = d.title.clone();
        let ok = d.ok.clone();
        let cancel = d.cancel.clone();
        let danger = d.danger;
        let streak_body = matches!(d.body, DialogBody::Streak);
        let body = match &d.body {
            DialogBody::Html(b) => b.clone(),
            DialogBody::Streak => vec![],
        };
        egui::Area::new(Id::new("modal")).order(Order::Foreground).pivot(pivot).fixed_pos(pos).show(ctx, |ui| {
            ui.set_opacity(fade);
            let rr = if sheet { egui::CornerRadius { nw: 18, ne: 18, sw: 0, se: 0 } } else { radius(30.0) };
            egui::Frame::new()
                .fill(p.bg)
                .corner_radius(rr)
                .inner_margin(egui::Margin { left: 24, right: 24, top: 22, bottom: if sheet { 28 } else { 22 } })
                .shadow(egui::epaint::Shadow { offset: [0, 10], blur: 40, spread: 0, color: Color32::from_black_alpha(64) })
                .show(ui, |ui| {
                    ui.set_width(w - 48.0);
                    widgets::plain(ui, &t, &title, 1.2, W::Semi, p.text);
                    ui.add_space(6.0);
                    if streak_body {
                        self.streak_dialog_body(ui);
                    }
                    for b in &body {
                        rich::show(ui, b, &Style::new(t.px(1.0), p.text, p.muted));
                    }
                    ui.add_space(t.px(1.0));
                    if sheet {
                        ui.vertical(|ui| {
                            if btn(&ok).kind(if danger { Kind::Danger } else { Kind::Primary }).full().show(ui, &t).clicked() {
                                close = Some(true);
                            }
                            if let Some(c) = &cancel {
                                if btn(c).full().show(ui, &t).clicked() {
                                    close = Some(false);
                                }
                            }
                        });
                    } else {
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            if btn(&ok).kind(if danger { Kind::Danger } else { Kind::Primary }).show(ui, &t).clicked() {
                                close = Some(true);
                            }
                            if let Some(c) = &cancel {
                                if btn(c).show(ui, &t).clicked() {
                                    close = Some(false);
                                }
                            }
                        });
                    }
                });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = Some(false);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            close = Some(true);
        }
        if let Some(okd) = close {
            self.close_dialog(okd);
        }
    }

    fn streak_dialog_body(&mut self, ui: &mut Ui) {
        let t = self.t;
        let p = t.p;
        let st = streak::status(&self.p, streak::today());
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
            if st.today_met {
                widgets::paint_icon(ui, "flame-fill", r, theme::with_alpha(p.flame, 0.3), 1.4);
            }
            widgets::paint_icon(ui, "flame", r, if st.today_met { p.flame } else { p.muted }, 1.4);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                widgets::plain(ui, &t, &st.current.to_string(), 2.4, W::Heavy, p.text);
                widgets::muted(ui, &t, &format!("day{} in a row", if st.current == 1 { "" } else { "s" }), 1.0);
            });
        });
        ui.add_space(8.0);
        let days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        let w = ui.available_width() / 7.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for d in &st.week {
                ui.allocate_ui_with_layout(vec2(w, 56.0), Layout::top_down(Align::Center), |ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 16.0, if d.met { p.flame } else { p.surface2 });
                    if d.today && !d.met {
                        ui.painter().circle_stroke(r.center(), 15.0, Stroke::new(2.0, p.border_strong));
                    }
                    if d.met {
                        widgets::paint_icon(ui, "flame", Rect::from_center_size(r.center(), vec2(18.0, 18.0)), Color32::WHITE, 1.8);
                    }
                    use chrono::Datelike;
                    widgets::plain(ui, &t, days[d.date.weekday().num_days_from_sunday() as usize], 0.78, if d.today { W::Semi } else { W::Regular }, if d.today { p.text } else { p.muted });
                });
            }
        });
        ui.add_space(8.0);
        let left = (streak::GOAL_QUESTIONS - st.questions_today).max(0.0) as i64;
        if st.today_met {
            widgets::plain(ui, &t, "Today's goal is done. Come back tomorrow to keep your streak.", 1.0, W::Semi, theme::hsl(145.0, 63.0, 33.0));
        } else {
            widgets::text(ui, &t, &format!("Check <strong>{left} more practice question{}</strong> or finish <strong>1 exam</strong> today to {}.", if left == 1 { "" } else { "s" }, if st.current > 0 { "keep your streak" } else { "start a streak" }), 1.0, W::Regular, p.text);
            ui.horizontal(|ui| {
                let goal = streak::GOAL_QUESTIONS;
                let done = st.questions_today.min(goal);
                widgets::bar(ui, &t, (done / goal * 100.0) as f32, ui.available_width() - 110.0, None);
                widgets::muted(ui, &t, &format!("{} / {} questions", done, goal), 0.85);
            });
        }
        widgets::muted(ui, &t, &format!("Longest streak: {} day{}", st.longest, if st.longest == 1 { "" } else { "s" }), 1.0);
    }

    fn dialog_ok(&mut self, action: DialogAction) {
        match action {
            DialogAction::None => {}
            DialogAction::CheckAll { key } => self.check_all(&key),
            DialogAction::SubmitExam => self.submit_exam(false),
            DialogAction::RestartExam { key } => {
                self.p.exam_drafts.shift_remove(&key);
                if self.session.as_ref().is_some_and(|s| s.key == key) {
                    self.stop_exam();
                }
                self.save();
            }
            DialogAction::StartMarking => {
                if let Some(s) = &self.session {
                    let (subject, exam) = (s.subject.clone(), s.exam_id.clone());
                    self.go(Tab::Exams, View::Mark { subject, exam });
                }
            }
            DialogAction::QuitLesson => {
                let id = self.lesson.as_ref().map(|l| l.course.clone());
                self.lesson = None;
                if let Some(id) = id {
                    self.go(Tab::Study, View::Course { course: id });
                }
            }
            DialogAction::Import(mut data) => {
                self.stop_exam();
                // Keep this device's buddy if the file doesn't have one.
                if self.p.settings.buddy.is_some() && data.settings.buddy.is_none() {
                    data.settings.buddy = self.p.settings.buddy.clone();
                }
                self.p = *data;
                self.apply_buddy();
                self.sound.enabled = self.p.settings.sounds != Some(false);
                self.flush();
                self.toast("Progress imported.");
                self.nav_key.clear();
                let tab = self.tab;
                let v = self.view();
                self.go(tab, v);
            }
            DialogAction::Reset => {
                self.stop_exam();
                let s = self.p.settings.clone();
                let mut fresh = Progress::default();
                fresh.settings.grade = s.grade;
                fresh.settings.text_size = s.text_size;
                fresh.settings.animations = s.animations;
                fresh.settings.sounds = s.sounds;
                fresh.settings.buddy = s.buddy;
                fresh.settings.text_rendering = s.text_rendering;
                self.p = fresh;
                self.flush();
                self.toast("Progress reset.");
            }
        }
    }

    // ---------------------------------------------------------------- files

    pub fn export(&mut self) {
        self.flush();
        let json = serde_json::to_string_pretty(&self.p).unwrap_or_default();
        let name = format!("freesheet-progress-{}.json", streak::day_key(streak::today()));
        let r = crate::platform::export(&json, &name);
        self.file_answer(true, r);
    }

    pub fn import(&mut self) {
        let r = crate::platform::import();
        self.file_answer(false, r);
    }

    fn file_answer(&mut self, is_export: bool, r: crate::platform::FileResult) {
        use crate::platform::FileResult::*;
        match r {
            Pending => self.file_pending = Some(is_export),
            Cancelled => self.file_pending = None,
            Failed(e) => {
                self.file_pending = None;
                self.toast(if is_export { format!("Could not export: {e}") } else { e });
            }
            Done(s) => {
                self.file_pending = None;
                if is_export {
                    self.toast("Progress exported.");
                } else {
                    let parsed = serde_json::from_str::<serde_json::Value>(s.trim_start_matches('\u{feff}'));
                    match parsed {
                        Ok(v) if v.get("topics").is_some() && v.get("exams").is_some() => match Progress::from_value(v) {
                            Some(p) => self.dialog(
                                "Replace your progress?",
                                vec![format!("<p>Importing replaces all progress on {} with the progress in the file.</p>", crate::platform::device())],
                                "Import",
                                Some("Cancel"),
                                true,
                                DialogAction::Import(Box::new(p)),
                            ),
                            None => self.toast("That file is not a FreeSheet progress file."),
                        },
                        Ok(_) => self.toast("That file is not a FreeSheet progress file."),
                        Err(_) => self.toast("That file could not be read."),
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- welcome

    fn view_welcome(&mut self, ui: &mut Ui) {
        let t = self.t;
        let p = t.p;
        widgets::page_column(ui, &t, 680.0, |ui| {
            ui.add_space(if t.narrow { 20.0 } else { 48.0 });
            widgets::h1(ui, &t, "Welcome to FreeSheet");
            widgets::lead(ui, &t, &format!("Notes, practice questions and full exams for the CAPS curriculum. Everything works offline and your progress is saved on {}.", crate::platform::device()));
            widgets::h2(ui, &t, "Which grade are you in?");
            let grades: Vec<i64> = content::catalog().grades.iter().map(|g| g.grade).collect();
            let mut picked = None;
            let cols = if t.narrow { 2 } else { 5 };
            let gap = 12.0;
            let bw = ((ui.available_width() - gap * (cols - 1) as f32) / cols as f32).floor();
            for chunk in grades.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for g in chunk {
                        let full = chunk.len() == 1 && t.narrow;
                        let w = if full { ui.available_width() } else { bw };
                        if btn(&format!("Grade {g}")).big().min_width(w).show(ui, &t).clicked() {
                            picked = Some(*g);
                        }
                    }
                });
                ui.add_space(gap - ui.spacing().item_spacing.y);
            }
            widgets::muted(ui, &t, "You can change the grade at any time from the top bar.", 1.0);
            let _ = p;
            if let Some(g) = picked {
                self.welcome = false;
                self.p.settings.grade = Some(g);
                self.save();
                self.nav_key.clear();
                self.go(Tab::Study, View::Home);
                if self.p.settings.buddy.is_none() {
                    self.later(0.6, Delayed::Setup(true, String::new()));
                }
            }
        });
    }

    // ---------------------------------------------------------------- helpers for pages

    pub fn crumbs(&mut self, ui: &mut Ui, items: &[(String, Option<(Tab, View)>)]) {
        let t = self.t;
        let p = t.p;
        let mut go = None;
        widgets::wrap(ui, vec2(6.0, 2.0), |ui| {
            for (i, (label, target)) in items.iter().enumerate() {
                let last = i == items.len() - 1;
                if last {
                    widgets::plain(ui, &t, label, 0.9, W::Regular, p.muted);
                } else {
                    if widgets::link(ui, &t, label, t.px(0.9), p.muted).clicked() {
                        go = target.clone();
                    }
                    widgets::plain(ui, &t, "›", 0.9, W::Regular, p.border_strong);
                }
            }
        });
        ui.add_space(t.px(0.4));
        if let Some((tab, v)) = go {
            self.go(tab, v);
        }
    }

    pub fn subject_stats(&self, s: &SubjectMeta) -> SubjectStats {
        let mut st = SubjectStats { total: s.topics.len(), ..Default::default() };
        let mut sum = 0i64;
        for tm in &s.topics {
            let Some(tp) = self.topic_prog(&self.tkey(&s.id, &tm.id)) else { continue };
            if tp.notes_read {
                st.notes += 1;
            }
            if let (Some(b), Some(bm)) = (tp.best, tp.best_max) {
                if bm.0 > 0.0 {
                    st.practised += 1;
                    sum += pct(b.0, bm.0);
                }
            }
        }
        st.avg = if st.practised > 0 { Some(marking::js_round(sum as f64 / st.practised as f64) as i64) } else { None };
        for x in &s.exams {
            let hist = self.p.exams.get(&self.tkey(&s.id, &x.id)).map(|v| v.as_slice()).unwrap_or(&[]);
            st.exams_written += hist.len();
            for a in hist {
                st.best_exam = Some(st.best_exam.map_or(a.percent, |b: i64| b.max(a.percent)));
            }
        }
        st
    }
}

#[derive(Default, Clone, Copy)]
pub struct SubjectStats {
    pub practised: usize,
    pub notes: usize,
    pub total: usize,
    pub avg: Option<i64>,
    pub exams_written: usize,
    pub best_exam: Option<i64>,
}

pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

pub fn unused(_: &Arc<TopicData>) {}
