//! The study buddy's artwork: SVG built from simple shapes (five body types, eight colours,
//! six accessories, one shared face), as in 1.1's buddy.js. A `Pose` carries every moving
//! part's transform for one frame; the SVG is rasterised when the pose changes.

use crate::model::BuddyPrefs;

pub const KINDS: [(&str, &str); 5] = [("blob", "Blob"), ("owl", "Owl"), ("robot", "Robot"), ("fox", "Fox"), ("dino", "Dino")];
pub const COLOURS: [(&str, &str, &str); 8] = [
    ("sky", "Sky", "#4F8DF7"),
    ("mint", "Mint", "#2EC4A0"),
    ("sun", "Sun", "#F7B731"),
    ("coral", "Coral", "#FF6B6B"),
    ("grape", "Grape", "#9B6DFF"),
    ("bubblegum", "Bubblegum", "#FF7EB6"),
    ("ocean", "Ocean", "#22AFCF"),
    ("leaf", "Leaf", "#74C043"),
];
pub const ACCESSORIES: [(&str, &str); 6] = [("none", "Nothing"), ("glasses", "Glasses"), ("cap", "Grad cap"), ("headphones", "Headphones"), ("bowtie", "Bow tie"), ("flower", "Flower")];
pub const NAMES: [&str; 20] = ["Pip", "Nova", "Milo", "Zuri", "Neo", "Tumi", "Kiki", "Sol", "Bean", "Juno", "Ollie", "Zinzi", "Kai", "Lulu", "Bongi", "Taz", "Mika", "Ziggy", "Lerato", "Sipho"];

pub fn label<'a>(list: &[(&'a str, &'a str)], id: &str) -> &'a str {
    list.iter().find(|x| x.0 == id).map(|x| x.1).unwrap_or("")
}
pub fn colour_label(id: &str) -> &'static str {
    COLOURS.iter().find(|c| c.0 == id).map(|c| c.1).unwrap_or("")
}

pub fn random_look() -> BuddyPrefs {
    BuddyPrefs {
        kind: KINDS[fastrand::usize(0..KINDS.len())].0.into(),
        color: COLOURS[fastrand::usize(0..COLOURS.len())].0.into(),
        accessory: ACCESSORIES[fastrand::usize(1..ACCESSORIES.len())].0.into(),
        name: NAMES[fastrand::usize(0..NAMES.len())].into(),
        ..BuddyPrefs::default()
    }
}

/// Names appear in speech bubbles, which use the app's markup, so drop what markup
/// treats specially ($ for maths, * for bold).
pub fn clean_name(n: &str) -> String {
    let t: String = n.chars().filter(|c| !(c.is_control() || "<>$*\\`".contains(*c))).collect();
    let t: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let t: String = t.chars().take(16).collect();
    if t.trim().is_empty() { "Pip".into() } else { t.trim().to_string() }
}

pub fn normalise(look: &BuddyPrefs) -> BuddyPrefs {
    let mut l = look.clone();
    if !KINDS.iter().any(|k| k.0 == l.kind) {
        l.kind = "blob".into();
    }
    if !COLOURS.iter().any(|c| c.0 == l.color) {
        l.color = "sky".into();
    }
    if !ACCESSORIES.iter().any(|a| a.0 == l.accessory) {
        l.accessory = "none".into();
    }
    l.name = clean_name(&l.name);
    l
}

fn shade(hex: &str, amt: f32) -> String {
    let n = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    let ch = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map(|v| {
        let v = v as f32;
        (if amt < 0.0 { v * (1.0 + amt) } else { v + (255.0 - v) * amt }).round() as u32
    });
    format!("#{:02x}{:02x}{:02x}", ch[0], ch[1], ch[2])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mood {
    Idle,
    Happy,
    Sad,
    Think,
    Surprised,
    Sleep,
    Listen,
}

/// One frame of the buddy.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub mood: Mood,
    pub talk: bool,
    pub float_y: f32,
    pub float_rot: f32,
    pub actor_ty: f32,
    pub actor_sx: f32,
    pub actor_sy: f32,
    pub actor_rot: f32,
    pub body_sx: f32,
    pub body_sy: f32,
    pub shadow_s: f32,
    pub shadow_o: f32,
    pub arm_l: f32,
    pub arm_r: f32,
    pub tail: f32,
    pub antenna: f32,
    pub light: f32,
    pub eyes: (f32, f32),
    pub face: (f32, f32),
    pub blink: f32,
    /// Effects: progress/time of each (None = hidden).
    pub sparks: Option<f32>,
    pub dots: Option<f32>,
    pub bulb: Option<f32>,
    pub zzz: Option<f32>,
}

impl Default for Pose {
    fn default() -> Self {
        Pose {
            mood: Mood::Idle,
            talk: false,
            float_y: 0.0,
            float_rot: 0.0,
            actor_ty: 0.0,
            actor_sx: 1.0,
            actor_sy: 1.0,
            actor_rot: 0.0,
            body_sx: 1.0,
            body_sy: 1.0,
            shadow_s: 1.0,
            shadow_o: 0.16,
            arm_l: 0.0,
            arm_r: 0.0,
            tail: 0.0,
            antenna: 0.0,
            light: 1.0,
            eyes: (0.0, 0.0),
            face: (0.0, 0.0),
            blink: 1.0,
            sparks: None,
            dots: None,
            bulb: None,
            zzz: None,
        }
    }
}

impl Pose {
    /// A rough key for "did anything visible change".
    pub fn key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.mood.hash(&mut h);
        self.talk.hash(&mut h);
        for v in [
            self.float_y, self.float_rot, self.actor_ty, self.actor_sx, self.actor_sy, self.actor_rot, self.body_sx, self.body_sy, self.shadow_s, self.shadow_o,
            self.arm_l, self.arm_r, self.tail, self.antenna, self.light, self.eyes.0, self.eyes.1, self.face.0, self.face.1, self.blink,
            self.sparks.unwrap_or(-1.0), self.dots.unwrap_or(-1.0), self.bulb.unwrap_or(-1.0), self.zzz.unwrap_or(-1.0),
        ] {
            ((v * 200.0).round() as i32).hash(&mut h);
        }
        h.finish()
    }
}

fn origin(ox: f32, oy: f32, t: &str) -> String {
    format!("translate({ox} {oy}) {t} translate({} {})", -ox, -oy)
}

fn face(fy: f32, my: f32, eye: &str, line: &str, robot: bool, cheeks: bool, p: &Pose) -> String {
    let hl = if robot { String::new() } else { r##"<circle cx="-1.9" cy="-2.7" r="2" fill="#fff"/><circle cx="1.6" cy="2.2" r="0.9" fill="#fff" opacity=".7"/>"##.to_string() };
    let eye_shape = if robot { format!(r#"<rect x="-5" y="-6.5" width="10" height="13" rx="4" fill="{eye}"/>"#) } else { format!(r#"<ellipse rx="5.8" ry="7.2" fill="{eye}"/>"#) };
    let open = !matches!(p.mood, Mood::Happy | Mood::Listen | Mood::Sleep);
    let happy = matches!(p.mood, Mood::Happy | Mood::Listen);
    let closed = p.mood == Mood::Sleep;
    let one = |x: f32| {
        let mut s = format!(r#"<g transform="translate({x} {fy})"><g transform="scale(1 {})">"#, p.blink);
        if open {
            s += &format!("<g>{eye_shape}{hl}</g>");
        }
        if happy {
            s += &format!(r#"<path d="M-5.5 1.5 Q0 -5 5.5 1.5" stroke="{eye}" stroke-width="2.8" fill="none" stroke-linecap="round"/>"#);
        }
        if closed {
            s += &format!(r#"<path d="M-5.5 0 Q0 3.6 5.5 0" stroke="{eye}" stroke-width="2.6" fill="none" stroke-linecap="round"/>"#);
        }
        s + "</g></g>"
    };
    let ch = if cheeks {
        format!(r##"<g><ellipse cx="38" cy="{}" rx="5" ry="3" fill="#FF7A9A" opacity=".45"/><ellipse cx="82" cy="{}" rx="5" ry="3" fill="#FF7A9A" opacity=".45"/></g>"##, fy + 9.0, fy + 9.0)
    } else {
        String::new()
    };
    let brow = |d: String| format!(r#"<path d="{d}" stroke="{line}" stroke-width="2.4" fill="none" stroke-linecap="round"/>"#);
    let brows = match p.mood {
        Mood::Sad => brow(format!("M41 {} L52 {}", fy - 11.0, fy - 14.0)) + &brow(format!("M79 {} L68 {}", fy - 11.0, fy - 14.0)),
        Mood::Think => brow(format!("M41 {} Q47 {} 53 {}", fy - 13.0, fy - 17.0, fy - 13.0)) + &brow(format!("M67 {} L79 {}", fy - 11.0, fy - 11.0)),
        _ => String::new(),
    };
    let mouth = if p.talk {
        format!(r#"<ellipse cy="0.5" rx="4" ry="3.8" fill="{line}"/>"#)
    } else {
        match p.mood {
            Mood::Happy => format!(r#"<path d="M-8 -2 Q0 11 8 -2 Z" fill="{line}"/>"#),
            Mood::Sad => format!(r#"<path d="M-5.5 3 Q0 -2.5 5.5 3" stroke="{line}" stroke-width="2.6" fill="none" stroke-linecap="round"/>"#),
            Mood::Think => format!(r#"<path d="M-4.5 0.5 L4.5 0.5" stroke="{line}" stroke-width="2.6" stroke-linecap="round"/>"#),
            Mood::Surprised | Mood::Sleep => format!(r#"<circle cy="0.5" r="2.6" fill="{line}"/>"#),
            _ => format!(r#"<path d="M-6.5 -1 Q0 5.5 6.5 -1" stroke="{line}" stroke-width="2.6" fill="none" stroke-linecap="round"/>"#),
        }
    };
    format!(
        r#"<g transform="translate({} {})">{ch}<g transform="translate({} {})">{}{}</g>{brows}<g transform="translate(60 {my})">{mouth}</g></g>"#,
        p.face.0, p.face.1, p.eyes.0, p.eyes.1, one(48.0), one(72.0)
    )
}

struct Body {
    fy: f32,
    my: f32,
    top: f32,
    sl: f32,
    sr: f32,
    neck: f32,
    back: String,
    arms: String,
    main: String,
    front: String,
    eye: &'static str,
    line: &'static str,
    robot: bool,
    cheeks: bool,
}

fn body(kind: &str, c: &str, acc: &str, p: &Pose) -> Body {
    let d = shade(c, -0.28);
    let l = shade(c, 0.6);
    let arms = |lx: f32, ly: f32, rx: f32, ry: f32, sl: String, sr: String| {
        format!(
            r#"<g transform="{}">{sl}</g><g transform="{}">{sr}</g>"#,
            origin(lx, ly, &format!("rotate({})", p.arm_l)),
            origin(rx, ry, &format!("rotate({})", p.arm_r))
        )
    };
    let sheen = |x: f32, y: f32| format!(r##"<ellipse cx="{x}" cy="{y}" rx="11" ry="6.5" fill="#fff" opacity=".22" transform="rotate(-28 {x} {y})"/>"##);
    let mut b = Body { fy: 66.0, my: 80.0, top: 31.0, sl: 21.0, sr: 99.0, neck: 96.0, back: String::new(), arms: String::new(), main: String::new(), front: String::new(), eye: "#1E1E28", line: "#1E1E28", robot: false, cheeks: true };
    match kind {
        "owl" => {
            (b.fy, b.my, b.top, b.sl, b.sr, b.neck) = (62.0, 84.0, 30.0, 23.0, 97.0, 99.0);
            b.back = format!(r#"<path d="M34 43 L27 15 L51 33 Z" fill="{d}"/><path d="M86 43 L93 15 L69 33 Z" fill="{d}"/>"#);
            b.arms = arms(30.0, 64.0, 90.0, 64.0, format!(r#"<path d="M29 62 C14 72 15 92 30 100 C27 87 31 74 29 62 Z" fill="{d}"/>"#), format!(r#"<path d="M91 62 C106 72 105 92 90 100 C93 87 89 74 91 62 Z" fill="{d}"/>"#));
            b.main = format!(r#"<path d="M60 28 C85 28 97 47 97 71 C97 97 81 109 60 109 C39 109 23 97 23 71 C23 47 35 28 60 28 Z" fill="{c}"/>{}<ellipse cx="60" cy="93" rx="21" ry="14" fill="{l}"/><path d="M50 89 q3 3 6 0 M62 89 q3 3 6 0 M56 96 q3 3 6 0" stroke="{d}" stroke-width="1.6" fill="none" opacity=".5" stroke-linecap="round"/><circle cx="48" cy="62" r="12.5" fill="{l}"/><circle cx="72" cy="62" r="12.5" fill="{l}"/>"#, sheen(42.0, 42.0));
            b.front = r##"<path d="M55.5 71 L64.5 71 L60 78.5 Z" fill="#F5A524"/><ellipse cx="50" cy="109" rx="5.5" ry="2.8" fill="#F5A524"/><ellipse cx="70" cy="109" rx="5.5" ry="2.8" fill="#F5A524"/>"##.into();
            b.cheeks = false;
        }
        "robot" => {
            (b.fy, b.my, b.top, b.sl, b.sr, b.neck) = (61.0, 73.0, 32.0, 24.0, 96.0, 88.0);
            let light = if acc == "cap" { String::new() } else { format!(r##"<line x1="60" y1="33" x2="60" y2="19" stroke="{d}" stroke-width="3" stroke-linecap="round"/><circle cx="60" cy="16" r="4.6" fill="#FF5D73" opacity="{}"/>"##, p.light) };
            b.back = format!(r#"{light}<rect x="16" y="52" width="10" height="21" rx="3.5" fill="{d}"/><rect x="94" y="52" width="10" height="21" rx="3.5" fill="{d}"/>"#);
            b.arms = arms(26.0, 82.0, 94.0, 82.0, format!(r#"<rect x="14" y="80" width="12" height="23" rx="6" fill="{d}" transform="rotate(14 20 82)"/>"#), format!(r#"<rect x="94" y="80" width="12" height="23" rx="6" fill="{d}" transform="rotate(-14 100 82)"/>"#));
            b.main = format!(r##"<rect x="24" y="32" width="72" height="75" rx="19" fill="{c}"/>{}<rect x="31" y="43" width="58" height="38" rx="12" fill="#1B2232"/><path d="M37 49 Q46 45 55 47" stroke="#fff" stroke-width="2" opacity=".18" fill="none" stroke-linecap="round"/><circle cx="50" cy="97" r="3.2" fill="{l}"/><circle cx="60" cy="97" r="3.2" fill="#FFD166"/><circle cx="70" cy="97" r="3.2" fill="#FF5D73"/>"##, sheen(38.0, 41.0));
            b.front = format!(r#"<rect x="37" y="105" width="17" height="7" rx="3.5" fill="{d}"/><rect x="66" y="105" width="17" height="7" rx="3.5" fill="{d}"/>"#);
            b.eye = "#7EF2FF";
            b.line = "#7EF2FF";
            b.robot = true;
            b.cheeks = false;
        }
        "fox" => {
            (b.fy, b.my, b.top, b.sl, b.sr, b.neck) = (64.0, 83.0, 33.0, 23.0, 97.0, 99.0);
            b.back = format!(
                r##"<g transform="{}"><path d="M82 101 C112 101 119 68 102 51 C107 71 99 86 79 90 Z" fill="{c}"/><path d="M102.5 52 C109 59 112 68 111 76 C106.5 70 103 62 102.5 52 Z" fill="#FFF8EE"/></g><path d="M32 53 L29 13 L57 37 Z" fill="{c}"/><path d="M35.5 45 L34 23 L50 37 Z" fill="{d}"/><path d="M88 53 L91 13 L63 37 Z" fill="{c}"/><path d="M84.5 45 L86 23 L70 37 Z" fill="{d}"/>"##,
                origin(82.0, 99.0, &format!("rotate({})", p.tail))
            );
            b.arms = arms(30.0, 86.0, 90.0, 86.0, format!(r#"<ellipse cx="25" cy="92" rx="7" ry="9" fill="{d}" transform="rotate(22 25 92)"/>"#), format!(r#"<ellipse cx="95" cy="92" rx="7" ry="9" fill="{d}" transform="rotate(-22 95 92)"/>"#));
            b.main = format!(r##"<path d="M60 32 C86 32 97 52 97 74 C97 98 81 109 60 109 C39 109 23 98 23 74 C23 52 34 32 60 32 Z" fill="{c}"/>{}<ellipse cx="60" cy="101" rx="15" ry="8" fill="#FFF8EE" opacity=".95"/><ellipse cx="60" cy="81" rx="18" ry="12.5" fill="#FFF8EE"/>"##, sheen(42.0, 46.0));
            b.front = format!(r##"<ellipse cx="60" cy="74.5" rx="3.8" ry="2.7" fill="#2B2B33"/><ellipse cx="47" cy="109" rx="7" ry="3.2" fill="{d}"/><ellipse cx="73" cy="109" rx="7" ry="3.2" fill="{d}"/>"##);
        }
        "dino" => {
            (b.fy, b.my, b.top, b.sl, b.sr, b.neck) = (62.0, 79.0, 31.0, 24.0, 95.0, 97.0);
            b.back = format!(
                r#"<g transform="{}"><path d="M84 86 C103 92 114 86 119 72 C112 79 102 81 86 76 Z" fill="{c}"/></g><g fill="{d}"><path d="M40 38 L41 25 L50 33 Z"/><path d="M52 31 L57 18 L63 30 Z"/><path d="M66 30 L73 18 L76 32 Z"/><path d="M79 36 L89 28 L87 42 Z"/><path d="M87 46 L98 42 L93 55 Z"/></g>"#,
                origin(88.0, 92.0, &format!("rotate({})", p.tail))
            );
            b.arms = arms(30.0, 84.0, 88.0, 84.0, format!(r#"<ellipse cx="25" cy="88" rx="5.5" ry="8" fill="{d}" transform="rotate(30 25 88)"/>"#), format!(r#"<ellipse cx="93" cy="88" rx="5.5" ry="8" fill="{d}" transform="rotate(-30 93 88)"/>"#));
            b.main = format!(r#"<path d="M58 30 C83 30 95 48 95 72 C95 98 80 109 58 109 C37 109 24 98 24 74 C24 50 35 30 58 30 Z" fill="{c}"/>{}<ellipse cx="58" cy="93" rx="19" ry="13" fill="{l}"/><path d="M46 89 H70 M44 95 H72 M47 101 H69" stroke="{d}" stroke-width="1.5" opacity=".35" stroke-linecap="round"/>"#, sheen(41.0, 44.0));
            b.front = format!(r#"<ellipse cx="45" cy="109" rx="7.5" ry="3.3" fill="{d}"/><ellipse cx="71" cy="109" rx="7.5" ry="3.3" fill="{d}"/>"#);
        }
        _ => {
            if acc != "cap" && acc != "headphones" {
                b.back = format!(
                    r#"<g transform="{}"><path d="M60 32 Q57 18 66 12" stroke="{d}" stroke-width="3" fill="none" stroke-linecap="round"/><circle cx="67" cy="11" r="5.2" fill="{l}"/></g>"#,
                    origin(60.0, 31.0, &format!("rotate({})", p.antenna))
                );
            }
            b.arms = arms(26.0, 76.0, 94.0, 76.0, format!(r#"<ellipse cx="18" cy="80" rx="7" ry="10.5" fill="{d}" transform="rotate(25 18 80)"/>"#), format!(r#"<ellipse cx="102" cy="80" rx="7" ry="10.5" fill="{d}" transform="rotate(-25 102 80)"/>"#));
            b.main = format!(r#"<path d="M60 30 C87 30 99 50 99 73 C99 97 83 108 60 108 C37 108 21 97 21 73 C21 50 33 30 60 30 Z" fill="{c}"/>{}"#, sheen(42.0, 47.0));
            b.front = format!(r#"<ellipse cx="46" cy="107.5" rx="9" ry="4.2" fill="{d}"/><ellipse cx="74" cy="107.5" rx="9" ry="4.2" fill="{d}"/>"#);
        }
    }
    b
}

fn accessory(id: &str, b: &Body) -> String {
    let (fy, t, sl, sr, n) = (b.fy, b.top, b.sl, b.sr, b.neck);
    match id {
        "glasses" => format!(
            r##"<g><circle cx="48" cy="{fy}" r="10.5" fill="#fff" fill-opacity=".16" stroke="#23232B" stroke-width="2.5"/><circle cx="72" cy="{fy}" r="10.5" fill="#fff" fill-opacity=".16" stroke="#23232B" stroke-width="2.5"/><path d="M58.5 {} Q60 {} 61.5 {}" stroke="#23232B" stroke-width="2.5" fill="none"/></g>"##,
            fy - 1.0, fy - 3.0, fy - 1.0
        ),
        "cap" => format!(
            r##"<g><path d="M43 {} L43 {} Q60 {} 77 {} L77 {} Z" fill="#2A2A33"/><path d="M60 {} L91 {t} L60 {} L29 {t} Z" fill="#383843"/><path d="M60 {t} L85 {} L85 {}" stroke="#F5C542" stroke-width="2.2" fill="none" stroke-linecap="round"/><circle cx="85" cy="{}" r="2.8" fill="#F5C542"/></g>"##,
            t + 2.0, t + 10.0, t + 16.0, t + 10.0, t + 2.0, t - 10.0, t + 10.0, t + 5.0, t + 17.0, t + 18.0
        ),
        "headphones" => format!(
            r##"<g><path d="M{} {} C{} {} {} {} {} {}" stroke="#2A2A33" stroke-width="5.5" fill="none" stroke-linecap="round"/><rect x="{}" y="{}" width="13" height="24" rx="6" fill="#FF5D73"/><rect x="{}" y="{}" width="13" height="24" rx="6" fill="#FF5D73"/></g>"##,
            sl + 5.0, fy + 2.0, sl + 2.0, t - 18.0, sr - 2.0, t - 18.0, sr - 5.0, fy + 2.0, sl - 4.0, fy - 11.0, sr - 9.0, fy - 11.0
        ),
        "bowtie" => format!(
            r##"<g><path d="M60 {n} L47 {} L47 {} Z" fill="#E5484D"/><path d="M60 {n} L73 {} L73 {} Z" fill="#E5484D"/><circle cx="60" cy="{n}" r="3.3" fill="#B8323A"/></g>"##,
            n - 7.0, n + 7.0, n - 7.0, n + 7.0
        ),
        "flower" => {
            let (x, y) = (sl + 15.0, t + 8.0);
            let petals: String = [0, 72, 144, 216, 288].iter().map(|a| format!(r##"<ellipse cx="{x}" cy="{}" rx="3.6" ry="5.2" fill="#FFF3F8" transform="rotate({a} {x} {y})"/>"##, y - 5.5)).collect();
            format!(r##"<g>{petals}<circle cx="{x}" cy="{y}" r="3.4" fill="#FFC531"/></g>"##)
        }
        _ => String::new(),
    }
}

/// Keyframes of the effects, as in the stylesheet.
fn fx(p: &Pose) -> String {
    let mut s = String::new();
    if let Some(t) = p.sparks {
        for (i, (x, y)) in [(16.0, 34.0), (104.0, 30.0), (10.0, 72.0), (110.0, 68.0)].iter().enumerate() {
            let local = ((t * 950.0 - i as f32 * 70.0) / 950.0).clamp(0.0, 1.0);
            let (sc, rot, op) = if local < 0.4 { let k = local / 0.4; (1.25 * k, 45.0 * k, k) } else { let k = (local - 0.4) / 0.6; (1.25 * (1.0 - k), 45.0 + 45.0 * k, 1.0 - k) };
            if op > 0.01 {
                s += &format!(r##"<g transform="translate({x} {y}) rotate({rot}) scale({sc})" opacity="{op}"><path d="M0 -6.5 L1.7 -1.7 L6.5 0 L1.7 1.7 L0 6.5 L-1.7 1.7 L-6.5 0 L-1.7 -1.7 Z" fill="#FFD166"/></g>"##);
            }
        }
    }
    if let Some(t) = p.zzz {
        for (i, (x, y, size)) in [(88.0, 30.0, 9.0), (96.0, 20.0, 11.0), (104.0, 9.0, 13.0)].iter().enumerate() {
            let local = (t - i as f32 * 0.6) / 2.7;
            if !(0.0..=1.0).contains(&local) {
                continue;
            }
            let op = if local < 0.3 { local / 0.3 } else { 1.0 - (local - 0.3) / 0.7 };
            let (dx, dy, sc) = (8.0 * local, 4.0 - 18.0 * local, 0.6 + 0.5 * local);
            s += &format!(r##"<g transform="translate({} {}) scale({sc})" opacity="{op}"><text font-size="{size}" font-weight="800" fill="#8AA4FF" font-family="Inter">z</text></g>"##, x + dx, y + dy);
        }
    }
    if let Some(t) = p.dots {
        for (i, (x, y, r)) in [(86.0, 28.0, 2.4), (93.0, 21.0, 3.0), (101.0, 14.0, 3.6)].iter().enumerate() {
            let ph = ((t - i as f32 * 0.15) / 0.9).rem_euclid(1.0);
            let k = (ph * std::f32::consts::TAU).cos() * -0.5 + 0.5; // 0 -> 1 -> 0
            s += &format!(r##"<circle cx="{x}" cy="{}" r="{r}" fill="#B7C3D9" opacity="{}"/>"##, y - 3.0 * k, 0.5 + 0.5 * k);
        }
    }
    if let Some(t) = p.bulb {
        let k = t.clamp(0.0, 1.0);
        // cubic-bezier(.2, 1.5, .4, 1): overshoots a little
        let e = super::anim::bezier(0.2, 1.5, 0.4, 1.0, k);
        let op = (k * 2.0).min(1.0);
        s += &format!(
            r##"<g transform="translate(100 {}) scale({})" opacity="{op}"><circle r="7.5" fill="#FFD84D"/><path d="M-3 7 h6 v3.5 h-6 Z" fill="#A6A6B3"/><path d="M-11 -9 l-3 -3 M11 -9 l3 -3 M0 -12 v-4" stroke="#FFD84D" stroke-width="2" stroke-linecap="round"/></g>"##,
            15.0 + 6.0 * (1.0 - e),
            e.max(0.0)
        );
    }
    s
}

/// The buddy as SVG markup (viewBox 0 0 120 120, with room around it for effects).
pub fn svg(look: &BuddyPrefs, p: &Pose) -> String {
    let lk = normalise(look);
    let c = COLOURS.iter().find(|x| x.0 == lk.color).map(|x| x.2).unwrap_or(COLOURS[0].2);
    let b = body(&lk.kind, c, &lk.accessory, p);
    let fc = face(b.fy, b.my, b.eye, b.line, b.robot, b.cheeks, p);
    let acc = accessory(&lk.accessory, &b);
    let float = format!("translate(0 {}) {}", p.float_y, origin(60.0, 112.0, &format!("rotate({})", p.float_rot)));
    let actor = origin(60.0, 112.0, &format!("translate(0 {}) rotate({}) scale({} {})", p.actor_ty, p.actor_rot, p.actor_sx, p.actor_sy));
    let bodyt = origin(60.0, 112.0, &format!("scale({} {})", p.body_sx, p.body_sy));
    let shadow = origin(60.0, 113.0, &format!("scale({})", p.shadow_s));
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-12 -12 144 144" width="144" height="144"><ellipse transform="{shadow}" cx="60" cy="113" rx="27" ry="4.5" fill="#000" opacity="{}"/><g transform="{float}"><g transform="{actor}"><g transform="{bodyt}">{}{}{}{}{fc}{acc}</g></g></g>{}</svg>"##,
        p.shadow_o, b.back, b.arms, b.main, b.front, fx(p)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_look_renders() {
        for (k, _) in KINDS {
            for (a, _) in ACCESSORIES {
                let look = BuddyPrefs { kind: k.into(), color: "mint".into(), accessory: a.into(), ..BuddyPrefs::default() };
                for mood in [Mood::Idle, Mood::Happy, Mood::Sad, Mood::Think, Mood::Sleep] {
                    let p = Pose { mood, sparks: Some(0.4), zzz: Some(0.5), dots: Some(0.2), bulb: Some(0.5), ..Pose::default() };
                    let s = svg(&look, &p);
                    let img = crate::ui::images::rasterise(&s, 96.0, 96.0);
                    assert!(img.is_some(), "{k}/{a}/{mood:?}");
                }
            }
        }
        assert_eq!(clean_name("  $Bo*b  the  great one!! "), "Bob the great on");
        assert_eq!(clean_name(""), "Pip");
    }
}
