//! Images drawn by the app: line icons, diagrams (SVG in the content) and formulas.
//! Everything is rasterised once at the screen's pixel density and kept as a texture.
//! Icons and formulas are drawn in white and tinted when painted, so one texture serves
//! every colour.

use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};
use resvg::{tiny_skia, usvg};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

// ---------------------------------------------------------------- icons

/// Line icons (24×24, stroke = currentColor), drawn for FreeSheet.
pub fn icon_path(name: &str) -> &'static str {
    match name {
        "maths" => r#"<path d="M4 7h6M7 4v6"/><path d="M14 7h6"/><path d="M4.8 14.8l4.4 4.4M9.2 14.8l-4.4 4.4"/><path d="M14 17h6"/><path d="M17 14h.01M17 20h.01"/>"#,
        "calculator" => r#"<rect x="5" y="2.5" width="14" height="19" rx="2"/><rect x="8" y="5.5" width="8" height="4" rx="0.5"/><path d="M8.5 13h.01M12 13h.01M15.5 13h.01M8.5 17h.01M12 17h.01M15.5 17h.01"/>"#,
        "flask" => r#"<path d="M9 3h6"/><path d="M10 3v6.5L4.6 18.2A1.8 1.8 0 0 0 6.1 21h11.8a1.8 1.8 0 0 0 1.5-2.8L14 9.5V3"/><path d="M7.3 15.5h9.4"/>"#,
        "atom" => r#"<circle cx="12" cy="12" r="1.2"/><ellipse cx="12" cy="12" rx="9.5" ry="3.8"/><ellipse cx="12" cy="12" rx="9.5" ry="3.8" transform="rotate(60 12 12)"/><ellipse cx="12" cy="12" rx="9.5" ry="3.8" transform="rotate(120 12 12)"/>"#,
        "leaf" => r#"<path d="M5 19C5 10 11 5 20 4c-.5 9-6 15-15 15z"/><path d="M5 19c3-4.5 6.5-7.5 10.5-10"/>"#,
        "landmark" => r#"<path d="M12 3l8.5 5h-17z"/><path d="M6 11v7M10 11v7M14 11v7M18 11v7"/><path d="M3.5 21h17"/>"#,
        "globe" => r#"<circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3c2.6 2.5 3.8 5.5 3.8 9s-1.2 6.5-3.8 9c-2.6-2.5-3.8-5.5-3.8-9S9.4 5.5 12 3z"/>"#,
        "wallet" => r#"<rect x="3" y="7" width="18" height="13" rx="2"/><path d="M5.5 7l9.5-3.5L16.5 7"/><path d="M15 13.5h3"/>"#,
        "receipt" => r#"<path d="M6 2.5h12v19l-2-1.4-2 1.4-2-1.4-2 1.4-2-1.4-2 1.4z"/><path d="M9 7.5h6M9 11h6M9 14.5h3.5"/>"#,
        "briefcase" => r#"<rect x="3" y="7" width="18" height="13" rx="2"/><path d="M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/><path d="M3 13h18"/>"#,
        "trend" => r#"<path d="M3.5 3.5v17h17"/><path d="M7.5 15l4-4 3 3 5.5-5.5"/><path d="M15.5 8.5H20V13"/>"#,
        "gear" => r#"<circle cx="12" cy="12" r="3"/><circle cx="12" cy="12" r="6.5"/><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1"/>"#,
        "compass" => r#"<circle cx="12" cy="12" r="9"/><path d="M15.8 8.2l-2.3 5.3-5.3 2.3 2.3-5.3z"/>"#,
        "book" => r#"<path d="M12 6.5C10 5 7 4.5 3 5v13c4-.5 7 0 9 1.5 2-1.5 5-2 9-1.5V5c-4-.5-7 0-9 1.5z"/><path d="M12 6.5v13"/>"#,
        "logo" => r#"<path d="M14 2.5H6.5a2 2 0 0 0-2 2v15a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V8z"/><path d="M14 2.5V8h5.5"/><path d="M8.5 14.5l2.3 2.3 4.7-4.7"/>"#,
        "study" => r#"<path d="M13 3.5H6a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-7"/><path d="M18.3 2.7a2 2 0 0 1 2.9 2.9L13 13.8l-3.6.8.8-3.6z"/>"#,
        "exams" => r#"<rect x="5" y="4" width="14" height="17.5" rx="2"/><path d="M9 2.5h6v3H9z"/><path d="M9 13.5l2 2 4-4"/>"#,
        "speaker" => r#"<path d="M11 5L6 9H3v6h3l5 4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7"/><path d="M18.5 5.5a9 9 0 0 1 0 13"/>"#,
        "play" => r#"<path d="M7 4.5v15l12-7.5z"/>"#,
        "pause" => r#"<path d="M8 5v14M16 5v14"/>"#,
        "stop" => r#"<rect x="6" y="6" width="12" height="12" rx="1.5"/>"#,
        "flame" => r#"<path d="M12 21.5c-3.9 0-7-2.9-7-6.8 0-3 1.7-5.1 3.5-6.9.3 1.9 1.3 3.2 2.6 3.8C11.2 8.1 12.4 5 15 2.5c.3 3 1.8 5 3 6.7 1 1.5 1 2.9 1 4.4 0 4.7-3.1 7.9-7 7.9z"/><path d="M12 21.5c-1.7 0-3-1.3-3-3 0-1.6 1.2-2.7 2.3-3.8.3 1 .9 1.6 1.6 1.9.3-1.2.9-2.2 1.8-2.9.4 1.4 1.3 2.4 1.3 4 0 2.2-1.5 3.8-4 3.8z"/>"#,
        "flame-fill" => r##"<path d="M12 21.5c-3.9 0-7-2.9-7-6.8 0-3 1.7-5.1 3.5-6.9.3 1.9 1.3 3.2 2.6 3.8C11.2 8.1 12.4 5 15 2.5c.3 3 1.8 5 3 6.7 1 1.5 1 2.9 1 4.4 0 4.7-3.1 7.9-7 7.9z" fill="#fff" stroke="none"/>"##,
        "progress" => r#"<path d="M3.5 20.5h17"/><path d="M6.5 20.5v-6M12 20.5V9M17.5 20.5V4"/>"#,
        "check" => r#"<path d="M4.5 12.5l5 5 10-11"/>"#,
        "close" => r#"<path d="M6 6l12 12M18 6L6 18"/>"#,
        "star" => r#"<path d="M12 3.2l2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17.2 6.6 20.1l1-6.1-4.4-4.3 6.1-.9z"/>"#,
        "lock" => r#"<rect x="5" y="10.5" width="14" height="10" rx="2.5"/><path d="M8.5 10.5V7.5a3.5 3.5 0 0 1 7 0v3"/>"#,
        "chat" => r#"<path d="M4 5.5h11a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H9l-4 3.5v-3.5H4a2 2 0 0 1-2-2v-6a2 2 0 0 1 2-2z"/><path d="M17 9.5h3a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2h-1v3l-3.5-3H12"/>"#,
        "bulb" => r#"<path d="M9 18h6M10 21.5h4"/><path d="M12 2.5a6.5 6.5 0 0 0-4 11.6c.7.6 1 1.3 1 2.2V17h6v-.7c0-.9.3-1.6 1-2.2a6.5 6.5 0 0 0-4-11.6z"/>"#,
        "settings" => r#"<path d="M10.03 4.87L10.38 2.54L13.62 2.54L13.97 4.87A7.4 7.4 0 0 1 15.65 5.56L17.54 4.16L19.84 6.46L18.44 8.35A7.4 7.4 0 0 1 19.13 10.03L21.46 10.38L21.46 13.62L19.13 13.97A7.4 7.4 0 0 1 18.44 15.65L19.84 17.54L17.54 19.84L15.65 18.44A7.4 7.4 0 0 1 13.97 19.13L13.62 21.46L10.38 21.46L10.03 19.13A7.4 7.4 0 0 1 8.35 18.44L6.46 19.84L4.16 17.54L5.56 15.65A7.4 7.4 0 0 1 4.87 13.97L2.54 13.62L2.54 10.38L4.87 10.03A7.4 7.4 0 0 1 5.56 8.35L4.16 6.46L6.46 4.16L8.35 5.56A7.4 7.4 0 0 1 10.03 4.87Z"/><circle cx="12" cy="12" r="3"/>"#,
        "chevron-down" => r#"<path d="M6 9l6 6 6-6"/>"#,
        "back" => r#"<path d="M15 5l-7 7 7 7"/>"#,
        _ => r#"<path d="M14 2.5H6.5a2 2 0 0 0-2 2v15a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V8z"/><path d="M14 2.5V8h5.5"/>"#,
    }
}

/// Subject id -> (icon, hue). Unknown ids fall back on keywords, then a plain file icon.
pub fn subject_icon(id: &str) -> (&'static str, f32) {
    let known = [
        ("mathematics", "maths", 222.0),
        ("mathematical-literacy", "calculator", 200.0),
        ("natural-sciences", "flask", 165.0),
        ("physical-sciences", "atom", 262.0),
        ("life-sciences", "leaf", 128.0),
        ("social-sciences-history", "landmark", 24.0),
        ("history", "landmark", 24.0),
        ("social-sciences-geography", "globe", 190.0),
        ("geography", "globe", 190.0),
        ("economic-and-management-sciences", "wallet", 42.0),
        ("accounting", "receipt", 32.0),
        ("business-studies", "briefcase", 300.0),
        ("economics", "trend", 340.0),
        ("technology", "gear", 212.0),
        ("life-orientation", "compass", 352.0),
        ("english-home-language", "book", 280.0),
        ("afrikaans", "chat", 140.0),
    ];
    if let Some((_, i, h)) = known.iter().find(|(k, _, _)| *k == id) {
        return (i, *h);
    }
    let keywords = [
        ("afrikaans", "chat", 140.0), ("english", "book", 280.0), ("language", "book", 280.0), ("math", "maths", 222.0), ("science", "flask", 165.0),
        ("history", "landmark", 24.0), ("geograph", "globe", 190.0), ("econom", "trend", 340.0), ("account", "receipt", 32.0),
        ("business", "briefcase", 300.0), ("tech", "gear", 212.0), ("life", "compass", 352.0),
    ];
    keywords.iter().find(|(k, _, _)| id.contains(k)).map(|(_, i, h)| (*i, *h)).unwrap_or(("file", 220.0))
}

// ---------------------------------------------------------------- SVG

pub fn svg_options() -> &'static usvg::Options<'static> {
    static OPT: OnceLock<usvg::Options<'static>> = OnceLock::new();
    OPT.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_font_data(include_bytes!("../../assets/fonts/Inter-Regular.ttf").to_vec());
        db.load_font_data(include_bytes!("../../assets/fonts/Inter-SemiBold.ttf").to_vec());
        db.load_font_data(include_bytes!("../../assets/fonts/Inter-Bold.ttf").to_vec());
        db.load_font_data(include_bytes!("../../assets/fonts/Inter-Italic.ttf").to_vec());
        db.load_font_data(include_bytes!("../../assets/fonts/KaTeX_Main-Regular.ttf").to_vec());
        db.load_font_data(include_bytes!("../../assets/fonts/KaTeX_Math-Italic.ttf").to_vec());
        db.set_sans_serif_family("Inter");
        db.set_serif_family("KaTeX_Main");
        db.set_monospace_family("Inter");
        db.set_cursive_family("Inter");
        db.set_fantasy_family("Inter");
        let mut o = usvg::Options::default();
        o.font_family = "Inter".into();
        o.fontdb = Arc::new(db);
        o
    })
}

/// Rasterise SVG markup to fit `max_w` × `max_h` pixels (keeping its proportions).
pub fn rasterise(svg: &str, max_w: f32, max_h: f32) -> Option<ColorImage> {
    let tree = usvg::Tree::from_str(svg, svg_options()).ok()?;
    let size = tree.size();
    let k = (max_w / size.width()).min(max_h / size.height());
    let (w, h) = ((size.width() * k).round().max(1.0) as u32, (size.height() * k).round().max(1.0) as u32);
    let mut pm = tiny_skia::Pixmap::new(w, h)?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(k, k), &mut pm.as_mut());
    Some(ColorImage::from_rgba_premultiplied([w as usize, h as usize], pm.data()))
}

/// The natural size of SVG markup (its width and height in SVG units).
pub fn svg_size(svg: &str) -> Option<(f32, f32)> {
    let tree = usvg::Tree::from_str(svg, svg_options()).ok()?;
    Some((tree.size().width(), tree.size().height()))
}

// ---------------------------------------------------------------- texture cache

pub struct MathTex {
    pub tex: TextureHandle,
    /// Size in points.
    pub w: f32,
    pub h: f32,
    pub d: f32,
    pub pad: f32,
}

#[derive(Default)]
struct Cache {
    icons: HashMap<(String, u32, u32), TextureHandle>,
    math: HashMap<(String, bool, u32), Arc<MathTex>>,
    svgs: HashMap<(u64, u32, u32), Option<TextureHandle>>,
    sizes: HashMap<u64, Option<(f32, f32)>>,
    frame_uses: usize,
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(Cache::default());
}

fn hash_str(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// A white icon texture, `size` points square, with the given stroke width.
pub fn icon(ctx: &Context, name: &str, size: f32, stroke: f32) -> TextureHandle {
    let ppp = ctx.pixels_per_point();
    let px = (size * ppp).round().max(4.0) as u32;
    let key = (name.to_string(), px, (stroke * 100.0) as u32);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(t) = c.icons.get(&key) {
            return t.clone();
        }
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="#fff" stroke-width="{stroke}" stroke-linecap="round" stroke-linejoin="round">{}</svg>"##,
            icon_path(name)
        );
        let img = rasterise(&svg, px as f32, px as f32).unwrap_or_else(|| ColorImage::new([1, 1], vec![Color32::TRANSPARENT]));
        let t = ctx.load_texture(format!("icon-{name}-{px}"), img, TextureOptions::LINEAR);
        c.icons.insert(key, t.clone());
        t
    })
}

/// A formula, rendered for `size` points per em.
pub fn math(ctx: &Context, tex: &str, display: bool, size: f32) -> Arc<MathTex> {
    let ppp = ctx.pixels_per_point();
    let px = size * ppp;
    let key = (tex.to_string(), display, (px * 4.0).round() as u32);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.frame_uses += 1;
        if let Some(m) = c.math.get(&key) {
            return m.clone();
        }
        let r = crate::math::render(tex, display, px);
        let img = ColorImage::from_rgba_premultiplied([r.width_px, r.height_px], &r.rgba);
        let t = ctx.load_texture(format!("math-{}", c.math.len()), img, TextureOptions::LINEAR);
        let m = Arc::new(MathTex { tex: t, w: r.w * size, h: r.h * size, d: r.d * size, pad: r.pad / ppp });
        if c.math.len() > 4000 {
            c.math.clear();
        }
        c.math.insert(key, m.clone());
        m
    })
}

/// A diagram (SVG markup) rasterised to `w` × `h` points.
pub fn svg_texture(ctx: &Context, svg: &str, w: f32, h: f32) -> Option<TextureHandle> {
    let ppp = ctx.pixels_per_point();
    let key = (hash_str(svg), (w * ppp) as u32, (h * ppp) as u32);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(t) = c.svgs.get(&key) {
            return t.clone();
        }
        let t = rasterise(svg, w * ppp, h * ppp).map(|img| ctx.load_texture(format!("svg-{}", key.0), img, TextureOptions::LINEAR));
        if c.svgs.len() > 200 {
            c.svgs.clear();
        }
        c.svgs.insert(key, t.clone());
        t
    })
}

pub fn svg_natural_size(svg: &str) -> Option<(f32, f32)> {
    let k = hash_str(svg);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(s) = c.sizes.get(&k) {
            return *s;
        }
        let s = svg_size(svg);
        c.sizes.insert(k, s);
        s
    })
}

/// Forget rendered formulas (after the text size changes).
pub fn clear_math() {
    CACHE.with(|c| c.borrow_mut().math.clear());
}
