//! Maths typesetting: a small TeX-style engine for the LaTeX that FreeSheet's content uses
//! (fractions, roots, scripts, accents, stretchy brackets, braces, operators, Greek and
//! the usual symbols), drawn with KaTeX's own fonts. Formulas are rendered once, in white,
//! to an image that the screens tint to the text colour.

pub mod layout;
pub mod parse;

use layout::{faces, Bx, Item, Style};
use resvg::tiny_skia::{self, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// A rendered formula: premultiplied white-on-transparent pixels and its metrics in em.
pub struct Rendered {
    pub width_px: usize,
    pub height_px: usize,
    pub rgba: Vec<u8>,
    /// Size of the box in em: width, height above the baseline, depth below it.
    pub w: f32,
    pub h: f32,
    pub d: f32,
    /// Pixels of padding around the box on each side.
    pub pad: f32,
    pub has_error: bool,
}

pub fn layout(tex: &str, display: bool) -> Bx {
    let nodes = parse::Parser::parse(tex);
    layout::layout_list(&nodes, if display { Style::D } else { Style::T })
}

struct Outline<'a> {
    pb: &'a mut PathBuilder,
    ox: f32,
    oy: f32,
    sx: f32,
    sy: f32,
}

impl ttf_parser::OutlineBuilder for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(self.ox + x * self.sx, self.oy - y * self.sy);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(self.ox + x * self.sx, self.oy - y * self.sy);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.pb.quad_to(self.ox + x1 * self.sx, self.oy - y1 * self.sy, self.ox + x * self.sx, self.oy - y * self.sy);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.pb.cubic_to(
            self.ox + x1 * self.sx,
            self.oy - y1 * self.sy,
            self.ox + x2 * self.sx,
            self.oy - y2 * self.sy,
            self.ox + x * self.sx,
            self.oy - y * self.sy,
        );
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

/// Render `tex` at `px` pixels per em.
pub fn render(tex: &str, display: bool, px: f32) -> Rendered {
    let b = layout(tex, display);
    let pad = 2.0f32;
    let wpx = ((b.w * px) + 2.0 * pad).ceil().max(1.0) as u32;
    let hpx = (((b.h + b.d) * px) + 2.0 * pad).ceil().max(1.0) as u32;
    let mut pm = Pixmap::new(wpx, hpx).unwrap_or_else(|| Pixmap::new(1, 1).unwrap());
    let base_y = pad + b.h * px;
    let mut white = Paint::default();
    white.set_color_rgba8(255, 255, 255, 255);
    white.anti_alias = true;
    let mut red = white.clone();
    red.set_color_rgba8(255, 60, 60, 255);
    let mut has_error = false;
    let mut fill = PathBuilder::new();
    let mut fill_red = PathBuilder::new();
    for it in &b.items {
        match it {
            Item::Glyph { face, gid, x, y, size, sy, red: is_red } => {
                let f = &faces()[*face];
                let k = px * size / f.units_per_em() as f32;
                let target = if *is_red {
                    has_error = true;
                    &mut fill_red
                } else {
                    &mut fill
                };
                let mut o = Outline { pb: target, ox: pad + x * px, oy: base_y - y * px, sx: k, sy: k * sy };
                f.outline_glyph(*gid, &mut o);
            }
            Item::Rule { x, y, w, h } => {
                // Rules snap to whole pixels in height so they stay crisp.
                let top = base_y - (y + h) * px;
                let hh = (h * px).max(1.0);
                if let Some(r) = tiny_skia::Rect::from_xywh(pad + x * px, top, (w * px).max(1.0), hh) {
                    fill.push_rect(r);
                }
            }
            Item::Stroke { pts, width } => {
                let mut sb = PathBuilder::new();
                for (i, (x, y)) in pts.iter().enumerate() {
                    let (px_, py_) = (pad + x * px, base_y - y * px);
                    if i == 0 {
                        sb.move_to(px_, py_);
                    } else {
                        sb.line_to(px_, py_);
                    }
                }
                if let Some(path) = sb.finish() {
                    let stroke = Stroke { width: (width * px).max(1.0), line_cap: tiny_skia::LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Stroke::default() };
                    pm.stroke_path(&path, &white, &stroke, Transform::identity(), None);
                }
            }
        }
    }
    if let Some(path) = fill.finish() {
        pm.fill_path(&path, &white, FillRule::Winding, Transform::identity(), None);
    }
    if let Some(path) = fill_red.finish() {
        pm.fill_path(&path, &red, FillRule::Winding, Transform::identity(), None);
    }
    // A touch of gamma so thin strokes hold up next to the (hinted) text.
    let mut rgba = pm.take();
    let lut: Vec<u8> = (0..256).map(|a| ((a as f32 / 255.0).powf(0.8) * 255.0).round() as u8).collect();
    for px in rgba.chunks_mut(4) {
        let a = px[3];
        if a > 0 && a < 255 {
            let na = lut[a as usize];
            let k = na as f32 / a as f32;
            for c in &mut px[..3] {
                *c = ((*c as f32) * k).round().min(na as f32) as u8;
            }
            px[3] = na;
        }
    }
    Rendered { width_px: wpx as usize, height_px: hpx as usize, rgba, w: b.w, h: b.h, d: b.d, pad, has_error }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_fraction() {
        let r = render("\\dfrac{x^2+1}{2}", true, 40.0);
        assert!(r.w > 1.0 && r.h > 0.8 && r.d > 0.5);
        assert!(r.rgba.chunks(4).any(|p| p[3] > 200));
        assert!(!r.has_error);
    }

    /// Every formula in the content can be typeset with no unknown commands.
    #[test]
    fn all_content_maths_parses() {
        use crate::content;
        use crate::markup::segments;
        let mut texs = Vec::new();
        let mut add = |s: &str| {
            for seg in segments(s) {
                if seg.math {
                    texs.push(seg.text);
                }
            }
        };
        for g in &content::catalog().grades {
            for s in &g.subjects {
                for t in &s.topics {
                    let topic = content::topic(g.grade, &s.id, &t.id).unwrap();
                    for b in &topic.notes {
                        for f in [&b.text, &b.term, &b.title, &b.problem, &b.answer, &b.caption, &b.label].into_iter().flatten() {
                            add(f);
                        }
                        b.items.iter().chain(&b.steps).chain(&b.headers).chain(b.rows.iter().flatten()).for_each(|x| add(x));
                        if let Some(l) = &b.latex {
                            add(&format!("${l}$"));
                        }
                    }
                    for q in &topic.questions {
                        add(&q.prompt);
                        q.options.iter().chain(&q.answers).for_each(|x| add(x));
                        for f in [&q.explanation, &q.memo, &q.unit].into_iter().flatten() {
                            add(f);
                        }
                    }
                }
            }
        }
        assert!(texs.len() > 5000);
        let mut bad = Vec::new();
        for t in &texs {
            fn errors(n: &[parse::Node], out: &mut Vec<String>) {
                for x in n {
                    match x {
                        parse::Node::Error(e) => out.push(e.clone()),
                        parse::Node::Group(g) | parse::Node::Boxed(g) | parse::Node::XArrow { above: g } => errors(g, out),
                        parse::Node::Frac { num, den, .. } => {
                            errors(num, out);
                            errors(den, out);
                        }
                        parse::Node::Sqrt { index, body } => {
                            errors(body, out);
                            if let Some(i) = index {
                                errors(i, out);
                            }
                        }
                        parse::Node::Scripts { base, sup, sub } => {
                            errors(std::slice::from_ref(base), out);
                            for s in [sup, sub].into_iter().flatten() {
                                errors(s, out);
                            }
                        }
                        parse::Node::Accent { body, .. } | parse::Node::Delimited { body, .. } => errors(body, out),
                        parse::Node::Underbrace { body, label } => {
                            errors(body, out);
                            if let Some(l) = label {
                                errors(l, out);
                            }
                        }
                        _ => {}
                    }
                }
            }
            let mut e = Vec::new();
            errors(&parse::Parser::parse(t), &mut e);
            if !e.is_empty() {
                bad.push(format!("{t}  ->  {e:?}"));
            }
        }
        assert!(bad.is_empty(), "{} formulas use unknown commands, e.g.\n{}", bad.len(), bad.iter().take(15).cloned().collect::<Vec<_>>().join("\n"));
    }
}

#[cfg(test)]
mod sheet {
    /// Writes a contact sheet of formulas (FS_MATH_SHEET=<png path>) for checking by eye.
    #[test]
    fn contact_sheet() {
        let Ok(out) = std::env::var("FS_MATH_SHEET") else { return };
        let samples = [
            (r"\dfrac{(x - 3)(x + 3)}{(x + 3)(x - 2)} \times \dfrac{2(x - 2)}{x - 3}", true),
            (r"= \dfrac{\dfrac{\sqrt{3}}{2} \times \dfrac{1}{\sqrt{3}}}{\left(\dfrac{1}{\sqrt{2}}\right)^2}", true),
            (r"\text{price incl. VAT} = \text{price excl. VAT} \times 1{,}15 \qquad \frac{15}{115}", true),
            (r"A = P(1 + in) = 12\,000(1 + 0{,}09 \times 5) = \text{R}17\ 400{,}00", false),
            (r"\sum f \cdot x = 4(5) + 14(15) \approx 930", true),
            (r"\cos 52^\circ + \sin 38^\circ = 0{,}6157... \approx 1{,}23", false),
            (r"x = \dfrac{-b \pm \sqrt{b^2 - 4ac}}{2a}", true),
            (r"a^m \times a^n = a^{m+n} \quad (a^m)^n = a^{mn} \quad x^{-2}", false),
            (r"\lim_{x \to 0} \dfrac{\sin x}{x} = 1 \qquad \sqrt[3]{27} = 3", true),
            (r"\hat{A} + \bar{x} + \overline{AB} \parallel CD, \; \triangle ABC \perp \angle B \neq 90^\circ", false),
            (r"\underbrace{2 + 2 + 2}_{3 \text{ terms}} \geq \mathbb{R} \Rightarrow T_n = a + (n-1)d", true),
            (r"\boxed{E = mc^2} \xrightarrow{\text{heat}} H_2O \leq \infty", false),
        ];
        let px = 32.0;
        let rows: Vec<_> = samples.iter().map(|(t, d)| super::render(t, *d, px)).collect();
        let w = rows.iter().map(|r| r.width_px).max().unwrap() as u32 + 20;
        let h: u32 = rows.iter().map(|r| r.height_px as u32 + 14).sum::<u32>() + 10;
        let mut sheet = resvg::tiny_skia::Pixmap::new(w, h).unwrap();
        sheet.fill(resvg::tiny_skia::Color::from_rgba8(255, 255, 255, 255));
        let mut y = 10;
        for r in &rows {
            // white-on-transparent -> black on white
            let mut img = resvg::tiny_skia::Pixmap::new(r.width_px as u32, r.height_px as u32).unwrap();
            let data = img.data_mut();
            for (i, p) in r.rgba.chunks(4).enumerate() {
                data[i * 4..i * 4 + 4].copy_from_slice(&[0, 0, 0, p[3]]);
            }
            sheet.draw_pixmap(10, y, img.as_ref(), &resvg::tiny_skia::PixmapPaint::default(), resvg::tiny_skia::Transform::identity(), None);
            // baseline marker
            let by = y as f32 + r.pad + r.h * px;
            let rect = resvg::tiny_skia::Rect::from_xywh(2.0, by, 5.0, 1.0).unwrap();
            let mut paint = resvg::tiny_skia::Paint::default();
            paint.set_color_rgba8(255, 0, 0, 255);
            sheet.fill_rect(rect, &paint, resvg::tiny_skia::Transform::identity(), None);
            y += r.height_px as i32 + 14;
        }
        sheet.save_png(out).unwrap();
    }
}
