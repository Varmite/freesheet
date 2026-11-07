//! The study buddy: an animated character in the corner that reacts to what you do,
//! gives tips and hints, and points you to the right notes (1.1's buddy.js). The app
//! decides what it says (ui::app); this file animates it and runs the speech bubble, the
//! help menu and the setup sheet. Moves in short bursts and is otherwise still, so an idle
//! app draws no frames.

use super::anim::{even, Ease, Prop, Tracks};
use super::buddy_art::{self, Mood, Pose};
use super::rich::{self, Style};
use super::theme::{Theme, W};
use super::widgets::{self, btn, radius, Kind};
use crate::model::BuddyPrefs;
use egui::{pos2, vec2, Align2, Color32, Context, Id, Order, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Ui};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Reaction = 1,
    Tip = 2,
    User = 3,
}

/// What a bubble button does (the app carries it out).
#[derive(Clone, Debug, PartialEq)]
pub enum Go {
    OpenTopic { subject: String, topic: String, questions: bool },
    OpenSubject(String),
    ExamList(String),
    ExamIntro { subject: String, exam: String },
    Review { subject: String, exam: String, attempt: usize },
    ShowFirstWrong,
    ShowInNotes(usize),
    StartQuestions,
    ReadNotes,
    QuizMe,
    Notes,
    Hint,
    StudyRecommendation,
    StreakAdvice,
    SubjectAdvice(String),
    ExamAdvice(String),
    ResultAdvice,
    ProgressAdvice,
    Look,
    Min,
}

#[derive(Clone, Debug)]
pub struct Action {
    pub label: String,
    pub primary: bool,
    pub go: Go,
}

impl Action {
    pub fn new(label: impl Into<String>, go: Go) -> Action {
        Action { label: label.into(), primary: true, go }
    }
    pub fn menu(label: impl Into<String>, go: Go) -> Action {
        Action { label: label.into(), primary: false, go }
    }
}

struct Bubble {
    html: String,
    plain: String,
    actions: Vec<Action>,
    priority: Priority,
    until: f64,
    hide_at: f64,
    start: f64,
    typing: bool,
    menu: bool,
    closing: Option<f64>,
}

/// An animated figure (the corner buddy, or a puppet inside a lesson).
pub struct Figure {
    pub look: BuddyPrefs,
    tracks: Tracks,
    pub mood: Mood,
    mood_until: Option<f64>,
    talk_until: f64,
    blink_at: f64,
    sparks_at: Option<f64>,
    dots: Option<(f64, f64)>,
    bulb_at: Option<f64>,
    zzz_at: Option<f64>,
    pub listening: bool,
    pub sleeping: bool,
    /// Loop the idle animations continuously (the setup preview).
    pub looping: bool,
    eyes: (f32, f32),
    eyes_target: (f32, f32),
    tex: Option<(u64, u32, TextureHandle)>,
}

impl Figure {
    pub fn new(look: BuddyPrefs) -> Figure {
        Figure {
            look,
            tracks: Tracks::default(),
            mood: Mood::Idle,
            mood_until: None,
            talk_until: 0.0,
            blink_at: 1.5,
            sparks_at: None,
            dots: None,
            bulb_at: None,
            zzz_at: None,
            listening: false,
            sleeping: false,
            looping: false,
            eyes: (0.0, 0.0),
            eyes_target: (0.0, 0.0),
            tex: None,
        }
    }

    pub fn set_mood(&mut self, m: Mood, ms: Option<f32>, now: f64) {
        self.mood = m;
        self.mood_until = ms.map(|ms| now + ms as f64 / 1000.0);
    }

    pub fn hop(&mut self, big: bool, now: f64) {
        let h = if big { -22.0 } else { -13.0 };
        let d = if big { 720.0 } else { 560.0 };
        let keys = |v: [f32; 6]| vec![(0.0, v[0]), (0.18, v[1]), (0.45, v[2]), (0.72, v[3]), (0.86, v[4]), (1.0, v[5])];
        self.tracks.add(Prop::ActorTy, now, d, keys([0.0, 0.0, h, 0.0, 0.0, 0.0]), Ease::EaseOut);
        self.tracks.add(Prop::ActorSx, now, d, keys([1.0, 1.12, 0.94, 1.1, 0.98, 1.0]), Ease::EaseOut);
        self.tracks.add(Prop::ActorSy, now, d, keys([1.0, 0.86, 1.1, 0.9, 1.03, 1.0]), Ease::EaseOut);
        self.tracks.add(Prop::ShadowS, now, d, vec![(0.0, 1.0), (0.45, 0.7), (1.0, 1.0)], Ease::EaseOut);
        self.tracks.add(Prop::ShadowO, now, d, vec![(0.0, 0.16), (0.45, 0.08), (1.0, 0.16)], Ease::EaseOut);
        self.tracks.add(Prop::ArmL, now, d, vec![(0.0, 0.0), (0.4, 70.0), (1.0, 0.0)], Ease::EaseOut);
        self.tracks.add(Prop::ArmR, now, d, vec![(0.0, 0.0), (0.4, -70.0), (1.0, 0.0)], Ease::EaseOut);
    }

    pub fn wave(&mut self, now: f64) {
        self.tracks.add(Prop::ArmR, now, 1300.0, vec![(0.0, 0.0), (0.2, -100.0), (0.35, -70.0), (0.5, -100.0), (0.65, -70.0), (0.8, -100.0), (1.0, 0.0)], Ease::EaseInOut);
        self.set_mood(Mood::Happy, Some(1300.0), now);
    }

    pub fn talk(&mut self, ms: f32, now: f64) {
        self.talk_until = now + ms as f64 / 1000.0;
    }

    pub fn react(&mut self, kind: &str, now: f64) {
        match kind {
            "correct" => {
                self.set_mood(Mood::Happy, Some(1500.0), now);
                self.hop(false, now);
                self.sparks_at = Some(now);
            }
            "celebrate" => {
                self.set_mood(Mood::Happy, Some(2600.0), now);
                self.hop(true, now);
                self.sparks_at = Some(now);
                // A second hop follows.
                let later = now + 0.76;
                let h = -22.0;
                let keys = |v: [f32; 6]| vec![(0.0, v[0]), (0.18, v[1]), (0.45, v[2]), (0.72, v[3]), (0.86, v[4]), (1.0, v[5])];
                self.tracks.0.push(super::anim::Track { prop: Prop::ActorTy, start: later, dur: 0.72, keys: keys([0.0, 0.0, h, 0.0, 0.0, 0.0]), ease: Ease::EaseOut });
                self.tracks.0.push(super::anim::Track { prop: Prop::ActorSy, start: later, dur: 0.72, keys: keys([1.0, 0.86, 1.1, 0.9, 1.03, 1.0]), ease: Ease::EaseOut });
                self.tracks.0.push(super::anim::Track { prop: Prop::ActorSx, start: later, dur: 0.72, keys: keys([1.0, 1.12, 0.94, 1.1, 0.98, 1.0]), ease: Ease::EaseOut });
            }
            "wrong" => {
                self.set_mood(Mood::Sad, Some(1700.0), now);
                self.tracks.add(Prop::ActorRot, now, 900.0, vec![(0.0, 0.0), (0.3, -6.0), (0.6, 4.0), (1.0, 0.0)], Ease::EaseInOut);
                self.tracks.add(Prop::ActorSx, now, 900.0, vec![(0.0, 1.0), (0.3, 0.96), (0.6, 0.97), (1.0, 1.0)], Ease::EaseInOut);
                self.tracks.add(Prop::ActorSy, now, 900.0, vec![(0.0, 1.0), (0.3, 0.96), (0.6, 0.97), (1.0, 1.0)], Ease::EaseInOut);
            }
            "partial" => {
                self.set_mood(Mood::Think, Some(1400.0), now);
                self.hop(false, now);
            }
            "hint" => {
                self.set_mood(Mood::Think, Some(1500.0), now);
                self.dots = Some((now, now + 0.9));
                self.tracks.add(Prop::ActorRot, now, 1100.0, vec![(0.0, 0.0), (0.4, -7.0), (0.7, -7.0), (1.0, 0.0)], Ease::EaseInOut);
                self.bulb_at = Some(now + 0.9);
            }
            "wave" => self.wave(now),
            "think" => {
                self.set_mood(Mood::Think, Some(1600.0), now);
                self.dots = Some((now, now + 1.2));
            }
            _ => {}
        }
    }

    pub fn fidget(&mut self, now: f64) {
        if self.sleeping {
            self.zzz_at = Some(now);
            return;
        }
        let has_tail = matches!(self.look.kind.as_str(), "fox" | "dino");
        let has_antenna = self.look.kind == "blob" && self.look.accessory != "cap" && self.look.accessory != "headphones";
        let has_light = self.look.kind == "robot" && self.look.accessory != "cap";
        let mut moves = vec!["breathe", "breathe", "look"];
        if has_tail {
            moves.extend(["wag", "wag"]);
        }
        if has_antenna {
            moves.push("sway");
        }
        if has_light {
            moves.push("light");
        }
        match moves[fastrand::usize(0..moves.len())] {
            "breathe" => {
                self.tracks.add(Prop::BodySx, now, 2000.0, even(&[1.0, 0.99, 1.0]), Ease::EaseInOut);
                self.tracks.add(Prop::BodySy, now, 2000.0, even(&[1.0, 1.03, 1.0]), Ease::EaseInOut);
                self.tracks.add(Prop::FloatY, now, 2000.0, even(&[0.0, -3.0, 0.0]), Ease::EaseInOut);
            }
            "wag" => self.tracks.add(Prop::Tail, now, 1400.0, even(&[0.0, -9.0, 9.0, -7.0, 6.0, 0.0]), Ease::EaseInOut),
            "sway" => self.tracks.add(Prop::Antenna, now, 1600.0, even(&[0.0, -10.0, 8.0, 0.0]), Ease::EaseInOut),
            "light" => self.tracks.add(Prop::Light, now, 1500.0, even(&[1.0, 0.3, 1.0, 0.3, 1.0]), Ease::EaseInOut),
            _ => {
                self.tracks.add(Prop::EyesX, now, 1700.0, vec![(0.0, 0.0), (0.25, -2.6), (0.45, -2.6), (0.7, 2.6), (1.0, 0.0)], Ease::EaseInOut);
                self.tracks.add(Prop::EyesY, now, 1700.0, vec![(0.0, 0.0), (0.25, -0.6), (0.45, -0.6), (0.7, -0.6), (1.0, 0.0)], Ease::EaseInOut);
            }
        }
    }

    pub fn look_at(&mut self, target: (f32, f32)) {
        self.eyes_target = target;
    }

    fn pose(&mut self, now: f64, motion: bool) -> (Pose, bool) {
        if let Some(u) = self.mood_until {
            if now >= u {
                self.mood = if self.listening { Mood::Listen } else if self.sleeping { Mood::Sleep } else { Mood::Idle };
                self.mood_until = None;
            }
        }
        let mut busy = false;
        // Blinking.
        if now >= self.blink_at {
            if matches!(self.mood, Mood::Idle | Mood::Think | Mood::Sad | Mood::Surprised) && motion {
                if fastrand::f32() < 0.18 {
                    self.tracks.add(Prop::Blink, now, 380.0, vec![(0.0, 1.0), (0.2, 0.1), (0.42, 1.0), (0.62, 0.1), (1.0, 1.0)], Ease::EaseInOut);
                } else {
                    self.tracks.add(Prop::Blink, now, 170.0, vec![(0.0, 1.0), (0.45, 0.1), (1.0, 1.0)], Ease::EaseInOut);
                }
            }
            self.blink_at = now + 2.2 + fastrand::f64() * 3.8;
        }
        let mut p = Pose { mood: self.mood, ..Pose::default() };
        if motion {
            let g = |pr| self.tracks.get(pr, now);
            p.float_y = g(Prop::FloatY).unwrap_or(0.0);
            p.actor_ty = g(Prop::ActorTy).unwrap_or(0.0);
            p.actor_sx = g(Prop::ActorSx).unwrap_or(1.0);
            p.actor_sy = g(Prop::ActorSy).unwrap_or(1.0);
            p.actor_rot = g(Prop::ActorRot).unwrap_or(0.0);
            p.body_sx = g(Prop::BodySx).unwrap_or(1.0);
            p.body_sy = g(Prop::BodySy).unwrap_or(1.0);
            p.shadow_s = g(Prop::ShadowS).unwrap_or(1.0);
            p.shadow_o = g(Prop::ShadowO).unwrap_or(0.16);
            p.arm_l = g(Prop::ArmL).unwrap_or(0.0);
            p.arm_r = g(Prop::ArmR).unwrap_or(0.0);
            p.tail = g(Prop::Tail).unwrap_or(0.0);
            p.antenna = g(Prop::Antenna).unwrap_or(0.0);
            p.light = g(Prop::Light).unwrap_or(1.0);
            p.blink = g(Prop::Blink).unwrap_or(1.0);
            let ex = g(Prop::EyesX).unwrap_or(0.0);
            let ey = g(Prop::EyesY).unwrap_or(0.0);
            // Eyes ease towards the pointer.
            let k = 0.35;
            self.eyes.0 += (self.eyes_target.0 - self.eyes.0) * k;
            self.eyes.1 += (self.eyes_target.1 - self.eyes.1) * k;
            if (self.eyes.0 - self.eyes_target.0).abs() > 0.05 || (self.eyes.1 - self.eyes_target.1).abs() > 0.05 {
                busy = true;
            }
            let q = |v: f32| (v * 4.0).round() / 4.0;
            p.eyes = (q(self.eyes.0 + ex), q(self.eyes.1 + ey));
            p.face = (q(self.eyes.0 * 0.35), q(self.eyes.1 * 0.35));
            busy |= self.tracks.active(now);
            if self.looping {
                let t = now as f32;
                let wave = |period: f32| (t / period * std::f32::consts::TAU).sin();
                p.float_y += -1.5 + 1.5 * wave(3.8);
                p.body_sy *= 1.0 + 0.014 * (1.0 + wave(3.2));
                p.body_sx *= 1.0 - 0.005 * (1.0 + wave(3.2));
                p.tail += 0.5 + 7.5 * wave(2.4);
                p.antenna += 0.5 + 6.5 * wave(3.2);
                p.light = 0.675 + 0.325 * wave(1.9);
                busy = true;
            }
            if self.listening {
                p.float_rot = 3.0 * (now as f32 / 1.8 * std::f32::consts::TAU).sin();
                busy = true;
            }
            if let Some(s) = self.sparks_at {
                let t = (now - s) as f32;
                if t < 1.2 {
                    p.sparks = Some(t / 0.95);
                    busy = true;
                } else {
                    self.sparks_at = None;
                }
            }
            if let Some((s, e)) = self.dots {
                if now < e {
                    p.dots = Some((now - s) as f32);
                    busy = true;
                } else {
                    self.dots = None;
                }
            }
            if let Some(s) = self.bulb_at {
                let t = (now - s) as f32;
                if t >= 0.0 && t < 1.25 {
                    p.bulb = Some(t / 1.1);
                    if t < 0.7 {
                        p.mood = Mood::Surprised;
                    }
                    busy = true;
                } else if t >= 1.25 {
                    self.bulb_at = None;
                } else {
                    busy = true;
                }
            }
            if let Some(s) = self.zzz_at {
                let t = (now - s) as f32;
                if t < 4.0 {
                    p.zzz = Some(t);
                    busy = true;
                } else {
                    self.zzz_at = None;
                }
            }
            if now < self.talk_until {
                p.talk = ((now * 1000.0 / 115.0) as i64) % 2 == 0;
                busy = true;
            }
        }
        self.tracks.prune(now);
        if self.mood_until.is_some() {
            busy = true;
        }
        (p, busy)
    }

    /// Paint the figure filling `rect` (the 120×120 artwork box). Returns true while it moves.
    pub fn paint(&mut self, ui: &Ui, rect: Rect, now: f64, motion: bool, opacity: f32) -> bool {
        let (pose, busy) = self.pose(now, motion);
        let ppp = ui.ctx().pixels_per_point();
        // The artwork has room around it for effects: 144 units drawn where 120 fit.
        let draw = Rect::from_center_size(rect.center(), rect.size() * (144.0 / 120.0));
        let px = (draw.width() * ppp).round() as u32;
        let key = pose.key() ^ look_key(&self.look);
        let stale = self.tex.as_ref().is_none_or(|(k, p, _)| *k != key || *p != px);
        if stale {
            let svg = buddy_art::svg(&self.look, &pose);
            if let Some(img) = super::images::rasterise(&svg, px as f32, px as f32) {
                let tex = match self.tex.take() {
                    Some((_, _, mut t)) => {
                        t.set(img, egui::TextureOptions::LINEAR);
                        t
                    }
                    None => ui.ctx().load_texture("buddy", img, egui::TextureOptions::LINEAR),
                };
                self.tex = Some((key, px, tex));
            }
        }
        if let Some((_, _, t)) = &self.tex {
            ui.painter().image(t.id(), draw, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE.gamma_multiply(opacity));
        }
        busy
    }
}

fn look_key(l: &BuddyPrefs) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (&l.kind, &l.color, &l.accessory).hash(&mut h);
    h.finish()
}

/// A still picture of a look (setup tiles, Settings card).
pub fn still(ui: &mut Ui, look: &BuddyPrefs, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let ppp = ui.ctx().pixels_per_point();
    let draw = Rect::from_center_size(rect.center(), rect.size() * (144.0 / 120.0));
    let px = (draw.width() * ppp).round() as u32;
    let id = Id::new(("buddy-still", look_key(look), px));
    let tex: Option<TextureHandle> = ui.ctx().memory(|m| m.data.get_temp(id));
    let tex = tex.or_else(|| {
        let img = super::images::rasterise(&buddy_art::svg(look, &Pose::default()), px as f32, px as f32)?;
        let t = ui.ctx().load_texture("buddy-still", img, egui::TextureOptions::LINEAR);
        ui.ctx().memory_mut(|m| m.data.insert_temp(id, t.clone()));
        Some(t)
    });
    if let Some(t) = tex {
        ui.painter().image(t.id(), draw, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    resp
}

// ---------------------------------------------------------------- the corner buddy

pub enum Event {
    /// The figure was tapped: the app answers with a menu (say with actions).
    Tapped,
    Go(Go),
    Prefs { min: bool },
}

pub struct Buddy {
    pub fig: Figure,
    pub on: bool,
    pub quiet: bool,
    pub min: bool,
    hidden: HashSet<&'static str>,
    bubble: Option<Bubble>,
    enter: Option<f64>,
    leave: Option<f64>,
    visible_last: bool,
    last_activity: f64,
    fidget_at: f64,
    pub told: HashSet<String>,
}

const SLEEP_AFTER: f64 = 90.0;

impl Buddy {
    pub fn new() -> Buddy {
        Buddy {
            fig: Figure::new(buddy_art::random_look()),
            on: false,
            quiet: false,
            min: false,
            hidden: HashSet::new(),
            bubble: None,
            enter: None,
            leave: None,
            visible_last: false,
            last_activity: 0.0,
            fidget_at: 8.0,
            told: HashSet::new(),
        }
    }

    pub fn configure(&mut self, look: Option<&BuddyPrefs>, on: bool, quiet: bool, min: bool) {
        if let Some(l) = look {
            let l = buddy_art::normalise(l);
            if l.kind != self.fig.look.kind || l.color != self.fig.look.color || l.accessory != self.fig.look.accessory || l.name != self.fig.look.name {
                self.fig.look = l;
            }
        }
        self.on = on;
        self.quiet = quiet;
        self.min = min;
    }

    pub fn hide(&mut self, reason: &'static str, yes: bool) {
        if yes {
            self.hidden.insert(reason);
        } else {
            self.hidden.remove(reason);
        }
    }

    pub fn visible(&self) -> bool {
        self.on && self.hidden.is_empty()
    }

    pub fn is_talking_free(&self) -> bool {
        self.visible() && !self.min
    }

    /// Say something (markup). Returns false if the buddy is busy or switched off.
    pub fn say(&mut self, text: &str, priority: Priority, mood: Option<Mood>, actions: Vec<Action>, now: f64, sound: &mut crate::audio::Engine) -> bool {
        if !self.visible() || self.min {
            return false;
        }
        if self.quiet && priority != Priority::User {
            return false;
        }
        if let Some(b) = &self.bubble {
            if b.closing.is_none() && b.priority > priority && now < b.until {
                return false;
            }
        }
        self.wake(now);
        let html = crate::markup::inline(text);
        let plain = crate::markup::strip_tags(&html);
        let ms = if !actions.is_empty() { 16000.0 } else { (2600.0 + plain.chars().count() as f64 * 55.0).max(5200.0) };
        let typing = plain.chars().count() <= 260;
        self.bubble = Some(Bubble { html, plain, actions, priority, until: now + (ms.min(8000.0)) / 1000.0, hide_at: now + ms / 1000.0, start: now, typing, menu: false, closing: None });
        if let Some(m) = mood {
            self.fig.set_mood(m, Some(2600.0), now);
        }
        sound.play("buddy");
        true
    }

    /// Say something once per session (key).
    pub fn tell(&mut self, key: &str, text: &str, priority: Priority, mood: Option<Mood>, actions: Vec<Action>, now: f64, sound: &mut crate::audio::Engine) -> bool {
        if self.told.contains(key) {
            return false;
        }
        let ok = self.say(text, priority, mood, actions, now, sound);
        if ok {
            self.told.insert(key.to_string());
        }
        ok
    }

    pub fn mark_menu(&mut self) {
        if let Some(b) = &mut self.bubble {
            b.menu = true;
        }
    }

    pub fn close_bubble(&mut self, now: f64) {
        if let Some(b) = &mut self.bubble {
            if b.closing.is_none() {
                b.closing = Some(now);
            }
        }
    }

    pub fn react(&mut self, kind: &str, now: f64, sound: &mut crate::audio::Engine) {
        if !self.visible() {
            return;
        }
        self.wake(now);
        if kind == "celebrate" {
            sound.play("buddyHappy");
        }
        self.fig.react(kind, now);
    }

    pub fn listen(&mut self, on: bool, now: f64) {
        self.fig.listening = on;
        if on {
            self.wake(now);
            self.fig.set_mood(Mood::Listen, None, now);
        } else if self.fig.mood == Mood::Listen {
            self.fig.set_mood(Mood::Idle, None, now);
        }
    }

    fn wake(&mut self, now: f64) {
        self.last_activity = now;
        if self.fig.sleeping {
            self.fig.sleeping = false;
            self.fig.set_mood(Mood::Surprised, Some(500.0), now);
            self.fig.tracks.add(Prop::ActorSx, now, 600.0, vec![(0.0, 1.0), (0.35, 0.94), (0.7, 1.04), (1.0, 1.0)], Ease::Spring);
            self.fig.tracks.add(Prop::ActorSy, now, 600.0, vec![(0.0, 1.0), (0.35, 1.1), (0.7, 0.97), (1.0, 1.0)], Ease::Spring);
        }
    }

    /// Any input from the learner.
    pub fn activity(&mut self, now: f64) {
        if now - self.last_activity > 0.4 {
            self.wake(now);
        }
    }

    pub fn has_bubble(&self) -> bool {
        self.bubble.as_ref().is_some_and(|b| b.closing.is_none())
    }

    /// Draw the buddy and its bubble. `bottom_gap`: space to keep free at the bottom.
    pub fn show(&mut self, ctx: &Context, t: &Theme, now: f64, bottom_gap: f32, sound: &mut crate::audio::Engine) -> Vec<Event> {
        let mut events = Vec::new();
        let vis = self.visible();
        if vis && !self.visible_last {
            self.enter = Some(now);
            self.leave = None;
            if !self.fig.sleeping {
                self.fig.wave(now + 0.38);
            }
            self.last_activity = now;
        } else if !vis && self.visible_last {
            self.leave = Some(now);
            self.bubble = None;
        }
        self.visible_last = vis;
        let leaving = self.leave.map(|l| now - l);
        if !vis && leaving.is_none_or(|d| d > 0.4) {
            return events;
        }
        // Idle life: a small move now and then, and dozing off after a while.
        if vis && t.motion && !self.min && !self.fig.listening {
            if now >= self.fidget_at {
                self.fig.fidget(now);
                self.fidget_at = now + 6.0 + fastrand::f64() * 8.0;
            }
            if !self.fig.sleeping && now - self.last_activity > SLEEP_AFTER && self.bubble.is_none() {
                self.fig.sleeping = true;
                self.fig.set_mood(Mood::Sleep, None, now);
                self.fig.zzz_at = Some(now);
            }
        }
        let screen = ctx.content_rect();
        let size = if self.min { 50.0 } else { 96.0 };
        let mut fig_rect = Rect::from_min_size(pos2(screen.right() - 16.0 - size, screen.bottom() - 12.0 - bottom_gap - size), vec2(size, size));
        // Entrance: rises from below with a spring; exit: drops away.
        let mut opacity = if self.min { 0.85 } else { 1.0 };
        if let Some(e) = self.enter {
            let k = ((now - e) / 0.65) as f32;
            if k < 1.0 && t.motion {
                let e2 = super::anim::bezier(0.2, 1.4, 0.4, 1.0, k);
                let dy = (1.0 - e2) * size * 1.4;
                let s = 0.6 + 0.4 * e2;
                fig_rect = Rect::from_center_size(fig_rect.center() + vec2(0.0, dy), fig_rect.size() * s);
            } else {
                self.enter = None;
            }
        }
        if let Some(d) = leaving {
            let k = (d / 0.38) as f32;
            fig_rect = fig_rect.translate(vec2(0.0, super::anim::bezier(0.5, 0.0, 0.75, 0.0, k) * size * 1.5));
            opacity *= 1.0 - k.min(1.0) * 0.3;
        }
        // Eyes follow the pointer.
        if let Some(ptr) = ctx.pointer_latest_pos() {
            let c = pos2(fig_rect.center().x, fig_rect.top() + fig_rect.height() * 0.52);
            let (dx, dy) = (ptr.x - c.x, ptr.y - c.y);
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let k = (dist / 300.0).min(1.0);
            if !self.fig.sleeping {
                self.fig.look_at((dx / dist * 2.8 * k, dy / dist * 2.4 * k));
            }
        }
        let mut busy = self.enter.is_some() || leaving.is_some();
        let fig_id = Id::new("buddy-fig");
        egui::Area::new(fig_id).order(Order::Foreground).fixed_pos(fig_rect.min).interactable(true).show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(fig_rect.size(), Sense::click());
            let hover = resp.hovered();
            let k = ui.ctx().animate_bool_with_time(fig_id.with("hover"), hover && t.motion, 0.25);
            let draw = Rect::from_center_size(rect.center(), rect.size() * (1.0 + 0.06 * k));
            busy |= self.fig.paint(ui, draw, now, t.motion, opacity);
            if resp.clicked() {
                if self.min {
                    self.min = false;
                    events.push(Event::Prefs { min: false });
                    self.fig.wave(now + 0.2);
                } else {
                    self.wake(now);
                    self.fig.hop(false, now);
                    if self.bubble.as_ref().is_some_and(|b| b.menu && b.closing.is_none()) {
                        self.close_bubble(now);
                    } else {
                        events.push(Event::Tapped);
                    }
                }
            }
            resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(format!("{}, your study buddy. Open the help menu.", self.fig.look.name));
        });
        // The speech bubble.
        if let Some(b) = &mut self.bubble {
            if b.closing.is_none() && now >= b.hide_at {
                b.closing = Some(now);
            }
            let close_k = b.closing.map(|c| ((now - c) / 0.18) as f32);
            if close_k.is_some_and(|k| k >= 1.0) || !t.motion && b.closing.is_some() {
                self.bubble = None;
            } else {
                let appear = ((now - b.start) / 0.38) as f32;
                let (mut op, mut dy, mut sc) = (1.0, 0.0, 1.0);
                if t.motion && appear < 1.0 {
                    let e = super::anim::Ease::Spring.at(appear);
                    op = appear.min(1.0);
                    dy = 10.0 * (1.0 - e);
                    sc = 0.9 + 0.1 * e;
                    busy = true;
                }
                if let Some(k) = close_k {
                    op = 1.0 - k;
                    dy = 6.0 * k;
                    sc = 1.0 - 0.06 * k;
                    busy = true;
                }
                // Typing: two characters every 32 ms while the mouth moves.
                let typed = if b.typing && t.motion { (((now - b.start) * 1000.0 / 32.0) as usize + 1) * 2 } else { usize::MAX };
                let done_typing = typed >= b.plain.chars().count();
                if !done_typing {
                    self.fig.talk(150.0, now);
                    busy = true;
                } else {
                    b.typing = false;
                }
                let max_w = (screen.width() - 110.0).clamp(160.0, 330.0);
                let anchor = pos2(fig_rect.left() + 22.0, fig_rect.top() + 20.0 + dy);
                let name = self.fig.look.name.clone();
                let html = if done_typing { b.html.clone() } else { crate::markup::escape_html(&b.plain.chars().take(typed).collect::<String>()) };
                let actions = b.actions.clone();
                let resp = egui::Area::new(Id::new("buddy-bubble")).order(Order::Foreground).pivot(Align2::RIGHT_BOTTOM).fixed_pos(anchor).show(ctx, |ui| {
                    ui.set_opacity(op);
                    let _ = sc;
                    let p = &t.p;
                    let frame = egui::Frame::new()
                        .fill(p.bg)
                        .stroke(Stroke::new(1.0, p.border_strong))
                        .corner_radius(radius(20.0))
                        .inner_margin(egui::Margin { left: 15, right: 15, top: 11, bottom: 13 })
                        .shadow(egui::epaint::Shadow { offset: [0, 14], blur: 40, spread: 0, color: Color32::from_black_alpha(46) });
                    let mut clicked_go = None;
                    let mut close = false;
                    let fr = frame.show(ui, |ui| {
                        ui.set_max_width(max_w);
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let st = Style::new(t.px(0.95), p.text, p.muted).line(1.45);
                        let text_w = rich::layout(ui.ctx(), &b.html, &st, max_w - 30.0).size.x;
                        let name_w = rich::layout(ui.ctx(), &crate::markup::escape_html(&name.to_uppercase()), &Style::new(t.px(0.72), p.accent, p.muted).weight(W::Bold), 300.0).size.x;
                        let mut w = text_w.max(name_w + 28.0).min(max_w);
                        if !actions.is_empty() {
                            w = w.max(200.0_f32.min(max_w));
                        }
                        ui.set_width(w);
                        ui.horizontal(|ui| {
                            rich::plain(ui, &name.to_uppercase(), &Style::new(t.px(0.72), p.accent, p.muted).weight(W::Bold).line(1.3));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let r = btn("").kind(Kind::Ghost).icon("close").small().show(ui, t);
                                if r.clicked() {
                                    close = true;
                                }
                            });
                        });
                        let r = rich::show(ui, &html, &st).0;
                        if ui.interact(r.rect, Id::new("bubble-text"), Sense::click()).clicked() {
                            b.typing = false;
                        }
                        if !actions.is_empty() {
                            ui.add_space(8.0);
                            ui.spacing_mut().item_spacing.y = 5.0;
                            for a in &actions {
                                if bubble_action(ui, t, &crate::markup::inline(&a.label), a.primary).clicked() {
                                    clicked_go = Some(a.go.clone());
                                }
                            }
                        }
                    });
                    // The bubble's tail, pointing at the buddy.
                    let r = fr.response.rect;
                    let tip = [pos2(r.right() - 1.0, r.bottom() - 30.0), pos2(r.right() + 8.0, r.bottom() - 23.0), pos2(r.right() - 1.0, r.bottom() - 16.0)];
                    ui.painter().add(egui::Shape::convex_polygon(tip.to_vec(), p.bg, Stroke::NONE));
                    ui.painter().line_segment([tip[0], tip[1]], Stroke::new(1.0, p.border_strong));
                    ui.painter().line_segment([tip[1], tip[2]], Stroke::new(1.0, p.border_strong));
                    (clicked_go, close, fr.response.hovered())
                });
                let (go, close, hovered) = resp.inner;
                if hovered {
                    b.hide_at = b.hide_at.max(now + 2.5);
                }
                if close {
                    b.closing = Some(now);
                }
                if let Some(go) = go {
                    b.closing = Some(now);
                    match go {
                        Go::Min => {
                            self.min = true;
                            self.bubble = None;
                            events.push(Event::Prefs { min: true });
                        }
                        g => events.push(Event::Go(g)),
                    }
                }
            }
        }
        let _ = sound;
        if busy || self.bubble.is_some() {
            ctx.request_repaint();
        } else if t.motion && vis {
            // Wake for the next blink or fidget.
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((self.fig.blink_at - now).clamp(0.05, 6.0).min((self.fidget_at - now).max(0.05))));
        }
        events
    }
}

impl Default for Buddy {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------- setup sheet

pub struct Setup {
    pub first: bool,
    pub intro: String,
    pub look: BuddyPrefs,
    pub fig: Figure,
    name: String,
    opened: f64,
}

pub enum SetupResult {
    Open,
    Done(BuddyPrefs),
    Cancelled,
}

impl Setup {
    pub fn new(first: bool, current: Option<&BuddyPrefs>, intro: &str, now: f64) -> Setup {
        let look = buddy_art::normalise(&current.cloned().unwrap_or_else(buddy_art::random_look));
        let mut fig = Figure::new(look.clone());
        fig.looping = true;
        fig.set_mood(Mood::Happy, Some(1000.0), now + 0.3);
        fig.hop(true, now + 0.3);
        Setup { first, intro: intro.to_string(), name: look.name.clone(), look, fig, opened: now }
    }

    fn update(&mut self, look: BuddyPrefs, now: f64, sound: &mut crate::audio::Engine) {
        self.look = buddy_art::normalise(&look);
        self.fig.look = self.look.clone();
        self.fig.set_mood(Mood::Happy, Some(900.0), now);
        self.fig.hop(false, now);
        sound.play("select");
    }

    pub fn show(&mut self, ctx: &Context, t: &Theme, now: f64, sound: &mut crate::audio::Engine) -> SetupResult {
        let p = t.p;
        let screen = ctx.content_rect();
        let mut result = SetupResult::Open;
        let fade = if t.motion { (((now - self.opened) / 0.24) as f32).min(1.0) } else { 1.0 };
        egui::Area::new(Id::new("setup-back")).order(Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(r, 0.0, Color32::from_black_alpha((115.0 * fade) as u8));
            let _ = resp;
        });
        let narrow = screen.width() < 760.0;
        let w = (screen.width() - 32.0).min(880.0);
        let enter = if t.motion { super::anim::Ease::Spring.at((((now - self.opened) / 0.56) as f32).min(1.0)) } else { 1.0 };
        let top = (screen.height() * 0.06 + 40.0 * (1.0 - enter)).max(12.0);
        egui::Area::new(Id::new("setup-sheet")).order(Order::Foreground).fixed_pos(pos2(screen.center().x - w / 2.0, screen.top() + top)).show(ctx, |ui| {
            ui.set_opacity(fade);
            egui::Frame::new()
                .fill(p.bg)
                .corner_radius(radius(30.0))
                .inner_margin(egui::Margin { left: 26, right: 26, top: 24, bottom: 21 })
                .shadow(egui::epaint::Shadow { offset: [0, 24], blur: 80, spread: 0, color: Color32::from_black_alpha(90) })
                .show(ui, |ui| {
                    ui.set_width(w - 52.0);
                    let max_h = screen.height() - top - 60.0;
                    egui::ScrollArea::vertical().max_height(max_h).min_scrolled_height(max_h.min(if narrow { max_h } else { 640.0 })).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.set_max_width(w - 52.0 - if narrow { 0.0 } else { 150.0 });
                                widgets::plain(ui, t, if self.first { "Meet your study buddy" } else { "Change your buddy" }, 1.55, W::Semi, p.text);
                                let intro = if self.intro.is_empty() { String::new() } else { format!("{} ", self.intro) };
                                widgets::muted(ui, t, &format!("{intro}Your buddy cheers you on, gives hints and shows you where answers are in the notes. Pick a look and a name. You can change this any time in Settings."), 1.0);
                            });
                            if !narrow {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                                    if btn("Surprise me").show(ui, t).clicked() {
                                        let r = buddy_art::random_look();
                                        self.name = r.name.clone();
                                        self.update(r, now, sound);
                                    }
                                });
                            }
                        });
                        if narrow && btn("Surprise me").show(ui, t).clicked() {
                            let r = buddy_art::random_look();
                            self.name = r.name.clone();
                            self.update(r, now, sound);
                        }
                        ui.add_space(18.0);
                        let mut preview = |ui: &mut Ui, me: &mut Setup| {
                            let (rect, _) = ui.allocate_exact_size(vec2(250.0_f32.min(ui.available_width()), 280.0), Sense::hover());
                            ui.painter().rect_filled(rect, radius(24.0), p.surface);
                            let stage = Rect::from_center_size(pos2(rect.center().x, rect.top() + 120.0), vec2(200.0, 200.0));
                            me.fig.paint(ui, stage, now, t.motion, 1.0);
                            let st = Style::new(t.px(1.35), p.text, p.muted).weight(W::Bold).center();
                            let l = rich::layout(ui.ctx(), &crate::markup::escape_html(&me.look.name), &st, rect.width());
                            rich::paint(ui.painter(), &l, pos2(rect.left(), rect.bottom() - 48.0), &st);
                        };
                        let mut options = |ui: &mut Ui, me: &mut Setup| {
                            widgets::label(ui, t, "Look");
                            let mut new_look = None;
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                                for (id, lab) in buddy_art::KINDS {
                                    let mut l = me.look.clone();
                                    l.kind = id.into();
                                    l.accessory = "none".into();
                                    if tile(ui, t, &l, lab, me.look.kind == id).clicked() {
                                        let mut nl = me.look.clone();
                                        nl.kind = id.into();
                                        new_look = Some(nl);
                                    }
                                }
                            });
                            ui.add_space(10.0);
                            widgets::label(ui, t, "Colour");
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                                for (id, lab, hex) in buddy_art::COLOURS {
                                    let (r, resp) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::click());
                                    let c = Color32::from_hex(hex).unwrap_or(Color32::GRAY);
                                    let on = me.look.color == id;
                                    ui.painter().circle_filled(r.center(), 18.0, if on { p.accent } else { p.border_strong });
                                    ui.painter().circle_filled(r.center(), if on { 15.0 } else { 16.5 }, p.bg);
                                    ui.painter().circle_filled(r.center(), 13.5, c);
                                    if resp.on_hover_text(lab).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                        let mut nl = me.look.clone();
                                        nl.color = id.into();
                                        new_look = Some(nl);
                                    }
                                }
                            });
                            ui.add_space(10.0);
                            widgets::label(ui, t, "Extra");
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                                for (id, lab) in buddy_art::ACCESSORIES {
                                    let mut l = me.look.clone();
                                    l.accessory = id.into();
                                    if tile(ui, t, &l, lab, me.look.accessory == id).clicked() {
                                        new_look = Some(l);
                                    }
                                }
                            });
                            ui.add_space(10.0);
                            widgets::label(ui, t, "Name");
                            ui.horizontal(|ui| {
                                let r = widgets::text_box(ui, t, Id::new("bs-name"), &mut me.name, "", 220.0, true);
                                if r.changed() {
                                    me.look.name = buddy_art::clean_name(&me.name);
                                    me.fig.look.name = me.look.name.clone();
                                }
                                if btn("Random name").show(ui, t).clicked() {
                                    let options: Vec<&&str> = buddy_art::NAMES.iter().filter(|n| **n != me.look.name).collect();
                                    let n = options[fastrand::usize(0..options.len())].to_string();
                                    me.name = n.clone();
                                    let mut nl = me.look.clone();
                                    nl.name = n;
                                    new_look = Some(nl);
                                }
                            });
                            if let Some(l) = new_look {
                                me.update(l, now, sound);
                            }
                        };
                        if narrow {
                            preview(ui, self);
                            ui.add_space(12.0);
                            options(ui, self);
                        } else {
                            ui.horizontal_top(|ui| {
                                preview(ui, self);
                                ui.add_space(24.0);
                                ui.vertical(|ui| options(ui, self));
                            });
                        }
                        ui.add_space(20.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            if btn("Let's go!").primary().show(ui, t).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                self.look.name = buddy_art::clean_name(if self.name.trim().is_empty() { &self.look.name } else { &self.name });
                                result = SetupResult::Done(self.look.clone());
                            }
                            let other = if self.first { "Skip for now" } else { "Cancel" };
                            if btn(other).show(ui, t).clicked() {
                                result = if self.first {
                                    self.look.name = buddy_art::clean_name(&self.name);
                                    SetupResult::Done(self.look.clone())
                                } else {
                                    SetupResult::Cancelled
                                };
                            }
                        });
                    });
                });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            result = if self.first { SetupResult::Done(self.look.clone()) } else { SetupResult::Cancelled };
        }
        ctx.request_repaint();
        result
    }
}

fn tile(ui: &mut Ui, t: &Theme, look: &BuddyPrefs, label: &str, on: bool) -> egui::Response {
    let p = &t.p;
    let (rect, resp) = ui.allocate_exact_size(vec2(86.0, 86.0), Sense::click());
    let hover = resp.hovered();
    ui.painter().rect(rect, radius(18.0), if on { p.accent_soft } else { p.bg }, Stroke::new(1.5, if on { p.accent } else if hover { p.border_strong } else { p.border }), StrokeKind::Inside);
    let pic = Rect::from_center_size(pos2(rect.center().x, rect.top() + 34.0), vec2(58.0, 58.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(pic));
    still(&mut child, look, 58.0);
    let st = Style::new(t.px(0.78), if on || hover { p.text } else { p.muted }, p.muted).weight(if on { W::Semi } else { W::Regular }).center();
    let l = rich::layout(ui.ctx(), &crate::markup::escape_html(label), &st, rect.width());
    rich::paint(ui.painter(), &l, pos2(rect.left(), rect.bottom() - 22.0), &st);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A button in the speech bubble (.buddy-act): left-aligned, compact.
fn bubble_action(ui: &mut Ui, t: &Theme, html: &str, primary: bool) -> egui::Response {
    let p = &t.p;
    let w = ui.available_width();
    let pad = vec2(t.px(0.75), t.px(0.42));
    let st = Style::new(t.px(0.9), if primary { Color32::WHITE } else { p.text }, p.muted).line(1.35);
    let l = rich::layout(ui.ctx(), html, &st, w - 2.0 * pad.x);
    let (r, resp) = ui.allocate_exact_size(vec2(w, l.size.y + 2.0 * pad.y), Sense::click());
    let hover = resp.hovered();
    let (fill, border) = if primary { (if hover { p.btn_hover } else { p.btn }, if hover { p.btn_hover } else { p.btn }) } else { (p.surface, if hover { p.accent } else { p.border }) };
    ui.painter().rect(r, radius(12.0), fill, Stroke::new(1.0, border), StrokeKind::Inside);
    rich::paint(ui.painter(), &l, r.min + pad, &st);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn unused(_: Pos2) {}
