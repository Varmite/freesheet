//! Hints for practice questions (never offered in exams). A port of src/js/hints.js
//! (FreeSheet 1.1) that produces the same hints, checked against it by tests.
//!
//! Hints get stronger one level at a time: a clue from the notes (the sentence that best
//! matches the question, with the answer blanked out), then per type: ruling out options,
//! what to watch for, the answer's letter pattern, the first step, a range, one pair, or
//! for written answers how many points to make, key words and the first point.

use crate::markup::{self, escape_html};
use crate::marking;
use crate::model::{js_num, text_of, value_f64, Block, Question, Topic};
use crate::sections;
use fancy_regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const LETTERS: [&str; 8] = ["A", "B", "C", "D", "E", "F", "G", "H"];
pub const BLANK: &str = "\u{27E6}blank\u{27E7}";
pub const BLANK_HTML: &str = "<span class=\"hint-blank\" aria-label=\"hidden answer\">\u{2003}\u{2003}\u{2003}</span>";

fn stop() -> &'static HashSet<&'static str> {
    static S: OnceLock<HashSet<&'static str>> = OnceLock::new();
    S.get_or_init(|| {
        "a an the of to in on at for and or but is are was were be been being it its this that these those with as by \
         from which what who whom whose when where why how do does did not no yes than then so such can could will would should may might \
         must has have had into about over under between your you we they he she i their there here also each one two three more most \
         very only just use used using called answer question give write following true false correct both other same make made"
            .split_whitespace()
            .collect()
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hint {
    pub kind: &'static str,
    pub section: Option<usize>,
    pub eliminate: Vec<usize>,
    pub pair: Option<usize>,
    pub html: String,
}

// ---------------------------------------------------------------- text helpers

fn re(cell: &'static OnceLock<Regex>, p: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(p).unwrap())
}

/// Text without $maths$ and bold/italic markers.
pub fn plain(raw: &str) -> String {
    static M: OnceLock<Regex> = OnceLock::new();
    re(&M, r"\$[^$]*\$").replace_all(raw, " ").replace("**", "").replace('*', "")
}

fn u16len(s: &str) -> usize {
    s.encode_utf16().count()
}

fn stem(w: &str) -> String {
    if u16len(w) > 4 {
        for suf in ["ings", "ing", "ed", "es", "s"] {
            if let Some(t) = w.strip_suffix(suf) {
                return t.to_string();
            }
        }
    }
    w.to_string()
}

/// Letter runs as /\p{L}[\p{L}'-]*/gu.
fn letter_runs(s: &str, extra: &[char]) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_alphabetic() && !c.is_numeric() || (!cur.is_empty() && extra.contains(&c)) {
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

fn tokens(raw: &str) -> Vec<String> {
    let lower = marking::js_lower(&plain(raw));
    let mut out = Vec::new();
    for w in letter_runs(&lower, &['\'', '-']) {
        let t = w.trim_matches('\'');
        if u16len(t) >= 3 && !stop().contains(t) {
            out.push(stem(t));
        }
    }
    out
}

/// Split markup text into sentences without breaking inside $maths$.
pub fn sentences(raw: &str) -> Vec<String> {
    static MATH: OnceLock<Regex> = OnceLock::new();
    static SPLIT: OnceLock<Regex> = OnceLock::new();
    static BACK: OnceLock<Regex> = OnceLock::new();
    let mut maths: Vec<String> = Vec::new();
    let t = re(&MATH, r"\$[^$]*\$").replace_all(raw, |c: &fancy_regex::Captures<'_, str>| {
        maths.push(c[0].to_string());
        format!("\u{2060}{}\u{2060}", maths.len() - 1)
    });
    let split = re(&SPLIT, "(?<=[.!?])\\s+(?=[A-Z\u{2060}(*\"'])|\\n+");
    let mut pieces = Vec::new();
    let mut last = 0;
    for m in split.find_iter(t.as_ref()).flatten() {
        pieces.push(&t[last..m.start()]);
        last = m.end();
    }
    pieces.push(&t[last..]);
    let back = re(&BACK, "\u{2060}([0-9]+)\u{2060}");
    pieces
        .into_iter()
        .map(|s| back.replace_all(s, |c: &fancy_regex::Captures<'_, str>| maths.get(c[1].parse::<usize>().unwrap_or(usize::MAX)).cloned().unwrap_or_default()).trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// The explaining text of a block, with how much to trust it as a clue.
fn block_strings(b: &Block) -> Vec<(String, f64)> {
    let w = |arr: Vec<Option<String>>, weight: f64| arr.into_iter().flatten().filter(|s| !s.is_empty()).map(|s| (s, weight)).collect::<Vec<_>>();
    match b.kind.as_str() {
        "paragraph" | "passage" => w(vec![b.text.clone()], 1.0),
        "list" => w(b.items.iter().cloned().map(Some).collect(), 1.0),
        "definition" => {
            let term = b.term.clone().unwrap_or_else(|| "undefined".into());
            let text = b.text.clone().unwrap_or_else(|| "undefined".into());
            let t = if term.contains('$') { term } else { format!("**{term}**") };
            w(vec![Some(format!("{t}: {text}"))], 1.25)
        }
        "callout" => w(vec![b.text.clone()], 1.0),
        "formula" => match &b.caption {
            Some(c) if !c.is_empty() => w(vec![Some(format!("{c}: ${}$", b.latex.clone().unwrap_or_else(|| "undefined".into())))], 1.1),
            _ => vec![],
        },
        "table" => w(b.rows.iter().map(|r| Some(r.iter().map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect::<Vec<_>>().join("; "))).collect(), 0.6),
        "diagram" => match &b.caption {
            Some(c) if !c.is_empty() => w(vec![Some(c.clone())], 0.8),
            _ => vec![],
        },
        _ => vec![],
    }
}

// ---------------------------------------------------------------- notes index

struct Sent {
    raw: String,
    section: usize,
    toks: Vec<String>,
    tokset: HashSet<String>,
    weight: f64,
}

pub struct NotesIndex {
    sents: Vec<Sent>,
    df: HashMap<String, f64>,
    n: f64,
}

impl NotesIndex {
    pub fn new(topic: &Topic) -> Self {
        let mut sents = Vec::new();
        let mut df: HashMap<String, f64> = HashMap::new();
        for (si, sec) in sections::split(&topic.notes).iter().enumerate() {
            for b in &sec.blocks {
                for (s, weight) in block_strings(b) {
                    for raw in sentences(&s) {
                        let mut toks = Vec::new();
                        let mut tokset = HashSet::new();
                        for t in tokens(&raw) {
                            if tokset.insert(t.clone()) {
                                toks.push(t);
                            }
                        }
                        if toks.len() < 4 {
                            continue;
                        }
                        for t in &toks {
                            *df.entry(t.clone()).or_insert(0.0) += 1.0;
                        }
                        sents.push(Sent { raw, section: si, toks, tokset, weight });
                    }
                }
            }
        }
        let n = sents.len().max(1) as f64;
        NotesIndex { sents, df, n }
    }
    fn idf(&self, t: &str) -> f64 {
        (1.0 + self.n / self.df.get(t).copied().unwrap_or(0.5)).ln()
    }
}

// ---------------------------------------------------------------- answers (to hide)

fn to_fixed2(n: f64) -> String {
    format!("{n:.2}")
}

fn number_forms(v: &serde_json::Value) -> Vec<String> {
    let Some(n) = value_f64(v).filter(|n| n.is_finite()) else { return vec![] };
    let s = js_num(n);
    let mut out: Vec<String> = Vec::new();
    for f in [s.clone(), s.replacen('.', ",", 1), to_fixed2(n), to_fixed2(n).replacen('.', ",", 1)] {
        if !out.contains(&f) {
            out.push(f);
        }
    }
    out
}

pub fn answer_texts(q: &Question) -> Vec<String> {
    match q.kind.as_str() {
        "mcq" => {
            let t = plain(&option_text(q, q.answer_index()));
            if u16len(t.trim()) >= 2 { vec![t] } else { vec![] }
        }
        "short" => q.answers.iter().map(|a| plain(a)).collect(),
        "numeric" => number_forms(&q.answer),
        _ => vec![],
    }
}

/// q.options[i] as JavaScript would show it ("undefined" when missing).
fn option_text(q: &Question, i: Option<i64>) -> String {
    i.and_then(|i| usize::try_from(i).ok()).and_then(|i| q.options.get(i)).cloned().unwrap_or_else(|| "undefined".into())
}

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Case-insensitive char comparison (as a JavaScript /i regex).
fn ci_eq(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase()) || a.to_uppercase().eq(b.to_uppercase())
}

/// Replace every match of `needle` in `hay`: words with \b boundaries case-insensitively,
/// numbers with (?<![\d,.]) ... (?![\d]).
fn replace_answer(hay: &str, needle: &str, with: &str) -> String {
    let h: Vec<char> = hay.chars().collect();
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() {
        return hay.to_string();
    }
    let numeric = n[0].is_ascii_digit();
    let mut out = String::new();
    let mut i = 0;
    while i < h.len() {
        if i + n.len() <= h.len() && matches_at(&h, i, &n, numeric) {
            out.push_str(with);
            i += n.len();
        } else {
            out.push(h[i]);
            i += 1;
        }
    }
    out
}

fn matches_at(h: &[char], i: usize, n: &[char], numeric: bool) -> bool {
    let before = if i > 0 { Some(h[i - 1]) } else { None };
    let after = h.get(i + n.len()).copied();
    if numeric {
        if !h[i..i + n.len()].iter().zip(n).all(|(a, b)| a == b) {
            return false;
        }
        !before.is_some_and(|c| c.is_ascii_digit() || c == ',' || c == '.') && !after.is_some_and(|c| c.is_ascii_digit())
    } else {
        if !h[i..i + n.len()].iter().zip(n).all(|(a, b)| ci_eq(*a, *b)) {
            return false;
        }
        // \b at both ends: word/non-word changes across the edge.
        is_word(before) != is_word(Some(h[i])) && is_word(Some(h[i + n.len() - 1])) != is_word(after)
    }
}

fn contains_number(hay: &str, needle: &str) -> bool {
    let h: Vec<char> = hay.chars().collect();
    let n: Vec<char> = needle.chars().collect();
    (0..h.len()).any(|i| i + n.len() <= h.len() && matches_at(&h, i, &n, true))
}

/// Blank every occurrence of the answer outside maths. None if the answer still shows
/// (for example inside a formula), so the clue isn't used.
pub fn hide_answer(raw: &str, q: &Question) -> Option<String> {
    let mut answers: Vec<String> = answer_texts(q).into_iter().map(|a| a.trim().to_string()).filter(|a| u16len(a) >= 2).collect();
    answers.sort_by_key(|a| std::cmp::Reverse(u16len(a)));
    if answers.is_empty() {
        return Some(raw.to_string());
    }
    let mut out = String::new();
    for seg in markup::segments(raw) {
        if seg.math {
            out += &format!("${}$", seg.text);
            continue;
        }
        let mut t = seg.text;
        for a in &answers {
            t = replace_answer(&t, a, BLANK);
        }
        out += &t;
    }
    let squash = |t: &str| {
        static SQ: OnceLock<Regex> = OnceLock::new();
        re(&SQ, r"\s+|\\[a-z]+|[{}]").replace_all(&marking::js_lower(t), "").into_owned()
    };
    let math_text = markup::segments(&out).iter().filter(|s| s.math).map(|s| squash(&s.text)).collect::<Vec<_>>().join(" ");
    let starts_digit = |a: &str| a.chars().next().is_some_and(|c| c.is_ascii_digit());
    if answers.iter().any(|a| !starts_digit(a) && u16len(&squash(a)) >= 2 && math_text.contains(&squash(a))) {
        return None;
    }
    let lower = marking::js_lower(&plain(&out));
    if answers.iter().any(|a| if starts_digit(a) { contains_number(&lower, a) } else { lower.contains(&marking::js_lower(a)) }) {
        return None;
    }
    if q.kind == "numeric" {
        let forms = number_forms(&q.answer);
        if markup::segments(&out).iter().any(|s| s.math && forms.iter().any(|f| s.text.contains(f.as_str()))) {
            return None;
        }
    }
    Some(out)
}

/// Render a clue; leftover unpaired bold markers are dropped.
pub fn render_raw(raw: &str) -> String {
    markup::inline(raw).replace("**", "").split(BLANK).collect::<Vec<_>>().join(BLANK_HTML)
}

// ---------------------------------------------------------------- the clue from the notes

pub struct Clue {
    pub raw: String,
    pub section: usize,
}

pub fn clue_for(q: &Question, idx: &NotesIndex) -> Option<Clue> {
    if idx.sents.is_empty() {
        return None;
    }
    let mut weights: HashMap<String, f64> = HashMap::new();
    let mut add = |raw: &str, w: f64| {
        for t in tokens(raw) {
            let e = weights.entry(t).or_insert(0.0);
            *e = e.max(w);
        }
    };
    add(&q.prompt, 1.0);
    if q.kind == "mcq" {
        add(&option_text(q, q.answer_index()), 1.5);
    }
    if q.kind == "short" {
        for a in &q.answers {
            add(a, 1.5);
        }
    }
    if q.kind == "match" {
        for p in &q.pairs {
            add(&p.left, 0.7);
            add(&p.right, 0.7);
        }
    }
    if q.kind == "open" {
        add(q.memo.as_deref().unwrap_or("undefined"), 0.6);
    }
    if let Some(e) = q.explanation.as_deref().filter(|e| !e.is_empty()) {
        add(e, 0.8);
    }
    let mut scored: Vec<(&Sent, f64)> = idx
        .sents
        .iter()
        .map(|s| {
            let mut score = 0.0;
            for t in &s.toks {
                if let Some(w) = weights.get(t) {
                    score += idx.idf(t) * w;
                }
            }
            (s, (score * s.weight) / (s.toks.len() as f64).sqrt())
        })
        .filter(|(_, sc)| *sc > 1.6)
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let prompt_plain = marking::js_lower(plain(&q.prompt).trim());
    for (s, _) in scored.into_iter().take(6) {
        let Some(hidden) = hide_answer(&s.raw, q) else { continue };
        if marking::js_lower(plain(&hidden).trim()) == prompt_plain {
            continue;
        }
        let _ = &s.tokset;
        return Some(Clue { raw: hidden, section: s.section });
    }
    None
}

// ---------------------------------------------------------------- per-type hints

pub fn seeded_order<T: Clone>(arr: &[T], seed: &str) -> Vec<T> {
    let mut h: u32 = 2166136261;
    for c in seed.chars() {
        let mut buf = [0u16; 2];
        h ^= c.encode_utf16(&mut buf)[0] as u32;
        h = h.wrapping_mul(16777619);
    }
    let mut a = arr.to_vec();
    for i in (1..a.len()).rev() {
        h = h.wrapping_mul(1103515245).wrapping_add(12345);
        let j = (h % (i as u32 + 1)) as usize;
        a.swap(i, j);
    }
    a
}

fn fmt(n: f64) -> String {
    let r = if n.abs() >= 100.0 { marking::js_round(n) } else { marking::js_round(n * 100.0) / 100.0 };
    js_num(r).replacen('.', ",", 1)
}

/// A range around a number that contains it without giving it away.
pub fn range_for(v: &serde_json::Value) -> Option<(f64, f64)> {
    let n = value_f64(v).filter(|n| n.is_finite())?;
    if n == 0.0 {
        return Some((-1.0, 1.0));
    }
    let mag = 10f64.powf(n.abs().log10().floor());
    let step = mag / 2.0;
    let mut lo = ((n - n.abs() * 0.35) / step).floor() * step;
    let mut hi = ((n + n.abs() * 0.35) / step).ceil() * step;
    if lo == n {
        lo -= step;
    }
    if hi == n {
        hi += step;
    }
    Some((lo, hi))
}

fn shape_of(answer: &str, reveal_half: bool) -> String {
    plain(answer)
        .split_whitespace()
        .map(|w| {
            let keep = if reveal_half { u16len(w).div_ceil(2) } else { 1 };
            w.chars().enumerate().map(|(i, ch)| if i < keep || !(ch.is_alphabetic() && !ch.is_numeric()) { ch.to_string() } else { "_".into() }).collect::<Vec<_>>().join(" ")
        })
        .collect::<Vec<_>>()
        .join("\u{2003}") // an em space between words
}

fn first_step(explanation: Option<&str>, q: &Question) -> Option<String> {
    static SPLIT: OnceLock<Regex> = OnceLock::new();
    static ENDEQ: OnceLock<Regex> = OnceLock::new();
    let e = explanation.filter(|e| !e.is_empty())?;
    let split = re(&SPLIT, r";\s+|,\s+so\s+");
    let mut parts: Vec<String> = Vec::new();
    for s in sentences(e) {
        let mut last = 0;
        for m in split.find_iter(&s).flatten() {
            parts.push(s[last..m.start()].to_string());
            last = m.end();
        }
        parts.push(s[last..].to_string());
    }
    if parts.len() < 2 {
        return None;
    }
    let hidden = hide_answer(&parts[0], q)?;
    if re(&ENDEQ, r"=\s*$").is_match(&plain(&hidden)).unwrap_or(false) {
        return None;
    }
    Some(hidden)
}

fn absolute(text: &str) -> Option<String> {
    static R: OnceLock<regex::Regex> = OnceLock::new();
    let r = R.get_or_init(|| regex::Regex::new(r"(?i)(?-u:\b)(always|never|all|none|only|every|everyone|no one|cannot|must)(?-u:\b)").unwrap());
    r.captures(text).map(|c| marking::js_lower(&c[1]))
}

pub fn build(q: &Question, idx: &NotesIndex) -> Vec<Hint> {
    let mut out = Vec::new();
    let clue = clue_for(q, idx);
    let hint = |kind: &'static str, html: String| Hint { kind, section: None, eliminate: vec![], pair: None, html };
    let clue_hint = || clue.as_ref().map(|c| Hint { section: Some(c.section), ..hint("clue", format!("From your notes: \u{201C}{}\u{201D}", render_raw(&c.raw))) });
    match q.kind.as_str() {
        "mcq" => {
            out.extend(clue_hint());
            let answer = q.answer_index();
            let wrong_all: Vec<usize> = (0..q.options.len()).filter(|i| Some(*i as i64) != answer).collect();
            let wrong = seeded_order(&wrong_all, &q.id);
            let gone: Vec<usize> = wrong.into_iter().take(q.options.len().saturating_sub(2)).collect();
            for (k, i) in gone.iter().enumerate() {
                let l = LETTERS.get(*i).copied().unwrap_or("?");
                out.push(Hint {
                    eliminate: gone[..=k].to_vec(),
                    ..hint("eliminate", if k == 0 { format!("It isn't <strong>{l}</strong>. I've crossed it out.") } else { format!("It isn't <strong>{l}</strong> either. Choose from what's left.") })
                });
            }
        }
        "truefalse" => {
            out.extend(clue_hint());
            out.push(hint(
                "tip",
                match absolute(&plain(&q.prompt)) {
                    Some(w) => format!("Watch the word \u{201C}<strong>{}</strong>\u{201D}. A single exception would make the statement false.", escape_html(&w)),
                    None => "Read the statement word by word and check each part. If any part is wrong, the whole statement is false.".into(),
                },
            ));
        }
        "short" => {
            out.extend(clue_hint());
            if let Some(a) = q.answers.first() {
                let p = plain(a);
                let letters = p.chars().filter(|c| c.is_alphabetic() && !c.is_numeric()).count();
                let words: Vec<&str> = p.split_whitespace().collect();
                let word_count = words.len().max(1);
                let hidden: usize = words.iter().map(|w| w.chars().filter(|c| c.is_alphabetic() && !c.is_numeric()).count().saturating_sub(1)).sum();
                if hidden >= 2 {
                    out.push(hint("shape", format!("The answer has {}{} letters: <span class=\"hint-shape\">{}</span>", if word_count > 1 { format!("{word_count} words, ") } else { String::new() }, letters, escape_html(&shape_of(a, false)))));
                }
                if hidden >= 4 {
                    out.push(hint("shape", format!("It starts like this: <span class=\"hint-shape\">{}</span>", escape_html(&shape_of(a, true)))));
                }
            }
        }
        "numeric" => {
            out.extend(clue_hint());
            if let Some(step) = first_step(q.explanation.as_deref(), q) {
                out.push(hint("step", format!("Start like this: {}", render_raw(&step))));
            }
            if let Some((lo, hi)) = range_for(&q.answer) {
                let unit = q.unit.as_deref().filter(|u| !u.is_empty()).map(|u| format!(" {}", markup::inline(u))).unwrap_or_default();
                out.push(hint("range", format!("The answer is between <strong>{}</strong> and <strong>{}</strong>{unit}.", fmt(lo), fmt(hi))));
            }
        }
        "match" => {
            out.extend(clue_hint());
            let order = seeded_order(&(0..q.pairs.len()).collect::<Vec<_>>(), &q.id);
            for (k, i) in order.into_iter().take(q.pairs.len().saturating_sub(1)).enumerate() {
                let p = &q.pairs[i];
                out.push(Hint { pair: Some(i), ..hint("pair", format!("{}{} goes with <strong>{}</strong>.", if k > 0 { "And " } else { "" }, markup::inline(&p.left), markup::inline(&p.right))) });
            }
        }
        "open" => {
            let marks = marking::marks_of(q);
            let ms = js_num(marks);
            let s = if marks == 1.0 { "" } else { "s" };
            let mut html = format!("This is worth {ms} mark{s}, so aim for about {ms} clear point{s}{}", if clue.is_some() { "" } else { "." });
            if let Some(c) = &clue {
                html += &format!(". Your notes say: \u{201C}{}\u{201D}", render_raw(&c.raw));
            }
            out.push(Hint { section: clue.as_ref().map(|c| c.section), ..hint("tip", html) });
            let memo = q.memo.clone().unwrap_or_default();
            let mut seen = HashSet::new();
            let mut words: Vec<String> = Vec::new();
            for w in letter_runs(&plain(if q.memo.is_some() { &memo } else { "undefined" }), &['-']) {
                if w.chars().count() < 5 {
                    continue;
                }
                let w = marking::js_lower(&w);
                if !stop().contains(w.as_str()) && seen.insert(w.clone()) {
                    words.push(w);
                }
            }
            words.sort_by_key(|w| std::cmp::Reverse(u16len(w)));
            words.truncate(6);
            let keys = seeded_order(&words, &q.id);
            if keys.len() >= 2 {
                out.push(hint("keywords", format!("Words worth using: {}", keys.iter().map(|w| format!("<span class=\"hint-chip\">{}</span>", escape_html(w))).collect::<Vec<_>>().join(" "))));
            }
            static FIRST: OnceLock<Regex> = OnceLock::new();
            let first_line = memo.split('\n').next().unwrap_or("");
            let first = re(&FIRST, r"✓\s*\([0-9]+\)|\([0-9]+\)\s*$").replace_all(first_line, "").trim().to_string();
            if !first.is_empty() {
                out.push(hint("point", format!("One point the memo wants: {}", markup::inline(&first))));
            }
        }
        _ => {}
    }
    if out.is_empty() {
        out.push(hint("tip", "This one is a key term from this topic. Skim the headings and bold words in the notes: the answer is one of them.".into()));
    }
    out
}

/// The notes section that best explains a question (for "show me in the notes").
pub fn related_section(q: &Question, idx: &NotesIndex) -> Option<usize> {
    clue_for(q, idx).map(|c| c.section)
}

/// The (plain) text of a value, for tests.
#[allow(dead_code)]
pub fn value_text(v: &serde_json::Value) -> String {
    text_of(v)
}
