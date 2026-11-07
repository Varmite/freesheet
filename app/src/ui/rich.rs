//! Rich inline text: the markup HTML subset (see markup.rs) laid out in lines, with
//! bold/italic text, inline maths, answer blanks, word chips and letter patterns. Layouts
//! are cached; each word's box is kept so read-along can highlight the word being spoken.

use super::images;
use super::theme::{self, W};
use egui::{pos2, vec2, Align, Color32, Context, CornerRadius, FontFamily, FontId, Galley, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Fmt {
    pub bold: bool,
    pub italic: bool,
    pub mono: bool,
    pub muted: bool,
    pub chip: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    Text(String, Fmt),
    Math(String),
    Blank,
}

#[derive(Clone, Debug, Default)]
pub struct Word {
    pub pieces: Vec<Piece>,
    /// Whitespace after the word ("" at the end).
    pub space: String,
    /// A line break after the word.
    pub brk: bool,
    /// A paragraph break after the word (from </p>).
    pub para: bool,
}

fn entity(s: &str) -> Option<char> {
    Some(match s {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "#39" | "apos" => '\'',
        "nbsp" => '\u{a0}',
        _ => return None,
    })
}

/// Parse the HTML subset into words.
pub fn parse(html: &str) -> Vec<Word> {
    let c: Vec<char> = html.chars().collect();
    let mut words: Vec<Word> = Vec::new();
    let mut cur = Word::default();
    let mut text = String::new();
    let mut stack: Vec<Fmt> = vec![Fmt::default()];
    let mut i = 0;
    let fmt = |stack: &Vec<Fmt>| *stack.last().unwrap();
    let flush_text = |cur: &mut Word, text: &mut String, f: Fmt| {
        if !text.is_empty() {
            cur.pieces.push(Piece::Text(std::mem::take(text), f));
        }
    };
    while i < c.len() {
        let ch = c[i];
        if ch == '<' {
            let Some(end) = c[i..].iter().position(|&x| x == '>').map(|p| p + i) else {
                text.push(ch);
                i += 1;
                continue;
            };
            let tag: String = c[i + 1..end].iter().collect();
            i = end + 1;
            let closing = tag.starts_with('/');
            let name = tag.trim_start_matches('/').split([' ', '/']).next().unwrap_or("").to_lowercase();
            let class = tag.split("class=\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("");
            match (name.as_str(), closing) {
                ("br", _) => {
                    flush_text(&mut cur, &mut text, fmt(&stack));
                    cur.brk = true;
                    words.push(std::mem::take(&mut cur));
                }
                ("p", true) => {
                    flush_text(&mut cur, &mut text, fmt(&stack));
                    cur.brk = true;
                    cur.para = true;
                    words.push(std::mem::take(&mut cur));
                }
                ("p", false) => {}
                ("code", false) => {
                    flush_text(&mut cur, &mut text, fmt(&stack));
                    // Maths up to </code>.
                    let rest: String = c[i..].iter().collect();
                    let stop = rest.find("</code>").unwrap_or(rest.len());
                    let tex = crate::markup::unescape_html(&rest[..stop]);
                    cur.pieces.push(Piece::Math(tex));
                    i += rest[..stop].chars().count() + "</code>".len().min(rest.len() - stop);
                }
                ("span", false) if class.contains("hint-blank") => {
                    flush_text(&mut cur, &mut text, fmt(&stack));
                    cur.pieces.push(Piece::Blank);
                    let rest: String = c[i..].iter().collect();
                    let stop = rest.find("</span>").map(|p| p + "</span>".len()).unwrap_or(rest.len());
                    i += rest[..stop].chars().count();
                }
                (_, false) => {
                    flush_text(&mut cur, &mut text, fmt(&stack));
                    let mut f = fmt(&stack);
                    match name.as_str() {
                        "strong" | "b" => f.bold = true,
                        "em" | "i" => f.italic = true,
                        "span" if class.contains("hint-shape") => {
                            f.mono = true;
                            f.bold = true;
                        }
                        "span" if class.contains("hint-chip") => f.chip = true,
                        "span" if class.contains("muted") => f.muted = true,
                        _ => {}
                    }
                    stack.push(f);
                }
                (_, true) => {
                    flush_text(&mut cur, &mut text, fmt(&stack));
                    if stack.len() > 1 {
                        stack.pop();
                    }
                }
            }
            continue;
        }
        if ch == '&' {
            if let Some(semi) = c[i..].iter().take(8).position(|&x| x == ';') {
                let name: String = c[i + 1..i + semi].iter().collect();
                if let Some(e) = entity(&name) {
                    push_char(e, &mut cur, &mut text, &mut words, fmt(&stack));
                    i += semi + 1;
                    continue;
                }
            }
        }
        push_char(ch, &mut cur, &mut text, &mut words, fmt(&stack));
        i += 1;
    }
    flush_text(&mut cur, &mut text, fmt(&stack));
    if !cur.pieces.is_empty() || !cur.space.is_empty() {
        words.push(cur);
    }
    words
}

fn push_char(ch: char, cur: &mut Word, text: &mut String, words: &mut Vec<Word>, f: Fmt) {
    if ch.is_whitespace() && ch != '\u{a0}' && ch != '\u{202f}' && !f.chip {
        if !text.is_empty() {
            cur.pieces.push(Piece::Text(std::mem::take(text), f));
        }
        if cur.pieces.is_empty() && !words.is_empty() {
            // more space after the previous word
            if let Some(last) = words.last_mut() {
                if !last.brk {
                    last.space.push(ch);
                    return;
                }
            }
        }
        cur.space.push(ch);
        words.push(std::mem::take(cur));
    } else {
        if !cur.space.is_empty() {
            words.push(std::mem::take(cur));
        }
        text.push(ch);
    }
}

// ---------------------------------------------------------------- style

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Family {
    Sans,
    Serif,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub size: f32,
    pub color: Color32,
    pub muted: Color32,
    pub weight: W,
    pub italic: bool,
    pub family: Family,
    /// Line height as a multiple of the font size.
    pub line: f32,
    pub align: Align,
    pub display_math: bool,
    /// Chip colours (background, border).
    pub chip_bg: Color32,
    pub chip_border: Color32,
}

impl Style {
    pub fn new(size: f32, color: Color32, muted: Color32) -> Style {
        Style {
            size,
            color,
            muted,
            weight: W::Regular,
            italic: false,
            family: Family::Sans,
            line: 1.6,
            align: Align::Min,
            display_math: false,
            chip_bg: Color32::TRANSPARENT,
            chip_border: muted,
        }
    }
    pub fn weight(mut self, w: W) -> Self {
        self.weight = w;
        self
    }
    pub fn line(mut self, l: f32) -> Self {
        self.line = l;
        self
    }
    pub fn color(mut self, c: Color32) -> Self {
        self.color = c;
        self
    }
    pub fn center(mut self) -> Self {
        self.align = Align::Center;
        self
    }
    pub fn serif(mut self) -> Self {
        self.family = Family::Serif;
        self
    }
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
    pub fn chips(mut self, bg: Color32, border: Color32) -> Self {
        self.chip_bg = bg;
        self.chip_border = border;
        self
    }

    fn font(&self, f: Fmt) -> FontId {
        let size = if f.chip { self.size * 0.88 } else { self.size };
        if f.mono {
            return FontId::new(size * 0.95, FontFamily::Monospace);
        }
        let bold = f.bold || matches!(self.weight, W::Semi | W::Bold | W::Heavy);
        let italic = f.italic || self.italic;
        match self.family {
            Family::Serif => FontId::new(
                size,
                if bold { theme::serif_bold() } else if italic { theme::serif_italic() } else { theme::serif() },
            ),
            Family::Sans => {
                let w = if f.bold {
                    match self.weight {
                        W::Regular | W::Medium => W::Semi,
                        W::Semi => W::Bold,
                        w => w,
                    }
                } else {
                    self.weight
                };
                FontId::new(size, theme::family(w, italic))
            }
        }
    }

    fn key(&self, h: &mut impl Hasher) {
        self.size.to_bits().hash(h);
        self.color.hash(h);
        self.muted.hash(h);
        self.weight.hash(h);
        self.italic.hash(h);
        (self.family == Family::Serif).hash(h);
        self.line.to_bits().hash(h);
        (self.align == Align::Center).hash(h);
        self.display_math.hash(h);
        self.chip_bg.hash(h);
    }
}

// ---------------------------------------------------------------- layout

#[derive(Clone)]
pub enum Item {
    Text { pos: Pos2, galley: Arc<Galley> },
    Math { rect: Rect, tex: Arc<images::MathTex> },
    Blank { rect: Rect },
    Chip { rect: Rect },
}

#[derive(Clone, Debug)]
pub struct WordBox {
    pub rect: Rect,
    pub line: usize,
    /// The word's text (without maths), for read-along timing.
    pub text: String,
    /// The TeX of maths in the word.
    pub math: Vec<String>,
}

pub struct Laid {
    pub size: Vec2,
    pub items: Vec<Item>,
    pub words: Vec<WordBox>,
    pub lines: Vec<(f32, f32)>,
}

#[derive(Clone, Copy)]
struct Metrics {
    ascent: f32,
    height: f32,
}

thread_local! {
    static LAYOUTS: RefCell<HashMap<u64, Arc<Laid>>> = RefCell::new(HashMap::new());
    static METRICS: RefCell<HashMap<(u32, String), Metrics>> = RefCell::new(HashMap::new());
}

fn metrics(ctx: &Context, font: &FontId) -> Metrics {
    let key = (font.size.to_bits(), format!("{:?}", font.family));
    if let Some(m) = METRICS.with(|m| m.borrow().get(&key).copied()) {
        return m;
    }
    let g = ctx.fonts_mut(|f| f.layout_no_wrap("Hg".into(), font.clone(), Color32::WHITE));
    let m = g
        .rows
        .first()
        .and_then(|r| r.row.glyphs.first().map(|gl| Metrics { ascent: gl.font_ascent, height: gl.font_height }))
        .unwrap_or(Metrics { ascent: font.size * 0.97, height: font.size * 1.21 });
    METRICS.with(|mm| mm.borrow_mut().insert(key, m));
    m
}

pub fn clear_cache() {
    LAYOUTS.with(|l| l.borrow_mut().clear());
    METRICS.with(|l| l.borrow_mut().clear());
}

struct Placed {
    w: f32,
    asc: f32,
    desc: f32,
    parts: Vec<(f32, PartKind)>, // x offset within the word
}

enum PartKind {
    Text(Arc<Galley>, f32), // galley, its baseline offset from top
    Math(Arc<images::MathTex>),
    Blank(f32, f32),
    Chip(Arc<Galley>, f32, f32, f32), // galley, baseline, pad x, height
}

/// Lay out `html` in `width` points.
pub fn layout(ctx: &Context, html: &str, st: &Style, width: f32) -> Arc<Laid> {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    html.hash(&mut h);
    st.key(&mut h);
    (width.round() as i32).hash(&mut h);
    ctx.pixels_per_point().to_bits().hash(&mut h);
    let key = h.finish();
    if let Some(l) = LAYOUTS.with(|l| l.borrow().get(&key).cloned()) {
        return l;
    }
    let laid = Arc::new(do_layout(ctx, &parse(html), st, width));
    LAYOUTS.with(|l| {
        let mut l = l.borrow_mut();
        if l.len() > 6000 {
            l.clear();
        }
        l.insert(key, laid.clone());
    });
    laid
}

fn do_layout(ctx: &Context, words: &[Word], st: &Style, width: f32) -> Laid {
    let base = st.font(Fmt::default());
    let bm = metrics(ctx, &base);
    let line_h = st.size * st.line;
    let half = (line_h - bm.height) / 2.0;
    let strut_asc = half + bm.ascent;
    let strut_desc = line_h - strut_asc;
    let math_size = st.size * 1.08;
    // Measure every word.
    let mut placed: Vec<(Placed, f32, &Word)> = Vec::new(); // (word, trailing space width)
    for w in words {
        let mut x = 0.0;
        let mut asc: f32 = 0.0;
        let mut desc: f32 = 0.0;
        let mut parts = Vec::new();
        for p in &w.pieces {
            match p {
                Piece::Text(t, f) => {
                    let font = st.font(*f);
                    let m = metrics(ctx, &font);
                    let color = if f.muted { st.muted } else { st.color };
                    let g = ctx.fonts_mut(|fs| fs.layout_no_wrap(t.clone(), font.clone(), color));
                    if f.chip {
                        let padx = st.size * 0.55 * 0.88;
                        let ch = m.height + 2.0;
                        let gw = g.size().x;
                        parts.push((x, PartKind::Chip(g, m.ascent, padx, ch)));
                        x += gw + 2.0 * padx + st.size * 0.2;
                        asc = asc.max(m.ascent + 1.0);
                        desc = desc.max(ch - m.ascent);
                    } else {
                        let gw = g.size().x;
                        parts.push((x, PartKind::Text(g, m.ascent)));
                        x += gw;
                        asc = asc.max(m.ascent);
                        desc = desc.max(m.height - m.ascent);
                    }
                }
                Piece::Math(tex) => {
                    let mt = images::math(ctx, tex, st.display_math, math_size);
                    asc = asc.max(mt.h + 1.0);
                    desc = desc.max(mt.d + 1.0);
                    let mw = mt.w;
                    parts.push((x, PartKind::Math(mt)));
                    x += mw;
                }
                Piece::Blank => {
                    let bw = st.size * 3.2;
                    let bh = st.size * 0.95;
                    x += st.size * 0.15;
                    parts.push((x, PartKind::Blank(bw, bh)));
                    x += bw + st.size * 0.15;
                    asc = asc.max(bh);
                }
            }
        }
        let space_w = if w.space.is_empty() {
            0.0
        } else {
            ctx.fonts_mut(|fs| w.space.chars().map(|c| fs.glyph_width(&base, c)).sum::<f32>())
        };
        placed.push((Placed { w: x, asc, desc, parts }, space_w, w));
    }
    // Break into lines.
    let mut lines: Vec<Vec<usize>> = vec![vec![]];
    let mut para_after: Vec<bool> = vec![false];
    let mut x = 0.0;
    for (i, (p, sw, w)) in placed.iter().enumerate() {
        let cur = lines.last().unwrap();
        if !cur.is_empty() && x + p.w > width + 0.5 {
            lines.push(vec![]);
            para_after.push(false);
            x = 0.0;
        }
        lines.last_mut().unwrap().push(i);
        x += p.w + sw;
        if w.brk {
            *para_after.last_mut().unwrap() = w.para;
            lines.push(vec![]);
            para_after.push(false);
            x = 0.0;
        }
    }
    if lines.last().is_some_and(|l| l.is_empty()) && lines.len() > 1 {
        lines.pop();
        para_after.pop();
    }
    // Place lines.
    let mut items = Vec::new();
    let mut wboxes = Vec::new();
    let mut line_spans = Vec::new();
    let mut y = 0.0;
    let mut max_w: f32 = 0.0;
    for (li, line) in lines.iter().enumerate() {
        let asc = line.iter().map(|&i| placed[i].0.asc).fold(strut_asc, f32::max);
        let desc = line.iter().map(|&i| placed[i].0.desc).fold(strut_desc, f32::max);
        let base_y = y + asc;
        // Width of the line without its trailing space.
        let mut lw = 0.0;
        for (k, &i) in line.iter().enumerate() {
            lw += placed[i].0.w;
            if k + 1 < line.len() {
                lw += placed[i].1;
            }
        }
        max_w = max_w.max(lw);
        let mut x = match st.align {
            Align::Center => ((width - lw) / 2.0).max(0.0),
            Align::Max => (width - lw).max(0.0),
            Align::Min => 0.0,
        };
        for &i in line {
            let (p, sw, w) = &placed[i];
            let mut text = String::new();
            let mut math = Vec::new();
            for pc in &w.pieces {
                match pc {
                    Piece::Text(t, _) => text.push_str(t),
                    Piece::Math(m) => math.push(m.clone()),
                    Piece::Blank => {}
                }
            }
            for (dx, part) in &p.parts {
                let px = x + dx;
                match part {
                    PartKind::Text(g, a) => items.push(Item::Text { pos: pos2(px, base_y - a), galley: g.clone() }),
                    PartKind::Math(mt) => items.push(Item::Math { rect: Rect::from_min_size(pos2(px - mt.pad, base_y - mt.h - mt.pad), vec2(mt.w + 2.0 * mt.pad, mt.h + mt.d + 2.0 * mt.pad)), tex: mt.clone() }),
                    PartKind::Blank(bw, bh) => items.push(Item::Blank { rect: Rect::from_min_size(pos2(px, base_y - bh), vec2(*bw, *bh)) }),
                    PartKind::Chip(g, a, padx, ch) => {
                        let r = Rect::from_min_size(pos2(px, base_y - a - 1.0), vec2(g.size().x + 2.0 * padx, *ch));
                        items.push(Item::Chip { rect: r });
                        items.push(Item::Text { pos: pos2(px + padx, base_y - a), galley: g.clone() });
                    }
                }
            }
            let wh = st.size * 1.25;
            wboxes.push(WordBox { rect: Rect::from_min_size(pos2(x, base_y - st.size * 0.95), vec2(p.w, wh.max(p.asc.min(st.size * 2.0)))), line: li, text, math });
            x += p.w + sw;
        }
        line_spans.push((y, y + asc + desc));
        y += asc + desc;
        if para_after[li] {
            y += st.size * 0.6;
        }
    }
    Laid { size: vec2(if st.align == Align::Min { max_w } else { width }, y), items, words: wboxes, lines: line_spans }
}

/// Paint a layout with its top-left at `origin`.
pub fn paint(painter: &Painter, laid: &Laid, origin: Pos2, st: &Style) {
    let off = origin.to_vec2();
    for it in &laid.items {
        match it {
            Item::Text { pos, galley } => {
                painter.galley(*pos + off, galley.clone(), st.color);
            }
            Item::Math { rect, tex } => {
                painter.image(tex.tex.id(), rect.translate(off), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), st.color);
            }
            Item::Blank { rect } => {
                let r = rect.translate(off);
                painter.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(2.0, st.color));
            }
            Item::Chip { rect } => {
                let r = rect.translate(off);
                painter.rect(r, CornerRadius::same(255), st.chip_bg, Stroke::new(1.0, st.chip_border), StrokeKind::Inside);
            }
        }
    }
}

/// Lay out and draw rich text in the available width; returns its response and layout.
pub fn show(ui: &mut Ui, html: &str, st: &Style) -> (Response, Arc<Laid>) {
    let width = ui.available_width().max(10.0);
    let laid = layout(ui.ctx(), html, st, width);
    let w = if st.align == Align::Min { laid.size.x.min(width) } else { width };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, laid.size.y), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint(ui.painter(), &laid, rect.min, st);
    }
    (resp, laid)
}

/// Plain text (no markup) in one style.
pub fn plain(ui: &mut Ui, text: &str, st: &Style) -> Response {
    show(ui, &crate::markup::escape_html(text), st).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_markup() {
        let w = parse("a <strong>bold</strong>, <code>x^2</code>!<br>next &amp; <span class=\"hint-blank\" aria-label=\"x\">\u{2003}</span>.");
        assert_eq!(w[0].pieces, vec![Piece::Text("a".into(), Fmt::default())]);
        assert_eq!(w[0].space, " ");
        assert!(matches!(&w[1].pieces[0], Piece::Text(t, f) if t == "bold" && f.bold));
        assert!(matches!(&w[2].pieces[0], Piece::Math(t) if t == "x^2"));
        assert!(w[2].brk);
        assert!(matches!(&w[4].pieces[0], Piece::Text(t, _) if t == "&"));
        assert_eq!(w[5].pieces, vec![Piece::Blank, Piece::Text(".".into(), Fmt::default())]);
    }
}
