//! Colours, sizes and fonts: the look of FreeSheet 1.1's stylesheet (src/styles.css), a
//! calm, mostly monochrome interface where colour is kept for actions (blue) and
//! right/wrong (green/red). The learner picks a theme and an accent colour in Settings;
//! src/js/theme.js and src/styles.css define the same ones for the web version.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
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
    bg: hex(0xfaf8f4),
    surface: hex(0xf3f1ea),
    surface2: hex(0xebe8df),
    border: hex(0xe1ddd2),
    border_strong: hex(0xc9c4b6),
    text: hex(0x2c2a26),
    muted: hex(0x69665e),
    accent: hex(0x1f5fcc),
    accent_hover: hex(0x194fab),
    accent_soft: hex(0xe7eefa),
    btn: hex(0x1f5fcc),
    btn_hover: hex(0x194fab),
    good: hex(0x17793a),
    good_soft: hex(0xe7f3e8),
    bad: hex(0xb42318),
    bad_soft: hex(0xfaeae6),
    warn_soft: hex(0xf6efdd),
    flame: hex(0xff8a1f),
    shadow: Color32::from_black_alpha(20),
};

pub const DARK: Palette = Palette {
    bg: hex(0x1a1b1e),
    surface: hex(0x222327),
    surface2: hex(0x2a2c30),
    border: hex(0x33353a),
    border_strong: hex(0x4a4c52),
    text: hex(0xdcdcd8),
    muted: hex(0xa3a39d),
    accent: hex(0x7aa2ee),
    accent_hover: hex(0x96b6f2),
    accent_soft: hex(0x232f45),
    btn: hex(0x3063c6),
    btn_hover: hex(0x3b70d6),
    good: hex(0x5fbd87),
    good_soft: hex(0x1a2c21),
    bad: hex(0xe8867d),
    bad_soft: hex(0x372120),
    warn_soft: hex(0x2c281e),
    flame: hex(0xff8a1f),
    shadow: Color32::from_black_alpha(60),
};

pub const SEPIA: Palette = Palette {
    bg: hex(0xf8f1e3),
    surface: hex(0xf1e8d6),
    surface2: hex(0xe9dfc9),
    border: hex(0xe0d4bc),
    border_strong: hex(0xcbbd9f),
    text: hex(0x33291c),
    muted: hex(0x6a5e4b),
    accent: hex(0x9a5b13),
    accent_hover: hex(0x7e4a0e),
    accent_soft: hex(0xf5ead6),
    btn: hex(0x9a5b13),
    btn_hover: hex(0x7e4a0e),
    good: hex(0x2f6f3a),
    good_soft: hex(0xe4ecd3),
    bad: hex(0xa6341f),
    bad_soft: hex(0xf5dfd0),
    warn_soft: hex(0xf2e6c4),
    flame: hex(0xe87a12),
    shadow: Color32::from_black_alpha(24),
};

pub const MIDNIGHT: Palette = Palette {
    bg: hex(0x0f1626),
    surface: hex(0x162034),
    surface2: hex(0x1d2a42),
    border: hex(0x26344f),
    border_strong: hex(0x3a4c6e),
    text: hex(0xdbe2ef),
    muted: hex(0x9dabc4),
    accent: hex(0x6ea8fe),
    accent_hover: hex(0x8fbcff),
    accent_soft: hex(0x1b2c4b),
    btn: hex(0x2f6ad9),
    btn_hover: hex(0x3f7aea),
    good: hex(0x4fc98a),
    good_soft: hex(0x12302a),
    bad: hex(0xf47c74),
    bad_soft: hex(0x3a1f28),
    warn_soft: hex(0x2b2a24),
    flame: hex(0xff8a1f),
    shadow: Color32::from_black_alpha(70),
};

pub const FOREST: Palette = Palette {
    bg: hex(0x121a15),
    surface: hex(0x18231c),
    surface2: hex(0x1f2c24),
    border: hex(0x2a3a30),
    border_strong: hex(0x40544a),
    text: hex(0xdde6df),
    muted: hex(0x9db0a3),
    accent: hex(0x57c489),
    accent_hover: hex(0x77d6a2),
    accent_soft: hex(0x1a3527),
    btn: hex(0x237a4d),
    btn_hover: hex(0x2a8a58),
    good: hex(0x62d394),
    good_soft: hex(0x163022),
    bad: hex(0xee7a70),
    bad_soft: hex(0x37201c),
    warn_soft: hex(0x2a2a1c),
    flame: hex(0xff8a1f),
    shadow: Color32::from_black_alpha(70),
};

pub const BLACK: Palette = Palette {
    bg: hex(0x000000),
    surface: hex(0x0e0e0e),
    surface2: hex(0x181818),
    border: hex(0x262626),
    border_strong: hex(0x3d3d3d),
    text: hex(0xe4e4e0),
    muted: hex(0xa3a39e),
    accent: hex(0x6b9cf5),
    accent_hover: hex(0x8bb2f8),
    accent_soft: hex(0x142038),
    btn: hex(0x2f68d8),
    btn_hover: hex(0x3b76e8),
    good: hex(0x4cc276),
    good_soft: hex(0x0f2417),
    bad: hex(0xf07167),
    bad_soft: hex(0x2e1513),
    warn_soft: hex(0x211d12),
    flame: hex(0xff8a1f),
    shadow: Color32::from_black_alpha(90),
};

pub const CONTRAST: Palette = Palette {
    bg: hex(0xffffff),
    surface: hex(0xf2f2f2),
    surface2: hex(0xe4e4e4),
    border: hex(0x767676),
    border_strong: hex(0x000000),
    text: hex(0x000000),
    muted: hex(0x333333),
    accent: hex(0x0040b0),
    accent_hover: hex(0x00308a),
    accent_soft: hex(0xdce8ff),
    btn: hex(0x0040b0),
    btn_hover: hex(0x00308a),
    good: hex(0x0a5c28),
    good_soft: hex(0xd9f2e2),
    bad: hex(0x9c0f05),
    bad_soft: hex(0xfde3e0),
    warn_soft: hex(0xfff1c9),
    flame: hex(0xc05a00),
    shadow: Color32::from_black_alpha(30),
};

/// (id, label) of every theme, in the order Settings shows them.
pub const THEMES: [(&str, &str); 8] = [
    ("system", "Automatic"),
    ("light", "Light"),
    ("dark", "Dark"),
    ("sepia", "Sepia"),
    ("midnight", "Midnight"),
    ("forest", "Forest"),
    ("black", "Black"),
    ("contrast", "High contrast"),
];

/// (id, label, colour on light themes, colour on dark themes).
pub const ACCENTS: [(&str, &str, u32, u32); 7] = [
    ("blue", "Blue", 0x1f5fcc, 0x5b8ff0),
    ("violet", "Violet", 0x6d3fd1, 0xa78bfa),
    ("pink", "Pink", 0xc0307f, 0xf17ab9),
    ("red", "Red", 0xc0372b, 0xf0807a),
    ("orange", "Orange", 0xb75a0a, 0xf5a25c),
    ("green", "Green", 0x1b7f4b, 0x56c98b),
    ("teal", "Teal", 0x0b7c86, 0x4fc5cf),
];

/// A theme's palette and whether it is a dark one.
pub fn named(theme: &str, system_dark: bool) -> (Palette, bool) {
    match theme {
        "light" => (LIGHT, false),
        "dark" => (DARK, true),
        "sepia" => (SEPIA, false),
        "midnight" => (MIDNIGHT, true),
        "forest" => (FOREST, true),
        "black" => (BLACK, true),
        "contrast" => (CONTRAST, false),
        _ => (if system_dark { DARK } else { LIGHT }, system_dark),
    }
}

/// The accent colour shown for `id` on a light or dark theme, if it is one of ACCENTS.
pub fn accent_colour(id: &str, dark: bool) -> Option<Color32> {
    ACCENTS.iter().find(|a| a.0 == id).map(|a| hex(if dark { a.3 } else { a.2 }))
}

/// The colours for a theme and accent. High contrast keeps its own, tested colours.
pub fn resolve(theme: &str, accent: &str, system_dark: bool) -> (Palette, bool) {
    let (mut p, dark) = named(theme, system_dark);
    if theme == "contrast" {
        return (p, dark);
    }
    let Some(a) = ACCENTS.iter().find(|a| a.0 == accent) else { return (p, dark) };
    let (l, d) = (hex(a.2), hex(a.3));
    if dark {
        p.accent = d;
        p.accent_hover = mix(d, Color32::WHITE, 0.8);
        p.accent_soft = mix(d, p.bg, 0.16);
        p.btn = l;
        p.btn_hover = mix(l, Color32::WHITE, 0.88);
    } else {
        p.accent = l;
        p.accent_hover = mix(l, Color32::BLACK, 0.82);
        p.accent_soft = mix(l, p.bg, 0.11);
        p.btn = l;
        p.btn_hover = p.accent_hover;
    }
    (p, dark)
}

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
    /// Notes: multiplies the normal line height.
    pub spacing: f32,
    /// Notes: the widest the reading column may be, in points.
    pub reading: f32,
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

pub fn spacing_of(setting: Option<&str>) -> f32 {
    match setting {
        Some("compact") => 0.92,
        Some("relaxed") => 1.16,
        _ => 1.0,
    }
}

pub fn reading_of(setting: Option<&str>) -> f32 {
    match setting {
        Some("narrow") => 600.0,
        Some("wide") => 860.0,
        _ => 680.0,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Color32) -> f32 {
        let f = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn every_theme_and_accent_is_readable() {
        for system_dark in [false, true] {
            for (theme, _) in THEMES {
                for accent in ACCENTS.iter().map(|a| a.0).chain(["default"]) {
                    let (p, _) = resolve(theme, accent, system_dark);
                    assert!(contrast(p.text, p.bg) >= 7.0, "{theme}: text");
                    assert!(contrast(p.muted, p.bg) >= 4.5, "{theme}: muted");
                    assert!(contrast(p.accent, p.bg) >= 3.0, "{theme}/{accent}: accent");
                    assert!(contrast(Color32::WHITE, p.btn) >= 4.5, "{theme}/{accent}: button text");
                }
            }
        }
    }

    #[test]
    fn automatic_follows_the_system_and_unknown_names_are_safe() {
        assert!(resolve("system", "default", true).1);
        assert!(!resolve("system", "default", false).1);
        assert_eq!(resolve("no-such-theme", "no-such-accent", false).0, LIGHT);
        assert_eq!(resolve("contrast", "pink", false).0, CONTRAST);
    }
}
