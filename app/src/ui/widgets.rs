//! FreeSheet's controls, drawn to match the 1.1 stylesheet: buttons, cards, rows,
//! segmented controls, bars, subject tiles, links and text boxes.

use super::images;
use super::rich::{self, Style};
use super::theme::{self, hsl, with_alpha, Theme, W};
use egui::{
    pos2, vec2, Align, Color32, CornerRadius, CursorIcon, Id, Layout, Margin, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, TextEdit, Ui, UiBuilder,
    Vec2,
};

pub fn radius(r: f32) -> CornerRadius {
    CornerRadius::same(r.round().clamp(0.0, 255.0) as u8)
}

/// Draw an icon (tinted) centred in `rect`.
pub fn paint_icon(ui: &Ui, name: &str, rect: Rect, color: Color32, stroke: f32) {
    let tex = images::icon(ui.ctx(), name, rect.width(), stroke);
    ui.painter().image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), color);
}

pub fn icon(ui: &mut Ui, name: &str, size: f32, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    paint_icon(ui, name, rect, color, 1.8);
    resp
}

// ---------------------------------------------------------------- buttons

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Normal,
    Primary,
    Danger,
    DangerOutline,
    /// A plain, borderless icon button.
    Ghost,
    /// The green/red "Continue" in lesson feedback.
    Good,
    Bad,
}

pub struct Btn<'a> {
    pub text: &'a str,
    pub kind: Kind,
    pub big: bool,
    pub small: bool,
    pub enabled: bool,
    pub icon: Option<&'a str>,
    pub icon_color: Option<Color32>,
    pub min_width: f32,
    pub full: bool,
    pub selected: bool,
}

pub fn btn(text: &str) -> Btn<'_> {
    Btn { text, kind: Kind::Normal, big: false, small: false, enabled: true, icon: None, icon_color: None, min_width: 0.0, full: false, selected: false }
}

impl<'a> Btn<'a> {
    pub fn primary(mut self) -> Self {
        self.kind = Kind::Primary;
        self
    }
    pub fn kind(mut self, k: Kind) -> Self {
        self.kind = k;
        self
    }
    pub fn big(mut self) -> Self {
        self.big = true;
        self
    }
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }
    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }
    pub fn icon(mut self, name: &'a str) -> Self {
        self.icon = Some(name);
        self
    }
    pub fn icon_color(mut self, c: Color32) -> Self {
        self.icon_color = Some(c);
        self
    }
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }
    pub fn full(mut self) -> Self {
        self.full = true;
        self
    }
    pub fn selected(mut self, s: bool) -> Self {
        self.selected = s;
        self
    }

    pub fn show(self, ui: &mut Ui, t: &Theme) -> Response {
        let p = &t.p;
        let size = if self.big { t.px(1.1) } else if self.small { t.px(0.88) } else { t.px(0.95) };
        let (padx, pady) = if self.big { (t.px(1.5), t.px(0.9)) } else if self.small { (t.px(0.7), t.px(0.25)) } else { (t.px(1.0), t.px(0.5)) };
        let fg = match self.kind {
            Kind::Primary | Kind::Danger | Kind::Good | Kind::Bad => Color32::WHITE,
            Kind::DangerOutline => p.bad,
            _ => p.text,
        };
        let st = Style::new(size, fg, p.muted).line(1.3).weight(if self.kind == Kind::Primary || self.big { W::Medium } else { W::Regular });
        let max_text = (ui.available_width() - 2.0 * padx - if self.icon.is_some() { size * 1.6 } else { 0.0 }).max(40.0);
        let laid = rich::layout(ui.ctx(), &crate::markup::escape_html(self.text), &st, max_text);
        let icon_w = if self.icon.is_some() { size * 1.2 + if self.text.is_empty() { 0.0 } else { size * 0.45 } } else { 0.0 };
        let content = vec2(laid.size.x + icon_w, laid.size.y.max(size * 1.3));
        let mut desired = content + vec2(2.0 * padx, 2.0 * pady);
        desired.x = desired.x.max(self.min_width);
        if self.full {
            desired.x = desired.x.max(ui.available_width());
        }
        let sense = if self.enabled { Sense::click() } else { Sense::hover() };
        let (rect, resp) = ui.allocate_exact_size(desired, sense);
        if ui.is_rect_visible(rect) {
            let hover = resp.hovered() && self.enabled;
            let down = resp.is_pointer_button_down_on() && self.enabled;
            let (mut fill, mut border) = match self.kind {
                Kind::Normal => (if hover || self.selected { p.surface } else { p.bg }, p.border_strong),
                Kind::Primary => (if hover { p.btn_hover } else { p.btn }, if hover { p.btn_hover } else { p.btn }),
                Kind::Danger => (p.bad, p.bad),
                Kind::DangerOutline => (if hover { p.bad_soft } else { p.bg }, p.bad),
                Kind::Ghost => (if hover { p.surface } else { Color32::TRANSPARENT }, Color32::TRANSPARENT),
                Kind::Good => (p.good, p.good),
                Kind::Bad => (p.bad, p.bad),
            };
            if self.selected && self.kind == Kind::Normal {
                border = p.accent;
                fill = p.accent_soft;
            }
            let r = if down { rect.shrink(1.0) } else { rect };
            let rad = if self.big { 18.0 } else { 13.0 };
            let alpha = if self.enabled { 1.0 } else { 0.45 };
            ui.painter().rect(r, radius(rad), fill.gamma_multiply(alpha), Stroke::new(1.0, border.gamma_multiply(alpha)), StrokeKind::Inside);
            let mut x = r.center().x - content.x / 2.0;
            let y = r.center().y;
            if let Some(name) = self.icon {
                let ir = Rect::from_center_size(pos2(x + size * 0.6, y), vec2(size * 1.2, size * 1.2));
                paint_icon(ui, name, ir, self.icon_color.unwrap_or(fg).gamma_multiply(alpha), 1.8);
                x += icon_w;
            }
            let mut stc = st;
            stc.color = fg.gamma_multiply(alpha);
            rich::paint(ui.painter(), &laid, pos2(x, y - laid.size.y / 2.0), &stc);
        }
        if self.enabled {
            resp.on_hover_cursor(CursorIcon::PointingHand)
        } else {
            resp
        }
    }
}

/// A text link.
pub fn link(ui: &mut Ui, t: &Theme, text: &str, size: f32, color: Color32) -> Response {
    let st = Style::new(size, color, t.p.muted).line(1.4);
    let laid = rich::layout(ui.ctx(), &crate::markup::escape_html(text), &st, ui.available_width().max(20.0));
    let (rect, resp) = ui.allocate_exact_size(laid.size, Sense::click());
    let mut s2 = st;
    if resp.hovered() {
        s2.color = if color == t.p.muted { t.p.text } else { color };
        let y = rect.bottom() - 2.0;
        ui.painter().line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], Stroke::new(1.0, s2.color));
    }
    rich::paint(ui.painter(), &laid, rect.min, &s2);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

// ---------------------------------------------------------------- containers

/// Paints a card-like background into a reserved slot under content drawn afterwards.
pub fn framed<R>(ui: &mut Ui, sense: Sense, margin: Margin, content: impl FnOnce(&mut Ui) -> R) -> (Response, R, egui::layers::ShapeIdx) {
    let slot = ui.painter().add(Shape::Noop);
    let inner = ui.scope_builder(UiBuilder::new().sense(sense), |ui| {
        egui::Frame::new().inner_margin(margin).show(ui, |ui| {
            ui.set_width(ui.available_width());
            content(ui)
        })
        .inner
    });
    (inner.response, inner.inner, slot)
}

pub fn set_bg(ui: &Ui, slot: egui::layers::ShapeIdx, rect: Rect, fill: Color32, stroke: Stroke, r: f32) {
    ui.painter().set(slot, Shape::Rect(egui::epaint::RectShape::new(rect, radius(r), fill, stroke, StrokeKind::Inside)));
}

/// A card: the Study home's subject tiles and similar. Content is laid out inside padding.
/// `min_h`: the card's minimum height (cards in a grid row match the tallest).
/// Returns the click response, the content's result and the height the card needs.
pub fn card<R>(ui: &mut Ui, t: &Theme, width: f32, min_h: f32, content: impl FnOnce(&mut Ui) -> R) -> (Response, R, f32) {
    let slot = ui.painter().add(Shape::Noop);
    let shadow_slot = ui.painter().add(Shape::Noop);
    let inner = ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
        ui.set_width(width);
        let r = ui.scope_builder(UiBuilder::new().sense(Sense::click()), |ui| {
            egui::Frame::new().inner_margin(Margin::symmetric(t.px(1.1) as i8, t.px(1.0) as i8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = t.px(0.35);
                let top = ui.cursor().top();
                let out = content(ui);
                let natural = ui.min_rect().bottom() - top + 2.0 * t.px(1.0);
                ui.set_min_height((min_h - 2.0 * t.px(1.0)).max(0.0));
                (out, natural)
            })
            .inner
        });
        r
    });
    let resp = inner.inner.response;
    let hovered = resp.hovered();
    let down = resp.is_pointer_button_down_on();
    let lift = ui.ctx().animate_bool_with_time(resp.id.with("lift"), hovered && t.motion, 0.18);
    let p = &t.p;
    let rect = resp.rect;
    if lift > 0.0 {
        ui.painter().set(shadow_slot, Shape::Noop);
        let sh = egui::epaint::Shadow { offset: [0, 8], blur: 26, spread: 0, color: Color32::from_black_alpha((20.0 * lift) as u8) };
        ui.painter().set(slot, sh.as_shape(rect, radius(18.0)));
        ui.painter().set(shadow_slot, Shape::Rect(egui::epaint::RectShape::new(rect, radius(18.0), if hovered { p.surface } else { p.bg }, Stroke::new(1.0, if hovered { p.border_strong } else { p.border }), StrokeKind::Inside)));
    } else {
        ui.painter().set(slot, Shape::Rect(egui::epaint::RectShape::new(rect.translate(vec2(0.0, 1.0)), radius(18.0), p.shadow.gamma_multiply(0.4), Stroke::NONE, StrokeKind::Inside)));
        ui.painter().set(shadow_slot, Shape::Rect(egui::epaint::RectShape::new(rect, radius(18.0), p.bg, Stroke::new(1.0, p.border), StrokeKind::Inside)));
    }
    let _ = down;
    let (out, natural) = inner.inner.inner;
    (resp.on_hover_cursor(CursorIcon::PointingHand), out, natural)
}

/// Remembered heights of grid rows, so every card in a row is as tall as the tallest.
pub fn row_height(ui: &Ui, id: Id) -> f32 {
    ui.ctx().memory(|m| m.data.get_temp::<f32>(id)).unwrap_or(0.0)
}
pub fn set_row_height(ui: &Ui, id: Id, h: f32) {
    let old = row_height(ui, id);
    if (old - h).abs() > 0.5 {
        ui.ctx().memory_mut(|m| m.data.insert_temp(id, h));
        ui.ctx().request_repaint();
    }
}

/// One row of a `.rows` list: title (and optional sub) on the left, status on the right.
pub fn row(ui: &mut Ui, t: &Theme, title: &str, sub: Option<&str>, status: &str, status_strong: bool, status_text: bool, first: bool) -> Response {
    let p = &t.p;
    let slot = ui.painter().add(Shape::Noop);
    let width = ui.available_width();
    let r = ui.scope_builder(UiBuilder::new().sense(Sense::click()), |ui| {
        egui::Frame::new().inner_margin(Margin::symmetric(t.px(1.0) as i8, t.px(0.8) as i8)).show(ui, |ui| {
            ui.set_width(width - 2.0 * t.px(1.0));
            ui.horizontal(|ui| {
                let status_st = Style::new(t.px(0.82), if status_text { p.text } else { p.muted }, p.muted).weight(if status_strong { W::Semi } else { W::Regular }).line(1.3);
                let sl = rich::layout(ui.ctx(), &crate::markup::escape_html(status), &status_st, 300.0);
                let avail = ui.available_width() - sl.size.x - t.px(1.0);
                ui.allocate_ui_with_layout(vec2(avail.max(40.0), 0.0), Layout::top_down(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    rich::plain(ui, title, &Style::new(t.px(1.0), p.text, p.muted).weight(W::Medium).line(1.45));
                    if let Some(s) = sub {
                        rich::plain(ui, s, &Style::new(t.px(0.85), p.muted, p.muted).line(1.4));
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (rect, _) = ui.allocate_exact_size(sl.size, Sense::hover());
                    rich::paint(ui.painter(), &sl, rect.min, &status_st);
                });
            });
        });
    });
    let resp = r.response;
    let fill = if resp.hovered() { p.surface } else { p.bg };
    ui.painter().set(slot, Shape::rect_filled(resp.rect, 0.0, fill));
    if !first {
        ui.painter().hline(resp.rect.x_range(), resp.rect.top(), Stroke::new(1.0, p.border));
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// A bordered group of rows.
pub fn rows<R>(ui: &mut Ui, t: &Theme, content: impl FnOnce(&mut Ui) -> R) -> R {
    let slot = ui.painter().add(Shape::Noop);
    let inner = ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        content(ui)
    });
    let rect = inner.response.rect;
    ui.painter().set(slot, Shape::Noop);
    ui.painter().rect_stroke(rect, radius(18.0), Stroke::new(1.0, t.p.border), StrokeKind::Inside);
    inner.inner
}

/// A segmented control; returns the index clicked (if any).
pub fn segmented(ui: &mut Ui, t: &Theme, id: Id, options: &[&str], selected: usize, small: bool) -> Option<usize> {
    let p = &t.p;
    let size = t.px(0.92);
    let pad = vec2(t.px(1.0), t.px(0.4));
    let gap = 2.0;
    let outer = 3.0;
    let fonts: Vec<_> = options
        .iter()
        .enumerate()
        .map(|(i, o)| rich::layout(ui.ctx(), &crate::markup::escape_html(o), &Style::new(size, p.text, p.muted).weight(if i == selected { W::Semi } else { W::Regular }).line(1.3), 400.0))
        .collect();
    let widths: Vec<f32> = fonts.iter().map(|l| l.size.x + 2.0 * pad.x).collect();
    let h = size * 1.3 + 2.0 * pad.y;
    let total = widths.iter().sum::<f32>() + gap * (options.len().saturating_sub(1)) as f32 + 2.0 * outer;
    let (rect, _) = ui.allocate_exact_size(vec2(total, h + 2.0 * outer), Sense::hover());
    let _ = small;
    ui.painter().rect_filled(rect, radius(14.0), p.surface2);
    let mut x = rect.left() + outer;
    let mut clicked = None;
    let mut seg_rects = Vec::new();
    for (i, w) in widths.iter().enumerate() {
        let r = Rect::from_min_size(pos2(x, rect.top() + outer), vec2(*w, h));
        seg_rects.push(r);
        x += w + gap;
    }
    // The thumb slides to the selected segment.
    let target = seg_rects.get(selected).copied().unwrap_or(seg_rects[0]);
    let ax = ui.ctx().animate_value_with_time(id.with("thumb-x"), target.left(), if t.motion { 0.22 } else { 0.0 });
    let aw = ui.ctx().animate_value_with_time(id.with("thumb-w"), target.width(), if t.motion { 0.22 } else { 0.0 });
    let thumb = Rect::from_min_size(pos2(ax, target.top()), vec2(aw, h));
    let sh = egui::epaint::Shadow { offset: [0, 1], blur: 3, spread: 0, color: Color32::from_black_alpha(if t.dark { 70 } else { 36 }) };
    ui.painter().add(sh.as_shape(thumb, radius(11.0)));
    ui.painter().rect_filled(thumb, radius(11.0), p.bg);
    for (i, r) in seg_rects.iter().enumerate() {
        let resp = ui.interact(*r, id.with(i), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        if resp.clicked() && i != selected {
            clicked = Some(i);
        }
        let color = if i == selected || resp.hovered() { p.text } else { p.muted };
        let st = Style::new(size, color, p.muted).weight(if i == selected { W::Semi } else { W::Regular }).line(1.3);
        let l = &fonts[i];
        rich::paint(ui.painter(), l, pos2(r.center().x - l.size.x / 2.0, r.center().y - l.size.y / 2.0), &st);
    }
    clicked
}

/// A thin progress bar.
pub fn bar(ui: &mut Ui, t: &Theme, percent: f32, width: f32, color: Option<Color32>) -> Response {
    let h = 6.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
    ui.painter().rect_filled(rect, radius(99.0), t.p.surface2);
    let pc = (percent / 100.0).clamp(0.0, 1.0);
    if pc > 0.0 {
        let r = Rect::from_min_size(rect.min, vec2((rect.width() * pc).max(h), h));
        ui.painter().rect_filled(r, radius(99.0), color.unwrap_or(with_alpha(t.p.text, 0.75)));
    }
    resp
}

pub fn subject_colors(t: &Theme, hue: f32) -> (Color32, Color32) {
    if t.dark {
        (hsl(hue, 35.0, 20.0), hsl(hue, 75.0, 74.0))
    } else {
        (hsl(hue, 80.0, 94.0), hsl(hue, 60.0, 36.0))
    }
}

/// A subject's coloured icon tile. `size` in rem: 2.4 normal, 2.9 large, 1.6 small.
pub fn subject_icon(ui: &mut Ui, t: &Theme, id: &str, size: f32) -> Response {
    let (name, hue) = images::subject_icon(id);
    let s = t.px(size);
    let (rect, resp) = ui.allocate_exact_size(vec2(s, s), Sense::hover());
    let (bg, fg) = subject_colors(t, hue);
    let r = if size >= 2.9 { 16.0 } else if size <= 1.6 { 9.0 } else { 13.0 };
    ui.painter().rect_filled(rect, radius(r), bg);
    let ic = s * if size >= 2.9 { 0.55 } else if size <= 1.6 { 0.62 } else { 0.56 };
    paint_icon(ui, name, Rect::from_center_size(rect.center(), vec2(ic, ic)), fg, 1.8);
    resp
}

/// Rich text in one call.
pub fn text(ui: &mut Ui, t: &Theme, html: &str, rem: f32, w: W, color: Color32) -> Response {
    rich::show(ui, html, &Style::new(t.px(rem), color, t.p.muted).weight(w)).0
}

pub fn plain(ui: &mut Ui, t: &Theme, s: &str, rem: f32, w: W, color: Color32) -> Response {
    rich::plain(ui, s, &Style::new(t.px(rem), color, t.p.muted).weight(w))
}

pub fn h1(ui: &mut Ui, t: &Theme, s: &str) -> Response {
    rich::show(ui, s, &Style::new(t.px(1.75), t.p.text, t.p.muted).weight(W::Semi).line(1.25)).0
}

pub fn h2(ui: &mut Ui, t: &Theme, s: &str) -> Response {
    ui.add_space(t.px(1.6));
    let r = rich::show(ui, s, &Style::new(t.px(1.2), t.p.text, t.p.muted).weight(W::Semi).line(1.25)).0;
    ui.add_space(t.px(0.4));
    r
}

/// Muted lead paragraph.
pub fn lead(ui: &mut Ui, t: &Theme, html: &str) -> Response {
    rich::show(ui, html, &Style::new(t.px(1.05), t.p.muted, t.p.muted)).0
}

/// An uppercase small label (.n-label, .bs-label).
pub fn label(ui: &mut Ui, t: &Theme, s: &str) -> Response {
    rich::plain(ui, &s.to_uppercase(), &Style::new(t.px(0.78), t.p.muted, t.p.muted).weight(W::Semi).line(1.3))
}

/// A styled one-line text box.
pub fn text_box(ui: &mut Ui, t: &Theme, id: Id, value: &mut String, hint: &str, width: f32, enabled: bool) -> Response {
    let te = TextEdit::singleline(value)
        .id(id)
        .hint_text(hint)
        .desired_width(width)
        .font(t.font(1.0, W::Regular))
        .margin(vec2(t.px(0.6), t.px(0.45)))
        .interactive(enabled);
    let r = ui.add(te);
    r
}

pub fn text_area(ui: &mut Ui, t: &Theme, id: Id, value: &mut String, hint: &str, rows: usize, enabled: bool) -> Response {
    let te = TextEdit::multiline(value)
        .id(id)
        .hint_text(hint)
        .desired_width(f32::INFINITY)
        .desired_rows(rows)
        .font(t.font(1.0, W::Regular))
        .margin(vec2(t.px(0.6), t.px(0.45)))
        .interactive(enabled);
    ui.add(te)
}

/// Styles egui's own widgets (text boxes, drop-downs, scroll bars) to match.
pub fn apply_visuals(ctx: &egui::Context, t: &Theme) {
    let p = &t.p;
    let mut v = if t.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = p.bg;
    v.window_fill = p.bg;
    v.extreme_bg_color = p.bg;
    v.text_edit_bg_color = Some(p.bg);
    v.faint_bg_color = p.surface;
    v.code_bg_color = p.surface2;
    v.override_text_color = Some(p.text);
    v.hyperlink_color = p.accent;
    v.selection.bg_fill = with_alpha(p.accent, 0.3);
    v.selection.stroke = Stroke::new(1.5, p.accent);
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = radius(16.0);
    v.menu_corner_radius = radius(12.0);
    v.popup_shadow = egui::epaint::Shadow { offset: [0, 6], blur: 20, spread: 0, color: Color32::from_black_alpha(40) };
    let r = radius(13.0);
    for (w, fill, stroke) in [
        (&mut v.widgets.noninteractive, p.bg, p.border),
        (&mut v.widgets.inactive, p.bg, p.border_strong),
        (&mut v.widgets.hovered, p.surface, p.border_strong),
        (&mut v.widgets.active, p.surface2, p.accent),
        (&mut v.widgets.open, p.surface, p.accent),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0, stroke);
        w.corner_radius = r;
        w.fg_stroke = Stroke::new(1.0, p.text);
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.interact_cursor = Some(CursorIcon::PointingHand);
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = vec2(t.px(0.6), t.px(0.6));
        s.spacing.button_padding = vec2(t.px(0.6), t.px(0.35));
        s.spacing.interact_size.y = t.px(2.1);
        s.spacing.scroll = egui::style::ScrollStyle { bar_width: 8.0, floating: true, ..egui::style::ScrollStyle::floating() };
        s.text_styles.insert(egui::TextStyle::Body, t.font(1.0, W::Regular));
        s.text_styles.insert(egui::TextStyle::Button, t.font(0.95, W::Regular));
        s.text_styles.insert(egui::TextStyle::Small, t.font(0.85, W::Regular));
        s.text_styles.insert(egui::TextStyle::Heading, t.font(1.2, W::Semi));
        s.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::new(t.px(0.95), egui::FontFamily::Monospace));
        s.interaction.selectable_labels = false;
        #[cfg(debug_assertions)]
        {
            s.debug.show_unaligned = false;
        }
        s.animation_time = if t.motion { 0.18 } else { 0.0 };
    });
}

/// A drop-down; returns the new index if changed.
pub fn select(ui: &mut Ui, t: &Theme, id: Id, current: usize, options: &[String], width: f32, enabled: bool) -> Option<usize> {
    let mut sel = current;
    let text = options.get(current).cloned().unwrap_or_else(|| "Choose…".into());
    ui.add_enabled_ui(enabled, |ui| {
        egui::ComboBox::from_id_salt(id).selected_text(egui::RichText::new(text).font(t.font(0.95, W::Regular))).width(width).show_ui(ui, |ui| {
            for (i, o) in options.iter().enumerate() {
                ui.selectable_value(&mut sel, i, egui::RichText::new(o).font(t.font(0.95, W::Regular)));
            }
        });
    });
    (sel != current).then_some(sel)
}

/// Lay out children in rows that wrap (like CSS flex-wrap).
pub fn wrap<R>(ui: &mut Ui, gap: Vec2, content: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = gap;
        content(ui)
    })
    .inner
}

/// The centred page column.
pub fn page_column<R>(ui: &mut Ui, t: &Theme, max_w: f32, content: impl FnOnce(&mut Ui) -> R) -> R {
    let avail = ui.available_width();
    let pad = if t.narrow { t.px(1.0) } else { t.px(1.5) };
    let w = (avail - 2.0 * pad).min(max_w - 2.0 * pad).max(100.0);
    let left = ((avail - w) / 2.0).max(0.0);
    let top = if t.narrow { t.px(1.0) } else { t.px(1.5) };
    ui.add_space(top);
    let r = ui
        .horizontal(|ui| {
            ui.add_space(left);
            ui.vertical(|ui| {
                ui.set_width(w);
                ui.spacing_mut().item_spacing.y = t.px(0.5);
                content(ui)
            })
            .inner
        })
        .inner;
    ui.add_space(t.px(4.0));
    r
}

/// Muted small text.
pub fn muted(ui: &mut Ui, t: &Theme, s: &str, rem: f32) -> Response {
    rich::plain(ui, s, &Style::new(t.px(rem), t.p.muted, t.p.muted))
}

pub fn pos(x: f32, y: f32) -> Pos2 {
    pos2(x, y)
}

pub fn unused(_: &theme::Palette) {}
