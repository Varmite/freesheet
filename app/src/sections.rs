//! Splits topic notes into sections (one per heading) and turns each section into plain
//! text for text-to-speech. The speech text's hash names the section's audio file, so this
//! must agree exactly with the JavaScript version used by scripts/tts (tests check it).

use crate::markup;
use crate::model::Block;
use fancy_regex::Regex;
use std::sync::OnceLock;

/// Bump when speech conversion changes, so all audio is regenerated.
pub const SPEECH_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct Section {
    pub index: usize,
    pub blocks: Vec<Block>,
    pub title: String,
    pub text: String,
    pub id: String,
}

pub fn split(notes: &[Block]) -> Vec<Section> {
    let mut groups: Vec<Vec<Block>> = Vec::new();
    for b in notes {
        if groups.is_empty() || (b.kind == "heading" && !groups.last().unwrap().is_empty()) {
            groups.push(Vec::new());
        }
        groups.last_mut().unwrap().push(b.clone());
    }
    groups
        .into_iter()
        .enumerate()
        .map(|(i, blocks)| {
            let text = section_speech(&blocks);
            let title = if blocks[0].kind == "heading" { blocks[0].text.clone().unwrap_or_default() } else { String::new() };
            let id = hash(&format!("{SPEECH_VERSION}|{text}"));
            Section { index: i, blocks, title, text, id }
        })
        .collect()
}

/// FNV-1a 32-bit over UTF-16 code units (as JavaScript's charCodeAt), as 8 hex digits.
pub fn hash(s: &str) -> String {
    let mut h: u32 = 0x811c9dc5;
    for u in s.encode_utf16() {
        h ^= u as u32;
        h = h.wrapping_mul(0x01000193);
    }
    format!("{h:08x}")
}

// ---------- maths to words

fn symbol(cmd: &str) -> Option<&'static str> {
    Some(match cmd {
        "\\times" | "\\cdot" => " times ",
        "\\div" => " divided by ",
        "\\pm" => " plus or minus ",
        "\\leq" | "\\le" => " is less than or equal to ",
        "\\geq" | "\\ge" => " is greater than or equal to ",
        "\\neq" => " is not equal to ",
        "\\approx" => " is approximately ",
        "\\infty" => " infinity ",
        "\\rightarrow" => " gives ",
        "\\to" => " to ",
        "\\Rightarrow" | "\\therefore" => " therefore ",
        "\\pi" => " pi ",
        "\\theta" => " theta ",
        "\\alpha" => " alpha ",
        "\\beta" => " beta ",
        "\\Delta" => " delta ",
        "\\angle" => " angle ",
        "\\triangle" => " triangle ",
        "\\parallel" => " is parallel to ",
        "\\perp" => " is perpendicular to ",
        "\\circ" => " degrees ",
        "\\%" => " percent ",
        "\\sin" => " sine ",
        "\\cos" => " cos ",
        "\\tan" => " tan ",
        "\\log" => " log ",
        "\\ldots" | "\\dots" => " and so on ",
        "\\in" => " in ",
        "\\cup" => " union ",
        "\\cap" => " intersection ",
        _ => return None,
    })
}

fn is_alpha(c: char) -> bool {
    c.is_ascii_alphabetic()
}

/// `\name` at s[i..] (letters only), if any.
fn command_at(s: &[char], i: usize) -> Option<String> {
    if s.get(i) != Some(&'\\') || !s.get(i + 1).is_some_and(|c| is_alpha(*c)) {
        return None;
    }
    let mut j = i + 1;
    while j < s.len() && is_alpha(s[j]) {
        j += 1;
    }
    Some(s[i..j].iter().collect())
}

/// The {...} group starting at s[i] (or a single token); returns (content, next index).
fn group(s: &[char], i: usize) -> (String, usize) {
    if s.get(i) != Some(&'{') {
        if let Some(cmd) = command_at(s, i) {
            let n = cmd.chars().count();
            return (cmd, i + n);
        }
        return match s.get(i) {
            Some(&c) if c != '\n' && c != '\r' && c != '\u{2028}' && c != '\u{2029}' => (c.to_string(), i + 1),
            _ => (String::new(), i),
        };
    }
    let mut depth = 0;
    for j in i..s.len() {
        if s[j] == '{' {
            depth += 1;
        } else if s[j] == '}' {
            depth -= 1;
            if depth == 0 {
                return (s[i + 1..j].iter().collect(), j + 1);
            }
        }
    }
    (s[i + 1..].iter().collect(), s.len())
}

fn power(e: &str) -> String {
    let t = e.trim();
    match t {
        "2" => " squared ".into(),
        "3" => " cubed ".into(),
        "\\circ" => " degrees ".into(),
        _ => format!(" to the power {} ", latex(t)),
    }
}

fn starts(s: &[char], i: usize, lit: &str) -> bool {
    let mut k = i;
    for c in lit.chars() {
        if s.get(k) != Some(&c) {
            return false;
        }
        k += 1;
    }
    true
}

/// The LaTeX of a formula read out in words.
pub fn latex(tex: &str) -> String {
    let s: Vec<char> = tex.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < s.len() {
        let cmd = command_at(&s, i);
        let c = cmd.as_deref();
        // \frac \dfrac \tfrac (as a prefix: "\fracx" still counts, as in the regex /^\\[dt]?frac/)
        let frac_len = if starts(&s, i, "\\frac") { Some(5) } else if starts(&s, i, "\\dfrac") || starts(&s, i, "\\tfrac") { Some(6) } else { None };
        if let Some(n) = frac_len {
            let (a, j) = group(&s, i + n);
            let (b, k) = group(&s, j);
            out += &format!(" {} over {} ", latex(&a), latex(&b));
            i = k;
        } else if starts(&s, i, "\\sqrt") {
            let mut j = i + 5;
            let mut root = String::new();
            if s.get(j) == Some(&'[') {
                if let Some(e) = s[j..].iter().position(|&c| c == ']').map(|p| p + j) {
                    root = s[j + 1..e].iter().collect();
                    j = e + 1;
                } else {
                    // indexOf returned -1: JS slices to -1 and jumps to 0; mimic by ending here.
                    root = s[j + 1..s.len().saturating_sub(1)].iter().collect();
                    j = 0;
                }
            }
            let (a, k) = group(&s, j);
            out += &if !root.is_empty() {
                format!(" the {} root of {} ", if root == "3" { "cube".to_string() } else { format!("{root}th") }, latex(&a))
            } else {
                format!(" the square root of {} ", latex(&a))
            };
            i = k;
        } else if let Some(n) = ["\\text", "\\mathrm", "\\textbf", "\\mathbf", "\\operatorname"].iter().find(|p| starts(&s, i, p)).map(|p| p.len()) {
            let (a, j) = group(&s, i + n);
            out += &format!(" {} ", a.replace("\\,", " ").replace("\\ ", " "));
            i = j;
        } else if let Some(n) = word_cmd(&s, i, &["left", "right", "big", "Big", "bigg", "displaystyle", "quad", "qquad"]) {
            i += n;
        } else if let Some(n) = ["\\overline", "\\hat", "\\vec", "\\bar"].iter().find(|p| starts(&s, i, p)).map(|p| p.len()) {
            let (a, j) = group(&s, i + n);
            out += &format!(" {} ", latex(&a));
            i = j;
        } else if starts(&s, i, "\\xrightarrow") {
            let mut j = i + 12;
            if s.get(j) == Some(&'[') {
                if let Some(e) = s[j..].iter().position(|&c| c == ']') {
                    j += e + 1;
                }
            }
            let (_, k) = group(&s, j);
            out += " gives ";
            i = k;
        } else if let Some(n) = ["\\underbrace", "\\overbrace"].iter().find(|p| starts(&s, i, p)).map(|p| p.len()) {
            let (a, j) = group(&s, i + n);
            let mut k = j;
            if s.get(k) == Some(&'_') || s.get(k) == Some(&'^') {
                k = group(&s, k + 1).1;
            }
            out += &format!(" {} ", latex(&a));
            i = k;
        } else if let Some(c) = c {
            out += symbol(c).unwrap_or(" ");
            i += c.chars().count();
        } else if s[i] == '^' {
            let (a, j) = group(&s, i + 1);
            out += &power(&a);
            i = j;
        } else if s[i] == '_' {
            let (a, j) = group(&s, i + 1);
            out += &format!(" {} ", latex(&a));
            i = j;
        } else if starts(&s, i, "{,}") {
            out += " comma ";
            i += 3;
        } else if s[i] == '\\' && s.get(i + 1).is_some_and(|c| [',', ';', ':', '!', ' '].contains(c)) {
            out += " ";
            i += 2;
        } else if s[i] == '~' {
            out += " ";
            i += 1;
        } else if s[i] == '\\' && s.get(i + 1) == Some(&'\\') {
            out += ", ";
            i += 2;
        } else if s[i] == '{' || s[i] == '}' {
            i += 1;
        } else {
            let ch = s[i];
            match ch {
                '=' => out += " equals ",
                '+' => out += " plus ",
                '-' => out += " minus ",
                '<' => out += " is less than ",
                '>' => out += " is greater than ",
                '(' | ')' | '[' | ']' | '|' | '&' => out += " ",
                '/' => out += " over ",
                ';' => out += ", ",
                '!' => out += " factorial ",
                c => out.push(c),
            }
            i += 1;
        }
    }
    crate::marking::collapse_ws(&out).trim().to_string()
}

/// `\word` followed by a word boundary, for the listed words (longest that fits).
fn word_cmd(s: &[char], i: usize, words: &[&str]) -> Option<usize> {
    if s.get(i) != Some(&'\\') {
        return None;
    }
    for w in words {
        let lit = format!("\\{w}");
        if starts(s, i, &lit) {
            let end = i + lit.chars().count();
            let next_word = s.get(end).is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_');
            if !next_word {
                return Some(lit.chars().count());
            }
        }
    }
    None
}

// ---------- plain text to speech-friendly text

fn rx(cell: &'static OnceLock<Regex>, p: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(p).unwrap())
}

pub fn plain(text: &str) -> String {
    let mut s = String::new();
    for seg in markup::segments(text) {
        if seg.math {
            s += &format!(" {} ", latex(&seg.text));
        } else {
            s += &seg.text;
        }
    }
    let mut s = s.replace("**", "").replace('*', "").replace("\\n", " ");
    for (from, to) in [
        ("²", " squared"), ("³", " cubed"), ("⁻¹", " to the minus one"), ("°C", " degrees Celsius"), ("°", " degrees"),
        ("×", " times "), ("÷", " divided by "), ("±", " plus or minus "), ("≈", " approximately "), ("≤", " less than or equal to "),
        ("≥", " greater than or equal to "), ("→", " to "), ("←", " "), ("✓", ""), ("★", ""), ("☆", ""), ("√", " square root of "),
        ("π", " pi "), ("Ω", " ohms"), ("µF", " microfarads"), ("Ø", "diameter "), ("–", ", "), ("—", ", "), ("…", "..."),
    ] {
        s = s.replace(from, to);
    }
    s = s.chars().map(|c| match c {
        '₀'..='₉' => format!(" {}", (c as u32 - '₀' as u32)),
        c => c.to_string(),
    }).collect();
    static THOUSANDS: OnceLock<Regex> = OnceLock::new();
    static USD: OnceLock<Regex> = OnceLock::new();
    static UNDERS: OnceLock<Regex> = OnceLock::new();
    static RAND: OnceLock<Regex> = OnceLock::new();
    static COMMA: OnceLock<Regex> = OnceLock::new();
    static KMH: OnceLock<Regex> = OnceLock::new();
    static MS: OnceLock<Regex> = OnceLock::new();
    static NKG: OnceLock<Regex> = OnceLock::new();
    static EG: OnceLock<Regex> = OnceLock::new();
    static IE: OnceLock<Regex> = OnceLock::new();
    static ETC: OnceLock<Regex> = OnceLock::new();
    static SPACE_PUNCT: OnceLock<Regex> = OnceLock::new();
    static EMPTY: OnceLock<Regex> = OnceLock::new();
    const W: &str = "A-Za-z0-9_";
    s = rx(&THOUSANDS, &format!(r"([0-9]) (?=[0-9]{{3}}(?![{W}]))")).replace_all(&s, "$1").into_owned();
    s = rx(&USD, r"US\$\s?([0-9]+)").replace_all(&s, |c: &fancy_regex::Captures<'_, str>| format!("{} US dollar{}", &c[1], if &c[1] == "1" { "" } else { "s" })).into_owned();
    s = rx(&UNDERS, r"_{3,}").replace_all(&s, " ").into_owned().replace(" = ", " equals ");
    s = rx(&RAND, &format!(r"(?<![{W}])R([0-9]+)(?:,([0-9]{{2}}))?(?![{W}])"))
        .replace_all(&s, |c: &fancy_regex::Captures<'_, str>| match c.get(2).map(|m| m.as_str()) {
            Some(cents) if cents != "00" => format!("{} rand {} cents", &c[1], cents.parse::<u32>().unwrap_or(0)),
            _ => format!("{} rand", &c[1]),
        })
        .into_owned();
    s = rx(&COMMA, r"([0-9]),([0-9])").replace_all(&s, "$1 comma $2").into_owned();
    s = rx(&KMH, &format!(r"(?<![{W}])km/h(?![{W}])")).replace_all(&s, "kilometres per hour").into_owned();
    s = rx(&MS, &format!(r"(?<![{W}])m/s(?![{W}])")).replace_all(&s, "metres per second").into_owned();
    s = rx(&NKG, &format!(r"(?<![{W}])N/kg(?![{W}])")).replace_all(&s, "newtons per kilogram").into_owned();
    s = rx(&EG, &format!(r"(?i)(?<![{W}])e\.g\.\s*")).replace_all(&s, "for example, ").into_owned();
    s = rx(&IE, &format!(r"(?i)(?<![{W}])i\.e\.\s*")).replace_all(&s, "that is, ").into_owned();
    s = rx(&ETC, &format!(r"(?i)(?<![{W}])etc\.")).replace_all(&s, "and so on.").into_owned();
    s = rx(&SPACE_PUNCT, r"\s+([,.;:!?])").replace_all(&s, "$1").into_owned();
    let s = crate::marking::collapse_ws(&s).trim().to_string();
    if rx(&EMPTY, r"^[\s,.;:!?]*$").is_match(&s).unwrap_or(false) {
        String::new()
    } else {
        s
    }
}

fn sentence(t: Option<&str>) -> String {
    let s = plain(t.unwrap_or(""));
    if s.is_empty() {
        s
    } else if s.ends_with(['.', '!', '?', ':']) {
        s
    } else {
        s + "."
    }
}

fn strip_dot_colon(s: String) -> String {
    match s.strip_suffix('.') {
        Some(t) => format!("{t}:"),
        None => s,
    }
}

pub fn block_speech(b: &Block) -> String {
    let t = |o: &Option<String>| o.clone();
    match b.kind.as_str() {
        "heading" | "paragraph" => sentence(b.text.as_deref()),
        "list" => b
            .items
            .iter()
            .enumerate()
            .map(|(i, it)| format!("{}{}", if b.ordered { format!("{}. ", i + 1) } else { String::new() }, sentence(Some(it))))
            .collect::<Vec<_>>()
            .join(" "),
        "definition" => format!("{} {}", strip_dot_colon(sentence(b.term.as_deref())), sentence(b.text.as_deref())),
        "callout" => format!("{} {}", strip_dot_colon(sentence(b.label.as_deref())), sentence(b.text.as_deref())),
        "example" => {
            let mut parts = vec![sentence(Some(b.title.as_deref().filter(|s| !s.is_empty()).unwrap_or("Example"))), sentence(b.problem.as_deref())];
            for st in &b.steps {
                parts.push(sentence(Some(st)));
            }
            if let Some(a) = t(&b.answer).filter(|a| !a.is_empty()) {
                parts.push(format!("Answer: {}", sentence(Some(&a))));
            }
            parts.join(" ")
        }
        "formula" => {
            let mut s = sentence(Some(&latex(b.latex.as_deref().unwrap_or(""))));
            if let Some(c) = b.caption.as_deref().filter(|c| !c.is_empty()) {
                s += &format!(" {}", sentence(Some(c)));
            }
            s
        }
        "passage" => {
            let title = b.title.as_deref().filter(|s| !s.is_empty()).map(|s| format!("{} ", sentence(Some(s)))).unwrap_or_default();
            format!("{title}{}", sentence(b.text.as_deref()))
        }
        "diagram" => format!("Diagram: {}", sentence(b.caption.as_deref())),
        "table" => {
            let mut parts = Vec::new();
            if let Some(c) = b.caption.as_deref().filter(|c| !c.is_empty()) {
                parts.push(format!("Table: {}", sentence(Some(c))));
            }
            let heads: Vec<String> = b.headers.iter().map(|h| plain(h)).collect();
            for row in &b.rows {
                let cells: Vec<String> = row
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| {
                        let v = plain(c);
                        if v.is_empty() {
                            return None;
                        }
                        Some(if i == 0 || heads.get(i).is_none_or(|h| h.is_empty()) { v } else { format!("{}: {v}", heads[i]) })
                    })
                    .collect();
                parts.push(format!("{}.", cells.join("; ")));
            }
            parts.join(" ")
        }
        _ => String::new(),
    }
}

pub fn section_speech(blocks: &[Block]) -> String {
    blocks.iter().map(block_speech).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maths_in_words() {
        assert_eq!(latex("x^2 + \\frac{1}{2}"), "x squared plus 1 over 2");
        assert_eq!(latex("\\sqrt[3]{8} = 2"), "the cube root of 8 equals 2");
        assert_eq!(latex("12{,}5\\ \\text{m}"), "12 comma 5 m");
        assert_eq!(plain("R1 250,00 and 3,5 km/h"), "1250 rand and 3 comma 5 kilometres per hour");
        assert_eq!(hash("1|abc"), hash("1|abc"));
        assert_eq!(hash(""), "811c9dc5");
    }

    /// Every read-aloud file belongs to a section of its topic, so the ids still match
    /// the ones the audio was made with.
    #[test]
    fn audio_files_match_sections() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../audio");
        let mut checked = 0;
        for g in &crate::content::catalog().grades {
            for s in &g.subjects {
                for t in &s.topics {
                    let dir = root.join(format!("grade-{}/{}/{}", g.grade, s.id, t.id));
                    let Ok(rd) = std::fs::read_dir(&dir) else { continue };
                    let topic = crate::content::topic(g.grade, &s.id, &t.id).unwrap();
                    let ids: std::collections::HashSet<String> = split(&topic.notes).into_iter().map(|x| x.id).collect();
                    for e in rd.flatten() {
                        let name = e.file_name().to_string_lossy().to_string();
                        if let Some(id) = name.strip_suffix(".ogg") {
                            assert!(ids.contains(id), "{} has audio {id} that matches no section", dir.display());
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert!(checked > 1000, "only {checked} audio files checked");
    }
}
