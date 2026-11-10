//! Study: the subject grid, a subject's topics, and a topic's notes (revealed one section
//! at a time, with read-aloud and read-along) and practice questions.

use super::app::{pct, App, Delayed, Mode, Tab, View};
use super::blocks::{self, Collect};
use super::buddy::{Action, Go, Priority};
use super::buddy_art::Mood;
use crate::ai::Target;
use super::questions::{self, Ai, Card, Event};
use super::rich::{self, Style};
use super::theme::{self, W};
use super::widgets::{self, btn, radius};
use crate::audio::readalong::{self, Tok, Unit};
use crate::markup;
use crate::marking;
use crate::model::{Num, Topic};
use egui::{pos2, vec2, Align, Color32, Id, Rect, Sense, Shape, Stroke, StrokeKind, Ui};
use std::sync::Arc;

pub struct Score {
    pub score: f64,
    pub max: f64,
    pub done: usize,
    pub pending: usize,
    pub total: usize,
    pub complete: bool,
}

impl App {
    pub fn topic_score(&self, topic: &Topic, key: &str) -> Score {
        let tp = self.topic_prog(key);
        let (mut score, mut max, mut done, mut pending) = (0.0, 0.0, 0, 0);
        for q in &topic.questions {
            max += marking::marks_of(q);
            if let Some(tp) = tp {
                if tp.checked.get(&q.id).copied().unwrap_or(false) {
                    let r = marking::mark(q, tp.responses.get(&q.id), tp.self_marks.get(&q.id).copied().flatten().map(|n| n.0));
                    if r.needs_self_mark {
                        pending += 1;
                    } else {
                        done += 1;
                        score += r.score;
                    }
                }
            }
        }
        Score { score, max, done, pending, total: topic.questions.len(), complete: done == topic.questions.len() }
    }

    fn topic_status(&self, sid: &str, tid: &str) -> (String, bool, bool) {
        let Some(tp) = self.topic_prog(&self.tkey(sid, tid)) else { return ("Not started".into(), false, false) };
        if let (Some(b), Some(bm)) = (tp.best, tp.best_max) {
            if bm.0 > 0.0 {
                return (format!("Best {}%", pct(b.0, bm.0)), true, true);
            }
        }
        let checked = tp.checked.len();
        if checked > 0 {
            return (format!("{checked} answered"), false, true);
        }
        if tp.notes_read {
            return ("Notes read".into(), false, true);
        }
        ("Not started".into(), false, false)
    }

    // ---------------------------------------------------------------- home

    pub fn view_study_home(&mut self, ui: &mut Ui) {
        let t = self.t;
        let p = t.p;
        let g = self.grade_data();
        widgets::page_column(ui, &t, 1040.0, |ui| {
            widgets::h1(ui, &t, &format!("Grade {} subjects", g.grade));
            widgets::lead(ui, &t, "Choose a subject to read the notes and practise questions on each topic.");
            // Continue where you left off.
            if let Some(last) = self.p.settings.last_topic.clone() {
                if last.grade == self.grade() {
                    if let Some(s) = self.subject(&last.subject_id) {
                        if let Some(tm) = s.topics.iter().find(|x| x.id == last.topic_id) {
                            ui.add_space(t.px(0.5));
                            let slot = ui.painter().add(Shape::Noop);
                            let r = ui.scope_builder(egui::UiBuilder::new().sense(Sense::click()), |ui| {
                                egui::Frame::new().inner_margin(egui::Margin::symmetric(t.px(1.1) as i8, t.px(0.9) as i8)).show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    widgets::muted(ui, &t, "Continue where you left off", 0.85);
                                    widgets::plain(ui, &t, &tm.title, 1.05, W::Semi, p.text);
                                    widgets::muted(ui, &t, &s.name, 1.0);
                                });
                            });
                            let hover = r.response.hovered();
                            ui.painter().set(slot, Shape::Rect(egui::epaint::RectShape::new(r.response.rect, radius(18.0), p.surface, Stroke::new(1.0, if hover { p.border_strong } else { p.border }), StrokeKind::Inside)));
                            if r.response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                let mode = if last.mode.as_deref() == Some("questions") { Mode::Questions } else { Mode::Notes };
                                self.go(Tab::Study, View::Topic { subject: last.subject_id.clone(), topic: last.topic_id.clone(), mode });
                            }
                        }
                    }
                }
            }
            ui.add_space(t.px(1.25));
            // The grid of subjects (and language courses).
            let courses: Vec<_> = crate::content::catalog().courses.iter().filter(|c| c.grade.is_none_or(|cg| cg == self.grade())).collect();
            let n = g.subjects.len() + courses.len();
            let gap = t.px(0.9);
            let cols = ((ui.available_width() + gap) / (230.0 + gap)).floor().max(1.0) as usize;
            let w = (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32;
            let mut clicked: Option<View> = None;
            for row in 0..n.div_ceil(cols) {
                let rid = Id::new(("study-row", row, cols));
                let min_h = widgets::row_height(ui, rid);
                let mut row_max: f32 = 0.0;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for k in 0..cols {
                        let i = row * cols + k;
                        if i >= n {
                            break;
                        }
                        if i < g.subjects.len() {
                            let s = &g.subjects[i];
                            let st = self.subject_stats(s);
                            let pc = if st.total > 0 { (st.practised as f32 / st.total as f32) * 100.0 } else { 0.0 };
                            let (r, _, nh) = widgets::card(ui, &t, w, min_h, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = t.px(0.75);
                                    widgets::subject_icon(ui, &t, &s.id, 2.4);
                                    ui.vertical(|ui| {
                                        ui.add_space(4.0);
                                        widgets::plain(ui, &t, &s.name, 1.05, W::Semi, p.text);
                                    });
                                });
                                widgets::muted(ui, &t, &super::app::plural(st.total, "topic", "topics"), 0.85);
                                widgets::bar(ui, &t, pc, ui.available_width(), None);
                                let avg = st.avg.map(|a| format!(" · avg {a}%")).unwrap_or_default();
                                widgets::muted(ui, &t, &format!("{} of {} practised{avg}", st.practised, st.total), 0.85);
                            });
                            row_max = row_max.max(nh);
                            if r.clicked() {
                                clicked = Some(View::Subject { subject: s.id.clone() });
                            }
                        } else {
                            let c = courses[i - g.subjects.len()];
                            let prog = self.p.courses.get(&c.id);
                            let done = prog.map_or(0, |p| p.done.len());
                            let pc = if c.lessons > 0 { done as f32 / c.lessons as f32 * 100.0 } else { 0.0 };
                            let (r, _, nh) = widgets::card(ui, &t, w, min_h, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = t.px(0.75);
                                    widgets::subject_icon(ui, &t, &c.id, 2.4);
                                    ui.vertical(|ui| {
                                        ui.add_space(4.0);
                                        widgets::plain(ui, &t, &c.name, 1.05, W::Semi, p.text);
                                    });
                                });
                                let lvl = if c.level.is_empty() { String::new() } else { format!("{} · ", c.level) };
                                widgets::muted(ui, &t, &format!("{lvl}{} lessons", c.lessons), 0.85);
                                widgets::bar(ui, &t, pc, ui.available_width(), None);
                                let xp = prog.filter(|p| p.xp.0 > 0.0).map(|p| format!(" · {} XP", crate::model::js_num(p.xp.0))).unwrap_or_default();
                                widgets::muted(ui, &t, &format!("{done} of {} lessons{xp}", c.lessons), 0.85);
                            });
                            row_max = row_max.max(nh);
                            if r.clicked() {
                                clicked = Some(View::Course { course: c.id.clone() });
                            }
                        }
                    }
                });
                widgets::set_row_height(ui, rid, row_max);
                ui.add_space(gap - ui.spacing().item_spacing.y);
            }
            if g.subjects.is_empty() {
                widgets::muted(ui, &t, "No subjects are available for this grade yet.", 1.0);
            }
            if let Some(v) = clicked {
                self.go(Tab::Study, v);
            }
        });
    }

    // ---------------------------------------------------------------- subject

    pub fn view_subject(&mut self, ui: &mut Ui, sid: &str) {
        let t = self.t;
        let p = t.p;
        let Some(s) = self.subject(sid) else { return self.go(Tab::Study, View::Home) };
        let st = self.subject_stats(s);
        widgets::page_column(ui, &t, 1040.0, |ui| {
            self.crumbs(ui, &[(format!("Grade {}", self.grade()), Some((Tab::Study, View::Home))), (s.name.clone(), None)]);
            let head = |ui: &mut Ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = t.px(0.8);
                    widgets::subject_icon(ui, &t, &s.id, 2.9);
                    ui.vertical(|ui| {
                        ui.add_space(6.0);
                        widgets::h1(ui, &t, &markup::escape_html(&s.name));
                    });
                });
                widgets::lead(ui, &t, &markup::inline(&s.description));
            };
            let stats = |ui: &mut Ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = t.px(1.25);
                    for (big, small) in [(format!("{}/{}", st.practised, st.total), "practised"), (st.avg.map(|a| format!("{a}%")).unwrap_or("–".into()), "average")] {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            widgets::plain(ui, &t, &big, 1.2, W::Bold, p.text);
                            widgets::muted(ui, &t, small, 0.82);
                        });
                    }
                });
            };
            if t.narrow {
                head(ui);
                stats(ui);
            } else {
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(vec2(ui.available_width() - 200.0, 0.0), egui::Layout::top_down(Align::Min), head);
                    ui.with_layout(egui::Layout::right_to_left(Align::Min), stats);
                });
            }
            ui.add_space(t.px(0.8));
            let r = widgets::text_box(ui, &t, Id::new("filter"), &mut self.filter, "Find a topic", 360f32.min(ui.available_width()), true);
            let _ = r;
            let q = self.filter.trim().to_lowercase();
            let mut any = false;
            let mut clicked = None;
            for term in 1..=4 {
                let topics: Vec<_> = s.topics.iter().filter(|x| x.term == term && (q.is_empty() || x.title.to_lowercase().contains(&q))).collect();
                if topics.is_empty() {
                    continue;
                }
                any = true;
                ui.add_space(t.px(1.5));
                rich::plain(ui, &format!("TERM {term}"), &Style::new(t.px(0.85), p.muted, p.muted).weight(W::Semi).line(1.3));
                ui.add_space(t.px(0.5));
                widgets::rows(ui, &t, |ui| {
                    for (i, tm) in topics.iter().enumerate() {
                        let (label, strong, text) = self.topic_status(&s.id, &tm.id);
                        if widgets::row(ui, &t, &tm.title, None, &label, strong, text, i == 0).clicked() {
                            clicked = Some(tm.id.clone());
                        }
                    }
                });
            }
            if !any {
                widgets::muted(ui, &t, "No topics match your search.", 1.0);
            }
            if !s.exams.is_empty() {
                ui.add_space(t.px(2.0));
                if btn(&format!("Practise {} exams →", s.name)).show(ui, &t).clicked() {
                    self.go(Tab::Exams, View::List { subject: s.id.clone() });
                }
            }
            if let Some(tid) = clicked {
                self.filter.clear();
                self.go(Tab::Study, View::Topic { subject: s.id.clone(), topic: tid, mode: Mode::Notes });
            }
        });
    }

    // ---------------------------------------------------------------- topic

    pub fn set_topic_mode(&mut self, mode: Mode) {
        if let View::Topic { subject, topic, .. } = self.view() {
            self.stop_reader();
            self.go(Tab::Study, View::Topic { subject, topic, mode });
        }
    }

    pub fn show_in_notes(&mut self, sec: usize) {
        if let View::Topic { subject, topic, .. } = self.view() {
            self.go(Tab::Study, View::Topic { subject, topic, mode: Mode::Notes });
            let need = sec + 1;
            if self.notes.shown < need {
                self.notes.shown = need;
            }
            self.notes.focus = Some((sec, self.now + 0.35));
            self.later(0.05, Delayed::ScrollToSection(sec));
        }
    }

    pub fn view_topic(&mut self, ui: &mut Ui, sid: &str, tid: &str, mode: Mode) {
        let t = self.t;
        let p = t.p;
        let Some(s) = self.subject(sid) else { return self.go(Tab::Study, View::Home) };
        let Some(meta) = s.topics.iter().find(|x| x.id == tid) else { return self.go(Tab::Study, View::Home) };
        let Some(td) = self.load_topic(sid, tid) else {
            widgets::page_column(ui, &t, 820.0, |ui| widgets::h1(ui, &t, "This topic could not be loaded"));
            return;
        };
        let key = self.tkey(sid, tid);
        let mode_str = if mode == Mode::Questions { "questions" } else { "notes" };
        let lt = crate::model::LastTopic { grade: self.grade(), subject_id: sid.into(), topic_id: tid.into(), mode: Some(mode_str.into()) };
        let same = self.p.settings.last_topic.as_ref().is_some_and(|l| l.grade == lt.grade && l.subject_id == lt.subject_id && l.topic_id == lt.topic_id && l.mode == lt.mode);
        if !same {
            self.p.settings.last_topic = Some(lt);
            self.save();
        }
        let idx = s.topics.iter().position(|x| x.id == tid).unwrap_or(0);
        let prev = idx.checked_sub(1).and_then(|i| s.topics.get(i));
        let next = s.topics.get(idx + 1);
        widgets::page_column(ui, &t, t.reading, |ui| {
            self.crumbs(
                ui,
                &[
                    (format!("Grade {}", self.grade()), Some((Tab::Study, View::Home))),
                    (s.name.clone(), Some((Tab::Study, View::Subject { subject: sid.into() }))),
                    (meta.title.clone(), None),
                ],
            );
            widgets::h1(ui, &t, &markup::escape_html(&td.topic.title));
            widgets::lead(ui, &t, &markup::inline(&td.topic.summary));
            ui.add_space(t.px(0.6));
            let qn = td.topic.questions.len();
            if let Some(i) = widgets::segmented(ui, &t, Id::new("topic-mode"), &["Notes", &format!("Questions ({qn})")], if mode == Mode::Notes { 0 } else { 1 }, false) {
                self.set_topic_mode(if i == 0 { Mode::Notes } else { Mode::Questions });
            }
            ui.add_space(t.px(1.0));
            if mode == Mode::Notes {
                self.notes_body(ui, sid, &td, &key);
            } else {
                self.practice_body(ui, &td, &key);
            }
            ui.add_space(t.px(2.0));
            let mut target = None;
            ui.columns(2, |cols| {
                let half = cols[0].available_width();
                if let Some(pv) = prev {
                    if btn(&format!("← {}", pv.title)).show_clipped(&mut cols[0], &t, half).clicked() {
                        target = Some(pv.id.clone());
                    }
                }
                if let Some(nx) = next {
                    cols[1].with_layout(egui::Layout::right_to_left(Align::Min), |ui| {
                        if btn(&format!("{} →", nx.title)).show_clipped(ui, &t, half).clicked() {
                            target = Some(nx.id.clone());
                        }
                    });
                }
            });
            if let Some(tid) = target {
                self.go(Tab::Study, View::Topic { subject: sid.into(), topic: tid, mode: Mode::Notes });
            }
            let _ = p;
        });
    }

    // ---------------------------------------------------------------- notes

    fn notes_body(&mut self, ui: &mut Ui, sid: &str, td: &Arc<super::app::TopicData>, key: &str) {
        let t = self.t;
        let p = t.p;
        let total = td.sections.len();
        if self.notes.key != key {
            let seen = self.topic_prog(key).and_then(|tp| tp.sections_seen).unwrap_or(1) as usize;
            self.notes = super::app::NotesState { key: key.into(), shown: seen.clamp(1, total.max(1)), ..Default::default() };
        }
        let has_audio = td.sections.iter().any(|s| td.audio.contains(&s.id));
        // The reader bar.
        if has_audio {
            let slot = ui.painter().add(Shape::Noop);
            let r = egui::Frame::new().inner_margin(egui::Margin::symmetric(t.px(0.75) as i8, t.px(0.5) as i8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = t.px(0.6);
                    let active = self.notes.reader.index.is_some();
                    let (label, icon) = if !active { ("Listen to these notes", "speaker") } else if self.notes.reader.paused { ("Resume", "play") } else { ("Pause", "pause") };
                    if btn(label).icon(icon).show(ui, &t).clicked() {
                        if !active {
                            self.sound.play("playStart");
                            self.start_reading(0);
                        } else if self.notes.reader.paused {
                            self.sound.play("playStart");
                            self.sound.pause_clip(false);
                            self.notes.reader.paused = false;
                            self.buddy.listen(true, self.now);
                        } else {
                            self.sound.play("playPause");
                            self.sound.pause_clip(true);
                            self.notes.reader.paused = true;
                            self.buddy.listen(false, self.now);
                        }
                    }
                    if active && btn("").icon("stop").show(ui, &t).on_hover_text("Stop reading").clicked() {
                        self.sound.play("playStop");
                        self.stop_reader();
                    }
                    widgets::plain(ui, &t, "Speed", 0.9, W::Regular, p.muted);
                    let speeds = [0.75, 1.0, 1.25, 1.5];
                    let cur_speed = self.p.settings.read_speed.map_or(1.0, |n| n.0);
                    let cur = speeds.iter().position(|s| (*s - cur_speed).abs() < 0.01).unwrap_or(1);
                    let labels: Vec<String> = speeds.iter().map(|s| format!("{}×", crate::model::js_num(*s))).collect();
                    if let Some(i) = widgets::select(ui, &t, Id::new("read-speed"), cur, &labels, 70.0, true) {
                        self.p.settings.read_speed = Some(Num(speeds[i]));
                        self.save();
                        if let Some(pcm) = self.notes.reader.pcm.clone() {
                            self.sound.set_clip_speed(&pcm, speeds[i] as f32);
                        }
                    }
                    let status = if let Some(i) = self.notes.reader.index { format!("Reading section {} of {total}", i + 1) } else { self.notes.reader.status.clone() };
                    widgets::muted(ui, &t, &status, 0.85);
                });
            });
            ui.painter().set(slot, Shape::Rect(egui::epaint::RectShape::new(r.response.rect, radius(18.0), p.surface, Stroke::new(1.0, p.border), StrokeKind::Inside)));
            ui.add_space(t.px(0.5));
        }
        // The sections shown so far.
        let shown = self.notes.shown.min(total);
        let mut play_section = None;
        let mut section_rects = Vec::new();
        for i in 0..shown {
            let sec = &td.sections[i];
            if i > 0 {
                ui.add_space(t.px(1.25));
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, p.border));
                ui.add_space(t.px(0.6));
            }
            let speaking = self.notes.reader.index == Some(i);
            let bg_slot = ui.painter().add(Shape::Noop);
            let hl_slot = ui.painter().add(Shape::Noop);
            let mut col = Collect::new(speaking);
            // Fade in a section that has just been revealed.
            let reveal = self.notes.revealed.filter(|(j, _)| *j == i).map(|(_, at)| ((self.now - at) / 0.35) as f32);
            let r = ui.scope(|ui| {
                if let Some(k) = reveal {
                    if k < 1.0 && t.motion {
                        ui.set_opacity(k.max(0.0));
                        ui.ctx().request_repaint();
                    }
                }
                let has = td.audio.contains(&sec.id);
                let top = ui.cursor().min;
                for (bi, b) in sec.blocks.iter().enumerate() {
                    blocks::block(ui, &t, b, bi, bi == 0, &mut col);
                }
                if has {
                    let br = Rect::from_min_size(pos2(top.x + ui.available_width() - 34.0, top.y + t.px(0.9) - 6.0), vec2(32.0, 32.0));
                    let resp = ui.interact(br, Id::new(("play", i)), Sense::click());
                    ui.painter().rect(br, radius(13.0), p.bg, Stroke::new(1.0, if resp.hovered() { p.accent } else { p.border }), StrokeKind::Inside);
                    widgets::paint_icon(ui, "speaker", br.shrink(6.0), if resp.hovered() { p.accent } else { p.muted }, 1.8);
                    if resp.on_hover_text("Listen to this section").on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        play_section = Some(i);
                    }
                }
            });
            let rect = r.response.rect;
            section_rects.push(rect);
            // Speaking: tinted; or flashing after "show me in the notes".
            let mut tint = if speaking { 1.0 } else { 0.0 };
            if let Some((f, at)) = self.notes.focus {
                if f == i && self.now >= at {
                    let k = ((self.now - at) / 2.6) as f32;
                    if k < 1.0 {
                        tint = if k < 0.35 { 1.0 } else { 1.0 - (k - 0.35) / 0.65 };
                        ui.ctx().request_repaint();
                    } else {
                        self.notes.focus = None;
                    }
                }
            }
            let tint = ui.ctx().animate_value_with_time(Id::new(("tint", i)), tint, if t.motion { 0.35 } else { 0.0 });
            if tint > 0.0 {
                ui.painter().set(bg_slot, Shape::rect_filled(rect.expand(t.px(0.6)), radius(18.0), theme::with_alpha(p.accent_soft, tint)));
            }
            if speaking {
                self.paint_read_along(ui, td, i, &col, hl_slot);
            }
            if self.notes.scroll_to == Some(i) {
                self.notes.scroll_to = None;
                self.scroll_rect = Some((rect.expand(t.px(1.0)), Align::TOP));
            }
        }
        if let Some(i) = play_section {
            self.sound.play("playStart");
            self.start_reading(i);
        }
        // Continue / the end.
        if shown < total {
            ui.add_space(t.px(2.0));
            let slot = ui.painter().add(Shape::Noop);
            let r = egui::Frame::new().inner_margin(egui::Margin::same(t.px(1.1) as i8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| {
                    widgets::muted(ui, &t, &format!("Section {shown} of {total}"), 0.9);
                    ui.add_space(t.px(0.5));
                    widgets::bar(ui, &t, shown as f32 / total as f32 * 100.0, 320f32.min(ui.available_width()), Some(p.accent));
                    ui.add_space(t.px(0.9));
                    let title = &td.sections[shown].title;
                    let label = if title.is_empty() { "Continue →".to_string() } else { format!("Continue: {} →", markup::strip_tags(&markup::inline(title))) };
                    if btn(&label).primary().show(ui, &t).clicked() {
                        self.sound.play("reveal");
                        self.reveal(shown + 1, td, key);
                        self.notes.scroll_to = Some(shown - 1);
                    }
                });
            });
            let rect = r.response.rect;
            ui.painter().set(slot, dashed_rect(rect, p.border_strong));
        } else {
            ui.add_space(t.px(2.5));
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, p.border));
            ui.add_space(t.px(1.25));
            let read = self.topic_prog(key).is_some_and(|tp| tp.notes_read);
            widgets::muted(ui, &t, if read { "You have marked these notes as read." } else { "You have reached the end of the notes. Test yourself on what you have just studied." }, 1.0);
            ui.add_space(t.px(0.6));
            if btn(&format!("Practise {} questions →", td.topic.questions.len())).primary().show(ui, &t).clicked() {
                self.stop_reader();
                self.topic_prog_mut(key).notes_read = true;
                self.save();
                self.set_topic_mode(Mode::Questions);
            }
        }
        let _ = sid;
    }

    fn reveal(&mut self, n: usize, td: &Arc<super::app::TopicData>, key: &str) {
        let total = td.sections.len();
        if n <= self.notes.shown {
            return;
        }
        self.notes.shown = n.min(total);
        self.notes.revealed = Some((self.notes.shown - 1, self.now));
        let shown = self.notes.shown as u32;
        let tp = self.topic_prog_mut(key);
        tp.sections_seen = Some(tp.sections_seen.unwrap_or(0).max(shown));
        self.save();
        if self.notes.shown == total && self.notes.reader.index.is_none() {
            let title = td.topic.title.clone();
            self.tell(&format!("notesdone:{key}"), &format!("That's all the notes for **{title}**! Ready to test yourself?"), Priority::Tip, Some(Mood::Happy), vec![Action::new("Start the questions", Go::StartQuestions)]);
        }
    }

    /// Highlight the sentence and word being read.
    fn paint_read_along(&mut self, ui: &mut Ui, td: &Arc<super::app::TopicData>, i: usize, col: &Collect, slot: egui::layers::ShapeIdx) {
        let t = self.t;
        let p = t.p;
        let words: Vec<&(Rect, String, Vec<String>, usize)> = col.blocks.iter().flat_map(|b| b.words.iter()).collect();
        // Plan the timing once the audio's pauses are known.
        if self.notes.reader.plan.as_ref().is_none_or(|pl| pl.len() != words.len()) {
            if let Some(segs) = &self.notes.reader.segs {
                let sec = &td.sections[i];
                let units: Vec<Unit> = col
                    .blocks
                    .iter()
                    .map(|b| {
                        let speech = crate::sections::block_speech(&sec.blocks[b.block]);
                        let toks = b
                            .words
                            .iter()
                            .map(|(_, text, math, line)| {
                                let (w, stop, pause) = readalong::word_weight(text, math);
                                Tok { weight: w, line: (b.block, *line), stop, pause, t: 0.0, sentence: 0 }
                            })
                            .collect();
                        Unit { toks, chars: speech.chars().count().max(1) }
                    })
                    .collect();
                self.notes.reader.plan = readalong::plan(units, segs);
            }
        }
        let Some(plan) = &self.notes.reader.plan else { return };
        let Some((_, pos, _, _)) = self.sound.clip_state() else { return };
        let Some(cur) = readalong::current(plan, pos as f32) else { return };
        if cur >= words.len() {
            return;
        }
        let sentence = plan[cur].sentence;
        let mut shapes = Vec::new();
        // The sentence: one tinted band per line.
        let mut lines: Vec<(usize, Rect)> = Vec::new();
        for (k, w) in words.iter().enumerate() {
            if plan[k].sentence != sentence {
                continue;
            }
            match lines.iter_mut().find(|(l, _)| *l == w.3) {
                Some((_, r)) => *r = r.union(w.0),
                None => lines.push((w.3, w.0)),
            }
        }
        for (_, r) in &lines {
            shapes.push(Shape::rect_filled(r.expand2(vec2(2.0, 1.0)), radius(5.0), theme::with_alpha(p.accent, 0.14)));
        }
        // The word marker glides from word to word.
        let wr = words[cur].0;
        let dur = if t.motion { 0.11 } else { 0.0 };
        let ctx = ui.ctx();
        let x = ctx.animate_value_with_time(Id::new("rw-x"), wr.left() - 3.0, dur);
        let y = ctx.animate_value_with_time(Id::new("rw-y"), wr.top() - 1.0, dur);
        let w = ctx.animate_value_with_time(Id::new("rw-w"), wr.width() + 6.0, dur);
        let h = ctx.animate_value_with_time(Id::new("rw-h"), wr.height() + 2.0, dur);
        shapes.push(Shape::rect_filled(Rect::from_min_size(pos2(x, y), vec2(w, h)), radius(7.0), theme::with_alpha(p.accent, if t.dark { 0.45 } else { 0.34 })));
        ui.painter().set(slot, Shape::Vec(shapes));
        // Keep the word in view (unless the learner scrolled away just now).
        if self.now - self.notes.reader.user_scrolled > 4.0 {
            let top = self.viewport_top + 120.0;
            let bottom = ui.ctx().content_rect().bottom() - 90.0;
            if wr.top() < top || wr.bottom() > bottom {
                self.scroll_rect = Some((wr, Align::Center));
            }
        }
        ui.ctx().request_repaint();
    }

    // ---------------------------------------------------------------- read aloud

    pub fn start_reading(&mut self, from: usize) {
        let View::Topic { subject, topic, .. } = self.view() else { return };
        let Some(td) = self.topics.get(&self.tkey(&subject, &topic)).cloned() else { return };
        self.sound.stop_clip();
        let mut i = from;
        while i < td.sections.len() && !td.audio.contains(&td.sections[i].id) {
            i += 1;
        }
        if i >= td.sections.len() {
            self.stop_reader();
            return;
        }
        let key = self.tkey(&subject, &topic);
        self.reveal(i + 1, &td, &key);
        self.notes.scroll_to = Some(i);
        let folder = format!("grade-{}/{subject}/{topic}", self.grade());
        let r = &mut self.notes.reader;
        r.index = Some(i);
        r.paused = false;
        r.plan = None;
        r.segs = None;
        r.status.clear();
        let pcm = crate::platform::read_audio(&folder, &td.sections[i].id).and_then(|b| crate::audio::decode(&b).ok());
        match pcm {
            Some(pcm) => {
                let pcm = Arc::new(pcm);
                self.notes.reader.segs = Some(readalong::analyse(&pcm, crate::audio::CLIP_RATE));
                let speed = self.p.settings.read_speed.map_or(1.0, |n| n.0) as f32;
                self.notes.reader.clip = self.sound.play_clip(pcm.clone(), speed);
                self.notes.reader.pcm = Some(pcm);
                if self.notes.reader.clip.is_none() {
                    self.stop_reader();
                    self.notes.reader.status = "This audio could not be played.".into();
                    return;
                }
                self.buddy.listen(true, self.now);
            }
            None => {
                self.stop_reader();
                self.notes.reader.status = "This audio could not be played.".into();
            }
        }
    }

    pub fn stop_reader(&mut self) {
        if self.notes.reader.index.is_some() || self.notes.reader.clip.is_some() {
            self.sound.stop_clip();
        }
        let status = std::mem::take(&mut self.notes.reader.status);
        self.notes.reader = super::app::Reader { status, ..Default::default() };
        self.buddy.listen(false, self.now);
    }

    pub fn tick_reader(&mut self, ctx: &egui::Context) {
        let Some(i) = self.notes.reader.index else { return };
        match self.sound.clip_state() {
            Some((id, _, done, _)) if Some(id) == self.notes.reader.clip => {
                if done {
                    self.start_reading(i + 1);
                } else {
                    ctx.request_repaint();
                }
            }
            _ => self.stop_reader(),
        }
    }

    // ---------------------------------------------------------------- practice

    pub fn score_bar(&mut self, ui: &mut Ui, sid: &str, tid: &str) {
        let t = self.t;
        let p = t.p;
        let Some(td) = self.load_topic(sid, tid) else { return };
        let key = self.tkey(sid, tid);
        let sc = self.topic_score(&td.topic, &key);
        let best = self.topic_prog(&key).and_then(|tp| tp.best.zip(tp.best_max)).map(|(b, m)| pct(b.0, m.0));
        let avail = ui.available_width();
        let pad = if t.narrow { t.px(1.0) } else { t.px(1.5) };
        let w = (avail - 2.0 * pad).min(t.reading - 2.0 * pad);
        let left = (avail - w) / 2.0;
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(left);
            ui.allocate_ui_with_layout(vec2(w, 0.0), egui::Layout::left_to_right(Align::Center).with_main_wrap(true), |ui| {
                ui.spacing_mut().item_spacing = vec2(t.px(1.25), 4.0);
                rich::show(ui, &format!("<strong>{}</strong> / {} marks", marking::format_marks(sc.score), crate::model::js_num(sc.max)), &Style::new(t.px(1.0), p.text, p.muted).line(1.4));
                let pend = if sc.pending > 0 { format!(" · {} to self-mark", sc.pending) } else { String::new() };
                widgets::muted(ui, &t, &format!("{} of {} marked{pend}", sc.done, sc.total), 1.0);
                if let Some(b) = best {
                    widgets::muted(ui, &t, &format!("Best: {b}%"), 1.0);
                }
            });
        });
        ui.add_space(6.0);
        let r = ui.max_rect();
        ui.painter().hline(r.x_range(), ui.cursor().top(), Stroke::new(1.0, p.border));
    }

    pub(super) fn record_if_complete(&mut self, topic: &Topic, key: &str) {
        let sc = self.topic_score(topic, key);
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let tp = self.topic_prog_mut(key);
        if sc.complete && !tp.recorded {
            tp.recorded = true;
            tp.attempts += 1;
            tp.last = Some(Num(sc.score));
            tp.last_max = Some(Num(sc.max));
            tp.completed_at = Some(now);
            let better = match (tp.best, tp.best_max) {
                (Some(b), Some(bm)) => pct(sc.score, sc.max) > pct(b.0, bm.0),
                _ => true,
            };
            if better {
                tp.best = Some(Num(sc.score));
                tp.best_max = Some(Num(sc.max));
            }
        }
    }

    pub fn check_all(&mut self, key: &str) {
        let View::Topic { subject, topic, .. } = self.view() else { return };
        let Some(td) = self.load_topic(&subject, &topic) else { return };
        let mut answered = 0.0;
        let mut n = 0;
        let mut right = 0;
        let mut wrong = 0;
        for q in &td.topic.questions {
            let tp = self.topic_prog(key);
            if tp.is_some_and(|tp| tp.checked.get(&q.id).copied().unwrap_or(false)) {
                continue;
            }
            let resp = tp.and_then(|tp| tp.responses.get(&q.id)).cloned();
            if marking::is_answered(q, resp.as_ref()) {
                answered += 1.0;
            }
            self.topic_prog_mut(key).checked.insert(q.id.clone(), true);
            let r = marking::mark(q, resp.as_ref(), None);
            let card_key = format!("{key}#{}", q.id);
            let delay = (n.min(12) as f64) * 0.03;
            if r.correct.is_some() {
                self.flashes.insert(card_key, (self.now + delay, r.correct == Some(true)));
            }
            if n < 8 {
                let s = if r.needs_self_mark { "reveal" } else if r.correct == Some(true) { "correct" } else if r.score > 0.0 { "partial" } else { "wrong" };
                self.later(n as f64 * 0.09, Delayed::Sound(s));
            }
            if r.correct == Some(true) {
                right += 1;
            } else if r.correct == Some(false) {
                wrong += 1;
            }
            n += 1;
        }
        self.record_streak("question", answered);
        self.record_if_complete(&td.topic, key);
        self.save();
        if right + wrong > 0 {
            let react = if wrong == 0 { "celebrate" } else if right >= wrong { "correct" } else { "wrong" };
            self.later(0.5, Delayed::BuddyReact(react));
            let text = if wrong == 0 { format!("All {right} right! Brilliant.") } else { format!("{right} right and {wrong} to look at again.") };
            let actions = if wrong > 0 { vec![Action::new("Show me the first one", Go::ShowFirstWrong)] } else { vec![] };
            self.later(0.5, Delayed::BuddySay(text, Priority::Tip, None, actions));
        }
    }

    pub fn show_first_wrong(&mut self) {
        if let Some(q) = self.first_wrong.clone() {
            if let Some(r) = self.card_rects.get(&q).copied() {
                self.scroll_rect = Some((r, Align::Center));
                self.flashes.insert(q, (self.now + 0.35, false));
            }
        }
    }

    pub fn buddy_hint(&mut self) {
        // The first question that still has a hint to give.
        let View::Topic { subject, topic, .. } = self.view() else { return };
        let Some(td) = self.load_topic(&subject, &topic) else { return };
        let key = self.tkey(&subject, &topic);
        let target = td.topic.questions.iter().enumerate().find(|(i, q)| {
            let tp = self.topic_prog(&key);
            let checked = tp.is_some_and(|tp| tp.checked.get(&q.id).copied().unwrap_or(false));
            let level = tp.and_then(|tp| tp.hints.get(&q.id)).copied().unwrap_or(0) as usize;
            !checked && level < td.hints[*i].len()
        });
        match target {
            None => {
                let now = self.now;
                self.buddy.say("No more hints on these. Give it your best shot. You can do it!", Priority::User, None, vec![], now, &mut self.sound);
            }
            Some((i, q)) => {
                let level = self.topic_prog(&key).and_then(|tp| tp.hints.get(&q.id)).copied().unwrap_or(0) + 1;
                let qid = q.id.clone();
                self.use_hint(&key, &qid, level, td.hints[i].len());
                let ck = format!("{key}#{qid}");
                if let Some(r) = self.card_rects.get(&ck).copied() {
                    self.scroll_rect = Some((r, Align::Center));
                }
            }
        }
    }

    fn use_hint(&mut self, key: &str, qid: &str, level: u32, max: usize) {
        let level = level.min(max as u32);
        let tp = self.topic_prog_mut(key);
        tp.hints.insert(qid.to_string(), level);
        self.save();
        self.sound.play("hint");
        let now = self.now;
        self.buddy.react("hint", now, &mut self.sound);
        if level == 1 {
            let line = super::app_buddy::HINT_LINES[fastrand::usize(0..2)];
            self.tell("hint-howto", line, Priority::Reaction, Some(Mood::Think), vec![]);
        }
    }

    fn practice_body(&mut self, ui: &mut Ui, td: &Arc<super::app::TopicData>, key: &str) {
        let t = self.t;
        let p = t.p;
        // Mark the notes read; forget answers to questions that no longer exist.
        {
            let ids: std::collections::HashSet<&str> = td.topic.questions.iter().map(|q| q.id.as_str()).collect();
            let tp = self.topic_prog_mut(key);
            let mut changed = !tp.notes_read;
            tp.notes_read = true;
            let before = tp.responses.len() + tp.checked.len() + tp.self_marks.len() + tp.hints.len();
            tp.responses.retain(|k, _| ids.contains(k.as_str()));
            tp.checked.retain(|k, _| ids.contains(k.as_str()));
            tp.self_marks.retain(|k, _| ids.contains(k.as_str()));
            tp.hints.retain(|k, _| ids.contains(k.as_str()));
            changed |= before != tp.responses.len() + tp.checked.len() + tp.self_marks.len() + tp.hints.len();
            if changed {
                self.save();
            }
        }
        let was_complete = self.topic_score(&td.topic, key).complete;
        let mut first_wrong = None;
        for (i, q) in td.topic.questions.iter().enumerate() {
            let tp = self.p.topics.get(key).cloned().unwrap_or_default();
            let checked = tp.checked.get(&q.id).copied().unwrap_or(false);
            let ck = format!("{key}#{}", q.id);
            let level = tp.hints.get(&q.id).copied().unwrap_or(0);
            let flash = self.flashes.get(&ck).copied();
            let ai = self.ai_card(&ck, q, tp.responses.get(&q.id));
            let ai_can_mark = !matches!(ai, Ai::Off);
            let card = Card {
                key: ck.clone(),
                number: format!("Question {}", i + 1),
                q,
                response: tp.responses.get(&q.id),
                feedback: checked,
                self_mark: tp.self_marks.get(&q.id).copied().flatten().map(|n| n.0),
                allow_check: true,
                can_self_mark: true,
                hints: Some((&td.hints[i], level)),
                ai,
            };
            let (mut events, rect) = questions::card(ui, &t, &mut self.inputs, flash, self.now, card);
            self.card_rects.insert(ck.clone(), rect);
            if self.auto_check == Some(i) {
                self.auto_check = None;
                events.push(Event::Check);
            }
            if checked && first_wrong.is_none() {
                let r = marking::mark(q, tp.responses.get(&q.id), tp.self_marks.get(&q.id).copied().flatten().map(|n| n.0));
                if r.correct == Some(false) || (r.correct.is_some() && r.score < r.max && !r.needs_self_mark) {
                    first_wrong = Some(ck.clone());
                }
            }
            for e in events {
                match e {
                    Event::Response(v) => {
                        if matches!(q.kind.as_str(), "mcq" | "truefalse" | "match") {
                            self.sound.play("select");
                        }
                        // A newly ruled-out option can't stay selected (handled by hints), store.
                        self.topic_prog_mut(key).responses.insert(q.id.clone(), v);
                        self.save();
                    }
                    Event::Check => {
                        let resp = self.topic_prog(key).and_then(|tp| tp.responses.get(&q.id)).cloned();
                        if !checked && marking::is_answered(q, resp.as_ref()) {
                            self.record_streak("question", 1.0);
                        }
                        self.topic_prog_mut(key).checked.insert(q.id.clone(), true);
                        let r = marking::mark(q, resp.as_ref(), None);
                        if r.correct.is_some() {
                            self.flashes.insert(ck.clone(), (self.now, r.correct == Some(true)));
                        }
                        let s = if r.needs_self_mark { "reveal" } else if r.correct == Some(true) { "correct" } else if r.score > 0.0 { "partial" } else { "wrong" };
                        self.sound.play(s);
                        let partial = r.correct == Some(false) && r.score > 0.0;
                        self.buddy_answer(if r.needs_self_mark { None } else { r.correct }, partial, td.related[i]);
                        self.record_if_complete(&td.topic, key);
                        self.save();
                        if let (true, View::Topic { subject, topic, .. }) = (ai_can_mark, self.view()) {
                            let target = Target::Topic { subject, topic, key: key.to_string(), question: q.id.clone() };
                            self.ai_start(&ck, target, q, resp.as_ref());
                        }
                    }
                    Event::AiManual => self.ai_forget(&ck),
                    Event::SelfMark(n) => {
                        self.ai.forget(&ck);
                        if let Some(v) = n {
                            let (_, voices) = crate::audio::sfx::mark(v, marking::marks_of(q));
                            self.sound.play_voices("mark", voices);
                        }
                        self.topic_prog_mut(key).self_marks.insert(q.id.clone(), n.map(Num));
                        self.record_if_complete(&td.topic, key);
                        self.save();
                    }
                    Event::Hint(level) => {
                        // A newly ruled-out option can't stay selected.
                        let out: Vec<usize> = td.hints[i].iter().take(level as usize).flat_map(|h| h.eliminate.iter().copied()).collect();
                        let sel = self.topic_prog(key).and_then(|tp| tp.responses.get(&q.id)).and_then(|v| v.as_i64());
                        if q.kind == "mcq" && sel.is_some_and(|s| out.contains(&(s as usize))) {
                            self.topic_prog_mut(key).responses.shift_remove(&q.id);
                        }
                        self.use_hint(key, &q.id, level, td.hints[i].len());
                    }
                    Event::ShowNotes(sec) => self.show_in_notes(sec),
                }
            }
        }
        self.first_wrong = first_wrong;
        // The end of the worksheet.
        let sc = self.topic_score(&td.topic, key);
        ui.add_space(t.px(1.5));
        if sc.complete {
            let pc = pct(sc.score, sc.max);
            let (lvl, label) = marking::caps_level(pc as f64);
            if !was_complete || self.completion_seen.as_deref() != Some(key) {
                if !was_complete {
                    self.later(0.38, Delayed::Sound(if pc >= 50 { "complete" } else { "finishSoft" }));
                    self.completion_at = Some(self.now);
                    self.topic_done_buddy(td, key, pc, lvl);
                }
                self.completion_seen = Some(key.to_string());
            }
            let pop = self.completion_at.map(|a| ((self.now - a) / 0.5) as f32).filter(|k| *k < 1.0 && t.motion);
            let slot = ui.painter().add(Shape::Noop);
            let r = ui.scope(|ui| {
                if let Some(k) = pop {
                    ui.set_opacity(k.min(1.0));
                    ui.ctx().request_repaint();
                }
                egui::Frame::new().inner_margin(egui::Margin::same(t.px(1.5) as i8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical_centered(|ui| {
                        widgets::plain(ui, &t, &format!("{pc}%"), 2.6, W::Bold, p.text);
                        widgets::plain(ui, &t, &format!("{} / {} marks · Level {lvl}: {label}", marking::format_marks(sc.score), crate::model::js_num(sc.max)), 1.0, W::Regular, p.text);
                        widgets::muted(ui, &t, if pc >= 80 { "Excellent work." } else if pc >= 50 { "Good effort. Review the notes for the questions you got wrong, then try again." } else { "Go back over the notes and try again." }, 1.0);
                        ui.add_space(t.px(0.8));
                        ui.horizontal(|ui| {
                            let w = 260.0;
                            ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                            if btn("Back to notes").show(ui, &t).clicked() {
                                self.set_topic_mode(Mode::Notes);
                            }
                            if btn("Try again").primary().show(ui, &t).clicked() {
                                let tp = self.topic_prog_mut(key);
                                tp.responses.clear();
                                tp.checked.clear();
                                tp.self_marks.clear();
                                tp.hints.clear();
                                tp.recorded = false;
                                let prefix = format!("{key}#");
                                self.inputs.retain(|k, _| !k.starts_with(&prefix));
                                self.completion_seen = None;
                                self.save();
                                self.scroll_rect = Some((Rect::from_min_size(pos2(0.0, -10000.0), vec2(1.0, 1.0)), Align::TOP));
                            }
                        });
                    });
                });
            });
            ui.painter().set(slot, Shape::Rect(egui::epaint::RectShape::new(r.response.rect, radius(18.0), p.surface, Stroke::new(1.0, p.border), StrokeKind::Inside)));
        } else {
            let unchecked = td.topic.questions.iter().filter(|q| !self.topic_prog(key).is_some_and(|tp| tp.checked.get(&q.id).copied().unwrap_or(false))).count();
            if unchecked > 0 {
                ui.with_layout(egui::Layout::right_to_left(Align::Min), |ui| {
                    if btn("Check all answers").primary().show(ui, &t).clicked() {
                        let unanswered = td
                            .topic
                            .questions
                            .iter()
                            .filter(|q| {
                                let tp = self.topic_prog(key);
                                !tp.is_some_and(|tp| tp.checked.get(&q.id).copied().unwrap_or(false)) && !marking::is_answered(q, tp.and_then(|tp| tp.responses.get(&q.id)))
                            })
                            .count();
                        if unanswered > 0 {
                            self.dialog(
                                "Check all answers?",
                                vec![format!("<p>{unanswered} question{} not answered yet and will be marked wrong.</p>", if unanswered == 1 { " is" } else { "s are" })],
                                "Check all",
                                Some("Cancel"),
                                false,
                                super::app::DialogAction::CheckAll { key: key.to_string() },
                            );
                        } else {
                            self.check_all(key);
                        }
                    }
                });
            } else if sc.pending > 0 {
                ui.vertical_centered(|ui| {
                    widgets::muted(ui, &t, "Award yourself marks for the written answers above to finish.", 1.0);
                });
            }
        }
    }

    fn topic_done_buddy(&mut self, td: &Arc<super::app::TopicData>, key: &str, pc: i64, lvl: u8) {
        self.later(0.45, Delayed::BuddyReact(if pc >= 50 { "celebrate" } else { "wrong" }));
        let tp = self.topic_prog(key).cloned().unwrap_or_default();
        let wrong = td.topic.questions.iter().enumerate().find(|(_, q)| tp.checked.get(&q.id).copied().unwrap_or(false) && marking::mark(q, tp.responses.get(&q.id), tp.self_marks.get(&q.id).copied().flatten().map(|n| n.0)).correct != Some(true));
        let sec = wrong.and_then(|(i, _)| td.related[i]);
        let (text, mood, actions) = if pc >= 80 {
            (format!("**{pc}%**: outstanding! That's level {lvl}."), Mood::Happy, vec![])
        } else if pc >= 50 {
            (format!("**{pc}%**: good work! Let's look at the ones you missed."), Mood::Happy, vec![Action::new("Show me the first one", Go::ShowFirstWrong)])
        } else {
            (
                format!("**{pc}%**. This topic is a tricky one. Going over the notes again will help."),
                Mood::Sad,
                vec![Action::new(if sec.is_some() { "Show me what to read" } else { "Back to the notes" }, Go::ShowInNotes(sec.unwrap_or(0)))],
            )
        };
        self.later(0.45, Delayed::BuddySay(text, Priority::User, Some(mood), actions));
    }
}

/// A rounded rectangle with a dashed border (.notes-next).
fn dashed_rect(r: Rect, c: Color32) -> Shape {
    let mut shapes = Vec::new();
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    for w in pts.windows(2) {
        shapes.extend(Shape::dashed_line(&[w[0], w[1]], Stroke::new(1.0, c), 5.0, 4.0));
    }
    Shape::Vec(shapes)
}

impl<'a> widgets::Btn<'a> {
    /// A button no wider than `max_w` (long labels are cut short).
    pub fn show_clipped(self, ui: &mut Ui, t: &theme::Theme, max_w: f32) -> egui::Response {
        ui.allocate_ui_with_layout(vec2(max_w, 0.0), egui::Layout::left_to_right(Align::Min), |ui| {
            ui.set_max_width(max_w);
            self.show(ui, t)
        })
        .inner
    }
}

pub fn unused(_: Color32) {}
