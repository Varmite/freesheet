//! Colours, sizes and fonts: the look of FreeSheet 1.1's stylesheet (src/styles.css), a
//! calm, mostly monochrome interface where colour is kept for actions (blue) and
//! right/wrong (green/red). Light and dark follow the system.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface2: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub btn: Color32,
    pub btn_hover: Color32,
    pub good: Color32,
    pub good_soft: Color32,
    pub bad: Color32,
    pub bad_soft: Color32,
    pub warn_soft: Color32,
    pub flame: Color32,
    pub shadow: Color32,
}

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub const LIGHT: Palette = Palette {
    bg: hex(0xffffff),
    surface: hex(0xf6f6f5),
    surface2: hex(0xefefed),
    border: hex(0xe1e1de),
    border_strong: hex(0xc9c9c5),
    text: hex(0x1b1b1a),
    muted: hex(0x6b6b67),
    accent: hex(0x1f5fcc),
    accent_hover: hex(0x194fab),
    accent_soft: hex(0xe9f0fc),
    btn: hex(0x1f5fcc),
    btn_hover: hex(0x194fab),
    good: hex(0x17793a),
    good_soft: hex(0xe9f6ed),
    bad: hex(0xb42318),
    bad_soft: hex(0xfcecea),
    warn_soft: hex(0xf7f3e8),
    flame: hex(0xff8a1f),
    shadow: Color32::from_black_alpha(20),
};

pub const DARK: Palette = Palette {
    bg: hex(0x161616),
    surface: hex(0x1f1f1f),
    surface2: hex(0x262626),
    border: hex(0x2f2f2f),
    border_strong: hex(0x444444),
    text: hex(0xececea),
    muted: hex(0xa0a09b),
    accent: hex(0x5b8ff0),
    accent_hover: hex(0x7aa5f5),
    accent_soft: hex(0x1d2a42),
    btn: hex(0x2f68d8),
    btn_hover: hex(0x3b76e8),
    good: hex(0x4cc276),
    good_soft: hex(0x15291c),
    bad: hex(0xf07167),
    bad_soft: hex(0x351b19),
    warn_soft: hex(0x2a261b),
    flame: hex(0xff8a1f),
    shadow: Color32::from_black_alpha(60),
};

/// Font weights (static Inter instances; see assets/fonts).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum W {
    Regular,
    Medium,
    Semi,
    Bold,
    Heavy,
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub p: Palette,
    pub dark: bool,
    /// Root font size in points (the CSS rem): 15, 16.5, 18.5 or 20.5.
    pub rem: f32,
    pub motion: bool,
    /// A phone-sized window: tabs at the bottom, tighter margins.
    pub narrow: bool,
}

impl Theme {
    pub fn px(&self, rem: f32) -> f32 {
        rem * self.rem
    }

    pub fn font(&self, rem: f32, w: W) -> FontId {
        FontId::new(self.px(rem), family(w, false))
    }

    pub fn italic(&self, rem: f32, w: W) -> FontId {
        FontId::new(self.px(rem), family(w, true))
    }
}

pub fn family(w: W, italic: bool) -> FontFamily {
    let name = match (w, italic) {
        (W::Regular, false) => return FontFamily::Proportional,
        (W::Medium, false) => "w500",
        (W::Semi, false) => "w620",
        (W::Bold, false) => "w720",
        (W::Heavy, false) => "w800",
        (W::Regular | W::Medium, true) => "i400",
        (_, true) => "i620",
    };
    FontFamily::Name(name.into())
}

pub fn serif() -> FontFamily {
    FontFamily::Name("serif".into())
}
pub fn serif_bold() -> FontFamily {
    FontFamily::Name("serif-b".into())
}
pub fn serif_italic() -> FontFamily {
    FontFamily::Name("serif-i".into())
}

macro_rules! font {
    ($name:literal) => {
        include_bytes!(concat!("../../assets/fonts/", $name))
    };
}

pub fn fonts() -> FontDefinitions {
    let mut f = FontDefinitions::default();
    let add = |f: &mut FontDefinitions, name: &str, data: &'static [u8]| {
        f.font_data.insert(name.into(), Arc::new(FontData::from_static(data)));
    };
    add(&mut f, "inter-400", font!("Inter-Regular.ttf"));
    add(&mut f, "inter-500", font!("Inter-Medium.ttf"));
    add(&mut f, "inter-620", font!("Inter-SemiBold.ttf"));
    add(&mut f, "inter-720", font!("Inter-Bold.ttf"));
    add(&mut f, "inter-800", font!("Inter-ExtraBold.ttf"));
    add(&mut f, "inter-i400", font!("Inter-Italic.ttf"));
    add(&mut f, "inter-i620", font!("Inter-SemiBoldItalic.ttf"));
    add(&mut f, "katex-main", font!("KaTeX_Main-Regular.ttf"));
    add(&mut f, "katex-main-b", font!("KaTeX_Main-Bold.ttf"));
    add(&mut f, "katex-main-i", font!("KaTeX_Main-Italic.ttf"));
    add(&mut f, "katex-ams", font!("KaTeX_AMS-Regular.ttf"));
    // Fallbacks for symbols Inter lacks: KaTeX's fonts, then egui's emoji fonts.
    let fallbacks: Vec<String> = ["katex-main", "katex-ams", "Ubuntu-Light", "egui-icons"].iter().map(|s| s.to_string()).collect();
    let fam = |first: &str| {
        let mut v = vec![first.to_string()];
        v.extend(fallbacks.iter().cloned());
        v
    };
    f.families.insert(FontFamily::Proportional, fam("inter-400"));
    let mut mono = vec!["Hack".to_string()];
    mono.extend(fam("inter-400"));
    f.families.insert(FontFamily::Monospace, mono);
    for (fname, data) in [("w500", "inter-500"), ("w620", "inter-620"), ("w720", "inter-720"), ("w800", "inter-800"), ("i400", "inter-i400"), ("i620", "inter-i620")] {
        f.families.insert(FontFamily::Name(fname.into()), fam(data));
    }
    let mut serif = vec!["katex-main".to_string(), "inter-400".to_string()];
    serif.extend(fallbacks.iter().cloned());
    f.families.insert(FontFamily::Name("serif".into()), serif);
    f.families.insert(FontFamily::Name("serif-b".into()), vec!["katex-main-b".into(), "inter-720".into(), "katex-main".into(), "Ubuntu-Light".into()]);
    f.families.insert(FontFamily::Name("serif-i".into()), vec!["katex-main-i".into(), "inter-i400".into(), "katex-main".into(), "Ubuntu-Light".into()]);
    f
}

/// hsl() as in CSS (h in degrees, s and l 0-100).
pub fn hsl(h: f32, s: f32, l: f32) -> Color32 {
    let (s, l) = (s / 100.0, l / 100.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let f = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(f(r), f(g), f(b))
}

/// color-mix(in srgb, a p%, b)
pub fn mix(a: Color32, b: Color32, p: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 * p + y as f32 * (1.0 - p)).round() as u8;
    Color32::from_rgba_unmultiplied(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()), m(a.a(), b.a()))
}

pub fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a * 255.0).round().clamp(0.0, 255.0) as u8)
}
