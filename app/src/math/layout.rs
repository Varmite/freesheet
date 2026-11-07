//! TeX-style layout of a parsed formula, in em, using KaTeX's fonts and font parameters.
//! Coordinates: x to the right, y upwards, origin on the baseline at the left edge.

use super::parse::{Accent, Class, Font, Node};
use std::sync::OnceLock;
use ttf_parser::{Face, GlyphId};

// ---------------------------------------------------------------- fonts

pub const FACE_MAIN: usize = 0;
pub const FACE_MAIN_BOLD: usize = 1;
pub const FACE_MATH: usize = 2;
pub const FACE_AMS: usize = 3;
pub const FACE_SIZE1: usize = 4;
pub const FACE_SIZE2: usize = 5;
pub const FACE_SIZE3: usize = 6;
pub const FACE_SIZE4: usize = 7;
pub const FACE_INTER: usize = 8;

static FONT_DATA: [&[u8]; 9] = [
    include_bytes!("../../assets/fonts/KaTeX_Main-Regular.ttf"),
    include_bytes!("../../assets/fonts/KaTeX_Main-Bold.ttf"),
    include_bytes!("../../assets/fonts/KaTeX_Math-Italic.ttf"),
    include_bytes!("../../assets/fonts/KaTeX_AMS-Regular.ttf"),
    include_bytes!("../../assets/fonts/KaTeX_Size1-Regular.ttf"),
    include_bytes!("../../assets/fonts/KaTeX_Size2-Regular.ttf"),
    include_bytes!("../../assets/fonts/KaTeX_Size3-Regular.ttf"),
    include_bytes!("../../assets/fonts/KaTeX_Size4-Regular.ttf"),
    include_bytes!("../../assets/fonts/Inter-Regular.ttf"),
];

pub fn faces() -> &'static [Face<'static>] {
    static FACES: OnceLock<Vec<Face<'static>>> = OnceLock::new();
    FACES.get_or_init(|| FONT_DATA.iter().map(|d| Face::parse(d, 0).expect("font")).collect())
}

// KaTeX's font parameters (em, text size).
const XHEIGHT: f32 = 0.431;
const AXIS: f32 = 0.25;
const RULE: f32 = 0.04;
const NUM1: f32 = 0.677;
const NUM2: f32 = 0.394;
const DENOM1: f32 = 0.686;
const DENOM2: f32 = 0.345;
const SUP1: f32 = 0.413;
const SUP2: f32 = 0.363;
const SUP3: f32 = 0.289;
const SUB1: f32 = 0.15;
const SUB2: f32 = 0.247;
const SUP_DROP: f32 = 0.386;
const SUB_DROP: f32 = 0.05;
const NULL_DELIM: f32 = 0.12;
const SCRIPT_SPACE: f32 = 0.05;

// ---------------------------------------------------------------- boxes

#[derive(Clone, Debug)]
pub enum Item {
    Glyph { face: usize, gid: GlyphId, x: f32, y: f32, size: f32, sy: f32, red: bool },
    Rule { x: f32, y: f32, w: f32, h: f32 },
    /// A stroked polyline (radicals, braces, arrows).
    Stroke { pts: Vec<(f32, f32)>, width: f32 },
}

#[derive(Clone, Debug, Default)]
pub struct Bx {
    pub w: f32,
    pub h: f32,
    pub d: f32,
    pub items: Vec<Item>,
    /// Italic correction (for superscripts after italic letters).
    pub italic: f32,
}

impl Bx {
    fn empty() -> Bx {
        Bx::default()
    }
    /// Place `other` with its origin at (x, y) in this box's coordinates.
    fn put(&mut self, other: &Bx, x: f32, y: f32) {
        for it in &other.items {
            self.items.push(match it {
                Item::Glyph { face, gid, x: gx, y: gy, size, sy, red } => Item::Glyph { face: *face, gid: *gid, x: gx + x, y: gy + y, size: *size, sy: *sy, red: *red },
                Item::Rule { x: rx, y: ry, w, h } => Item::Rule { x: rx + x, y: ry + y, w: *w, h: *h },
                Item::Stroke { pts, width } => Item::Stroke { pts: pts.iter().map(|(px, py)| (px + x, py + y)).collect(), width: *width },
            });
        }
        self.h = self.h.max(y + other.h);
        self.d = self.d.max(other.d - y);
    }
    fn hpack(parts: Vec<Bx>) -> Bx {
        let mut out = Bx::empty();
        let mut x = 0.0;
        for p in &parts {
            out.put(p, x, 0.0);
            x += p.w;
        }
        out.w = x;
        out.italic = parts.last().map_or(0.0, |p| p.italic);
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    D,
    T,
    S,
    SS,
}

impl Style {
    fn scale(self) -> f32 {
        match self {
            Style::D | Style::T => 1.0,
            Style::S => 0.7,
            Style::SS => 0.5,
        }
    }
    fn sup(self) -> Style {
        match self {
            Style::D | Style::T => Style::S,
            _ => Style::SS,
        }
    }
    fn frac(self) -> Style {
        match self {
            Style::D => Style::T,
            Style::T => Style::S,
            _ => Style::SS,
        }
    }
    fn is_script(self) -> bool {
        matches!(self, Style::S | Style::SS)
    }
}

fn glyph_box(face_ix: usize, c: char, style: Style, red: bool) -> Option<Bx> {
    let face = &faces()[face_ix];
    let gid = face.glyph_index(c)?;
    let upem = face.units_per_em() as f32;
    let s = style.scale();
    let adv = face.glyph_hor_advance(gid).unwrap_or(0) as f32 / upem;
    let bb = face.glyph_bounding_box(gid);
    let (h, d, xmax) = bb.map_or((0.0, 0.0, adv), |b| (b.y_max as f32 / upem, -(b.y_min as f32) / upem, b.x_max as f32 / upem));
    let italic = if face_ix == FACE_MATH { (xmax - adv).max(0.0) * s } else { 0.0 };
    Some(Bx { w: adv * s, h: h.max(0.0) * s, d: d.max(0.0) * s, italic, items: vec![Item::Glyph { face: face_ix, gid, x: 0.0, y: 0.0, size: s, sy: 1.0, red }] })
}

fn order(font: Font) -> &'static [usize] {
    match font {
        Font::Main | Font::Text => &[FACE_MAIN, FACE_AMS, FACE_SIZE1, FACE_MATH, FACE_INTER],
        Font::MainBold => &[FACE_MAIN_BOLD, FACE_MAIN, FACE_AMS, FACE_INTER],
        Font::MathItalic => &[FACE_MATH, FACE_MAIN, FACE_AMS, FACE_INTER],
        Font::Ams => &[FACE_AMS, FACE_MAIN, FACE_MATH, FACE_INTER],
    }
}

fn char_box(c: char, font: Font, style: Style, red: bool) -> Bx {
    if c != '≠' {
        for &f in order(font) {
            if let Some(b) = glyph_box(f, c, style, red) {
                return b;
            }
        }
    }
    if c == '≠' {
        // = with a stroke through it.
        let mut eq = char_box('=', Font::Main, style, red);
        let s = style.scale();
        eq.items.push(Item::Stroke { pts: vec![(eq.w * 0.62, 0.62 * s), (eq.w * 0.38, -0.12 * s)], width: RULE * 1.1 * s });
        eq.h = eq.h.max(0.62 * s);
        eq.d = eq.d.max(0.12 * s);
        return eq;
    }
    // Nothing has it: a gap the width of an "n".
    let mut b = char_box('n', Font::Main, style, red);
    b.items.clear();
    b
}

fn text_box(text: &str, bold: bool, style: Style, red: bool) -> Bx {
    let font = if bold { Font::MainBold } else { Font::Text };
    let parts: Vec<Bx> = text
        .chars()
        .map(|c| match c {
            ' ' | '\u{a0}' => Bx { w: 0.25 * style.scale(), ..Bx::empty() },
            '\u{2009}' => Bx { w: 3.0 / 18.0 * style.scale(), ..Bx::empty() },
            '\u{2005}' => Bx { w: 5.0 / 18.0 * style.scale(), ..Bx::empty() },
            c => char_box(c, font, style, red),
        })
        .collect();
    let mut b = Bx::hpack(parts);
    b.italic = 0.0;
    b
}

// ---------------------------------------------------------------- spacing

fn space(left: Class, right: Class, style: Style) -> f32 {
    use Class::*;
    // TeX's inter-atom spacing: (mu, also in script styles?)
    let (mu, script) = match (left, right) {
        (Ord, Op) | (Op, Ord) | (Op, Op) | (Close, Op) | (Inner, Op) => (3, true),
        (Ord, Bin) | (Bin, Ord) | (Bin, Op) | (Bin, Open) | (Bin, Inner) | (Close, Bin) | (Inner, Bin) => (4, false),
        (Ord, Rel) | (Op, Rel) | (Rel, Ord) | (Rel, Op) | (Rel, Open) | (Rel, Inner) | (Close, Rel) | (Inner, Rel) => (5, false),
        (Ord, Inner) | (Op, Inner) | (Close, Inner) | (Inner, Ord) | (Inner, Open) | (Inner, Punct) | (Inner, Inner) => (3, false),
        (Punct, Ord) | (Punct, Op) | (Punct, Rel) | (Punct, Open) | (Punct, Close) | (Punct, Punct) | (Punct, Inner) => (3, false),
        _ => (0, false),
    };
    if mu == 0 || (style.is_script() && !script) {
        0.0
    } else {
        mu as f32 / 18.0 * style.scale()
    }
}

// ---------------------------------------------------------------- layout

struct Atom {
    bx: Bx,
    class: Option<Class>,
}

pub fn layout_list(nodes: &[Node], style: Style) -> Bx {
    let mut style = style;
    let mut atoms: Vec<Atom> = Vec::new();
    for n in nodes {
        if let Node::DisplayStyle = n {
            style = Style::D;
            continue;
        }
        let (bx, class) = layout_node(n, style);
        atoms.push(Atom { bx, class });
    }
    // Binary operators at the start or after another operator act as ordinary symbols.
    let mut prev: Option<Class> = None;
    let n = atoms.len();
    for i in 0..n {
        let Some(c) = atoms[i].class else { continue };
        if c == Class::Bin {
            let next = atoms[i + 1..].iter().find_map(|a| a.class);
            let bad_prev = matches!(prev, None | Some(Class::Bin) | Some(Class::Op) | Some(Class::Rel) | Some(Class::Open) | Some(Class::Punct));
            let bad_next = matches!(next, None | Some(Class::Rel) | Some(Class::Close) | Some(Class::Punct));
            if bad_prev || bad_next {
                atoms[i].class = Some(Class::Ord);
            }
        }
        prev = atoms[i].class;
    }
    let mut parts = Vec::new();
    let mut prev: Option<Class> = None;
    for a in atoms {
        if let (Some(p), Some(c)) = (prev, a.class) {
            let s = space(p, c, style);
            if s != 0.0 {
                parts.push(Bx { w: s, ..Bx::empty() });
            }
        }
        if a.class.is_some() {
            prev = a.class;
        }
        parts.push(a.bx);
    }
    Bx::hpack(parts)
}

fn node_class(n: &Node) -> Class {
    match n {
        Node::Char { class, .. } | Node::Text { class, .. } | Node::Big { class, .. } => *class,
        Node::BigOp { .. } | Node::Limits { .. } | Node::Underbrace { .. } => Class::Op,
        Node::Delimited { .. } => Class::Inner,
        Node::XArrow { .. } => Class::Rel,
        _ => Class::Ord,
    }
}

fn is_single_char(n: &Node) -> bool {
    match n {
        Node::Char { .. } => true,
        Node::Group(g) => g.len() == 1 && matches!(g[0], Node::Char { .. }),
        _ => false,
    }
}

fn layout_node(n: &Node, style: Style) -> (Bx, Option<Class>) {
    let s = style.scale();
    let class = Some(node_class(n));
    let bx = match n {
        Node::Char { ch, font, .. } => char_box(*ch, *font, style, false),
        Node::Text { text, bold, .. } => text_box(text, *bold, style, false),
        Node::Error(src) => text_box(src, false, style, true),
        Node::Group(g) => layout_list(g, style),
        Node::Space(em) => return (Bx { w: em * s, ..Bx::empty() }, None),
        Node::DisplayStyle => return (Bx::empty(), None),
        Node::Frac { num, den, display } => frac(num, den, display.map(|d| if d { Style::D } else { Style::T }).unwrap_or(style)),
        Node::Sqrt { index, body } => sqrt(index.as_deref(), body, style),
        Node::Scripts { base, sup, sub } => scripts(base, sup.as_deref(), sub.as_deref(), style),
        Node::Accent { kind, body } => accent(kind, body, style),
        Node::Delimited { left, body, right } => {
            let inner = layout_list(body, style);
            let l = delim(*left, inner.h, inner.d, style);
            let r = delim(*right, inner.h, inner.d, style);
            Bx::hpack(vec![l, inner, r])
        }
        Node::Big { ch, size, .. } => {
            let target = [0.0, 1.2, 1.8, 2.4, 3.0][*size as usize] * s;
            delim_sized(*ch, target, style)
        }
        Node::BigOp { ch } => big_op(*ch, style),
        Node::Limits { text } => text_box(text, false, style, false),
        Node::Underbrace { body, label } => underbrace(body, label.as_deref(), style),
        Node::Boxed(body) => {
            let inner = layout_list(body, style);
            let pad = 0.3 * s;
            let t = RULE * s;
            let mut b = Bx::empty();
            b.put(&inner, pad + t, 0.0);
            b.w = inner.w + 2.0 * (pad + t);
            b.h = inner.h + pad + t;
            b.d = inner.d + pad + t;
            b.items.push(Item::Rule { x: 0.0, y: -b.d, w: b.w, h: t });
            b.items.push(Item::Rule { x: 0.0, y: b.h - t, w: b.w, h: t });
            b.items.push(Item::Rule { x: 0.0, y: -b.d, w: t, h: b.h + b.d });
            b.items.push(Item::Rule { x: b.w - t, y: -b.d, w: t, h: b.h + b.d });
            b
        }
        Node::XArrow { above } => {
            let label = layout_list(above, style.sup());
            let w = (label.w + 0.8 * s).max(1.4 * s);
            let mut b = Bx::empty();
            let y = AXIS * s;
            b.put(&label, (w - label.w) / 2.0, y + 0.12 * s + label.d);
            b.items.push(Item::Rule { x: 0.05 * s, y: y - RULE * s / 2.0, w: w - 0.1 * s, h: RULE * s });
            b.items.push(Item::Stroke { pts: vec![(w - 0.28 * s, y + 0.16 * s), (w - 0.05 * s, y), (w - 0.28 * s, y - 0.16 * s)], width: RULE * s * 1.1 });
            b.w = w;
            b.h = b.h.max(y + 0.2 * s);
            b.d = b.d.max(0.0);
            b
        }
    };
    (bx, class)
}

fn frac(num: &[Node], den: &[Node], style: Style) -> Bx {
    let s = style.scale();
    let n = layout_list(num, style.frac());
    let d = layout_list(den, style.frac());
    let t = RULE * s;
    let axis = AXIS * s;
    let display = style == Style::D;
    let (mut u, mut v, phi) = if display { (NUM1 * s, DENOM1 * s, 3.0 * t) } else { (NUM2 * s, DENOM2 * s, t) };
    let gap_num = (u - n.d) - (axis + t / 2.0);
    if gap_num < phi {
        u += phi - gap_num;
    }
    let gap_den = (axis - t / 2.0) - (d.h - v);
    if gap_den < phi {
        v += phi - gap_den;
    }
    let w = n.w.max(d.w);
    let pad = NULL_DELIM * s;
    let mut b = Bx::empty();
    b.put(&n, pad + (w - n.w) / 2.0, u);
    b.put(&d, pad + (w - d.w) / 2.0, -v);
    b.items.push(Item::Rule { x: pad, y: axis - t / 2.0, w, h: t });
    b.w = w + 2.0 * pad;
    b.h = b.h.max(axis + t / 2.0);
    b.d = b.d.max(0.0);
    b
}

fn sqrt(index: Option<&[Node]>, body: &[Node], style: Style) -> Bx {
    let s = style.scale();
    let inner = layout_list(body, style);
    let t = RULE * s;
    let phi = if style == Style::D { XHEIGHT * s } else { t };
    let gap = t + phi / 4.0;
    let top = inner.h.max(0.62 * s) + gap + t; // the bar's top
    let bottom = inner.d.max(0.05 * s);
    let total = top + bottom;
    let sw = (0.55 * s + 0.06 * total).min(1.0 * s);
    let mid = -bottom + total * 0.42;
    let mut b = Bx::empty();
    let mut x0 = 0.0;
    if let Some(ix) = index {
        let ib = layout_list(ix, Style::SS);
        let raise = -bottom + total * 0.6;
        b.put(&ib, 0.0, raise + ib.d);
        x0 = (ib.w - 0.32 * sw).max(0.0);
    }
    let stroke = t * 1.15;
    b.items.push(Item::Stroke {
        pts: vec![
            (x0 + 0.04 * sw, mid + 0.02 * s),
            (x0 + 0.2 * sw, mid + 0.12 * s),
            (x0 + 0.5 * sw, -bottom + stroke / 2.0),
            (x0 + sw, top - t / 2.0),
            (x0 + sw + inner.w + 0.06 * s, top - t / 2.0),
        ],
        width: stroke,
    });
    b.put(&inner, x0 + sw + 0.03 * s, 0.0);
    b.w = x0 + sw + inner.w + 0.1 * s;
    b.h = b.h.max(top);
    b.d = b.d.max(bottom);
    b
}

fn scripts(base: &Node, sup: Option<&[Node]>, sub: Option<&[Node]>, style: Style) -> Bx {
    let s = style.scale();
    let limits = style == Style::D && matches!(base, Node::BigOp { .. } | Node::Limits { .. } | Node::Underbrace { .. });
    let (b, _) = layout_node(base, style);
    let sup_b = sup.map(|x| layout_list(x, style.sup()));
    let sub_b = sub.map(|x| layout_list(x, style.sup()));
    if limits {
        let mut out = Bx::empty();
        let w = b.w.max(sup_b.as_ref().map_or(0.0, |x| x.w)).max(sub_b.as_ref().map_or(0.0, |x| x.w));
        out.put(&b, (w - b.w) / 2.0, 0.0);
        if let Some(x) = &sup_b {
            out.put(x, (w - x.w) / 2.0, b.h + 0.111 * s + x.d.max(0.2 * s - 0.111 * s));
        }
        if let Some(x) = &sub_b {
            out.put(x, (w - x.w) / 2.0, -(b.d + 0.166 * s + x.h.max(0.6 * s - 0.166 * s - x.d)));
        }
        out.w = w;
        return out;
    }
    let single = is_single_char(base);
    let sup_scale = style.sup().scale();
    let (u0, v0) = if single { (0.0, 0.0) } else { (b.h - SUP_DROP * sup_scale, b.d + SUB_DROP * sup_scale) };
    let mut out = Bx::empty();
    out.put(&b, 0.0, 0.0);
    let x = b.w;
    match (&sup_b, &sub_b) {
        (Some(p), None) => {
            let pv = if style == Style::D { SUP1 } else { SUP2 } * s;
            let u = u0.max(pv).max(p.d + 0.25 * XHEIGHT * s);
            out.put(p, x + b.italic, u);
            out.w = x + b.italic + p.w + SCRIPT_SPACE * s;
        }
        (None, Some(q)) => {
            let v = v0.max(SUB1 * s).max(q.h - 0.8 * XHEIGHT * s);
            out.put(q, x, -v);
            out.w = x + q.w + SCRIPT_SPACE * s;
        }
        (Some(p), Some(q)) => {
            let pv = if style == Style::D { SUP1 } else { SUP2 } * s;
            let mut u = u0.max(pv).max(p.d + 0.25 * XHEIGHT * s);
            let mut v = v0.max(SUB2 * s);
            let t = RULE * s;
            let gap = (u - p.d) - (q.h - v);
            if gap < 4.0 * t {
                v += 4.0 * t - gap;
                let psi = 0.8 * XHEIGHT * s - (u - p.d);
                if psi > 0.0 {
                    u += psi;
                    v -= psi;
                }
            }
            out.put(p, x + b.italic, u);
            out.put(q, x, -v);
            out.w = x + (p.w + b.italic).max(q.w) + SCRIPT_SPACE * s;
        }
        (None, None) => out.w = b.w,
    }
    let _ = SUP3;
    out
}

fn accent(kind: &Accent, body: &[Node], style: Style) -> Bx {
    let s = style.scale();
    let inner = layout_list(body, style);
    let t = RULE * s;
    let mut b = Bx::empty();
    b.put(&inner, 0.0, 0.0);
    b.w = inner.w;
    b.italic = inner.italic;
    let skew = if body.len() == 1 && matches!(body[0], Node::Char { font: Font::MathItalic, .. }) { inner.italic * 0.5 + 0.05 * s } else { 0.0 };
    match kind {
        Accent::Overline => {
            b.items.push(Item::Rule { x: 0.0, y: inner.h + 3.0 * t, w: inner.w, h: t });
            b.h = inner.h + 4.0 * t;
        }
        Accent::Underline => {
            b.items.push(Item::Rule { x: 0.0, y: -(inner.d + 3.0 * t) - t, w: inner.w, h: t });
            b.d = inner.d + 4.0 * t;
        }
        Accent::Vec => {
            let y = inner.h.max(XHEIGHT * s) + 0.15 * s;
            let w = inner.w.max(0.4 * s);
            let x0 = (inner.w - w) / 2.0 + skew;
            b.items.push(Item::Rule { x: x0, y: y - t / 2.0, w, h: t });
            b.items.push(Item::Stroke { pts: vec![(x0 + w - 0.12 * s, y + 0.09 * s), (x0 + w, y), (x0 + w - 0.12 * s, y - 0.09 * s)], width: t });
            b.h = b.h.max(y + 0.1 * s);
        }
        Accent::Hat | Accent::Bar | Accent::Dot => {
            let ch = match kind {
                Accent::Hat => 'ˆ',
                Accent::Bar => 'ˉ',
                _ => '˙',
            };
            let mut a = char_box(ch, Font::Main, style, false);
            if a.items.is_empty() {
                a = char_box(match kind { Accent::Hat => '^', Accent::Bar => '¯', _ => '.' }, Font::Main, style, false);
            }
            let lift = (inner.h - XHEIGHT * s).max(0.0);
            b.put(&a, (inner.w - a.w) / 2.0 + skew, lift);
        }
    }
    b
}

/// Sources for a delimiter, smallest first.
fn delim_sources(c: char) -> &'static [usize] {
    match c {
        '(' | ')' | '[' | ']' | '{' | '}' | '⟨' | '⟩' | '⌊' | '⌋' | '⌈' | '⌉' | '/' | '\\' => &[FACE_MAIN, FACE_SIZE1, FACE_SIZE2, FACE_SIZE3, FACE_SIZE4],
        _ => &[FACE_MAIN, FACE_SIZE1],
    }
}

fn delim(c: Option<char>, h: f32, d: f32, style: Style) -> Bx {
    let s = style.scale();
    let Some(c) = c else { return Bx { w: NULL_DELIM * s, ..Bx::empty() } };
    let axis = AXIS * s;
    let dist = (h - axis).max(d + axis);
    let target = (dist * 2.0 * 0.901).max(dist * 2.0 - 0.5 * s);
    delim_sized(c, target, style)
}

fn delim_sized(c: char, target: f32, style: Style) -> Bx {
    let s = style.scale();
    let axis = AXIS * s;
    let mut best: Option<Bx> = None;
    for &f in delim_sources(c) {
        if let Some(b) = glyph_box(f, c, style, false) {
            let total = b.h + b.d;
            let done = total >= target;
            best = Some(b);
            if done {
                break;
            }
        }
    }
    let mut b = best.unwrap_or_else(|| char_box(c, Font::Main, style, false));
    let total = b.h + b.d;
    if total < target && total > 0.0 {
        // Bigger than the largest glyph: stretch it.
        let k = target / total;
        for it in &mut b.items {
            if let Item::Glyph { sy, .. } = it {
                *sy = k;
            }
        }
        b.h *= k;
        b.d *= k;
    }
    // Centre on the maths axis.
    let shift = axis - (b.h - b.d) / 2.0;
    let mut out = Bx::empty();
    out.put(&b, 0.0, shift);
    out.w = b.w;
    out
}

fn big_op(c: char, style: Style) -> Bx {
    let face = if style == Style::D { FACE_SIZE2 } else { FACE_SIZE1 };
    let b = glyph_box(face, c, style, false).unwrap_or_else(|| char_box(c, Font::Main, style, false));
    let axis = AXIS * style.scale();
    let shift = axis - (b.h - b.d) / 2.0;
    let mut out = Bx::empty();
    out.put(&b, 0.0, shift);
    out.w = b.w;
    out
}

fn underbrace(body: &[Node], label: Option<&[Node]>, style: Style) -> Bx {
    let s = style.scale();
    let inner = layout_list(body, style);
    let mut b = Bx::empty();
    b.put(&inner, 0.0, 0.0);
    b.w = inner.w;
    let top = -(inner.d + 0.1 * s);
    let depth = 0.32 * s;
    let w = inner.w;
    let r = (depth * 0.5).min(w * 0.1);
    let m = w / 2.0;
    let bot = top - depth;
    let mid = top - depth * 0.5;
    b.items.push(Item::Stroke {
        pts: vec![(0.0, top), (r * 0.3, mid + r * 0.2), (r, mid), (m - r, mid), (m - r * 0.3, mid - r * 0.2), (m, bot)],
        width: RULE * s * 1.4,
    });
    b.items.push(Item::Stroke {
        pts: vec![(w, top), (w - r * 0.3, mid + r * 0.2), (w - r, mid), (m + r, mid), (m + r * 0.3, mid - r * 0.2), (m, bot)],
        width: RULE * s * 1.4,
    });
    b.d = b.d.max(-bot);
    if let Some(l) = label {
        let lb = layout_list(l, style.sup());
        b.put(&lb, (w - lb.w) / 2.0, bot - 0.1 * s - lb.h);
        b.w = b.w.max(lb.w);
    }
    b
}
