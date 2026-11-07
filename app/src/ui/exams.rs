//! Exams: full papers under timed conditions, then marked with the memo.

use super::app::{pct, App, DialogAction, ExamSession, Tab, View};
use super::blocks;
use super::questions::{self, Card, Event};
use super::rich::{self, Style};
use super::theme::W;
use super::widgets::{self, btn, radius};
use crate::markup;
use crate::marking;
use crate::model::{Attempt, Draft, Num, SectionScore};
use egui::{vec2, Align, Id, Sense, Shape, Stroke, StrokeKind, Ui};

pub fn format_duration(sec: f64) -> String {
    let sec = sec.max(0.0).round() as i64;
    let (h, m, s) = (sec / 3600, (sec % 3600) / 60, sec % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
}

pub fn format_date(iso: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(iso) {
        Ok(d) => {
            let d = d.with_timezone(&chrono::Local);
            use chrono::Datelike;
            let months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
            format!("{} {} {}", d.day(), months[d.month0() as usize], d.year())
        }
        Err(_) => iso.to_string(),
    }
}

impl App {
    fn exam_crumbs(&mut self, ui: &mut Ui, sid: &str, sname: &str, title: &str, last: Option<&str>, eid: &str) {
        let g = self.grade();
        let mut items = vec![(format!("Grade {g} exams"), Some((Tab::Exams, View::Home))), (sname.to_string(), Some((Tab::Exams, View::List { subject: sid.into() })))];
        match last {
            Some(l) => {
                items.push((title.to_string(), Some((Tab::Exams, View::Intro { subject: sid.into(), exam: eid.into() }))));
                items.truncate(2);
                items.push((l.to_string(), None));
            }
            None => items.push((title.to_string(), None)),
        }
        self.crumbs(ui, &items);
    }

    fn drafts_list(&mut self, ui: &mut Ui) {
        let t = self.t;
        let prefix = format!("{}/", self.grade());
        let drafts: Vec<String> = self.p.exam_drafts.keys().filter(|k| k.starts_with(&prefix)).cloned().collect();
        if drafts.is_empty() {
            return;
        }
        ui.add_space(t.px(1.25));
        widgets::plain(ui, &t, "Unfinished exams", 1.0, W::Semi, t.p.text);
        ui.add_space(t.px(0.5));
        let mut go = None;
        widgets::rows(ui, &t, |ui| {
            let mut first = true;
            for k in &drafts {
                let parts: Vec<&str> = k.split('/').collect();
                if parts.len() < 3 {
                    continue;
                }
                let Some(s) = self.subject(parts[1]) else { continue };
                let Some(x) = s.exams.iter().find(|e| e.id == parts[2]) else { continue };
                if widgets::row(ui, &t, &format!("{}: {}", s.name, x.title), None, "Resume", false, true, first).clicked() {
                    go = Some((s.id.clone(), x.id.clone()));
                }
                first = false;
            }
        });
        if let Some((s, x)) = go {
            self.go(Tab::Exams, View::Intro { subject: s, exam: x });
        }
    }

    pub fn view_exam_home(&mut self, ui: &mut Ui) {
        let t = self.t;
        let p = t.p;
        let g = self.grade_data();
        widgets::page_column(ui, &t, 1040.0, |ui| {
            widgets::h1(ui, &t, &format!("Grade {} exams", g.grade));
            widgets::lead(ui, &t, "Write full exam papers under timed conditions, then mark them with the memo.");
            self.drafts_list(ui);
            ui.add_space(t.px(1.25));
            let subs: Vec<_> = g.subjects.iter().filter(|s| !s.exams.is_empty()).collect();
            let gap = t.px(0.9);
            let cols = ((ui.available_width() + gap) / (230.0 + gap)).floor().max(1.0) as usize;
            let w = (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32;
            let mut clicked = None;
            for (ri, chunk) in subs.chunks(cols).enumerate() {
                let rid = Id::new(("exam-row", ri, cols));
                let min_h = widgets::row_height(ui, rid);
                let mut row_max: f32 = 0.0;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for s in chunk {
                        let st = self.subject_stats(s);
                        let (r, _, nh) = widgets::card(ui, &t, w, min_h, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = t.px(0.75);
                                widgets::subject_icon(ui, &t, &s.id, 2.4);
                                ui.vertical(|ui| {
                                    ui.add_space(4.0);
                                    widgets::plain(ui, &t, &s.name, 1.05, W::Semi, p.text);
                                });
                            });
                            widgets::muted(ui, &t, &super::app::plural(s.exams.len(), "paper", "papers"), 0.85);
                            let meta = if st.exams_written > 0 { format!("{} written · best {}%", st.exams_written, st.best_exam.unwrap_or(0)) } else { "Not written yet".into() };
                            widgets::muted(ui, &t, &meta, 0.85);
                        });
                        row_max = row_max.max(nh);
                        if r.clicked() {
                            clicked = Some(s.id.clone());
                        }
                    }
                });
                widgets::set_row_height(ui, rid, row_max);
                ui.add_space(gap - ui.spacing().item_spacing.y);
            }
            if subs.is_empty() {
                widgets::muted(ui, &t, "No exams are available for this grade yet.", 1.0);
            }
            if let Some(s) = clicked {
                self.go(Tab::Exams, View::List { subject: s });
            }
        });
    }

    pub fn view_exam_list(&mut self, ui: &mut Ui, sid: &str) {
        let t = self.t;
        let Some(s) = self.subject(sid) else { return self.go(Tab::Exams, View::Home) };
        widgets::page_column(ui, &t, 1040.0, |ui| {
            self.crumbs(ui, &[(format!("Grade {} exams", self.grade()), Some((Tab::Exams, View::Home))), (s.name.clone(), None)]);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = t.px(0.8);
                widgets::subject_icon(ui, &t, &s.id, 2.9);
                ui.vertical(|ui| {
                    ui.add_space(6.0);
                    widgets::h1(ui, &t, &markup::escape_html(&format!("{} exams", s.name)));
                });
            });
            ui.add_space(t.px(1.0));
            let mut go = None;
            widgets::rows(ui, &t, |ui| {
                for (i, x) in s.exams.iter().enumerate() {
                    let k = self.tkey(&s.id, &x.id);
                    let hist = self.p.exams.get(&k).cloned().unwrap_or_default();
                    let draft = self.p.exam_drafts.contains_key(&k);
                    let best = hist.iter().map(|a| a.percent).max();
                    let status = if draft {
                        "In progress".to_string()
                    } else if let Some(b) = best {
                        format!("Best {b}% · {}", super::app::plural(hist.len(), "attempt", "attempts"))
                    } else {
                        "Not written".into()
                    };
                    let sub = format!("{} minutes · {} marks", crate::model::js_num(x.duration_minutes), crate::model::js_num(x.total_marks));
                    if widgets::row(ui, &t, &x.title, Some(&sub), &status, best.is_some() && !draft, draft || best.is_some(), i == 0).clicked() {
                        go = Some(x.id.clone());
                    }
                }
            });
            if let Some(x) = go {
                self.go(Tab::Exams, View::Intro { subject: s.id.clone(), exam: x });
            }
        });
    }

    pub fn view_exam_intro(&mut self, ui: &mut Ui, sid: &str, eid: &str) {
        let t = self.t;
        let p = t.p;
        let Some(s) = self.subject(sid) else { return self.go(Tab::Exams, View::Home) };
        let Some(meta) = s.exams.iter().find(|x| x.id == eid) else { return self.go(Tab::Exams, View::Home) };
        let Some(exam) = self.load_exam(sid, eid) else { return };
        let key = self.tkey(sid, eid);
        let draft = self.p.exam_drafts.get(&key).cloned();
        let hist = self.p.exams.get(&key).cloned().unwrap_or_default();
        let nq: usize = exam.sections.iter().map(|x| x.questions.len()).sum();
        widgets::page_column(ui, &t, 820.0, |ui| {
            self.exam_crumbs(ui, sid, &s.name, &meta.title, None, eid);
            widgets::h1(ui, &t, &markup::escape_html(&exam.title));
            widgets::lead(ui, &t, &markup::escape_html(&format!("{} · Grade {}", s.name, self.grade())));
            facts(ui, &t, &[(crate::model::js_num(exam.duration_minutes), "minutes"), (crate::model::js_num(exam.total_marks), "marks"), (nq.to_string(), "questions")]);
            widgets::h2(ui, &t, "Instructions");
            for (i, ins) in exam.instructions.iter().enumerate() {
                ui.horizontal_top(|ui| {
                    widgets::plain(ui, &t, &format!("{}.", i + 1), 1.0, W::Regular, p.text);
                    ui.vertical(|ui| rich::show(ui, &markup::inline(ins), &Style::new(t.px(1.0), p.text, p.muted)));
                });
            }
            ui.add_space(t.px(0.5));
            widgets::muted(ui, &t, "Multiple-choice, one-word and calculation answers are marked automatically. For written answers you will compare your work with the memo and award your own marks after you submit.", 1.0);
            if let Some(d) = &draft {
                ui.add_space(t.px(1.0));
                let slot = ui.painter().add(Shape::Noop);
                let r = egui::Frame::new().inner_margin(egui::Margin::symmetric(t.px(0.9) as i8, t.px(0.7) as i8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let left = if d.timer_on { format!(" with {} left", format_duration(d.remaining_sec.0)) } else { String::new() };
                    widgets::plain(ui, &t, &format!("You have an unfinished attempt{left}."), 1.0, W::Regular, p.text);
                });
                ui.painter().set(slot, Shape::rect_filled(r.response.rect, radius(13.0), p.warn_soft));
            }
            ui.add_space(t.px(1.0));
            let timed_id = Id::new(("timed", &key));
            let mut timed = ui.ctx().memory(|m| m.data.get_temp::<bool>(timed_id)).unwrap_or(true);
            if let Some(d) = &draft {
                timed = d.timer_on;
            }
            ui.add_enabled_ui(draft.is_none(), |ui| {
                if ui.checkbox(&mut timed, egui::RichText::new("Use a countdown timer").font(t.font(1.0, W::Regular))).changed() {
                    ui.ctx().memory_mut(|m| m.data.insert_temp(timed_id, timed));
                }
            });
            ui.add_space(t.px(1.0));
            ui.horizontal(|ui| {
                if draft.is_some() {
                    if btn("Start over").show(ui, &t).clicked() {
                        self.dialog("Start over?", vec!["<p>Your answers in the unfinished attempt will be deleted.</p>".into()], "Start over", Some("Cancel"), true, DialogAction::RestartExam { key: key.clone() });
                    }
                    if btn("Resume exam").primary().show(ui, &t).clicked() {
                        self.start_exam(sid, eid, draft.clone(), timed);
                    }
                } else if btn("Start exam").primary().show(ui, &t).clicked() {
                    self.start_exam(sid, eid, None, timed);
                }
            });
            if !hist.is_empty() {
                widgets::h2(ui, &t, "Previous attempts");
                let headers: Vec<String> = ["Date", "Mark", "%", "Time", ""].iter().map(|s| s.to_string()).collect();
                let mut review = None;
                for (i, a) in hist.iter().enumerate().rev() {
                    ui.horizontal(|ui| {
                        let cols = [format_date(&a.date), format!("{} / {}", marking::format_marks(a.score.0), crate::model::js_num(a.max.0)), format!("{}%", a.percent), format_duration(a.time_used_sec.0)];
                        let w = (ui.available_width() - 70.0) / 4.0;
                        for c in cols {
                            ui.allocate_ui_with_layout(vec2(w, 0.0), egui::Layout::left_to_right(Align::Center), |ui| {
                                ui.set_width(w);
                                widgets::plain(ui, &t, &c, 0.95, W::Regular, p.text);
                            });
                        }
                        if widgets::link(ui, &t, "Review", t.px(0.95), p.accent).clicked() {
                            review = Some(i);
                        }
                    });
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, p.border));
                }
                let _ = headers;
                if let Some(i) = review {
                    self.go(Tab::Exams, View::Review { subject: sid.into(), exam: eid.into(), attempt: i });
                }
            }
        });
    }

    // ---------------------------------------------------------------- the session

    fn start_exam(&mut self, sid: &str, eid: &str, draft: Option<Draft>, timer_on: bool) {
        self.stop_exam();
        let Some(exam) = self.load_exam(sid, eid) else { return };
        let key = self.tkey(sid, eid);
        let marking_phase = draft.as_ref().is_some_and(|d| d.phase == "mark");
        self.session = Some(ExamSession {
            key: key.clone(),
            subject: sid.into(),
            exam_id: eid.into(),
            grade: self.grade(),
            remaining: draft.as_ref().map_or(exam.duration_minutes * 60.0, |d| d.remaining_sec.0),
            elapsed: draft.as_ref().map_or(0.0, |d| d.elapsed_sec.0),
            responses: draft.as_ref().map(|d| d.responses.clone()).unwrap_or_default(),
            self_marks: draft.as_ref().map(|d| d.self_marks.clone()).unwrap_or_default(),
            timer_on: draft.as_ref().map_or(timer_on, |d| d.timer_on),
            marking: marking_phase,
            tick_from: self.now,
            exam,
        });
        let prefix = format!("x:{key}#");
        self.inputs.retain(|k, _| !k.starts_with(&prefix));
        self.write_draft();
        self.flush();
        self.sound.play("examStart");
        self.go(Tab::Exams, if marking_phase { View::Mark { subject: sid.into(), exam: eid.into() } } else { View::Run { subject: sid.into(), exam: eid.into() } });
    }

    pub fn write_draft(&mut self) {
        let Some(e) = &self.session else { return };
        let d = Draft {
            phase: if e.marking { "mark".into() } else { "run".into() },
            responses: e.responses.clone(),
            self_marks: e.self_marks.clone(),
            remaining_sec: Num(e.remaining.round()),
            elapsed_sec: Num(e.elapsed.round()),
            timer_on: e.timer_on,
            saved_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            extra: Default::default(),
        };
        let k = e.key.clone();
        self.p.exam_drafts.insert(k, d);
    }

    pub fn stop_exam(&mut self) {
        self.session = None;
    }

    /// The exam clock: once a second while writing.
    pub fn tick_exam(&mut self, ctx: &egui::Context) {
        let now = self.now;
        let Some(e) = &mut self.session else { return };
        if e.marking {
            return;
        }
        let mut ticks = 0;
        while now - e.tick_from >= 1.0 {
            e.tick_from += 1.0;
            ticks += 1;
        }
        let mut save = false;
        let mut warn = false;
        let mut up = false;
        for _ in 0..ticks {
            e.elapsed += 1.0;
            if e.timer_on {
                e.remaining = (e.remaining - 1.0).max(0.0);
            }
            if (e.elapsed as i64) % 15 == 0 {
                save = true;
            }
            if e.timer_on && (e.remaining == 300.0 || e.remaining == 60.0) {
                warn = true;
            }
            if e.timer_on && e.remaining <= 0.0 {
                up = true;
                break;
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_secs_f64((1.0 - (now - e.tick_from)).max(0.05)));
        if save {
            self.write_draft();
            self.save();
        }
        if warn {
            self.sound.play("timeWarning");
        }
        if up {
            self.submit_exam(true);
        }
    }

    fn all_questions(exam: &crate::model::Exam) -> Vec<(usize, usize, String)> {
        let mut out = Vec::new();
        for (si, sec) in exam.sections.iter().enumerate() {
            for (qi, _) in sec.questions.iter().enumerate() {
                out.push((si, qi, format!("{}.{}", si + 1, qi + 1)));
            }
        }
        out
    }

    pub fn exam_bar(&mut self, ui: &mut Ui) {
        let t = self.t;
        let p = t.p;
        let Some(e) = &self.session else { return };
        let exam = e.exam.clone();
        let total: usize = exam.sections.iter().map(|s| s.questions.len()).sum();
        let answered = exam.sections.iter().flat_map(|s| &s.questions).filter(|q| marking::is_answered(q, e.responses.get(&q.id))).count();
        let (timer_on, remaining, elapsed) = (e.timer_on, e.remaining, e.elapsed);
        let avail = ui.available_width();
        let w = avail.min(1040.0);
        ui.horizontal(|ui| {
            ui.add_space(((avail - w) / 2.0).max(0.0));
            ui.allocate_ui_with_layout(vec2(w, 36.0), egui::Layout::left_to_right(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = t.px(1.0);
                let right_w = if t.narrow { 180.0 } else { 360.0 };
                ui.allocate_ui_with_layout(vec2((w - right_w).max(60.0), 30.0), egui::Layout::left_to_right(Align::Center), |ui| {
                    ui.set_width((w - right_w).max(60.0));
                    widgets::plain(ui, &t, &exam.title, 1.0, W::Semi, p.text);
                });
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if btn("Submit").primary().show(ui, &t).clicked() {
                        self.confirm_submit();
                    }
                    let low = timer_on && remaining <= 300.0;
                    let s = format_duration(if timer_on { remaining } else { elapsed });
                    widgets::plain(ui, &t, &s, 1.1, W::Semi, if low { p.bad }else { p.text }).on_hover_text(if timer_on { "Time remaining" } else { "Time spent" });
                    if !t.narrow {
                        widgets::muted(ui, &t, &format!("{answered} of {total} answered"), 0.9);
                    }
                });
            });
        });
    }

    fn confirm_submit(&mut self) {
        let Some(e) = &self.session else { return };
        let un = e.exam.sections.iter().flat_map(|s| &s.questions).filter(|q| !marking::is_answered(q, e.responses.get(&q.id))).count();
        let body = if un > 0 {
            format!("<p>You have <strong>{un}</strong> unanswered question{}. Unanswered questions get 0 marks.</p>", if un == 1 { "" } else { "s" })
        } else {
            "<p>You have answered every question.</p>".into()
        };
        self.dialog("Submit exam?", vec![body], "Submit", Some("Cancel"), false, DialogAction::SubmitExam);
    }

    pub fn view_exam_run(&mut self, ui: &mut Ui, sid: &str, eid: &str) {
        let t = self.t;
        let p = t.p;
        let ok = self.session.as_ref().is_some_and(|e| e.key == self.tkey(sid, eid) && e.grade == self.grade());
        if !ok {
            return self.go(Tab::Exams, View::Intro { subject: sid.into(), exam: eid.into() });
        }
        if self.session.as_ref().is_some_and(|e| e.marking) {
            return self.go(Tab::Exams, View::Mark { subject: sid.into(), exam: eid.into() });
        }
        let s = self.subject(sid).unwrap();
        let exam = self.session.as_ref().unwrap().exam.clone();
        widgets::page_column(ui, &t, 820.0, |ui| {
            self.exam_crumbs(ui, sid, &s.name, &exam.title, None, eid);
            widgets::h1(ui, &t, &markup::escape_html(&exam.title));
            widgets::muted(ui, &t, "Your answers are saved automatically. You can leave and come back to this exam later.", 1.0);
            for (si, sec) in exam.sections.iter().enumerate() {
                ui.add_space(t.px(2.0));
                let sub: f64 = sec.questions.iter().map(marking::marks_of).sum();
                section_heading(ui, &t, &format!("{} <span class=\"muted\">({} marks)</span>", markup::inline(&sec.title), crate::model::js_num(sub)));
                if let Some(ins) = sec.instructions.as_deref().filter(|s| !s.is_empty()) {
                    for para in markup::paragraph_list(ins) {
                        rich::show(ui, &para, &Style::new(t.px(1.0), p.muted, p.muted));
                    }
                }
                if let Some(ctx) = &sec.context {
                    blocks::blocks(ui, &t, ctx);
                }
                for (qi, q) in sec.questions.iter().enumerate() {
                    let resp = self.session.as_ref().unwrap().responses.get(&q.id).cloned();
                    let card = Card {
                        key: format!("x:{}#{}", self.tkey(sid, eid), q.id),
                        number: format!("{}.{}", si + 1, qi + 1),
                        q,
                        response: resp.as_ref(),
                        feedback: false,
                        self_mark: None,
                        allow_check: false,
                        can_self_mark: false,
                        hints: None,
                    };
                    let (events, _) = questions::card(ui, &t, &mut self.inputs, None, self.now, card);
                    for e in events {
                        if let Event::Response(v) = e {
                            if matches!(q.kind.as_str(), "mcq" | "truefalse" | "match") {
                                self.sound.play("select");
                            }
                            if let Some(s) = &mut self.session {
                                s.responses.insert(q.id.clone(), v);
                            }
                            self.write_draft();
                            self.save();
                        }
                    }
                }
            }
            ui.add_space(t.px(1.5));
            ui.with_layout(egui::Layout::right_to_left(Align::Min), |ui| {
                if btn("Submit exam").primary().show(ui, &t).clicked() {
                    self.confirm_submit();
                }
            });
        });
    }

    pub fn submit_exam(&mut self, time_up: bool) {
        let Some(e) = &mut self.session else { return };
        if e.marking {
            return;
        }
        e.marking = true;
        let has_open = e.exam.sections.iter().flat_map(|s| &s.questions).any(|q| q.kind == "open");
        let (sid, eid) = (e.subject.clone(), e.exam_id.clone());
        self.write_draft();
        self.flush();
        self.sound.play(if time_up { "timeWarning" } else { "submit" });
        if !has_open {
            return self.finish_exam();
        }
        self.go(Tab::Exams, View::Mark { subject: sid, exam: eid });
        if time_up {
            self.dialog("Time is up", vec!["<p>Your exam has been submitted. Now mark your written answers using the memo.</p>".into()], "Start marking", None, false, DialogAction::StartMarking);
        }
    }

    pub fn view_exam_mark(&mut self, ui: &mut Ui, sid: &str, eid: &str) {
        let t = self.t;
        let p = t.p;
        let ok = self.session.as_ref().is_some_and(|e| e.key == self.tkey(sid, eid) && e.grade == self.grade());
        if !ok {
            return self.go(Tab::Exams, View::Intro { subject: sid.into(), exam: eid.into() });
        }
        if self.session.as_ref().is_some_and(|e| !e.marking) {
            return self.go(Tab::Exams, View::Run { subject: sid.into(), exam: eid.into() });
        }
        let s = self.subject(sid).unwrap();
        let exam = self.session.as_ref().unwrap().exam.clone();
        // Unanswered written questions automatically get 0.
        let mut changed = false;
        if let Some(e) = &mut self.session {
            for q in exam.sections.iter().flat_map(|s| &s.questions).filter(|q| q.kind == "open") {
                if !marking::is_answered(q, e.responses.get(&q.id)) && e.self_marks.get(&q.id).copied().flatten().is_none() {
                    e.self_marks.insert(q.id.clone(), Some(Num(0.0)));
                    changed = true;
                }
            }
        }
        if changed {
            self.write_draft();
            self.save();
        }
        let open: Vec<_> = Self::all_questions(&exam).into_iter().filter(|(si, qi, _)| exam.sections[*si].questions[*qi].kind == "open").collect();
        widgets::page_column(ui, &t, 820.0, |ui| {
            self.exam_crumbs(ui, sid, &s.name, &exam.title, Some("Marking"), eid);
            widgets::h1(ui, &t, "Mark your written answers");
            widgets::lead(ui, &t, "The other questions have been marked automatically. For each written answer below, read the memo and award yourself marks honestly.");
            let marked = open.iter().filter(|(si, qi, _)| self.session.as_ref().unwrap().self_marks.get(&exam.sections[*si].questions[*qi].id).copied().flatten().is_some()).count();
            ui.add_space(t.px(0.6));
            rich::show(ui, &format!("<strong>{marked}</strong> of {} written answers marked", open.len()), &Style::new(t.px(1.0), p.text, p.muted));
            let mut last_si = usize::MAX;
            for (si, qi, number) in &open {
                let sec = &exam.sections[*si];
                if *si != last_si {
                    last_si = *si;
                    widgets::h2(ui, &t, &markup::inline(&sec.title));
                    if let Some(c) = &sec.context {
                        source_material(ui, &t, Id::new(("ctx", *si)), c);
                    }
                }
                let q = &sec.questions[*qi];
                let e = self.session.as_ref().unwrap();
                let resp = e.responses.get(&q.id).cloned();
                let sm = e.self_marks.get(&q.id).copied().flatten().map(|n| n.0);
                let card = Card { key: format!("x:{}#{}", self.tkey(sid, eid), q.id), number: number.clone(), q, response: resp.as_ref(), feedback: true, self_mark: sm, allow_check: false, can_self_mark: true, hints: None };
                let (events, _) = questions::card(ui, &t, &mut self.inputs, None, self.now, card);
                for ev in events {
                    if let Event::SelfMark(n) = ev {
                        if let Some(v) = n {
                            let (_, voices) = crate::audio::sfx::mark(v, marking::marks_of(q));
                            self.sound.play_voices("mark", voices);
                        }
                        if let Some(e) = &mut self.session {
                            e.self_marks.insert(q.id.clone(), n.map(Num));
                        }
                        self.write_draft();
                        self.save();
                    }
                }
            }
            ui.add_space(t.px(1.5));
            ui.with_layout(egui::Layout::right_to_left(Align::Min), |ui| {
                if btn("See my results").primary().enabled(marked == open.len()).show(ui, &t).clicked() {
                    self.finish_exam();
                }
            });
        });
    }

    fn finish_exam(&mut self) {
        let Some(e) = self.session.take() else { return };
        let mut per: Vec<SectionScore> = e.exam.sections.iter().map(|s| SectionScore { title: s.title.clone(), score: Num(0.0), max: Num(0.0) }).collect();
        let (mut score, mut max) = (0.0, 0.0);
        for (si, sec) in e.exam.sections.iter().enumerate() {
            for q in &sec.questions {
                let r = marking::mark(q, e.responses.get(&q.id), e.self_marks.get(&q.id).copied().flatten().map(|n| n.0));
                score += r.score;
                max += r.max;
                per[si].score.0 += r.score;
                per[si].max.0 += r.max;
            }
        }
        let attempt = Attempt {
            date: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            score: Num(marking::js_round(score * 10.0) / 10.0),
            max: Num(max),
            percent: pct(score, max),
            time_used_sec: Num(e.elapsed),
            timed: e.timer_on,
            sections: per,
            responses: e.responses.clone(),
            self_marks: e.self_marks.clone(),
            extra: Default::default(),
        };
        let percent = attempt.percent;
        let hist = self.p.exams.entry(e.key.clone()).or_default();
        hist.push(attempt);
        if hist.len() > 30 {
            let n = hist.len() - 30;
            hist.drain(..n);
        }
        let idx = hist.len() - 1;
        self.p.exam_drafts.shift_remove(&e.key);
        self.record_streak("exam", 1.0);
        self.flush();
        self.go(Tab::Exams, View::Result { subject: e.subject.clone(), exam: e.exam_id.clone(), attempt: idx });
        self.later(0.3, super::app::Delayed::Sound(if percent >= 50 { "complete" } else { "finishSoft" }));
    }

    pub fn view_exam_result(&mut self, ui: &mut Ui, sid: &str, eid: &str, attempt: usize) {
        let t = self.t;
        let p = t.p;
        let Some(s) = self.subject(sid) else { return self.go(Tab::Exams, View::Home) };
        let Some(meta) = s.exams.iter().find(|x| x.id == eid) else { return self.go(Tab::Exams, View::Home) };
        let Some(a) = self.p.exams.get(&self.tkey(sid, eid)).and_then(|h| h.get(attempt)).cloned() else { return self.go(Tab::Exams, View::Home) };
        let (lvl, label) = marking::caps_level(a.percent as f64);
        widgets::page_column(ui, &t, 820.0, |ui| {
            self.exam_crumbs(ui, sid, &s.name, &meta.title, Some("Results"), eid);
            widgets::h1(ui, &t, &markup::escape_html(&format!("{}: results", meta.title)));
            result_box(ui, &t, &format!("{}%", a.percent), &[
                format!("{} / {} marks · Level {lvl}: {label}", marking::format_marks(a.score.0), crate::model::js_num(a.max.0)),
                format!("Time used: {} of {} minutes", format_duration(a.time_used_sec.0), crate::model::js_num(meta.duration_minutes)),
            ]);
            widgets::h2(ui, &t, "Marks per question");
            let rows: Vec<Vec<String>> = a.sections.iter().map(|sec| vec![sec.title.clone(), format!("{} / {}", marking::format_marks(sec.score.0), crate::model::js_num(sec.max.0)), format!("{}%", pct(sec.score.0, sec.max.0))]).collect();
            let mut col = blocks::Collect::new(false);
            blocks::table(ui, &t, &["Question".into(), "Mark".into(), "%".into()], &rows, None, 0.95, &mut col, 0);
            ui.add_space(t.px(1.0));
            widgets::wrap(ui, vec2(t.px(0.6), t.px(0.6)), |ui| {
                if btn(&format!("Back to {} exams", s.name)).show(ui, &t).clicked() {
                    self.go(Tab::Exams, View::List { subject: sid.into() });
                }
                if btn("Write again").show(ui, &t).clicked() {
                    self.go(Tab::Exams, View::Intro { subject: sid.into(), exam: eid.into() });
                }
                if btn("Review answers and memo").primary().show(ui, &t).clicked() {
                    self.go(Tab::Exams, View::Review { subject: sid.into(), exam: eid.into(), attempt });
                }
            });
            let _ = p;
        });
    }

    pub fn view_exam_review(&mut self, ui: &mut Ui, sid: &str, eid: &str, attempt: usize) {
        let t = self.t;
        let Some(s) = self.subject(sid) else { return self.go(Tab::Exams, View::Home) };
        let Some(meta) = s.exams.iter().find(|x| x.id == eid) else { return self.go(Tab::Exams, View::Home) };
        let Some(a) = self.p.exams.get(&self.tkey(sid, eid)).and_then(|h| h.get(attempt)).cloned() else { return self.go(Tab::Exams, View::Home) };
        let Some(exam) = self.load_exam(sid, eid) else { return };
        widgets::page_column(ui, &t, 820.0, |ui| {
            self.exam_crumbs(ui, sid, &s.name, &meta.title, Some("Review"), eid);
            widgets::h1(ui, &t, &markup::escape_html(&format!("{}: review", meta.title)));
            widgets::lead(ui, &t, &markup::escape_html(&format!("{} · {} / {} ({}%)", format_date(&a.date), marking::format_marks(a.score.0), crate::model::js_num(a.max.0), a.percent)));
            for (si, sec) in exam.sections.iter().enumerate() {
                ui.add_space(t.px(2.0));
                section_heading(ui, &t, &markup::inline(&sec.title));
                if let Some(c) = &sec.context {
                    source_material(ui, &t, Id::new(("rctx", si)), c);
                }
                for (qi, q) in sec.questions.iter().enumerate() {
                    let card = Card {
                        key: format!("r:{}#{}#{}", self.tkey(sid, eid), attempt, q.id),
                        number: format!("{}.{}", si + 1, qi + 1),
                        q,
                        response: a.responses.get(&q.id),
                        feedback: true,
                        self_mark: a.self_marks.get(&q.id).copied().flatten().map(|n| n.0),
                        allow_check: false,
                        can_self_mark: false,
                        hints: None,
                    };
                    questions::card(ui, &t, &mut self.inputs, None, self.now, card);
                }
            }
            ui.add_space(t.px(1.5));
            ui.with_layout(egui::Layout::right_to_left(Align::Min), |ui| {
                if btn("Back to results").show(ui, &t).clicked() {
                    self.go(Tab::Exams, View::Result { subject: sid.into(), exam: eid.into(), attempt });
                }
            });
        });
    }
}

/// A section heading with a strong underline (.exam-section > h2).
fn section_heading(ui: &mut Ui, t: &super::theme::Theme, html: &str) {
    let r = rich::show(ui, html, &Style::new(t.px(1.2), t.p.text, t.p.muted).weight(W::Semi).line(1.3)).0;
    ui.add_space(t.px(0.4));
    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 2.0), Sense::hover());
    ui.painter().rect_filled(line, 0.0, t.p.text);
    ui.add_space(t.px(0.75));
    let _ = r;
}

/// "Show the source material" (a collapsible block list).
fn source_material(ui: &mut Ui, t: &super::theme::Theme, id: Id, list: &[crate::model::Block]) {
    let open = ui.ctx().memory(|m| m.data.get_temp::<bool>(id)).unwrap_or(false);
    let label = if open { "▾ Hide the source material" } else { "▸ Show the source material" };
    if widgets::link(ui, t, label, t.px(1.0), t.p.accent).clicked() {
        ui.ctx().memory_mut(|m| m.data.insert_temp(id, !open));
    }
    if open {
        blocks::blocks(ui, t, list);
    }
    ui.add_space(t.px(0.5));
}

/// The row of big numbers (.facts).
pub fn facts(ui: &mut Ui, t: &super::theme::Theme, items: &[(String, &str)]) {
    let p = &t.p;
    ui.add_space(t.px(1.0));
    let gap = t.px(0.75);
    let n = items.len();
    let cols = if t.narrow { 2.min(n) } else { n };
    let w = (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32;
    for chunk in items.chunks(cols) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (big, small) in chunk {
                let slot = ui.painter().add(Shape::Noop);
                let r = ui.allocate_ui_with_layout(vec2(w, 0.0), egui::Layout::top_down(Align::Min), |ui| {
                    egui::Frame::new().inner_margin(egui::Margin::symmetric(t.px(1.0) as i8, t.px(0.8) as i8)).show(ui, |ui| {
                        ui.set_width(w - 2.0 * t.px(1.0));
                        ui.spacing_mut().item_spacing.y = 0.0;
                        widgets::plain(ui, t, big, 1.35, W::Bold, p.text);
                        widgets::muted(ui, t, small, 0.82);
                    });
                });
                ui.painter().set(slot, Shape::rect_filled(r.response.rect, radius(18.0), p.surface));
            }
        });
        ui.add_space(gap - ui.spacing().item_spacing.y);
    }
    ui.add_space(t.px(0.5));
}

/// The big result (.result-box).
pub fn result_box(ui: &mut Ui, t: &super::theme::Theme, big: &str, lines: &[String]) {
    let p = &t.p;
    ui.add_space(t.px(1.0));
    let slot = ui.painter().add(Shape::Noop);
    let r = egui::Frame::new().inner_margin(egui::Margin::same(t.px(1.5) as i8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical_centered(|ui| {
            widgets::plain(ui, t, big, 2.6, W::Bold, p.text);
            for (i, l) in lines.iter().enumerate() {
                widgets::plain(ui, t, l, 1.0, W::Regular, if i == 0 { p.text } else { p.muted });
            }
        });
    });
    ui.painter().set(slot, Shape::Rect(egui::epaint::RectShape::new(r.response.rect, radius(18.0), p.surface, Stroke::new(1.0, p.border), StrokeKind::Inside)));
    ui.add_space(t.px(1.0));
}
