//! Question cards: answering, checking, hints and self-marking (1.1's questions.js). Used
//! by practice worksheets and exams.

use super::rich::{self, Style};
use super::theme::{self, Theme, W};
use super::widgets::{self, btn, radius};
use crate::ai::Review;
use crate::hints::Hint;
use crate::markup;
use crate::marking;
use crate::model::Question;
use crate::quiz::{self, LETTERS};
use egui::{pos2, vec2, Align, Color32, Id, Rect, Sense, Shape, Stroke, StrokeKind, Ui};
use serde_json::Value;
use std::collections::HashMap;

pub struct Card<'a> {
    /// A unique key (for text boxes and the verdict flash).
    pub key: String,
    pub number: String,
    pub q: &'a Question,
    pub response: Option<&'a Value>,
    pub feedback: bool,
    pub self_mark: Option<f64>,
    pub allow_check: bool,
    pub can_self_mark: bool,
    pub hints: Option<(&'a [Hint], u32)>,
    pub ai: Ai,
}

/// AI marking on this card (written answers only).
#[derive(Clone)]
pub enum Ai {
    Off,
    /// Can be marked: the "Show memo" button becomes "Mark my answer".
    Ready,
    Marking,
    Done(std::sync::Arc<Review>),
    Failed(String),
}

pub enum Event {
    Response(Value),
    Check,
    SelfMark(Option<f64>),
    /// Stop waiting for the AI and mark this one by hand.
    AiManual,
    Hint(u32),
    ShowNotes(usize),
}

fn options_out(c: &Card) -> Vec<usize> {
    let mut out = Vec::new();
    if let Some((list, level)) = c.hints {
        for h in list.iter().take(level as usize) {
            out.extend(h.eliminate.iter().copied());
        }
    }
    out
}

/// One option button (multiple choice, true/false). Returns clicked.
fn option(ui: &mut Ui, t: &Theme, id: Id, letter: Option<&str>, html: &str, state: OptState, enabled: bool, width: Option<f32>) -> bool {
    let p = &t.p;
    let (border, bg, letter_c, ring) = match state {
        OptState::Plain => (p.border_strong, p.bg, p.muted, false),
        OptState::Selected => (p.accent, p.accent_soft, p.accent, true),
        OptState::Correct => (p.good, p.good_soft, p.good, true),
        OptState::Wrong => (p.bad, p.bad_soft, p.bad, true),
        OptState::Out => (p.border_strong, p.bg, p.muted, false),
    };
    let out = state == OptState::Out;
    let pad = vec2(t.px(0.8), t.px(0.55));
    let full_w = width.unwrap_or(ui.available_width());
    let text_st = Style::new(t.px(1.0), p.text, p.muted).line(1.5);
    let letter_w = if letter.is_some() { t.px(1.1) + t.px(0.7) } else { 0.0 };
    let tw = full_w - 2.0 * pad.x - letter_w;
    let laid = rich::layout(ui.ctx(), html, &if letter.is_none() { text_st.center() } else { text_st }, tw.max(40.0));
    let h = laid.size.y + 2.0 * pad.y;
    let sense = if enabled && !out { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(full_w, h), sense);
    let resp = ui.interact(rect, id, sense);
    let hover = resp.hovered() && enabled && !out;
    let down = resp.is_pointer_button_down_on() && enabled;
    let r = if down && t.motion { rect.shrink(1.0) } else { rect };
    let b = if hover && state == OptState::Plain { p.accent } else { border };
    let alpha = if out { 0.36 } else { 1.0 };
    ui.painter().rect(r, radius(13.0), bg.gamma_multiply(alpha), Stroke::new(if ring { 2.0 } else { 1.0 }, b.gamma_multiply(alpha)), StrokeKind::Inside);
    let mut x = r.left() + pad.x;
    if let Some(l) = letter {
        let ls = Style::new(t.px(1.0), letter_c.gamma_multiply(alpha), p.muted).weight(W::Semi).line(1.5);
        let ll = rich::layout(ui.ctx(), l, &ls, 40.0);
        rich::paint(ui.painter(), &ll, pos2(x, r.top() + pad.y), &ls);
        x += letter_w;
    }
    let mut ts = if letter.is_none() { text_st.center() } else { text_st };
    ts.color = p.text.gamma_multiply(alpha);
    rich::paint(ui.painter(), &laid, pos2(x, r.top() + pad.y), &ts);
    if out {
        // Struck through.
        for w in &laid.words {
            let wr = w.rect.translate(vec2(x, r.top() + pad.y));
            ui.painter().hline(wr.x_range(), wr.center().y, Stroke::new(1.0, p.text.gamma_multiply(alpha)));
        }
    }
    resp.clicked() && enabled && !out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OptState {
    Plain,
    Selected,
    Correct,
    Wrong,
    Out,
}

/// Draw a card. Returns what the learner did, and the card's rectangle.
pub fn card(ui: &mut Ui, t: &Theme, inputs: &mut HashMap<String, String>, flash: Option<(f64, bool)>, now: f64, c: Card) -> (Vec<Event>, Rect) {
    let p = t.p;
    let q = c.q;
    let mut events = Vec::new();
    let max = marking::marks_of(q);
    let res = if c.feedback { Some(marking::mark(q, c.response, if q.kind == "open" { c.self_mark } else { None })) } else { None };
    let border = match &res {
        Some(r) if r.needs_self_mark => p.border,
        Some(r) if r.correct == Some(true) => p.good,
        Some(r) if r.score > 0.0 => p.border_strong,
        Some(_) => p.bad,
        None => p.border,
    };
    let locked = c.feedback;
    let id = Id::new(("q", &c.key));
    let slot = ui.painter().add(Shape::Noop);
    ui.add_space(t.px(0.9));
    let fr = egui::Frame::new().inner_margin(egui::Margin::symmetric(t.px(1.15) as i8, t.px(1.0) as i8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.0;
        // Head: number, marks, hints used, status.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = t.px(0.75);
            widgets::plain(ui, t, &c.number, 0.85, W::Semi, p.text);
            widgets::plain(ui, t, &format!("{} mark{}", crate::model::js_num(max), if max == 1.0 { "" } else { "s" }), 0.85, W::Regular, p.muted);
            let used = c.hints.map_or(0, |(_, l)| l);
            if used > 0 {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    widgets::icon(ui, "bulb", t.px(0.95), Color32::from_rgb(0xD9, 0x9A, 0x00));
                    widgets::plain(ui, t, &used.to_string(), 0.8, W::Regular, p.muted);
                })
                .response
                .on_hover_text("Hints used");
            }
            if let Some(r) = &res {
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let s = if r.needs_self_mark { "Mark yourself".to_string() } else { format!("{} / {}", marking::format_marks(r.score), crate::model::js_num(max)) };
                    widgets::plain(ui, t, &s, 0.85, W::Semi, p.text);
                });
            }
        });
        ui.add_space(t.px(0.35));
        // Prompt.
        for (i, para) in markup::paragraph_list(&q.prompt).iter().enumerate() {
            if i > 0 {
                ui.add_space(t.px(0.5));
            }
            rich::show(ui, para, &Style::new(t.px(1.0), p.text, p.muted));
        }
        ui.add_space(t.px(0.6));
        // Input.
        let out = if !c.feedback { options_out(&c) } else { vec![] };
        match q.kind.as_str() {
            "mcq" => {
                let sel = c.response.and_then(Value::as_i64);
                let ans = q.answer_index();
                for (i, o) in q.options.iter().enumerate() {
                    let ii = i as i64;
                    let state = if c.feedback && Some(ii) == ans {
                        OptState::Correct
                    } else if c.feedback && sel == Some(ii) {
                        OptState::Wrong
                    } else if sel == Some(ii) {
                        OptState::Selected
                    } else if out.contains(&i) {
                        OptState::Out
                    } else {
                        OptState::Plain
                    };
                    if option(ui, t, id.with(("o", i)), Some(LETTERS.get(i).copied().unwrap_or("?")), &markup::inline(o), state, !locked, None) {
                        events.push(Event::Response(Value::from(ii)));
                    }
                    ui.add_space(t.px(0.45));
                }
            }
            "truefalse" => {
                let sel = c.response.and_then(Value::as_bool);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = t.px(0.45);
                    for v in [true, false] {
                        let state = if c.feedback && Some(v) == q.answer_bool() {
                            OptState::Correct
                        } else if c.feedback && sel == Some(v) {
                            OptState::Wrong
                        } else if sel == Some(v) {
                            OptState::Selected
                        } else {
                            OptState::Plain
                        };
                        if option(ui, t, id.with(v), None, if v { "True" } else { "False" }, state, !locked, Some(t.px(7.0))) {
                            events.push(Event::Response(Value::Bool(v)));
                        }
                    }
                });
            }
            "short" | "numeric" => {
                let buf = inputs.entry(c.key.clone()).or_insert_with(|| c.response.and_then(Value::as_str).unwrap_or("").to_string());
                ui.horizontal(|ui| {
                    let w = if q.kind == "numeric" { 200.0 } else { 420.0f32.min(ui.available_width()) };
                    let r = widgets::text_box(ui, t, id.with("in"), buf, if q.kind == "numeric" { "Answer" } else { "Type your answer" }, w, !locked);
                    if r.changed() {
                        events.push(Event::Response(Value::String(buf.clone())));
                    }
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && c.allow_check && !locked {
                        events.push(Event::Check);
                    }
                    if let Some(u) = q.unit.as_deref().filter(|u| !u.is_empty()) {
                        rich::show(ui, &markup::inline(u), &Style::new(t.px(1.0), p.muted, p.muted));
                    }
                });
            }
            "match" => {
                let rights: Vec<String> = quiz::seeded_shuffle(&q.pairs.iter().map(|x| x.right.clone()).collect::<Vec<_>>(), &q.id);
                let given: Vec<Option<String>> = c.response.and_then(Value::as_array).map(|a| a.iter().map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
                let mut labels = vec!["Choose…".to_string()];
                labels.extend(rights.iter().map(|r| quiz::strip_markup(r)));
                for (i, pair) in q.pairs.iter().enumerate() {
                    let half = ui.available_width() / 2.0;
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(vec2(half - 8.0, 0.0), egui::Layout::top_down(Align::Min), |ui| {
                            ui.set_width(half - 8.0);
                            ui.add_space(t.px(0.35));
                            rich::show(ui, &markup::inline(&pair.left), &Style::new(t.px(1.0), p.text, p.muted));
                        });
                        ui.vertical(|ui| {
                            ui.add_space(t.px(0.3));
                            let sel = given.get(i).cloned().flatten();
                            let cur = sel.as_ref().and_then(|s| rights.iter().position(|r| r == s)).map_or(0, |k| k + 1);
                            ui.horizontal(|ui| {
                                if let Some(k) = widgets::select(ui, t, id.with(("m", i)), cur, &labels, (half - 40.0).clamp(80.0, 340.0), !locked) {
                                    let mut cur_resp: Vec<Value> = (0..q.pairs.len()).map(|j| given.get(j).cloned().flatten().map(Value::String).unwrap_or(Value::Null)).collect();
                                    cur_resp[i] = if k == 0 { Value::Null } else { Value::String(rights[k - 1].clone()) };
                                    events.push(Event::Response(Value::Array(cur_resp)));
                                }
                                if c.feedback {
                                    if sel.as_deref() == Some(pair.right.as_str()) {
                                        widgets::plain(ui, t, "✓", 1.0, W::Bold, p.good);
                                    } else {
                                        widgets::plain(ui, t, "✗", 1.0, W::Bold, p.bad);
                                    }
                                }
                            });
                            if c.feedback && sel.as_deref() != Some(pair.right.as_str()) {
                                rich::show(ui, &markup::inline(&pair.right), &Style::new(t.px(0.88), p.good, p.muted));
                            }
                            ui.add_space(t.px(0.3));
                        });
                    });
                    if i + 1 < q.pairs.len() {
                        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                        ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, p.border));
                    }
                }
            }
            "open" => {
                let buf = inputs.entry(c.key.clone()).or_insert_with(|| c.response.and_then(|r| r.get("text")).and_then(Value::as_str).unwrap_or("").to_string());
                let rows = (3.0 + (max / 2.0).ceil()).min(14.0) as usize;
                let r = widgets::text_area(ui, t, id.with("ta"), buf, "Write your answer", rows, !locked);
                if r.changed() {
                    events.push(Event::Response(serde_json::json!({ "text": buf.clone() })));
                }
            }
            _ => {
                widgets::muted(ui, t, "Unsupported question type.", 1.0);
            }
        }
        // Hints shown so far.
        if let Some((list, level)) = c.hints {
            let shown = &list[..(level as usize).min(list.len())];
            if !shown.is_empty() {
                ui.add_space(t.px(0.75));
            }
            for h in shown {
                let slot = ui.painter().add(Shape::Noop);
                let r = egui::Frame::new().inner_margin(egui::Margin::symmetric(t.px(0.8) as i8, t.px(0.55) as i8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_top(|ui| {
                        ui.add_space(0.0);
                        let (ir, _) = ui.allocate_exact_size(vec2(t.px(1.15), t.px(1.4)), Sense::hover());
                        widgets::paint_icon(ui, "bulb", Rect::from_min_size(ir.min + vec2(0.0, t.px(0.15)), vec2(t.px(1.15), t.px(1.15))), Color32::from_rgb(0xD9, 0x9A, 0x00), 1.8);
                        ui.vertical(|ui| {
                            rich::show(ui, &h.html, &Style::new(t.px(0.95), p.text, p.muted).chips(p.bg, p.border));
                            if let Some(sec) = h.section {
                                if widgets::link(ui, t, "Show me in the notes", t.px(0.9), p.accent).clicked() {
                                    events.push(Event::ShowNotes(sec));
                                }
                            }
                        });
                    });
                });
                ui.painter().set(slot, Shape::rect_filled(r.response.rect, radius(13.0), p.warn_soft));
                ui.add_space(t.px(0.45));
            }
        }
        // Actions.
        if !c.feedback && c.allow_check {
            ui.add_space(t.px(0.8));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = t.px(0.5);
                if btn(if q.kind == "open" && !matches!(c.ai, Ai::Off) { "Mark my answer" } else if q.kind == "open" { "Show memo" } else { "Check answer" }).primary().show(ui, t).clicked() {
                    events.push(Event::Check);
                }
                if let Some((list, level)) = c.hints {
                    if (level as usize) < list.len() && btn(if level > 0 { "Another hint" } else { "Hint" }).icon("bulb").icon_color(Color32::from_rgb(0xD9, 0x9A, 0x00)).show(ui, t).clicked() {
                        events.push(Event::Hint(level + 1));
                    }
                }
            });
        }
        // Feedback.
        if c.feedback {
            ui.add_space(t.px(0.85));
            feedback(ui, t, &c, &res.clone().unwrap(), &mut events);
        }
    });
    let rect = fr.response.rect;
    ui.painter().set(slot, Shape::Rect(egui::epaint::RectShape::new(rect, radius(18.0), p.bg, Stroke::new(1.0, border), StrokeKind::Inside)));
    // The verdict: a ring that spreads out and fades.
    if let Some((at, good)) = flash {
        let k = ((now - at) / 0.9) as f32;
        if (0.0..1.0).contains(&k) && t.motion {
            let e = super::anim::Ease::EaseOut.at(k);
            let c = if good { p.good } else { p.bad };
            ui.painter().rect_stroke(rect.expand(e * 14.0 / 2.0), radius(18.0 + e * 7.0), Stroke::new(e * 14.0, theme::with_alpha(c, 0.45 * (1.0 - e))), StrokeKind::Middle);
            ui.ctx().request_repaint();
        }
    }
    (events, rect)
}

fn feedback(ui: &mut Ui, t: &Theme, c: &Card, res: &marking::Mark, events: &mut Vec<Event>) {
    let p = t.p;
    let q = c.q;
    let max = marking::marks_of(q);
    let (bg, head): (Color32, Option<(String, Color32)>) = if q.kind == "open" {
        (p.surface, None)
    } else if q.kind == "match" {
        let ok = res.correct == Some(true);
        (if ok { p.good_soft } else { p.bad_soft }, Some((if ok { "All correct".into() } else { format!("{} of {} correct", res.pairs_right, q.pairs.len()) }, if ok { p.good } else { p.bad })))
    } else if res.correct == Some(true) {
        (p.good_soft, Some(("Correct".into(), p.good)))
    } else {
        (p.bad_soft, Some((if marking::is_answered(q, c.response) { "Incorrect".into() } else { "Not answered".into() }, p.bad)))
    };
    let slot = ui.painter().add(Shape::Noop);
    let r = egui::Frame::new().inner_margin(egui::Margin::symmetric(t.px(0.9) as i8, t.px(0.7) as i8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = t.px(0.3);
        let body = Style::new(t.px(1.0), p.text, p.muted);
        if q.kind == "open" && matches!(c.ai, Ai::Marking) {
            let dots = ".".repeat((ui.input(|i| i.time) * 2.0) as usize % 3 + 1);
            widgets::plain(ui, t, &format!("Marking your answer on this computer{dots}"), 1.0, W::Medium, p.text);
            if widgets::link(ui, t, "Mark it myself instead", t.px(0.9), p.accent).clicked() {
                events.push(Event::AiManual);
            }
            return;
        }
        let review = if let Ai::Done(r) = &c.ai { Some(r.clone()) } else { None };
        if let (true, Some(r)) = (q.kind == "open", &review) {
            widgets::label(ui, t, "Marked by AI");
            ai_points(ui, t, r, &body);
        } else if q.kind == "open" {
            if let Ai::Failed(m) = &c.ai {
                widgets::muted(ui, t, m, 0.9);
            }
            widgets::label(ui, t, "Memo");
            for para in markup::paragraph_list(q.memo.as_deref().unwrap_or("")) {
                rich::show(ui, &para, &body);
            }
        } else if let Some((h, color)) = head {
            if q.kind == "match" {
                rich::show(ui, &format!("<strong>{}</strong> <span class=\"muted\">({} / {})</span>", markup::escape_html(&h), marking::format_marks(res.score), crate::model::js_num(res.max)), &Style { color, ..body });
            } else {
                rich::show(ui, &format!("<strong>{}</strong>", markup::escape_html(&h)), &Style { color, ..body });
                if res.correct == Some(true) {
                    if let Some(sp) = &res.spelling {
                        rich::show(ui, &format!("Watch the spelling: <strong>{}</strong>", markup::escape_html(sp)), &Style::new(t.px(0.92), p.muted, p.muted));
                    }
                } else {
                    rich::show(ui, &format!("Correct answer: {}", quiz::correct_answer_html(q)), &body);
                }
            }
        }
        if let Some(e) = q.explanation.as_deref().filter(|e| !e.is_empty()) {
            for para in markup::paragraph_list(e) {
                rich::show(ui, &para, &body);
            }
        }
        if q.kind == "open" {
            if c.can_self_mark {
                widgets::muted(ui, t, if review.is_some() { "The AI can get it wrong. Compare each point with your answer and change the mark if you need to." } else { "Compare your answer with the memo and award yourself marks honestly." }, 0.85);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(t.px(0.35), t.px(0.35));
                    widgets::plain(ui, t, "Your mark:", 1.0, W::Regular, p.text);
                    if max <= 12.0 {
                        for n in 0..=(max as i64) {
                            let on = c.self_mark == Some(n as f64);
                            let k = if on { widgets::Kind::Primary } else { widgets::Kind::Normal };
                            if btn(&n.to_string()).kind(k).small().min_width(t.px(2.2)).show(ui, t).clicked() {
                                events.push(Event::SelfMark(Some(n as f64)));
                            }
                        }
                    } else {
                        let mut s = c.self_mark.map(|v| crate::model::js_num(v)).unwrap_or_default();
                        let r = widgets::text_box(ui, t, Id::new(("markin", &c.key)), &mut s, "", t.px(5.0), true);
                        if r.changed() {
                            let v = s.trim().parse::<f64>().ok().map(|v| v.round().clamp(0.0, max));
                            events.push(Event::SelfMark(v));
                        }
                    }
                    widgets::plain(ui, t, &format!("/ {}", crate::model::js_num(max)), 1.0, W::Regular, p.muted);
                });
            } else if let Some(sm) = c.self_mark {
                rich::show(ui, &format!("Your mark: <strong>{} / {}</strong>", marking::format_marks(sm), crate::model::js_num(max)), &body);
            }
        }
    });
    ui.painter().set(slot, Shape::rect_filled(r.response.rect, radius(13.0), bg));
}

/// The memo's points, each ticked or crossed by the AI.
fn ai_points(ui: &mut Ui, t: &Theme, r: &Review, body: &Style) {
    let p = t.p;
    if let Some(pre) = &r.scheme.preface {
        for para in markup::paragraph_list(pre) {
            rich::show(ui, &para, body);
        }
    }
    if let Some(n) = r.scheme.any {
        widgets::muted(ui, t, &format!("Any {n} of these earn marks:"), 0.85);
    }
    for (point, met) in r.scheme.points.iter().zip(&r.met) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = t.px(0.5);
            let (mark, color) = if *met { ("✓", p.good) } else { ("✗", p.bad) };
            widgets::plain(ui, t, mark, 1.0, W::Bold, color);
            ui.vertical(|ui| {
                rich::show(ui, &format!("{} <span class=\"muted\">({})</span>", markup::inline(&point.text), crate::model::js_num(point.marks)), body);
            });
        });
    }
}
