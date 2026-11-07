//! Answer marking: a line-by-line port of the JavaScript version (src/js/marking.js in
//! FreeSheet 1.1), checked against it by tests.

use crate::model::{value_f64, Question};
use fancy_regex::Regex;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

pub fn marks_of(q: &Question) -> f64 {
    match q.marks.as_ref().and_then(value_f64) {
        Some(m) if m.is_finite() && m > 0.0 => m,
        _ => 1.0,
    }
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).unwrap())
}

/// Strip simple markup so "**Photosynthesis**" and "photosynthesis" compare equal.
pub fn norm_text(s: &str) -> String {
    static LEAD: OnceLock<Regex> = OnceLock::new();
    static TRAIL: OnceLock<Regex> = OnceLock::new();
    static ART: OnceLock<Regex> = OnceLock::new();
    let t: String = s.nfkc().collect();
    let t = t.replace("**", "").replace(['$', '*'], "");
    let t: String = t
        .chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{02BC}' => '\'',
            '\u{201C}' | '\u{201D}' => '"',
            '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
            c => c,
        })
        .collect();
    let t = js_lower(&t);
    let t = collapse_ws(&t);
    let t = t.trim();
    let t = re(&LEAD, r#"^["'(\[]+"#).replace(t, "");
    let t = re(&TRAIL, r#"[\s.,;:!?"')\]]+$"#).replace(&t, "");
    re(&ART, r"^(the|a|an) ").replace(&t, "").into_owned()
}

/// JavaScript's toLowerCase.
pub fn js_lower(s: &str) -> String {
    s.to_lowercase()
}

/// /\s+/g -> ' '
pub fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut ws = false;
    for c in s.chars() {
        if c.is_whitespace() || c == '\u{feff}' {
            if !ws {
                out.push(' ');
            }
            ws = true;
        } else {
            out.push(c);
            ws = false;
        }
    }
    out
}

// ---------- Spelling slips in short answers
//
// Outside language subjects a typed answer with small spelling mistakes still counts
// ("resillience" for "resilience"). A word the learner typed that is itself a real word
// in the content is a different answer, not a typo, so it is never accepted.

pub fn is_language_subject(subject_id: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, "language|english|afrikaans|isizulu|isixhosa|isindebele|sepedi|sesotho|setswana|siswati|tshivenda|xitsonga|french|german")
        .is_match(subject_id)
        .unwrap_or(false)
}

fn allowed_edits(len: usize) -> usize {
    if len <= 5 {
        0
    } else if len <= 8 {
        1
    } else if len <= 12 {
        2
    } else {
        3
    }
}

/// Edit distance counting insertions, deletions, substitutions and swapped neighbours as
/// one slip each. Stops early once past `max`.
pub fn edit_distance(a: &[char], b: &[char], max: usize) -> usize {
    if a.len().abs_diff(b.len()) > max {
        return max + 1;
    }
    let mut prev2: Option<Vec<usize>> = None;
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i];
        let mut best = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            let mut v = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            if let Some(p2) = &prev2 {
                if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                    v = v.min(p2[j - 2] + 1);
                }
            }
            cur.push(v);
            best = best.min(v);
        }
        if best > max {
            return max + 1;
        }
        prev2 = Some(prev);
        prev = cur;
    }
    prev[b.len()]
}

fn words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() || c == '\'' {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn has_ascii_digit(s: &str) -> bool {
    s.bytes().any(|b| b.is_ascii_digit())
}

/// Is `given` the expected `answer` (both already through norm_text) with only small
/// spelling slips? `known` is the set of real words.
pub fn typo_of(given: &str, answer: &str, known: Option<&HashSet<String>>) -> bool {
    if given.is_empty() || answer.is_empty() || has_ascii_digit(given) || has_ascii_digit(answer) {
        return false;
    }
    let mut g = words(given);
    let mut a = words(answer);
    if g.concat() == a.concat() {
        return true; // only the spacing or hyphens differ
    }
    if g.len() != a.len() {
        g = vec![g.concat()];
        a = vec![a.concat()];
    }
    let phrase = a.len() > 1;
    let mut budget: i64 = if phrase { allowed_edits(a.concat().chars().count()) as i64 } else { i64::MAX };
    for i in 0..a.len() {
        if g[i] == a[i] {
            continue;
        }
        if known.is_some_and(|k| k.contains(&g[i])) {
            return false;
        }
        let len = a[i].chars().count();
        let max = if phrase { if len <= 3 { 0 } else { allowed_edits(len).max(1) } } else { allowed_edits(len) };
        let gc: Vec<char> = g[i].chars().collect();
        let ac: Vec<char> = a[i].chars().collect();
        let d = if max > 0 { edit_distance(&gc, &ac, max) } else { max + 1 };
        budget -= d as i64;
        if d > max || budget < 0 {
            return false;
        }
    }
    true
}

/// Parse a learner's number. Accepts SA style (12 500,75), "3.5", "-2", "R150", "3/4", "1e-3", "45%".
pub fn parse_number(input: &str) -> f64 {
    static FRAC: OnceLock<Regex> = OnceLock::new();
    static NUM: OnceLock<Regex> = OnceLock::new();
    static UNIT: OnceLock<Regex> = OnceLock::new();
    let t: String = input.nfkc().collect();
    let t = t.replace('\u{2212}', "-").replace(['\u{a0}', '\u{202f}'], " ");
    let t = t.trim();
    if t.is_empty() {
        return f64::NAN;
    }
    let t = t.strip_prefix(['R', 'r']).map(|r| r.trim_start()).unwrap_or(t);
    let t = t.strip_prefix('$').map(|r| r.trim_start()).unwrap_or(t);
    if let Ok(Some(c)) = re(&FRAC, r"^([-+]?[0-9]+(?:[.,][0-9]+)?)\s*/\s*([0-9]+(?:[.,][0-9]+)?)\s*[a-zA-Z%°/·⁻²³\s]*$").captures(t) {
        let a = parse_number(&c[1]);
        let b = parse_number(&c[2]);
        return if b == 0.0 { f64::NAN } else { a / b };
    }
    let Ok(Some(m)) = re(&NUM, r"^([-+]?\s*(?:[0-9]{1,3}(?: [0-9]{3})+(?![0-9])|[0-9]+)(?:[.,][0-9]+)*(?:[eE][-+]?[0-9]+)?)").captures(t) else {
        return f64::NAN;
    };
    let whole = m.get(0).unwrap();
    let rest = t[whole.end()..].trim();
    if !rest.is_empty() && has_ascii_digit(&rest.replace(['²', '³', '⁻', '¹'], "")) && !re(&UNIT, r"^[a-zA-Zµ°%·/.\-\s]+[0-9]*$").is_match(rest).unwrap_or(false) {
        return f64::NAN;
    }
    let mut num: String = m[1].chars().filter(|c| !c.is_whitespace()).collect();
    let mut exp = String::new();
    if let Some(pos) = num.find(['e', 'E']) {
        exp = num[pos..].to_string();
        num.truncate(pos);
    }
    let commas = num.matches(',').count();
    let dots = num.matches('.').count();
    if commas > 0 && dots > 0 {
        if num.rfind(',') > num.rfind('.') {
            num = num.replace('.', "").replacen(',', ".", 1);
        } else {
            num = num.replace(',', "");
        }
    } else if commas > 1 {
        num = num.replace(',', "");
    } else if commas == 1 {
        num = num.replacen(',', ".", 1);
    } else if dots > 1 {
        num = num.replace('.', "");
    }
    let s = format!("{num}{exp}");
    match s.trim_start_matches('+').parse::<f64>() {
        Ok(v) if v.is_finite() => v,
        _ => f64::NAN,
    }
}

fn is_int(v: &Value) -> bool {
    v.as_i64().is_some() || v.as_f64().is_some_and(|f| f.fract() == 0.0)
}

pub fn is_answered(q: &Question, r: Option<&Value>) -> bool {
    let Some(r) = r else { return false };
    match q.kind.as_str() {
        "mcq" => r.is_number() && is_int(r),
        "truefalse" => r.is_boolean(),
        "short" | "numeric" => r.as_str().is_some_and(|s| !s.trim().is_empty()),
        "match" => r.as_array().is_some_and(|a| a.iter().any(|x| !x.is_null() && x.as_str() != Some(""))),
        "open" => r.get("text").and_then(Value::as_str).is_some_and(|t| !t.trim().is_empty()),
        _ => false,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub score: f64,
    pub max: f64,
    /// None for an unmarked written answer.
    pub correct: Option<bool>,
    pub needs_self_mark: bool,
    pub spelling: Option<String>,
    pub pairs_right: usize,
}

pub fn mark(q: &Question, r: Option<&Value>, self_mark: Option<f64>) -> Mark {
    let max = marks_of(q);
    let out = |ok: bool| Mark { score: if ok { max } else { 0.0 }, max, correct: Some(ok), needs_self_mark: false, spelling: None, pairs_right: 0 };
    match q.kind.as_str() {
        "mcq" => {
            let ok = matches!(r, Some(v) if v.is_number() && is_int(v) && v.as_f64() == q.answer.as_f64());
            out(ok)
        }
        "truefalse" => {
            let ok = matches!(r, Some(Value::Bool(b)) if Some(*b) == q.answer.as_bool());
            out(ok)
        }
        "short" => {
            let given = norm_text(&r.map(crate::model::text_of_opt).unwrap_or_default());
            let mut ok = !given.is_empty() && q.answers.iter().any(|a| norm_text(a) == given);
            let mut spelling = None;
            if !ok && !given.is_empty() {
                if let Some(words) = &q.typo_words {
                    if let Some(hit) = q.answers.iter().find(|a| typo_of(&given, &norm_text(a), Some(words))) {
                        ok = true;
                        spelling = Some(hit.clone());
                    }
                }
            }
            let mut m = out(ok);
            m.spelling = spelling;
            m
        }
        "numeric" => {
            let v = match r {
                Some(Value::String(s)) => parse_number(s),
                Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
                _ => f64::NAN,
            };
            let tol = q.tolerance.as_ref().and_then(value_f64).filter(|t| t.is_finite()).map(f64::abs).unwrap_or(0.01);
            let ans = value_f64(&q.answer).unwrap_or(f64::NAN);
            out(v.is_finite() && (v - ans).abs() <= tol + 1e-9)
        }
        "match" => {
            let given = r.and_then(Value::as_array);
            let right = q
                .pairs
                .iter()
                .enumerate()
                .filter(|(i, p)| given.and_then(|g| g.get(*i)).and_then(Value::as_str) == Some(p.right.as_str()))
                .count();
            let raw = if q.pairs.is_empty() { 0.0 } else { max * right as f64 / q.pairs.len() as f64 };
            Mark { score: js_round(raw * 2.0) / 2.0, max, correct: Some(right == q.pairs.len()), needs_self_mark: false, spelling: None, pairs_right: right }
        }
        "open" => match self_mark.filter(|s| s.is_finite()) {
            None => Mark { score: 0.0, max, correct: None, needs_self_mark: true, spelling: None, pairs_right: 0 },
            Some(s) => {
                let s = s.clamp(0.0, max);
                Mark { score: s, max, correct: Some(s == max), needs_self_mark: false, spelling: None, pairs_right: 0 }
            }
        },
        _ => Mark { score: 0.0, max, correct: Some(false), needs_self_mark: false, spelling: None, pairs_right: 0 },
    }
}

/// Math.round (halves round up, towards +infinity).
pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// CAPS achievement levels.
pub fn caps_level(percent: f64) -> (u8, &'static str) {
    let p = percent;
    if p >= 80.0 {
        (7, "Outstanding achievement")
    } else if p >= 70.0 {
        (6, "Meritorious achievement")
    } else if p >= 60.0 {
        (5, "Substantial achievement")
    } else if p >= 50.0 {
        (4, "Adequate achievement")
    } else if p >= 40.0 {
        (3, "Moderate achievement")
    } else if p >= 30.0 {
        (2, "Elementary achievement")
    } else {
        (1, "Not achieved")
    }
}

pub fn format_marks(n: f64) -> String {
    let v = js_round(n * 10.0) / 10.0;
    crate::model::js_num(v).replace('.', ",")
}

pub fn pct(score: f64, max: f64) -> i64 {
    if max > 0.0 {
        js_round(score / max * 100.0) as i64
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        let n = |s: &str| parse_number(s);
        assert_eq!(n("12 500,75"), 12500.75);
        assert_eq!(n("3.5"), 3.5);
        assert_eq!(n("3,5"), 3.5);
        assert_eq!(n("R150"), 150.0);
        assert_eq!(n("3/4"), 0.75);
        assert_eq!(n("1e-3"), 0.001);
        assert_eq!(n("45%"), 45.0);
        assert_eq!(n("-2"), -2.0);
        assert!(n("3 4").is_nan());
        assert!(n("abc").is_nan());
        assert_eq!(n("1.234.567"), 1234567.0);
        assert_eq!(n("1,234.5"), 1234.5);
        assert_eq!(n("5 m/s2"), 5.0);
    }

    #[test]
    fn text_and_typos() {
        assert_eq!(norm_text("  The **Photosynthesis**. "), "photosynthesis");
        assert!(typo_of("resillience", "resilience", None));
        let known: HashSet<String> = ["endothermic".to_string()].into();
        assert!(!typo_of("endothermic", "exothermic", Some(&known)));
        assert!(typo_of("humman dignity", "human dignity", None));
        assert!(!typo_of("cell", "cells", None)); // short words exact
        assert_eq!(format_marks(2.5), "2,5");
        assert_eq!(format_marks(3.0), "3");
        assert_eq!(caps_level(72.0).0, 6);
    }
}
