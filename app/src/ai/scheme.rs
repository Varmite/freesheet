//! Turns a written question's memo into separate mark-earning points, so the model only
//! has to answer "did the learner make this point?" and the marks are added up here.
//!
//! Two memo layouts are understood:
//!   "✓ A point (1)\n✓ Another point (2)", also several on a line and "(a) ✓ ..."
//!   "Any three (1 each): one; two; three; four"
//! Anything else (essay rubrics, unusual layouts) gives None, and the learner marks that
//! answer themselves as before. The marks found must add up to the question's marks.

use regex::Regex;
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub text: String,
    pub marks: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scheme {
    pub points: Vec<Point>,
    /// "Any three": only this many points can earn marks.
    pub any: Option<usize>,
    /// What comes before the points in the memo (a model answer), for showing.
    pub preface: Option<String>,
}

fn number_word(s: &str) -> Option<usize> {
    let words = ["one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];
    let s = s.to_lowercase();
    words.iter().position(|w| *w == s).map(|i| i + 1).or_else(|| s.parse().ok())
}

/// Markup and numbering around a point.
fn clean(s: &str) -> String {
    static LEAD: OnceLock<Regex> = OnceLock::new();
    let lead = LEAD.get_or_init(|| Regex::new(r"^[\s✓•\-–]*(?:\(?[a-z]{1,3}\)|\(?[0-9]{1,2}\))?[\s✓•\-–:]*").unwrap());
    let s = s.replace('✓', " ").replace("**", "");
    let t = lead.replace(s.trim(), "");
    t.trim().trim_end_matches([';', ',']).trim().to_string()
}

fn usable(text: &str) -> bool {
    text.chars().any(char::is_alphanumeric)
}

fn any_scheme(memo: &str, marks: f64) -> Option<Scheme> {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| Regex::new(r"(?is)^\s*any\s+([a-z0-9]+)\b[^:\n]*?\(\s*([0-9]+)\s*(?:marks?\s+)?each\s*\)[^:\n]*:(.*)$").unwrap());
    let c = r.captures(memo)?;
    let n = number_word(&c[1])?;
    let each: f64 = c[2].parse().ok()?;
    if n as f64 * each != marks {
        return None;
    }
    let points: Vec<Point> = c[3].split([';', '\n']).map(clean).filter(|t| usable(t)).map(|text| Point { text, marks: each }).collect();
    (points.len() >= n).then_some(Scheme { points, any: Some(n), preface: None })
}

/// Replace maths with same-length filler, so "(1)" inside $f(1)$ isn't taken for a mark.
fn hide_math(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut inside = false;
    for c in s.chars() {
        if c == '$' {
            inside = !inside;
        }
        if inside && c != '$' {
            out.extend(std::iter::repeat_n('_', c.len_utf8()));
        } else {
            out.push(c);
        }
    }
    out
}

fn tick_scheme(memo: &str, marks: f64) -> Option<Scheme> {
    static MARK: OnceLock<Regex> = OnceLock::new();
    static HEAD: OnceLock<Regex> = OnceLock::new();
    // A model answer comes first in some memos; only what follows "Marking:" counts.
    let head = HEAD.get_or_init(|| Regex::new(r"(?i)\bmarking(?:\s+guideline)?\s*:").unwrap());
    let split = head.find_iter(memo).last();
    let body = split.map_or(memo, |m| &memo[m.end()..]);
    let preface = split.map(|m| memo[..m.start()].trim().to_string()).filter(|s| !s.is_empty());
    let mark = MARK.get_or_init(|| Regex::new(r"\(\s*([0-9]{1,2})\s*\)").unwrap());
    let masked = hide_math(body);
    let mut points = Vec::new();
    let mut from = 0;
    for c in mark.captures_iter(&masked) {
        let whole = c.get(0).unwrap();
        let text = clean(&body[from..whole.start()]);
        from = whole.end();
        if !usable(&text) {
            return None;
        }
        points.push(Point { text, marks: c[1].parse().ok()? });
    }
    if points.is_empty() || usable(&clean(&body[from..])) {
        return None; // text after the last mark: not a plain list of points
    }
    let total: f64 = points.iter().map(|p| p.marks).sum();
    (total == marks).then_some(Scheme { points, any: None, preface })
}

pub fn parse(memo: &str, marks: f64) -> Option<Scheme> {
    any_scheme(memo, marks).or_else(|| tick_scheme(memo, marks))
}

impl Scheme {
    /// The mark for these per-point verdicts (same order as `points`).
    pub fn score(&self, met: &[bool]) -> f64 {
        let got = self.points.iter().zip(met).filter(|(_, m)| **m).map(|(p, _)| p.marks);
        match self.any {
            Some(n) => got.take(n).sum(),
            None => got.sum(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks() {
        let s = parse("✓ Light travels in straight lines (1)\n✓ An opaque object blocks the light (1)", 2.0).unwrap();
        assert_eq!(s.points.len(), 2);
        assert_eq!(s.points[0].text, "Light travels in straight lines");
        assert_eq!(s.any, None);
    }

    #[test]
    fn several_on_a_line_and_maths() {
        let s = parse("(a) ✓ $C = 8d + 15$ (1)\n(b) ✓ $8d + 15 = f(1)$ (1) ✓ $d = 12$ km (1)", 3.0).unwrap();
        let t: Vec<&str> = s.points.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(t, ["$C = 8d + 15$", "$8d + 15 = f(1)$", "$d = 12$ km"]);
    }

    #[test]
    fn any_n() {
        let s = parse("Any three (1 each): collects taxes; makes laws; builds roads; pays grants", 3.0).unwrap();
        assert_eq!(s.any, Some(3));
        assert_eq!(s.points.len(), 4);
        assert_eq!(s.score(&[true, true, true, true]), 3.0);
        assert_eq!(s.score(&[true, false, false, true]), 2.0);
    }

    #[test]
    fn model_answer_is_skipped() {
        let s = parse("Model answer: It was (1) dark.\n\nMarking:\n✓ Hook (1)\n✓ Conflict (1)", 2.0).unwrap();
        assert_eq!(s.points.len(), 2);
        assert_eq!(s.points[0].text, "Hook");
        assert_eq!(s.preface.as_deref(), Some("Model answer: It was (1) dark."));
    }

    #[test]
    fn years_are_not_marks() {
        let s = parse("✓ Changes after diamonds (1867) and gold (1886) were found (1)\n✓ Mining grew (1)", 2.0).unwrap();
        assert_eq!(s.points[0].text, "Changes after diamonds (1867) and gold (1886) were found");
    }

    #[test]
    fn unknown_layouts_are_left_to_the_learner() {
        assert_eq!(parse("Credit any sensible answer.", 2.0), None);
        assert_eq!(parse("✓ One (1)\n✓ Two (1)", 3.0), None); // doesn't add up
        assert_eq!(parse("Content 15, language 10. Use the rubric.", 25.0), None);
    }
}
