//! Helpers for question cards (src/js/questions.js in FreeSheet 1.1).

use crate::markup;
use crate::model::{js_num, value_f64, Question};

pub const LETTERS: [&str; 8] = ["A", "B", "C", "D", "E", "F", "G", "H"];

fn hash(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for u in s.encode_utf16() {
        h ^= u as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

/// Deterministic shuffle so a question always shows the same order.
pub fn seeded_shuffle<T: Clone + PartialEq>(arr: &[T], seed: &str) -> Vec<T> {
    let mut a = arr.to_vec();
    let mut s = hash(seed);
    if s == 0 {
        s = 1;
    }
    for i in (1..a.len()).rev() {
        s = s.wrapping_mul(1103515245).wrapping_add(12345);
        let j = (s % (i as u32 + 1)) as usize;
        a.swap(i, j);
    }
    // Avoid showing the answers in their original order when possible.
    if a.len() > 1 && a.iter().zip(arr).all(|(x, y)| x == y) {
        let first = a.remove(0);
        a.push(first);
    }
    a
}

pub fn format_number(n: f64) -> String {
    js_num(n).replacen('.', ",", 1)
}

/// The correct answer, as HTML (markup).
pub fn correct_answer_html(q: &Question) -> String {
    match q.kind.as_str() {
        "mcq" => {
            let i = q.answer_index().unwrap_or(-1);
            let opt = usize::try_from(i).ok().and_then(|i| q.options.get(i)).cloned().unwrap_or_default();
            format!("{}. {}", usize::try_from(i).ok().and_then(|i| LETTERS.get(i)).copied().unwrap_or("?"), markup::inline(&opt))
        }
        "truefalse" => if q.answer_bool() == Some(true) { "True".into() } else { "False".into() },
        "short" => {
            let mut h = markup::inline(q.answers.first().map(String::as_str).unwrap_or(""));
            if q.answers.len() > 1 {
                h += &format!(" <span class=\"muted\">(also accepted: {})</span>", q.answers[1..].iter().map(|a| markup::inline(a)).collect::<Vec<_>>().join("; "));
            }
            h
        }
        "numeric" => {
            let mut h = markup::escape_html(&format_number(value_f64(&q.answer).unwrap_or(f64::NAN)));
            if let Some(u) = q.unit.as_deref().filter(|u| !u.is_empty()) {
                h += &format!(" {}", markup::inline(u));
            }
            h
        }
        _ => String::new(),
    }
}

/// For drop-down labels, which show plain text: drop ** and *, keep $maths$ source minus $.
pub fn strip_markup(s: &str) -> String {
    s.replace("**", "").replace('*', "").replace("\\$", "\u{0}").replace('$', "").replace('\u{0}', "$")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuffle_is_stable_and_never_identity() {
        let a = vec!["a", "b", "c", "d"];
        assert_eq!(seeded_shuffle(&a, "q1"), seeded_shuffle(&a, "q1"));
        assert_ne!(seeded_shuffle(&a, "q1"), a);
    }
}
