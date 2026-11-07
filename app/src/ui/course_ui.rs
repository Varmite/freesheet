//! Language course screens (1.1's course-ui.js): the course path (units and lessons) and
//! the lesson player. The engine is in course.rs.

use super::app::{App, DialogAction, Tab, View};
use super::buddy::Figure;
use super::rich::{self, Style};
use super::theme::{hsl, Theme, W};
use super::widgets::{self, btn, radius, Kind};
use crate::course::{self, CheckResult, Exercise, Model, Response};
use crate::markup::escape_html;
use crate::model::{Course, LessonRecord, Num};
use egui::{pos2, vec2, Align, Color32, Id, Rect, Sense, Shape, Stroke, StrokeKind, Ui};
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

pub struct CourseData {
    pub course: Course,
    pub model: Model,
    pub audio: HashSet<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Af,
    En,
}

pub struct LessonSession {
    pub course: String,
    practice: bool,
    lesson: Option<String>,
    unit: Option<usize>,
    queue: VecDeque<(Exercise, u32)>,
    done: usize,
    mistakes: usize,
    first_try: usize,
    answered: usize,
    pair_misses: usize,
    started: f64,
    current: Option<(Exercise, u32)>,
    feedback: Option<CheckResult>,
    picked: Option<i64>,
    placed: Vec<usize>,
    text: String,
    pair_pick: Option<(Side, usize)>,
    pair_matched: HashSet<usize>,
    pair_miss: Option<(Side, usize, Side, usize, f64)>,
    ex_start: f64,
    puppet: Option<Figure>,
    finished: Option<(String, i64, u32, String)>,
    run: u64,
}

const PRAISE: [&str; 6] = ["Nice!", "Great!", "Correct!", "Lekker!", "Spot on!", "Excellent!"];

impl App {
    pub fn load_course(&mut self, id: &str) -> Option<Arc<CourseData>> {
        if let Some(c) = self.courses.get(id) {
            return Some(c.clone());
        }
        let course = crate::content::course(id).ok()?;
        let model = Model::new(&course);
        let audio = crate::platform::list_audio(&format!("courses/{id}")).into_iter().collect();
        let d = Arc::new(CourseData { course, model, audio });
        self.courses.insert(id.into(), d.clone());
        Some(d)
    }

    fn course_prog(&mut self, id: &str) -> &mut crate::model::CourseProg {
        self.p.courses.entry(id.to_string()).or_default()
    }

    pub fn view_course(&mut self, ui: &mut Ui, cid: &str) {
        let t = self.t;
        let p = t.p;
        let Some(cd) = self.load_course(cid) else { return self.go(Tab::Study, View::Home) };
        let c = &cd.course;
        let prog = self.p.courses.get(cid).cloned().unwrap_or_default();
        let lessons = &cd.model.lessons;
        let done = |id: &str| prog.done.contains_key(id);
        let open = |i: usize| i == 0 || done(&lessons[i - 1].lesson.id) || done(&lessons[i].lesson.id);
        let next = (0..lessons.len()).find(|&i| open(i) && !done(&lessons[i].lesson.id));
        let done_count = lessons.iter().filter(|x| done(&x.lesson.id)).count();
        let today = prog.days.get(&crate::streak::day_key(crate::streak::today())).map_or(0.0, |n| n.0);
        let mut start: Option<String> = None;
        widgets::page_column(ui, &t, 820.0, |ui| {
            self.crumbs(ui, &[(format!("Grade {}", self.grade()), Some((Tab::Study, View::Home))), (c.name.clone(), None)]);
            // The hero box.
            let slot = ui.painter().add(Shape::Noop);
            let r = egui::Frame::new().inner_margin(egui::Margin::symmetric(t.px(1.4) as i8, t.px(1.2) as i8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let main = |ui: &mut Ui, start: &mut Option<String>| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = t.px(0.8);
                        widgets::subject_icon(ui, &t, &c.id, 2.9);
                        widgets::h1(ui, &t, &escape_html(&c.name));
                    });
                    if let Some(l) = &c.level {
                        widgets::plain(ui, &t, l, 1.0, W::Semi, p.muted);
                    }
                    widgets::lead(ui, &t, &escape_html(c.about.as_deref().unwrap_or("")));
                    ui.add_space(t.px(0.6));
                    widgets::wrap(ui, vec2(t.px(0.6), t.px(0.6)), |ui| {
                        if let Some(n) = next {
                            let label = format!("{}: {}", if done_count > 0 { "Continue" } else { "Start" }, lessons[n].lesson.title);
                            if btn(&label).primary().show(ui, &t).clicked() {
                                *start = Some(lessons[n].lesson.id.clone());
                            }
                        }
                        let r = btn("Practise what I've learnt").enabled(done_count > 0).show(ui, &t);
                        if done_count == 0 {
                            r.clone().on_hover_text("Finish a lesson first");
                        }
                        if r.clicked() {
                            *start = Some("_practice".into());
                        }
                    });
                };
                let box_w = t.px(5.4);
                let stats_w = 3.0 * box_w + 2.0 * t.px(0.6);
                let stats = |ui: &mut Ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = t.px(0.6);
                        for (big, small) in [(crate::model::js_num(prog.xp.0), "XP"), (crate::model::js_num(today), "XP today"), (format!("{done_count}/{}", lessons.len()), "lessons")] {
                            let (r, _) = ui.allocate_exact_size(vec2(box_w, t.px(3.6)), Sense::hover());
                            ui.painter().rect_filled(r, radius(16.0), p.bg);
                            let bs = Style::new(t.px(1.3), p.text, p.muted).weight(W::Bold).center().line(1.2);
                            let bl = rich::layout(ui.ctx(), &big, &bs, box_w);
                            let ss = Style::new(t.px(0.78), p.muted, p.muted).center().line(1.3);
                            let sl = rich::layout(ui.ctx(), small, &ss, box_w);
                            let top = r.center().y - (bl.size.y + sl.size.y) / 2.0;
                            rich::paint(ui.painter(), &bl, pos2(r.left(), top), &bs);
                            rich::paint(ui.painter(), &sl, pos2(r.left(), top + bl.size.y), &ss);
                        }
                    });
                };
                if t.narrow {
                    main(ui, &mut start);
                    ui.add_space(t.px(0.6));
                    stats(ui);
                } else {
                    ui.horizontal_top(|ui| {
                        let main_w = ui.available_width() - stats_w - t.px(1.5);
                        ui.allocate_ui_with_layout(vec2(main_w, 0.0), egui::Layout::top_down(Align::Min), |ui| {
                            ui.set_width(main_w);
                            main(ui, &mut start)
                        });
                        ui.add_space(t.px(1.5));
                        stats(ui);
                    });
                }
            });
            ui.painter().set(slot, Shape::rect_filled(r.response.rect, radius(26.0), p.surface));
            ui.add_space(t.px(1.4));
            // Units and their lesson trails.
            for (ui_idx, u) in c.units.iter().enumerate() {
                let hue = u.hue.unwrap_or(200.0) as f32;
                let in_unit: Vec<usize> = (0..lessons.len()).filter(|&i| lessons[i].unit_index == ui_idx).collect();
                let udone = in_unit.iter().filter(|&&i| done(&lessons[i].lesson.id)).count();
                let slot = ui.painter().add(Shape::Noop);
                let r = egui::Frame::new().inner_margin(egui::Margin::symmetric(t.px(1.2) as i8, t.px(0.9) as i8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            rich::plain(ui, &format!("UNIT {}", ui_idx + 1), &Style::new(t.px(0.78), Color32::from_white_alpha(217), p.muted).weight(W::Bold).line(1.3));
                            widgets::plain(ui, &t, &u.title, 1.3, W::Semi, Color32::WHITE);
                            if let Some(e) = &u.english {
                                widgets::plain(ui, &t, e, 0.92, W::Regular, Color32::from_white_alpha(230));
                            }
                        });
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            let s = format!("{udone}/{}", in_unit.len());
                            let st = Style::new(t.px(1.05), Color32::WHITE, p.muted).weight(W::Bold);
                            let l = rich::layout(ui.ctx(), &s, &st, 100.0);
                            let (pr, _) = ui.allocate_exact_size(l.size + vec2(26.0, 10.0), Sense::hover());
                            ui.painter().rect_filled(pr, radius(99.0), Color32::from_white_alpha(46));
                            rich::paint(ui.painter(), &l, pr.min + vec2(13.0, 5.0), &st);
                        });
                    });
                });
                ui.painter().set(slot, Shape::rect_filled(r.response.rect, radius(22.0), if t.dark { hsl(hue, 45.0, 32.0) } else { hsl(hue, 62.0, 44.0) }));
                ui.add_space(t.px(0.6));
                let trail_top = ui.cursor().top();
                let line_slot = ui.painter().add(Shape::Noop);
                let mut last_center = trail_top;
                for &i in &in_unit {
                    let at = &lessons[i];
                    let is_done = done(&at.lesson.id);
                    let is_open = open(i);
                    let cur = next == Some(i);
                    ui.add_space(t.px(0.45));
                    ui.horizontal(|ui| {
                        ui.add_space(t.px(1.6));
                        let (r, resp) = ui.allocate_exact_size(vec2(66.0, 66.0), Sense::click());
                        let hover = resp.hovered() && is_open;
                        let down = resp.is_pointer_button_down_on() && is_open;
                        let (fill, shadow, fg) = if is_done {
                            (hsl(hue, 45.0, 62.0), hsl(hue, 40.0, 46.0), Color32::WHITE)
                        } else if is_open {
                            (hsl(hue, 62.0, 48.0), hsl(hue, 55.0, 34.0), Color32::WHITE)
                        } else {
                            (p.surface2, p.border_strong, p.muted)
                        };
                        let lift = if down { 4.0 } else if hover && t.motion { -2.0 } else { 0.0 };
                        let br = r.translate(vec2(0.0, lift));
                        if cur {
                            let pulse = if t.motion { 4.0 + 2.0 * ((self.now * std::f64::consts::TAU / 2.4).sin() as f32 + 1.0) } else { 4.0 };
                            ui.painter().rect_stroke(br.expand(pulse + 2.0), radius(28.0), Stroke::new(4.0, hsl(hue, 70.0, 70.0).gamma_multiply(0.55)), StrokeKind::Middle);
                            if t.motion {
                                ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
                            }
                        }
                        ui.painter().rect_filled(br.translate(vec2(0.0, if down { 1.0 } else { 5.0 })), radius(24.0), shadow);
                        ui.painter().rect_filled(br, radius(24.0), fill);
                        let icon = if is_done { "check" } else if is_open { "star" } else { "lock" };
                        widgets::paint_icon(ui, icon, Rect::from_center_size(br.center(), vec2(30.0, 30.0)), fg, 2.4);
                        last_center = r.center().y;
                        if resp.on_hover_cursor(if is_open { egui::CursorIcon::PointingHand } else { egui::CursorIcon::NotAllowed }).clicked() {
                            if is_open {
                                start = Some(at.lesson.id.clone());
                            } else {
                                self.sound.play("error");
                                self.toast("Finish the lessons before this one first.");
                            }
                        }
                        ui.add_space(t.px(0.6));
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            widgets::plain(ui, &t, &at.lesson.title, 1.05, W::Semi, if is_open { p.text } else { p.muted });
                            let extra = if is_done {
                                let rec = &prog.done[&at.lesson.id];
                                format!(" · done{} · best {}%", if rec.times > 1 { format!(" ×{}", rec.times) } else { String::new() }, crate::model::js_num(rec.best.0))
                            } else if is_open {
                                String::new()
                            } else {
                                " · locked".into()
                            };
                            widgets::muted(ui, &t, &format!("Lesson {}{extra}", at.index_in_unit + 1), 0.85);
                        });
                        if cur {
                            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                                if self.buddy_look_on() {
                                    let (fr, _) = ui.allocate_exact_size(vec2(70.0, 70.0), Sense::hover());
                                    let look = self.p.settings.buddy.clone().unwrap();
                                    let fig = self.course_fig.get_or_insert_with(|| {
                                        let mut f = Figure::new(look.clone());
                                        f.wave(0.0);
                                        f
                                    });
                                    fig.look = look;
                                    if fig.paint(ui, fr, self.now, t.motion, 1.0) {
                                        ui.ctx().request_repaint();
                                    }
                                }
                                let st = Style::new(t.px(0.85), if t.dark { hsl(hue, 70.0, 72.0) } else { hsl(hue, 60.0, 36.0) }, p.muted).weight(W::Bold);
                                let l = rich::layout(ui.ctx(), "Start here!", &st, 200.0);
                                let (cr, _) = ui.allocate_exact_size(l.size + vec2(22.0, 10.0), Sense::hover());
                                ui.painter().rect(cr, radius(12.0), p.bg, Stroke::new(1.5, hsl(hue, 60.0, 60.0)), StrokeKind::Inside);
                                rich::paint(ui.painter(), &l, cr.min + vec2(11.0, 5.0), &st);
                            });
                        }
                    });
                }
                let x = ui.min_rect().left() + t.px(1.6) + 31.0;
                ui.painter().set(line_slot, Shape::rect_filled(Rect::from_min_max(pos2(x, trail_top + 20.0), pos2(x + 4.0, last_center)), radius(2.0), p.border));
                ui.add_space(t.px(1.6));
            }
        });
        if let Some(id) = start {
            self.go(Tab::Study, View::Lesson { course: cid.into(), lesson: id, run: (self.now * 1000.0) as u64 });
        }
    }

    pub fn buddy_look_on(&self) -> bool {
        self.p.settings.buddy.as_ref().is_some_and(|b| b.on)
    }

    fn start_lesson(&mut self, cid: &str, lid: &str, run: u64) -> bool {
        let Some(cd) = self.load_course(cid) else { return false };
        let practice = lid == "_practice";
        let prog = self.course_prog(cid).clone();
        let at = cd.model.lessons.iter().find(|x| x.lesson.id == lid);
        if !practice && at.is_none() {
            return false;
        }
        let attempt = if practice { prog.practice as usize } else { prog.done.get(lid).map_or(0, |r| r.times as usize) };
        let seen: HashSet<String> = prog.seen.iter().cloned().collect();
        let has = |a: &str| cd.audio.contains(a);
        let list = if practice {
            let done: Vec<String> = prog.done.keys().cloned().collect();
            course::practice_exercises(&cd.model, &done, attempt, &has)
        } else {
            course::lesson_exercises(&cd.model, lid, attempt, &seen, &has)
        };
        let puppet = self.p.settings.buddy.clone().filter(|b| b.on).map(Figure::new);
        self.lesson = Some(LessonSession {
            course: cid.into(),
            practice,
            lesson: at.map(|a| a.lesson.id.clone()),
            unit: at.map(|a| a.unit_index),
            queue: list.into_iter().map(|e| (e, 0)).collect(),
            done: 0,
            mistakes: 0,
            first_try: 0,
            answered: 0,
            pair_misses: 0,
            started: self.now,
            current: None,
            feedback: None,
            picked: None,
            placed: vec![],
            text: String::new(),
            pair_pick: None,
            pair_matched: HashSet::new(),
            pair_miss: None,
            ex_start: self.now,
            puppet,
            finished: None,
            run,
        });
        self.next_exercise();
        true
    }

    fn next_exercise(&mut self) {
        let now = self.now;
        let Some(s) = &mut self.lesson else { return };
        if s.queue.is_empty() {
            return self.finish_lesson();
        }
        s.current = s.queue.front().cloned();
        s.feedback = None;
        s.picked = None;
        s.placed.clear();
        s.text.clear();
        s.pair_pick = None;
        s.pair_matched.clear();
        s.pair_miss = None;
        s.ex_start = now;
        if let Some(f) = &mut s.puppet {
            f.talk(700.0, now);
        }
        if let Some((ex, _)) = &s.current {
            if let Exercise::Listen { audio, .. } | Exercise::Meaning { audio, .. } = ex {
                let a = audio.clone();
                self.play_course_audio(&a);
            }
        }
    }

    fn play_course_audio(&mut self, id: &str) {
        let Some(s) = &self.lesson else { return };
        let cid = s.course.clone();
        let Some(cd) = self.courses.get(&cid).cloned() else { return };
        if !cd.audio.contains(id) {
            return;
        }
        if let Some(pcm) = crate::platform::read_audio(&format!("courses/{cid}"), id).and_then(|b| crate::audio::decode(&b).ok()) {
            self.sound.play_clip(Arc::new(pcm), 1.0);
            let now = self.now;
            if let Some(f) = self.lesson.as_mut().and_then(|s| s.puppet.as_mut()) {
                f.talk(1200.0, now);
            }
        }
    }

    pub fn quit_lesson(&mut self) {
        let left = self.lesson.as_ref().map_or(0, |s| if s.finished.is_some() { 0 } else { s.queue.len() });
        if left > 0 {
            self.dialog("Quit this lesson?", vec!["<p>You'll lose your progress in this lesson.</p>".into()], "Quit", Some("Keep going"), false, DialogAction::QuitLesson);
        } else {
            let id = self.lesson.as_ref().map(|s| s.course.clone());
            self.lesson = None;
            if let Some(id) = id {
                self.go(Tab::Study, View::Course { course: id });
            }
        }
    }

    fn answer(&mut self, result: CheckResult) {
        let now = self.now;
        let Some(s) = &mut self.lesson else { return };
        let Some(item) = s.queue.pop_front() else { return };
        s.answered += 1;
        let ex = item.0.clone();
        let correct = result.correct;
        if correct {
            s.done += 1;
            if item.1 == 0 {
                s.first_try += 1;
            }
        } else {
            s.mistakes += 1;
            let tries = item.1 + 1;
            // Mistakes come back at the end of the lesson (up to twice each).
            if tries <= 2 {
                s.queue.push_back((item.0, tries));
            } else {
                s.done += 1;
            }
        }
        if let Some(f) = &mut s.puppet {
            f.react(if correct { "correct" } else { "wrong" }, now);
        }
        let pairs = matches!(ex, Exercise::Pairs { .. });
        if !pairs {
            s.feedback = Some(result);
        }
        self.sound.play(if correct { "correct" } else { "wrong" });
        let answer_audio = match &ex {
            Exercise::Build { answer_audio, .. } | Exercise::Type { answer_audio, .. } => answer_audio.clone(),
            Exercise::Transform { answer_audio, .. } => Some(answer_audio.clone()),
            _ => None,
        };
        if correct {
            if let Some(a) = answer_audio {
                self.play_course_audio(&a);
            }
        }
        if pairs {
            self.next_exercise();
        }
    }

    fn finish_lesson(&mut self) {
        let now = self.now;
        let Some(s) = &mut self.lesson else { return };
        let secs = (now - s.started).round().max(0.0) as i64;
        let graded = s.answered as f64;
        let accuracy = if graded > 0.0 { (((graded - s.mistakes as f64 - s.pair_misses as f64 * 0.5) / graded) * 100.0).round().max(0.0) as i64 } else { 100 };
        let xp = course::xp_for(s.practice, s.mistakes);
        let (cid, practice, lesson, unit, first_try) = (s.course.clone(), s.practice, s.lesson.clone(), s.unit, s.first_try);
        let day = crate::streak::day_key(crate::streak::today());
        let cd = self.courses.get(&cid).cloned();
        let prog = self.course_prog(&cid);
        prog.xp = Num(prog.xp.0 + xp as f64);
        let d = prog.days.entry(day).or_default();
        d.0 += xp as f64;
        let mut unit_done = false;
        let mut first_time = false;
        if practice {
            prog.practice += 1;
        } else if let (Some(lid), Some(cd)) = (&lesson, &cd) {
            let r = prog.done.entry(lid.clone()).or_insert(LessonRecord::default());
            r.times += 1;
            r.best = Num(r.best.0.max(accuracy as f64));
            r.xp = Num(r.xp.0 + xp as f64);
            first_time = r.times == 1;
            let mut seen: Vec<String> = prog.seen.clone();
            if let Some(at) = cd.model.lessons.iter().find(|x| &x.lesson.id == lid) {
                for w in &at.lesson.words {
                    let k = course::norm(&w.af, "af", None);
                    if !seen.contains(&k) {
                        seen.push(k);
                    }
                }
            }
            prog.seen = seen;
            if let Some(ui) = unit {
                unit_done = cd.course.units[ui].lessons.iter().all(|l| prog.done.contains_key(&l.id));
            }
        }
        self.save();
        self.record_streak("question", (first_try as f64).max(1.0));
        let headline = if practice {
            "Practice complete!".to_string()
        } else if unit_done && first_time {
            format!("Unit complete: {}!", cd.as_ref().and_then(|c| unit.map(|u| c.course.units[u].title.clone())).unwrap_or_default())
        } else {
            "Lesson complete!".into()
        };
        self.sound.stop_clip();
        self.sound.play(if accuracy >= 50 { "complete" } else { "finishSoft" });
        if let Some(s) = &mut self.lesson {
            s.finished = Some((headline, accuracy, xp, format!("{}:{:02}", secs / 60, secs % 60)));
            s.current = None;
            s.ex_start = now;
            if let Some(f) = &mut s.puppet {
                f.react("celebrate", now + 0.2);
            }
        }
    }

    pub fn view_lesson(&mut self, ui: &mut Ui, cid: &str, lid: &str, run: u64) {
        let t = self.t;
        let p = t.p;
        if self.lesson.as_ref().is_none_or(|s| s.run != run || s.course != cid) && !self.start_lesson(cid, lid, run) {
            return self.go(Tab::Study, View::Course { course: cid.into() });
        }
        let now = self.now;
        let avail = ui.available_width();
        let w = avail.min(860.0) - if t.narrow { 32.0 } else { 48.0 };
        let left = (avail - w) / 2.0;
        // Keys: Enter = the main button, 1-6 = options, Backspace = take back a tile.
        let (enter, num, backspace) = ui.input(|i| {
            let n = (1..=6).find(|k| i.key_pressed([egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4, egui::Key::Num5, egui::Key::Num6][k - 1]));
            (i.key_pressed(egui::Key::Enter) && !i.modifiers.shift, n, i.key_pressed(egui::Key::Backspace))
        });
        let typing = ui.ctx().memory(|m| m.focused().is_some());
        ui.horizontal(|ui| {
            ui.add_space(left);
            ui.vertical(|ui| {
                ui.set_width(w);
                // Top: quit and the progress bar.
                ui.add_space(t.px(1.1));
                ui.horizontal(|ui| {
                    if btn("").kind(Kind::Ghost).icon("close").show(ui, &t).on_hover_text("Quit").clicked() {
                        self.quit_lesson();
                    }
                    let s = self.lesson.as_ref().unwrap();
                    let pc = s.done as f32 / (s.done + s.queue.len()).max(1) as f32;
                    let pc = if s.finished.is_some() { 1.0 } else { pc };
                    let shown = ui.ctx().animate_value_with_time(Id::new(("lesson-bar", run)), pc, if t.motion { 0.5 } else { 0.0 });
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 16.0), Sense::hover());
                    ui.painter().rect_filled(r, radius(99.0), p.surface2);
                    if shown > 0.0 {
                        ui.painter().rect_filled(Rect::from_min_size(r.min, vec2((r.width() * shown).max(16.0), 16.0)), radius(99.0), p.good);
                    }
                });
                ui.add_space(t.px(1.4));
                if self.lesson.as_ref().is_some_and(|s| s.finished.is_some()) {
                    self.lesson_done(ui);
                    return;
                }
                self.lesson_stage(ui, now, typing, num, backspace);
            });
        });
        if enter {
            self.lesson_primary();
        }
    }

    fn lesson_done(&mut self, ui: &mut Ui) {
        let t = self.t;
        let p = t.p;
        let now = self.now;
        let s = self.lesson.as_mut().unwrap();
        let (headline, acc, xp, time) = s.finished.clone().unwrap();
        let age = (now - s.ex_start) as f32;
        let cid = s.course.clone();
        let mut leave = false;
        ui.vertical_centered(|ui| {
            ui.add_space(t.px(1.5));
            if let Some(f) = &mut s.puppet {
                let (r, _) = ui.allocate_exact_size(vec2(190.0, 190.0), Sense::hover());
                if f.paint(ui, r, now, t.motion, 1.0) {
                    ui.ctx().request_repaint();
                }
            }
            widgets::plain(ui, &t, &headline, 2.0, W::Semi, p.text);
            widgets::lead(ui, &t, if acc == 100 { "Perfect, not a single mistake!" } else if acc >= 80 { "Great work!" } else { "Nice effort. Practice makes perfect." });
            ui.add_space(t.px(1.2));
            let stats = [("XP", format!("+{xp}"), Color32::from_rgb(0xF5, 0xB4, 0x00)), ("ACCURACY", format!("{acc}%"), p.good), ("TIME", time, p.accent)];
            let total = 3.0 * 120.0 + 2.0 * t.px(0.9);
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                ui.spacing_mut().item_spacing.x = t.px(0.9);
                for (i, (label, val, c)) in stats.iter().enumerate() {
                    let k = if t.motion { ((age - 0.25 - i as f32 * 0.12) / 0.35).clamp(0.0, 1.0) } else { 1.0 };
                    let (r, _) = ui.allocate_exact_size(vec2(120.0, 84.0), Sense::hover());
                    let r = r.translate(vec2(0.0, 10.0 * (1.0 - k)));
                    let op = k;
                    ui.painter().rect(r, radius(18.0), p.bg.gamma_multiply(op), Stroke::new(2.0, c.gamma_multiply(op)), StrokeKind::Inside);
                    let band = Rect::from_min_size(r.min, vec2(r.width(), 24.0));
                    ui.painter().rect_filled(band, egui::CornerRadius { nw: 18, ne: 18, sw: 0, se: 0 }, c.gamma_multiply(op));
                    let ls = Style::new(t.px(0.78), Color32::WHITE.gamma_multiply(op), p.muted).weight(W::Bold).center();
                    let l = rich::layout(ui.ctx(), label, &ls, r.width());
                    rich::paint(ui.painter(), &l, pos2(r.left(), band.center().y - l.size.y / 2.0), &ls);
                    let vs = Style::new(t.px(1.5), p.text.gamma_multiply(op), p.muted).weight(W::Bold).center();
                    let v = rich::layout(ui.ctx(), val, &vs, r.width());
                    rich::paint(ui.painter(), &v, pos2(r.left(), band.bottom() + 12.0), &vs);
                }
            });
            if age < 1.0 && t.motion {
                ui.ctx().request_repaint();
            }
            ui.add_space(t.px(1.8));
            if btn("Continue").primary().big().min_width(150.0).show(ui, &t).clicked() {
                leave = true;
            }
        });
        if leave {
            self.lesson = None;
            self.go(Tab::Study, View::Course { course: cid });
        }
    }

    fn lesson_title(ex: &Exercise) -> String {
        match ex {
            Exercise::Meaning { is_new, .. } => if *is_new { "New word! What does it mean?".into() } else { "What does this word mean?".into() },
            Exercise::Pick { .. } => "What does this mean?".into(),
            Exercise::Build { dir, .. } => if dir == "af-en" { "Translate this sentence".into() } else { "Write this in Afrikaans".into() },
            Exercise::Type { dir, .. } => if dir == "af-en" { "Type this in English".into() } else { "Type this in Afrikaans".into() },
            Exercise::Blank { .. } => "Fill in the missing word".into(),
            Exercise::Listen { .. } => "Tap what you hear".into(),
            Exercise::Transform { task, .. } => task.clone(),
            Exercise::Pairs { .. } => "Tap the matching pairs".into(),
        }
    }

    /// A speech bubble from the buddy (the sentence to work with).
    fn say_line(&mut self, ui: &mut Ui, text: &str, audio: Option<&str>) {
        let t = self.t;
        let p = t.p;
        let now = self.now;
        let has_audio = audio.is_some_and(|a| self.lesson.as_ref().and_then(|s| self.courses.get(&s.course)).is_some_and(|c| c.audio.contains(a)));
        let mut play = false;
        ui.horizontal_top(|ui| {
            let size = if t.narrow { 80.0 } else { 108.0 };
            if let Some(f) = self.lesson.as_mut().and_then(|s| s.puppet.as_mut()) {
                let (r, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
                if f.paint(ui, r, now, t.motion, 1.0) {
                    ui.ctx().request_repaint();
                }
                ui.add_space(t.px(0.9));
            }
            ui.vertical(|ui| {
                ui.add_space((size * 0.25).max(0.0));
                let slot = ui.painter().add(Shape::Noop);
                let r = egui::Frame::new().inner_margin(egui::Margin::symmetric(t.px(1.1) as i8, t.px(0.85) as i8)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if has_audio {
                            let (br, resp) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::click());
                            ui.painter().rect_filled(br, radius(12.0), p.accent);
                            widgets::paint_icon(ui, "speaker", br.shrink(9.0), Color32::WHITE, 1.8);
                            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                play = true;
                            }
                        }
                        ui.set_max_width(ui.available_width().min(560.0));
                        rich::plain(ui, text, &Style::new(t.px(1.25), p.text, p.muted).weight(W::Medium).line(1.45));
                    });
                });
                let rect = r.response.rect;
                ui.painter().set(slot, Shape::Rect(egui::epaint::RectShape::new(rect, radius(20.0), p.bg, Stroke::new(1.5, p.border_strong), StrokeKind::Inside)));
                // The bubble's tail.
                let tip = [pos2(rect.left() + 1.0, rect.bottom() - 30.0), pos2(rect.left() - 9.0, rect.bottom() - 22.0), pos2(rect.left() + 1.0, rect.bottom() - 14.0)];
                ui.painter().add(Shape::convex_polygon(tip.to_vec(), p.bg, Stroke::NONE));
                ui.painter().line_segment([tip[0], tip[1]], Stroke::new(1.5, p.border_strong));
                ui.painter().line_segment([tip[1], tip[2]], Stroke::new(1.5, p.border_strong));
            });
        });
        ui.add_space(t.px(1.4));
        if play {
            if let Some(a) = audio {
                self.play_course_audio(a);
            }
        }
    }

    fn lesson_stage(&mut self, ui: &mut Ui, now: f64, typing: bool, num: Option<usize>, backspace: bool) {
        let t = self.t;
        let p = t.p;
        let Some((ex, _)) = self.lesson.as_ref().and_then(|s| s.current.clone()) else { return };
        let fb = self.lesson.as_ref().is_some_and(|s| s.feedback.is_some());
        let appear = self.lesson.as_ref().map_or(1.0, |s| ((now - s.ex_start) / 0.3) as f32);
        if appear < 1.0 && t.motion {
            ui.set_opacity(appear.max(0.0));
            ui.ctx().request_repaint();
        }
        widgets::plain(ui, &t, &Self::lesson_title(&ex), 1.55, W::Semi, p.text);
        ui.add_space(t.px(1.1));
        let opt_choice = |ui: &mut Ui, app: &mut App, options: &[String], answer: i64, grid: bool, row: bool| {
            let picked = app.lesson.as_ref().and_then(|s| s.picked);
            let mut clicked = None;
            let gap = t.px(0.7);
            let cols = if grid && !t.narrow { 2 } else { 1 };
            let w = if row { 0.0 } else { (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32 };
            let mut draw = |ui: &mut Ui, i: usize| {
                let o = &options[i];
                let state = if fb {
                    if i as i64 == answer { 2 } else if picked == Some(i as i64) { 3 } else { 0 }
                } else if picked == Some(i as i64) {
                    1
                } else {
                    0
                };
                if lesson_option(ui, &t, Id::new(("lopt", i)), Some(i + 1), o, state, !fb, if row { None } else { Some(w) }) {
                    clicked = Some(i);
                }
            };
            if row {
                widgets::wrap(ui, vec2(t.px(0.6), t.px(0.6)), |ui| {
                    for i in 0..options.len() {
                        draw(ui, i);
                    }
                });
            } else {
                for chunk in (0..options.len()).collect::<Vec<_>>().chunks(cols) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        for &i in chunk {
                            draw(ui, i);
                        }
                    });
                    ui.add_space(gap - ui.spacing().item_spacing.y);
                }
            }
            if !fb && !typing {
                if let Some(n) = num.filter(|n| *n <= options.len()) {
                    clicked = Some(n - 1);
                }
            }
            if let Some(i) = clicked {
                app.sound.play("select");
                if let Some(s) = &mut app.lesson {
                    s.picked = Some(i as i64);
                }
            }
        };
        match &ex {
            Exercise::Meaning { is_new, af, options, answer, audio } => {
                if *is_new {
                    let st = Style::new(t.px(0.78), Color32::from_rgb(0x8a, 0x5a, 0x00), p.muted).weight(W::Bold);
                    let l = rich::layout(ui.ctx(), "NEW WORD", &st, 200.0);
                    let (r, _) = ui.allocate_exact_size(l.size + vec2(19.0, 4.0), Sense::hover());
                    ui.painter().rect_filled(r, radius(99.0), Color32::from_rgb(0xff, 0xe7, 0xa3));
                    rich::paint(ui.painter(), &l, r.min + vec2(9.5, 2.0), &st);
                    ui.add_space(t.px(0.6));
                }
                self.say_line(ui, af, Some(audio));
                opt_choice(ui, self, options, *answer, true, false);
            }
            Exercise::Pick { prompt, options, answer, audio } => {
                self.say_line(ui, prompt, Some(audio));
                opt_choice(ui, self, options, *answer, false, false);
            }
            Exercise::Blank { sentence, options, answer, en, .. } => {
                let html = escape_html(sentence).replacen("___", "<span class=\"hint-blank\" aria-label=\"gap\">\u{2003}</span>", 1);
                rich::show(ui, &html, &Style::new(t.px(1.4), p.text, p.muted).weight(W::Medium));
                widgets::muted(ui, &t, en, 1.0);
                ui.add_space(t.px(1.2));
                opt_choice(ui, self, options, *answer, false, true);
            }
            Exercise::Build { prompt, tiles, audio, .. } => {
                self.say_line(ui, prompt, audio.as_deref());
                self.tile_bank(ui, tiles, fb, typing, backspace);
            }
            Exercise::Listen { audio, tiles, .. } => {
                ui.horizontal(|ui| {
                    let (r, resp) = ui.allocate_exact_size(vec2(96.0, 96.0), Sense::click());
                    ui.painter().rect_filled(r, radius(28.0), p.accent);
                    widgets::paint_icon(ui, "speaker", r.shrink(26.0), Color32::WHITE, 1.8);
                    if resp.on_hover_text("Play the sentence").clicked() {
                        let a = audio.clone();
                        self.play_course_audio(&a);
                    }
                    if let Some(f) = self.lesson.as_mut().and_then(|s| s.puppet.as_mut()) {
                        let (fr, _) = ui.allocate_exact_size(vec2(96.0, 96.0), Sense::hover());
                        if f.paint(ui, fr, now, t.motion, 1.0) {
                            ui.ctx().request_repaint();
                        }
                    }
                });
                ui.add_space(t.px(1.4));
                self.tile_bank(ui, tiles, fb, typing, backspace);
            }
            Exercise::Type { prompt, dir, audio, .. } => {
                self.say_line(ui, prompt, audio.as_deref());
                self.text_entry(ui, dir == "en-af", if dir == "af-en" { "Type in English" } else { "Type in Afrikaans" }, fb);
            }
            Exercise::Transform { input, en, prompt, tiles, audio, .. } => {
                if !en.is_empty() {
                    widgets::muted(ui, &t, en, 1.0);
                    ui.add_space(t.px(0.4));
                }
                self.say_line(ui, prompt, Some(audio));
                if input == "tiles" {
                    self.tile_bank(ui, tiles, fb, typing, backspace);
                } else {
                    self.text_entry(ui, true, "Skryf die nuwe sin", fb);
                }
            }
            Exercise::Pairs { pairs, left, right } => {
                let (pick, matched, miss) = {
                    let s = self.lesson.as_ref().unwrap();
                    (s.pair_pick, s.pair_matched.clone(), s.pair_miss)
                };
                let miss = miss.filter(|m| now - m.4 < 0.6);
                if miss.is_some() {
                    ui.ctx().request_repaint();
                }
                let mut tapped: Option<(Side, usize)> = None;
                let half = (ui.available_width() - t.px(1.0)) / 2.0;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = t.px(1.0);
                    for (side, order) in [(Side::Af, left), (Side::En, right)] {
                        ui.allocate_ui_with_layout(vec2(half, 0.0), egui::Layout::top_down(Align::Min), |ui| {
                            ui.set_width(half);
                            for &i in order.iter() {
                                let text = if side == Side::Af { &pairs[i].af } else { &pairs[i].en };
                                let is_matched = matched.contains(&i);
                                let is_picked = pick == Some((side, i));
                                let is_miss = miss.is_some_and(|m| (m.0 == side && m.1 == i) || (m.2 == side && m.3 == i));
                                let state = if is_miss { 3 } else if is_matched { 4 } else if is_picked { 1 } else { 0 };
                                if lesson_option(ui, &t, Id::new(("pair", side == Side::Af, i)), None, text, state, !is_matched, Some(half)) {
                                    tapped = Some((side, i));
                                }
                                ui.add_space(t.px(0.6) - ui.spacing().item_spacing.y);
                            }
                        });
                    }
                });
                if let Some((side, i)) = tapped {
                    let s = self.lesson.as_mut().unwrap();
                    match s.pair_pick {
                        Some((ps, pi)) if ps != side => {
                            s.pair_pick = None;
                            if pi == i {
                                s.pair_matched.insert(i);
                                self.sound.play("select");
                                if s.pair_matched.len() == pairs.len() {
                                    self.answer(CheckResult::right());
                                }
                            } else {
                                s.pair_misses += 1;
                                s.pair_miss = Some((ps, pi, side, i, now));
                                self.sound.play("wrong");
                            }
                        }
                        _ => {
                            s.pair_pick = Some((side, i));
                            self.sound.play("select");
                        }
                    }
                }
            }
        }
    }

    fn tile_bank(&mut self, ui: &mut Ui, tiles: &[String], fb: bool, typing: bool, backspace: bool) {
        let t = self.t;
        let p = t.p;
        let placed = self.lesson.as_ref().unwrap().placed.clone();
        // The answer line.
        let slot = ui.painter().add(Shape::Noop);
        let r = egui::Frame::new().inner_margin(egui::Margin::symmetric(0, t.px(0.6) as i8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(t.px(4.4) - 2.0 * t.px(0.6));
            widgets::wrap(ui, vec2(t.px(0.5), t.px(0.5)), |ui| {
                for (k, &i) in placed.iter().enumerate() {
                    if tile(ui, &t, Id::new(("placed", k)), &tiles[i], false, !fb) {
                        self.sound.play("tap");
                        self.lesson.as_mut().unwrap().placed.remove(k);
                        break;
                    }
                }
            });
        });
        let rect = r.response.rect;
        let mut lines = vec![Shape::hline(rect.x_range(), rect.top(), Stroke::new(2.0, p.border)), Shape::hline(rect.x_range(), rect.bottom(), Stroke::new(2.0, p.border))];
        if rect.height() > t.px(4.0) {
            lines.push(Shape::hline(rect.x_range(), rect.center().y, Stroke::new(2.0, p.border)));
        }
        ui.painter().set(slot, Shape::Vec(lines));
        ui.add_space(t.px(1.4));
        // The bank.
        ui.vertical_centered(|ui| {
            widgets::wrap(ui, vec2(t.px(0.5), t.px(0.5)), |ui| {
                for (i, tl) in tiles.iter().enumerate() {
                    let used = placed.contains(&i);
                    if tile(ui, &t, Id::new(("bank", i)), tl, used, !fb && !used) {
                        self.sound.play("select");
                        self.lesson.as_mut().unwrap().placed.push(i);
                    }
                }
            });
        });
        if backspace && !typing && !fb {
            let s = self.lesson.as_mut().unwrap();
            s.placed.pop();
        }
    }

    fn text_entry(&mut self, ui: &mut Ui, accents: bool, hint: &str, fb: bool) {
        let t = self.t;
        let s = self.lesson.as_mut().unwrap();
        let id = Id::new(("lesson-in", s.run, s.answered));
        let r = egui::TextEdit::multiline(&mut s.text)
            .id(id)
            .hint_text(hint)
            .desired_rows(2)
            .desired_width(f32::INFINITY)
            .font(t.font(1.15, W::Regular))
            .margin(vec2(t.px(1.0), t.px(0.8)))
            .interactive(!fb)
            .show(ui);
        let resp = r.response;
        if !fb && !resp.has_focus() && ui.ctx().memory(|m| m.focused().is_none()) {
            resp.request_focus();
        }
        // Enter checks (a newline is never part of the answer).
        if s.text.contains('\n') {
            s.text = s.text.replace('\n', "");
        }
        if accents && !fb {
            ui.add_space(t.px(0.6));
            let mut insert = None;
            widgets::wrap(ui, vec2(t.px(0.4), t.px(0.4)), |ui| {
                for k in ["ê", "ë", "ô", "é", "û", "'n"] {
                    if tile(ui, &t, Id::new(("acc", k)), k, false, true) {
                        insert = Some(k);
                    }
                }
            });
            if let Some(k) = insert {
                let s = self.lesson.as_mut().unwrap();
                let pos = r.cursor_range.map(|c| c.primary.index.0).unwrap_or(s.text.chars().count());
                let byte = s.text.char_indices().nth(pos).map_or(s.text.len(), |(b, _)| b);
                s.text.insert_str(byte, k);
                resp.request_focus();
                if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), id) {
                    let c = egui::text::CCursor::new(pos + k.chars().count());
                    st.cursor.set_char_range(Some(egui::text::CCursorRange::one(c)));
                    st.store(ui.ctx(), id);
                }
            }
        }
    }

    fn lesson_ready(&self) -> bool {
        let Some(s) = &self.lesson else { return false };
        match s.current.as_ref().map(|c| &c.0) {
            Some(Exercise::Meaning { .. } | Exercise::Pick { .. } | Exercise::Blank { .. }) => s.picked.is_some(),
            Some(Exercise::Build { .. } | Exercise::Listen { .. }) => !s.placed.is_empty(),
            Some(Exercise::Transform { input, .. }) if input == "tiles" => !s.placed.is_empty(),
            Some(Exercise::Type { .. } | Exercise::Transform { .. }) => !s.text.trim().is_empty(),
            _ => false,
        }
    }

    fn lesson_response(&self) -> Response {
        let s = self.lesson.as_ref().unwrap();
        let Some((ex, _)) = &s.current else { return Response::None };
        match ex {
            Exercise::Meaning { .. } | Exercise::Pick { .. } | Exercise::Blank { .. } => s.picked.map_or(Response::None, Response::Option),
            Exercise::Build { tiles, .. } | Exercise::Listen { tiles, .. } => Response::Text(s.placed.iter().map(|&i| tiles[i].clone()).collect::<Vec<_>>().join(" ")),
            Exercise::Transform { input, tiles, .. } if input == "tiles" => Response::Text(s.placed.iter().map(|&i| tiles[i].clone()).collect::<Vec<_>>().join(" ")),
            _ => Response::Text(s.text.clone()),
        }
    }

    fn check_lesson(&mut self) {
        let Some(s) = &self.lesson else { return };
        let Some(cd) = self.courses.get(&s.course).cloned() else { return };
        let Some((ex, _)) = s.current.clone() else { return };
        let r = course::check_exercise(&cd.model, &ex, &self.lesson_response());
        self.answer(r);
    }

    /// Enter: Check or Continue.
    fn lesson_primary(&mut self) {
        let Some(s) = &self.lesson else { return };
        if s.finished.is_some() {
            return;
        }
        if s.feedback.is_some() {
            self.next_exercise();
        } else if self.lesson_ready() {
            self.check_lesson();
        }
    }

    /// The bottom bar: Check, then the verdict with Continue.
    pub fn lesson_foot(&mut self, ui: &mut Ui) {
        let t = self.t;
        let p = t.p;
        let Some(s) = &self.lesson else { return };
        if s.finished.is_some() || s.current.is_none() {
            return;
        }
        let fb = s.feedback.clone();
        let ex = s.current.as_ref().unwrap().0.clone();
        let pairs = matches!(ex, Exercise::Pairs { .. });
        let bg = match &fb {
            Some(r) if r.correct => p.good_soft,
            Some(_) => p.bad_soft,
            None => p.bg,
        };
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, bg);
        if fb.is_none() {
            ui.painter().hline(full.x_range(), full.top(), Stroke::new(2.0, p.border));
        }
        let avail = ui.available_width();
        let w = avail.min(860.0) - if t.narrow { 32.0 } else { 48.0 };
        ui.add_space(t.px(1.0));
        ui.horizontal(|ui| {
            ui.add_space((avail - w) / 2.0);
            ui.allocate_ui_with_layout(vec2(w, 0.0), egui::Layout::left_to_right(Align::Center), |ui| {
                ui.set_width(w);
                match &fb {
                    None if pairs => {
                        widgets::muted(ui, &t, "Tap a word, then its match.", 1.0);
                    }
                    None => {
                        if btn("Skip").show(ui, &t).clicked() {
                            self.answer(CheckResult { skipped: true, ..CheckResult::wrong() });
                        }
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            if btn("Check").primary().big().min_width(150.0).enabled(self.lesson_ready()).show(ui, &t).clicked() {
                                self.check_lesson();
                            }
                        });
                    }
                    Some(r) => {
                        let right = r.correct;
                        let solution = match &ex {
                            Exercise::Meaning { options, answer, .. } | Exercise::Pick { options, answer, .. } => options.get(*answer as usize).cloned().unwrap_or_default(),
                            Exercise::Blank { full, .. } => full.clone(),
                            Exercise::Build { answer, .. } | Exercise::Type { answer, .. } | Exercise::Listen { answer, .. } | Exercise::Transform { answer, .. } => answer.clone(),
                            Exercise::Pairs { .. } => String::new(),
                        };
                        let msg = if right {
                            let k = self.lesson.as_ref().map_or(0, |s| s.answered) % PRAISE.len();
                            PRAISE[k].to_string()
                        } else if r.skipped {
                            "Skipped. Here's the answer:".into()
                        } else {
                            "Not quite. The answer is:".into()
                        };
                        let extra = if right && r.note == Some("typo") {
                            Some(format!("You have a typo: <strong>{}</strong>", escape_html(r.matched.as_deref().unwrap_or(""))))
                        } else if right && r.note == Some("accent") {
                            Some(format!("Watch the accents: <strong>{}</strong>", escape_html(r.matched.as_deref().unwrap_or(""))))
                        } else if right && r.matched.as_ref().is_some_and(|m| !solution.is_empty() && course::norm(m, "x", None) != course::norm(&solution, "x", None)) {
                            Some(format!("Also correct: <strong>{}</strong>", escape_html(&solution)))
                        } else {
                            None
                        };
                        let color = if right { p.good } else { p.bad };
                        ui.allocate_ui_with_layout(vec2((w - 170.0).max(120.0), 0.0), egui::Layout::left_to_right(Align::Center), |ui| {
                            let (ir, _) = ui.allocate_exact_size(vec2(52.0, 52.0), Sense::hover());
                            ui.painter().circle_filled(ir.center(), 26.0, p.bg);
                            widgets::paint_icon(ui, if right { "check" } else { "close" }, ir.shrink(12.0), color, 2.6);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                widgets::plain(ui, &t, &msg, 1.3, W::Bold, color);
                                if let Some(e) = extra {
                                    rich::show(ui, &e, &Style::new(t.px(0.98), p.text, p.muted));
                                } else if !right {
                                    widgets::plain(ui, &t, &solution, 1.05, W::Regular, p.bad);
                                }
                            });
                        });
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            if btn("Continue").kind(if right { Kind::Good } else { Kind::Bad }).big().min_width(150.0).show(ui, &t).clicked() {
                                self.next_exercise();
                            }
                        });
                    }
                }
            });
        });
        ui.add_space(t.px(1.2));
    }
}

/// A lesson option (.lopt). state: 0 plain, 1 selected, 2 right, 3 wrong, 4 matched.
fn lesson_option(ui: &mut Ui, t: &Theme, id: Id, kbd: Option<usize>, text: &str, state: u8, enabled: bool, width: Option<f32>) -> bool {
    let p = &t.p;
    let (border, bg) = match state {
        1 => (p.accent, p.accent_soft),
        2 => (p.good, p.good_soft),
        3 => (p.bad, p.bad_soft),
        4 => (p.good, p.bg),
        _ => (p.border, p.bg),
    };
    let st = Style::new(t.px(1.05), p.text, p.muted).line(1.4);
    let pad = vec2(t.px(1.0), t.px(0.85));
    let kw = if kbd.is_some() { t.px(1.6) + t.px(0.8) } else { 0.0 };
    let max_text = width.unwrap_or(400.0) - 2.0 * pad.x - kw;
    let laid = rich::layout(ui.ctx(), &escape_html(text), &if kbd.is_none() { st.center() } else { st }, max_text.max(40.0));
    let w = width.unwrap_or(laid.size.x + 2.0 * pad.x + kw).max(if width.is_none() { t.px(8.0) } else { 0.0 });
    let h = laid.size.y.max(t.px(1.6)) + 2.0 * pad.y;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h + 3.0), if enabled { Sense::click() } else { Sense::hover() });
    let resp = ui.interact(rect, id, if enabled { Sense::click() } else { Sense::hover() });
    let down = resp.is_pointer_button_down_on() && enabled;
    let r = Rect::from_min_size(rect.min + vec2(0.0, if down { 3.0 } else { 0.0 }), vec2(w, h));
    let alpha = if state == 4 { 0.4 } else { 1.0 };
    if !down {
        ui.painter().rect_filled(r.translate(vec2(0.0, 3.0)), radius(16.0), border.gamma_multiply(alpha));
    }
    let fill = if resp.hovered() && enabled && state == 0 { p.surface } else { bg };
    ui.painter().rect(r, radius(16.0), fill.gamma_multiply(alpha), Stroke::new(2.0, border.gamma_multiply(alpha)), StrokeKind::Inside);
    let mut x = r.left() + pad.x;
    if let Some(k) = kbd {
        let kr = Rect::from_min_size(pos2(x, r.center().y - t.px(0.8)), vec2(t.px(1.6), t.px(1.6)));
        ui.painter().rect(kr, radius(8.0), Color32::TRANSPARENT, Stroke::new(1.5, p.border_strong), StrokeKind::Inside);
        let ks = Style::new(t.px(0.8), p.muted, p.muted).center();
        let kl = rich::layout(ui.ctx(), &k.to_string(), &ks, kr.width());
        rich::paint(ui.painter(), &kl, pos2(kr.left(), kr.center().y - kl.size.y / 2.0), &ks);
        x += kw;
    } else {
        x = r.left() + (w - laid.size.x) / 2.0;
    }
    let mut s2 = if kbd.is_none() { st.center() } else { st };
    s2.color = p.text.gamma_multiply(alpha);
    if kbd.is_none() {
        s2.align = Align::Min;
    }
    rich::paint(ui.painter(), &laid, pos2(x, r.center().y - laid.size.y / 2.0), &s2);
    resp.on_hover_cursor(if enabled { egui::CursorIcon::PointingHand } else { egui::CursorIcon::Default }).clicked() && enabled
}

/// A word tile (.tile). `used` tiles are blank placeholders.
fn tile(ui: &mut Ui, t: &Theme, id: Id, text: &str, used: bool, enabled: bool) -> bool {
    let p = &t.p;
    let st = Style::new(t.px(1.05), if used { Color32::TRANSPARENT } else { p.text }, p.muted).line(1.3);
    let laid = rich::layout(ui.ctx(), &escape_html(text), &st, 400.0);
    let pad = vec2(t.px(0.85), t.px(0.5));
    let size = laid.size + 2.0 * pad;
    let (rect, _) = ui.allocate_exact_size(size + vec2(0.0, 3.0), Sense::hover());
    let resp = ui.interact(rect, id, if enabled { Sense::click() } else { Sense::hover() });
    let down = resp.is_pointer_button_down_on() && enabled;
    let r = Rect::from_min_size(rect.min + vec2(0.0, if down { 3.0 } else { 0.0 }), size);
    if used {
        ui.painter().rect_filled(Rect::from_min_size(rect.min, size), radius(13.0), p.surface2);
        return false;
    }
    if !down {
        ui.painter().rect_filled(r.translate(vec2(0.0, 3.0)), radius(13.0), p.border);
    }
    ui.painter().rect(r, radius(13.0), if resp.hovered() && enabled { p.surface } else { p.bg }, Stroke::new(2.0, p.border), StrokeKind::Inside);
    rich::paint(ui.painter(), &laid, r.min + pad, &st);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && enabled
}
