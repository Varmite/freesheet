//! Note blocks (docs/CONTENT_FORMAT.md): headings, paragraphs, lists, definitions, worked
//! examples, callouts, tables, formulas, passages and diagrams, drawn as in 1.1's
//! stylesheet. Words' positions are collected for read-along.

use super::images;
use super::rich::{self, Laid, Style};
use super::theme::{Theme, W};
use super::widgets::{self, radius};
use crate::markup;
use crate::model::Block;
use egui::{pos2, vec2, Align, Color32, Rect, Sense, Shape, Stroke, StrokeKind, Ui};
use std::sync::Arc;

/// Where one block's words ended up on screen (absolute positions), for read-along.
#[derive(Default, Clone)]
pub struct BlockWords {
    pub block: usize,
    /// (rect, word text, maths, line id)
    pub words: Vec<(Rect, String, Vec<String>, usize)>,
}

pub struct Collect {
    pub on: bool,
    pub blocks: Vec<BlockWords>,
    line_seq: usize,
}

impl Collect {
    pub fn new(on: bool) -> Collect {
        Collect { on, blocks: Vec::new(), line_seq: 0 }
    }
    fn add(&mut self, block: usize, laid: &Laid, origin: egui::Pos2) {
        if !self.on {
            return;
        }
        if self.blocks.last().is_none_or(|b| b.block != block) {
            self.blocks.push(BlockWords { block, words: Vec::new() });
        }
        let base = self.line_seq;
        let mut max_line = 0;
        let b = self.blocks.last_mut().unwrap();
        for w in &laid.words {
            b.words.push((w.rect.translate(origin.to_vec2()), w.text.clone(), w.math.clone(), base + w.line));
            max_line = max_line.max(w.line);
        }
        self.line_seq = base + max_line + 1;
    }
}

fn st(t: &Theme, rem: f32) -> Style {
    Style::new(t.px(rem), t.p.text, t.p.muted).chips(t.p.bg, t.p.border)
}

/// Show inline HTML, collecting its words for block `bi`.
fn line(ui: &mut Ui, html: &str, s: &Style, col: &mut Collect, bi: usize) -> egui::Response {
    let (r, laid) = rich::show(ui, html, s);
    col.add(bi, &laid, r.rect.min);
    r
}

fn paras(ui: &mut Ui, t: &Theme, text: &str, s: &Style, col: &mut Collect, bi: usize, gap: f32) {
    let list = markup::paragraph_list(text);
    for (i, p) in list.iter().enumerate() {
        if i > 0 {
            ui.add_space(gap);
        }
        line(ui, p, s, col, bi);
    }
    let _ = t;
}

/// A box with a background (examples, callouts, passages).
fn boxed(ui: &mut Ui, t: &Theme, left_bar: Option<Color32>, content: impl FnOnce(&mut Ui)) {
    let slot = ui.painter().add(Shape::Noop);
    let r = egui::Frame::new().inner_margin(egui::Margin::symmetric(t.px(1.1) as i8, t.px(0.9) as i8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = t.px(0.35);
        content(ui);
    });
    let rect = r.response.rect;
    ui.painter().set(slot, Shape::Rect(egui::epaint::RectShape::new(rect, radius(18.0), t.p.surface, Stroke::NONE, StrokeKind::Inside)));
    if let Some(c) = left_bar {
        ui.painter().rect_filled(Rect::from_min_size(rect.min, vec2(3.0, rect.height())), egui::CornerRadius { nw: 3, sw: 3, ne: 0, se: 0 }, c);
    }
}

fn list(ui: &mut Ui, t: &Theme, items: &[String], ordered: bool, s: &Style, col: &mut Collect, bi: usize) {
    for (i, it) in items.iter().enumerate() {
        ui.horizontal_top(|ui| {
            let marker = if ordered { format!("{}.", i + 1) } else { "•".into() };
            let ms = Style { align: Align::Max, ..*s };
            let ml = rich::layout(ui.ctx(), &marker, &ms, t.px(1.2));
            let (r, _) = ui.allocate_exact_size(vec2(t.px(1.4), ml.size.y), Sense::hover());
            rich::paint(ui.painter(), &ml, pos2(r.right() - t.px(0.4) - ml.size.x, r.top()), &ms);
            ui.vertical(|ui| {
                line(ui, &markup::inline(it), s, col, bi);
            });
        });
        ui.add_space(t.px(0.15));
    }
}

/// A formula on its own line, centred.
fn formula(ui: &mut Ui, t: &Theme, tex: &str, color: Color32) {
    let size = t.px(1.02) * 1.08;
    let m = images::math(ui.ctx(), tex, true, size);
    let w = m.w + 2.0 * m.pad;
    let h = m.h + m.d + 2.0 * m.pad;
    let avail = ui.available_width();
    if w > avail {
        egui::ScrollArea::horizontal().id_salt(("formula", tex)).show(ui, |ui| {
            let (r, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
            ui.painter().image(m.tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), color);
        });
    } else {
        let (r, _) = ui.allocate_exact_size(vec2(avail, h), Sense::hover());
        let img = Rect::from_min_size(pos2(r.center().x - w / 2.0, r.top()), vec2(w, h));
        ui.painter().image(m.tex.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), color);
    }
}

fn caption(ui: &mut Ui, t: &Theme, text: &str, col: &mut Collect, bi: usize) {
    let s = Style::new(t.px(0.88), t.p.muted, t.p.muted).center();
    line(ui, &markup::inline(text), &s, col, bi);
}

/// Lay out a table, fitting columns to the space (scrolling sideways if they can't fit).
pub fn table(ui: &mut Ui, t: &Theme, headers: &[String], rows: &[Vec<String>], caption_text: Option<&str>, size: f32, col: &mut Collect, bi: usize) {
    let p = &t.p;
    if let Some(c) = caption_text.filter(|c| !c.is_empty()) {
        line(ui, &markup::inline(c), &Style::new(t.px(0.88), p.muted, p.muted), col, bi);
    }
    let n = headers.len().max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
    if n == 0 {
        return;
    }
    let pad = vec2(t.px(0.65), t.px(0.45));
    let body = Style::new(t.px(size), p.text, p.muted).line(1.5);
    let head = body.weight(W::Semi);
    let html = |c: &String| markup::inline(c);
    // Natural (one-line) and minimum (longest word) width of every column.
    let mut natural = vec![0f32; n];
    let mut minimum = vec![0f32; n];
    let mut measure = |c: &String, i: usize, s: &Style| {
        let l = rich::layout(ui.ctx(), &html(c), s, 100000.0);
        natural[i] = natural[i].max(l.size.x);
        let mw = l.words.iter().map(|w| w.rect.width()).fold(0.0, f32::max);
        minimum[i] = minimum[i].max(mw);
    };
    for (i, h) in headers.iter().enumerate() {
        measure(h, i, &head);
    }
    for r in rows {
        for (i, c) in r.iter().enumerate().take(n) {
            measure(c, i, &body);
        }
    }
    let avail = ui.available_width() - 1.0;
    let extra = 2.0 * pad.x + 1.0;
    let sum_nat: f32 = natural.iter().map(|w| w + extra).sum();
    let sum_min: f32 = minimum.iter().map(|w| w + extra).sum();
    let widths: Vec<f32> = if sum_nat <= avail {
        // Fill the width, sharing the spare space by natural size.
        let k = avail / sum_nat;
        natural.iter().map(|w| (w + extra) * k).collect()
    } else if sum_min <= avail {
        let spare = avail - sum_min;
        let flex: f32 = natural.iter().zip(&minimum).map(|(a, b)| a - b).sum::<f32>().max(1.0);
        natural.iter().zip(&minimum).map(|(nat, min)| min + extra + spare * (nat - min) / flex).collect()
    } else {
        minimum.iter().map(|w| w + extra).collect()
    };
    let total: f32 = widths.iter().sum();
    let mut draw = |ui: &mut Ui, col: &mut Collect| {
        let mut y_rows: Vec<(bool, Vec<String>)> = Vec::new();
        if !headers.is_empty() {
            y_rows.push((true, headers.to_vec()));
        }
        for r in rows {
            y_rows.push((false, r.clone()));
        }
        for (is_head, cells) in &y_rows {
            let s = if *is_head { head } else { body };
            let lays: Vec<Arc<Laid>> = (0..n).map(|i| rich::layout(ui.ctx(), &cells.get(i).map(html).unwrap_or_default(), &s, widths[i] - extra)).collect();
            let h = lays.iter().map(|l| l.size.y).fold(0.0, f32::max) + 2.0 * pad.y;
            let (rect, _) = ui.allocate_exact_size(vec2(total, h), Sense::hover());
            let mut x = rect.left();
            for (i, l) in lays.iter().enumerate() {
                let cell = Rect::from_min_size(pos2(x, rect.top()), vec2(widths[i], h));
                if *is_head {
                    ui.painter().rect_filled(cell, 0.0, p.surface);
                }
                ui.painter().rect_stroke(cell, 0.0, Stroke::new(1.0, p.border), StrokeKind::Middle);
                let origin = cell.min + pad;
                rich::paint(ui.painter(), l, origin, &s);
                if !*is_head {
                    col.add(bi, l, origin);
                }
                x += widths[i];
            }
        }
    };
    ui.spacing_mut().item_spacing.y = 0.0;
    if total > avail + 1.0 {
        egui::ScrollArea::horizontal().id_salt(("table", bi, headers.first().cloned(), rows.len())).show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                draw(ui, col)
            });
        });
    } else {
        draw(ui, col);
    }
}

/// A diagram: SVG on a white card, at most 420 points tall.
fn diagram(ui: &mut Ui, t: &Theme, svg: &str) {
    let avail = ui.available_width();
    let pad = t.px(0.75);
    let (nw, nh) = images::svg_natural_size(svg).unwrap_or((400.0, 300.0));
    let k = ((avail - 2.0 * pad) / nw).min((420.0 - 2.0 * pad) / nh).min(1.6);
    let (w, h) = (nw * k, nh * k);
    let (r, _) = ui.allocate_exact_size(vec2(avail, h + 2.0 * pad), Sense::hover());
    let card = Rect::from_center_size(r.center(), vec2(w + 2.0 * pad, h + 2.0 * pad));
    ui.painter().rect(card, radius(18.0), Color32::WHITE, Stroke::new(1.0, t.p.border), StrokeKind::Inside);
    if let Some(tex) = images::svg_texture(ui.ctx(), svg, w, h) {
        ui.painter().image(tex.id(), Rect::from_center_size(card.center(), vec2(w, h)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
}

/// Draw one block. `first`: the first block of a section (no top margin on headings).
pub fn block(ui: &mut Ui, t: &Theme, b: &Block, bi: usize, first: bool, col: &mut Collect) {
    let p = &t.p;
    let text = st(t, 1.02).line(1.6 * t.spacing);
    let o = |s: &Option<String>| s.clone().unwrap_or_default();
    ui.spacing_mut().item_spacing.y = 0.0;
    match b.kind.as_str() {
        "heading" => {
            ui.add_space(if first { t.px(0.25) } else { t.px(1.6) });
            line(ui, &markup::inline(&o(&b.text)), &Style::new(t.px(1.2), p.text, p.muted).weight(W::Semi).line(1.3), col, bi);
            ui.add_space(t.px(0.5));
        }
        "paragraph" => {
            ui.add_space(t.px(0.6));
            paras(ui, t, &o(&b.text), &text, col, bi, t.px(0.6));
            ui.add_space(t.px(0.6));
        }
        "list" => {
            ui.add_space(t.px(0.6));
            list(ui, t, &b.items, b.ordered, &text, col, bi);
            ui.add_space(t.px(0.45));
        }
        "definition" => {
            ui.add_space(t.px(0.9));
            let slot = ui.painter().add(Shape::Noop);
            let r = ui.horizontal(|ui| {
                ui.add_space(t.px(0.9) + 3.0);
                ui.vertical(|ui| {
                    ui.add_space(t.px(0.35));
                    line(ui, &markup::inline(&o(&b.term)), &text.weight(W::Semi), col, bi);
                    line(ui, &markup::inline(&o(&b.text)), &text, col, bi);
                    ui.add_space(t.px(0.35));
                });
            });
            let rect = r.response.rect;
            ui.painter().set(slot, Shape::rect_filled(Rect::from_min_size(rect.min, vec2(3.0, rect.height())), 0.0, p.text));
            ui.add_space(t.px(0.9));
        }
        "example" => {
            ui.add_space(t.px(1.0));
            boxed(ui, t, None, |ui| {
                line(ui, &markup::inline(&b.title.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "Worked example".into())).to_uppercase(), &Style::new(t.px(0.78), p.muted, p.muted).weight(W::Semi).line(1.3), col, bi);
                paras(ui, t, &o(&b.problem), &text, col, bi, t.px(0.6));
                if !b.steps.is_empty() {
                    ui.add_space(t.px(0.4));
                    list(ui, t, &b.steps, true, &text, col, bi);
                }
                if let Some(a) = b.answer.as_deref().filter(|a| !a.is_empty()) {
                    ui.add_space(t.px(0.35));
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
                    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, p.border));
                    ui.add_space(t.px(0.45));
                    line(ui, &format!("<strong>Answer:</strong> {}", markup::inline(a)), &text, col, bi);
                }
            });
            ui.add_space(t.px(1.0));
        }
        "callout" => {
            ui.add_space(t.px(1.0));
            boxed(ui, t, Some(p.border_strong), |ui| {
                line(ui, &markup::inline(&b.label.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "Note".into())).to_uppercase(), &Style::new(t.px(0.78), p.muted, p.muted).weight(W::Semi).line(1.3), col, bi);
                paras(ui, t, &o(&b.text), &text, col, bi, t.px(0.6));
            });
            ui.add_space(t.px(1.0));
        }
        "passage" => {
            ui.add_space(t.px(1.0));
            boxed(ui, t, None, |ui| {
                if let Some(ti) = b.title.as_deref().filter(|s| !s.is_empty()) {
                    line(ui, &markup::inline(ti).to_uppercase(), &Style::new(t.px(0.78), p.muted, p.muted).weight(W::Semi).line(1.3), col, bi);
                }
                line(ui, &markup::inline(&o(&b.text)), &st(t, 1.06).serif().line(1.7), col, bi);
            });
            ui.add_space(t.px(1.0));
        }
        "table" => {
            ui.add_space(t.px(1.0));
            table(ui, t, &b.headers, &b.rows, b.caption.as_deref(), 0.95, col, bi);
            ui.add_space(t.px(1.0));
        }
        "formula" => {
            ui.add_space(t.px(1.0));
            formula(ui, t, &o(&b.latex), p.text);
            if let Some(c) = b.caption.as_deref().filter(|c| !c.is_empty()) {
                ui.add_space(t.px(0.3));
                caption(ui, t, c, col, bi);
            }
            ui.add_space(t.px(1.0));
        }
        "diagram" => {
            ui.add_space(t.px(1.25));
            if let Some(svg) = &b.svg {
                diagram(ui, t, svg);
            }
            if let Some(c) = b.caption.as_deref().filter(|c| !c.is_empty()) {
                ui.add_space(t.px(0.3));
                caption(ui, t, c, col, bi);
            }
            ui.add_space(t.px(1.25));
        }
        _ => {}
    }
}

/// Blocks in order (exam source material, review).
pub fn blocks(ui: &mut Ui, t: &Theme, list: &[Block]) {
    let mut col = Collect::new(false);
    for (i, b) in list.iter().enumerate() {
        block(ui, t, b, i, i == 0, &mut col);
    }
    let _ = widgets::radius;
}
