//! Progress and Settings.

use super::app::{App, DialogAction, Tab, View};
use super::buddy;
use super::buddy_art;
use super::exams::{facts, format_date};
use super::theme::W;
use super::widgets::{self, btn, Kind};
use crate::streak;
use egui::{vec2, Align, Id, Sense, Stroke, Ui};

impl App {
    pub fn view_progress(&mut self, ui: &mut Ui) {
        let t = self.t;
        let p = t.p;
        let g = self.grade_data();
        widgets::page_column(ui, &t, 1040.0, |ui| {
            widgets::h1(ui, &t, &format!("Grade {} progress", g.grade));
            let (mut practised, mut total, mut sum_pct, mut written, mut exam_sum) = (0usize, 0usize, 0i64, 0usize, 0i64);
            let mut rows = Vec::new();
            for s in &g.subjects {
                let st = self.subject_stats(s);
                practised += st.practised;
                total += st.total;
                if let Some(a) = st.avg {
                    sum_pct += a * st.practised as i64;
                }
                for x in &s.exams {
                    for a in self.p.exams.get(&self.tkey(&s.id, &x.id)).map(|v| v.as_slice()).unwrap_or(&[]) {
                        written += 1;
                        exam_sum += a.percent;
                    }
                }
                rows.push((s, st));
            }
            let sn = streak::status(&self.p, streak::today());
            let r = |n: i64, d: usize| crate::marking::js_round(n as f64 / d as f64) as i64;
            facts(ui, &t, &[
                (format!("{practised}/{total}"), "topics practised"),
                (if practised > 0 { format!("{}%", r(sum_pct, practised)) } else { "–".into() }, "average practice score"),
                (written.to_string(), "exams written"),
                (if written > 0 { format!("{}%", r(exam_sum, written)) } else { "–".into() }, "average exam score"),
                (format!("{} day{}", sn.current, if sn.current == 1 { "" } else { "s" }), &format!("current streak (best {})", sn.longest)),
            ]);
            widgets::h2(ui, &t, "By subject");
            let mut go_subject = None;
            let cols: [f32; 5] = if t.narrow { [0.40, 0.15, 0.25, 0.1, 0.1] } else { [0.32, 0.14, 0.28, 0.13, 0.13] };
            let w = ui.available_width();
            let header = ["Subject", "Notes read", "Topics practised", "Average", "Best exam"];
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (i, h) in header.iter().enumerate() {
                    ui.allocate_ui_with_layout(vec2(w * cols[i], 30.0), egui::Layout::left_to_right(Align::Center), |ui| {
                        ui.set_width(w * cols[i]);
                        widgets::plain(ui, &t, h, 0.88, W::Semi, p.text);
                    });
                }
            });
            for (s, st) in &rows {
                let (r, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
                ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, p.border));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.allocate_ui_with_layout(vec2(w * cols[0], 40.0), egui::Layout::left_to_right(Align::Center), |ui| {
                        ui.set_width(w * cols[0]);
                        ui.spacing_mut().item_spacing.x = 8.0;
                        widgets::subject_icon(ui, &t, &s.id, 1.6);
                        if widgets::link(ui, &t, &s.name, t.px(0.95), p.accent).clicked() {
                            go_subject = Some(s.id.clone());
                        }
                    });
                    let cells = [
                        format!("{}/{}", st.notes, st.total),
                        String::new(),
                        st.avg.map(|a| format!("{a}%")).unwrap_or("–".into()),
                        st.best_exam.map(|a| format!("{a}%")).unwrap_or("–".into()),
                    ];
                    for (k, c) in cells.iter().enumerate() {
                        let i = k + 1;
                        ui.allocate_ui_with_layout(vec2(w * cols[i], 40.0), egui::Layout::left_to_right(Align::Center), |ui| {
                            ui.set_width(w * cols[i]);
                            if i == 2 {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                let pc = if st.total > 0 { st.practised as f32 / st.total as f32 * 100.0 } else { 0.0 };
                                widgets::bar(ui, &t, pc, (w * cols[i] - 60.0).max(30.0), None);
                                widgets::plain(ui, &t, &format!("{}/{}", st.practised, st.total), 0.9, W::Regular, p.text);
                            } else {
                                widgets::plain(ui, &t, c, 0.95, W::Regular, p.text);
                            }
                        });
                    }
                });
            }
            if let Some(s) = go_subject {
                self.go(Tab::Study, View::Subject { subject: s });
            }
            widgets::h2(ui, &t, "Recent exams");
            let mut recent = Vec::new();
            for s in &g.subjects {
                for x in &s.exams {
                    for (i, a) in self.p.exams.get(&self.tkey(&s.id, &x.id)).map(|v| v.as_slice()).unwrap_or(&[]).iter().enumerate() {
                        recent.push((s, x, a.clone(), i));
                    }
                }
            }
            recent.sort_by(|a, b| b.2.date.cmp(&a.2.date));
            if recent.is_empty() {
                widgets::muted(ui, &t, "You have not written any exams in this grade yet.", 1.0);
            } else {
                let mut open = None;
                for (s, x, a, i) in recent.iter().take(15) {
                    ui.horizontal(|ui| {
                        let cw = w / 4.0;
                        for (k, c) in [format_date(&a.date), s.name.clone()].iter().enumerate() {
                            let _ = k;
                            ui.allocate_ui_with_layout(vec2(cw, 34.0), egui::Layout::left_to_right(Align::Center), |ui| {
                                ui.set_width(cw);
                                widgets::plain(ui, &t, c, 0.95, W::Regular, p.text);
                            });
                        }
                        ui.allocate_ui_with_layout(vec2(cw, 34.0), egui::Layout::left_to_right(Align::Center), |ui| {
                            ui.set_width(cw);
                            if widgets::link(ui, &t, &x.title, t.px(0.95), p.accent).clicked() {
                                open = Some((s.id.clone(), x.id.clone(), *i));
                            }
                        });
                        widgets::plain(ui, &t, &format!("{}%", a.percent), 0.95, W::Regular, p.text);
                    });
                    let (r, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
                    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, p.border));
                }
                if let Some((s, x, i)) = open {
                    self.go(Tab::Exams, View::Result { subject: s, exam: x, attempt: i });
                }
            }
        });
    }

    pub fn view_settings(&mut self, ui: &mut Ui) {
        let t = self.t;
        let p = t.p;
        widgets::page_column(ui, &t, 820.0, |ui| {
            if widgets::link(ui, &t, "‹ Back", t.px(0.95), p.accent).clicked() {
                self.leave_settings();
                return;
            }
            ui.add_space(t.px(0.5));
            widgets::h1(ui, &t, "Settings");
            let setting = |ui: &mut Ui, label: &str, content: &mut dyn FnMut(&mut Ui)| {
                ui.add_space(t.px(0.5));
                if t.narrow {
                    widgets::plain(ui, &t, label, 1.0, W::Medium, p.text);
                    ui.add_space(4.0);
                    content(ui);
                } else {
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(vec2(t.px(7.5), 30.0), egui::Layout::left_to_right(Align::Center), |ui| {
                            ui.set_width(t.px(7.5));
                            widgets::plain(ui, &t, label, 1.0, W::Medium, p.text);
                        });
                        ui.vertical(|ui| content(ui));
                    });
                }
                ui.add_space(t.px(0.5));
            };
            self.appearance_settings(ui);
            widgets::h2(ui, &t, "Display");
            let sizes = ["small", "normal", "large", "xlarge"];
            let cur = sizes.iter().position(|s| Some(*s) == self.p.settings.text_size.as_deref()).unwrap_or(1);
            setting(ui, "Text size", &mut |ui| {
                if let Some(i) = widgets::segmented(ui, &t, Id::new("ts"), &["Small", "Normal", "Large", "Extra large"], cur, true) {
                    self.sound.play_voices("size", crate::audio::sfx::size(i));
                    self.p.settings.text_size = Some(sizes[i].into());
                    super::images::clear_math();
                    self.save();
                }
            });
            let anim = self.p.settings.animations != Some(false);
            setting(ui, "Animations", &mut |ui| {
                if let Some(i) = widgets::segmented(ui, &t, Id::new("anim"), &["On", "Off"], if anim { 0 } else { 1 }, true) {
                    self.sound.play(if i == 0 { "toggleOn" } else { "toggleOff" });
                    self.p.settings.animations = Some(i == 0);
                    self.save();
                }
                widgets::muted(ui, &t, "Smooth transitions, a lively buddy and moving feedback.", 0.85);
            });
            let spacings = ["compact", "normal", "relaxed"];
            let cur = spacings.iter().position(|s| Some(*s) == self.p.settings.line_spacing.as_deref()).unwrap_or(1);
            setting(ui, "Line spacing", &mut |ui| {
                if let Some(i) = widgets::segmented(ui, &t, Id::new("spacing"), &["Compact", "Normal", "Relaxed"], cur, true) {
                    self.sound.play("select");
                    self.p.settings.line_spacing = Some(spacings[i].into());
                    self.save();
                }
                widgets::muted(ui, &t, "How much room there is between lines in the notes.", 0.85);
            });
            let widths = ["narrow", "normal", "wide"];
            let cur = widths.iter().position(|s| Some(*s) == self.p.settings.reading_width.as_deref()).unwrap_or(1);
            setting(ui, "Reading width", &mut |ui| {
                if let Some(i) = widgets::segmented(ui, &t, Id::new("reading"), &["Narrow", "Normal", "Wide"], cur, true) {
                    self.sound.play("select");
                    self.p.settings.reading_width = Some(widths[i].into());
                    self.save();
                }
            });
            let starts = ["study", "exams", "progress"];
            let cur = starts.iter().position(|s| Some(*s) == self.p.settings.start_tab.as_deref()).unwrap_or(0);
            setting(ui, "Open on", &mut |ui| {
                if let Some(i) = widgets::segmented(ui, &t, Id::new("start"), &["Study", "Exams", "Progress"], cur, true) {
                    self.sound.play("select");
                    self.p.settings.start_tab = Some(starts[i].into());
                    self.save();
                }
            });
            widgets::h2(ui, &t, "Sound");
            let snd = self.p.settings.sounds != Some(false);
            setting(ui, "Sound effects", &mut |ui| {
                if let Some(i) = widgets::segmented(ui, &t, Id::new("snd"), &["On", "Off"], if snd { 0 } else { 1 }, true) {
                    self.p.settings.sounds = Some(i == 0);
                    self.sound.enabled = i == 0;
                    self.sound.play("toggleOn"); // only heard when turning sound on: a preview
                    self.save();
                }
                widgets::muted(ui, &t, "Soft sounds for taps, page changes and answers.", 0.85);
            });
            widgets::h2(ui, &t, "Study buddy");
            let bud = self.p.settings.buddy.clone();
            setting(ui, "Your buddy", &mut |ui| match &bud {
                Some(b) => {
                    ui.horizontal(|ui| {
                        buddy::still(ui, b, 64.0);
                        ui.vertical(|ui| {
                            widgets::plain(ui, &t, &b.name, 1.0, W::Semi, p.text);
                            let acc = if b.accessory != "none" { format!(" · {}", buddy_art::label(&buddy_art::ACCESSORIES, &b.accessory)) } else { String::new() };
                            widgets::muted(ui, &t, &format!("{} · {}{acc}", buddy_art::label(&buddy_art::KINDS, &b.kind), buddy_art::colour_label(&b.color)), 0.85);
                        });
                        if btn("Change look and name").show(ui, &t).clicked() {
                            self.open_setup(false, "");
                        }
                    });
                }
                None => {
                    if btn("Set up your buddy").show(ui, &t).clicked() {
                        self.open_setup(true, "");
                    }
                }
            });
            let on = bud.as_ref().is_some_and(|b| b.on);
            setting(ui, "Buddy", &mut |ui| {
                if let Some(i) = widgets::segmented(ui, &t, Id::new("bud"), &["On", "Off"], if on { 0 } else { 1 }, true) {
                    self.sound.play(if i == 0 { "toggleOn" } else { "toggleOff" });
                    match &mut self.p.settings.buddy {
                        Some(b) => {
                            b.on = i == 0;
                            b.min = false;
                            self.save();
                            self.apply_buddy();
                        }
                        None if i == 0 => self.open_setup(true, ""),
                        None => {}
                    }
                }
                widgets::muted(ui, &t, "Hints on practice questions work either way.", 0.85);
            });
            let quiet = bud.as_ref().is_some_and(|b| b.quiet);
            setting(ui, "Tips", &mut |ui| {
                if let Some(i) = widgets::segmented(ui, &t, Id::new("bq"), &["As I go", "Only when I ask"], if quiet { 1 } else { 0 }, true) {
                    self.sound.play("select");
                    if let Some(b) = &mut self.p.settings.buddy {
                        b.quiet = i == 1;
                        self.save();
                        self.apply_buddy();
                    }
                }
            });
            self.ai_settings(ui);
            widgets::h2(ui, &t, "Your data");
            setting(ui, "Progress", &mut |ui| {
                widgets::wrap(ui, vec2(t.px(0.6), t.px(0.6)), |ui| {
                    if btn("Export progress").show(ui, &t).clicked() {
                        self.export();
                    }
                    if btn("Import progress").show(ui, &t).clicked() {
                        self.import();
                    }
                    if !crate::platform::is_android() && btn("Open data folder").show(ui, &t).clicked() {
                        crate::platform::open_folder(&self.store.dir());
                    }
                    if btn("Reset all progress").kind(Kind::DangerOutline).show(ui, &t).clicked() {
                        self.dialog(
                            "Reset all progress?",
                            vec!["<p>This deletes all notes read, practice answers and exam results for every grade. This cannot be undone. Consider exporting first.</p>".into()],
                            "Reset everything",
                            Some("Cancel"),
                            true,
                            DialogAction::Reset,
                        );
                    }
                });
            });
            let loc = if crate::platform::is_android() { "the FreeSheet app's private storage".to_string() } else { self.store.dir().display().to_string() };
            widgets::muted(ui, &t, &format!("Progress is saved on {} only, in {loc}", crate::platform::device()), 0.85);
            self.update_settings(ui);
        });
    }
}
