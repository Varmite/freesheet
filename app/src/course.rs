//! Language courses (Afrikaans): the lesson engine. A port of src/js/course.js
//! (FreeSheet 1.1) that generates the same exercises, checked against it by tests.
//!
//! A course is units of short lessons. Each lesson lists its new words and a few sentences
//! with every accepted translation; the exercises are generated from that: meaning, pairs,
//! build (word tiles), type, blank, pick, listen and transform (grammar drills).
//! Answers are checked like a forgiving teacher: case, punctuation, contractions and common
//! synonyms don't matter; a missing accent or a small typo still counts, with a note; but a
//! typo that makes a different real word is wrong.

use crate::marking;
use crate::model::{Course, Drill, Lesson, Sentence, Unit, Word};
use fancy_regex::Regex;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

// ---------------------------------------------------------------- text

pub fn hash(s: &str) -> String {
    crate::sections::hash(s)
}

/// The audio file for a piece of Afrikaans (scripts/tts/course-extract.js).
pub fn audio_id(text: &str) -> String {
    hash(&format!("af1|{}", text.trim()))
}

fn contractions() -> &'static [(Regex, &'static str)] {
    static C: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    C.get_or_init(|| {
        [
            ("i'm", "i am"), ("you're", "you are"), ("he's", "he is"), ("she's", "she is"), ("it's", "it is"), ("we're", "we are"),
            ("they're", "they are"), ("that's", "that is"), ("what's", "what is"), ("how's", "how is"), ("where's", "where is"),
            ("there's", "there is"), ("don't", "do not"), ("doesn't", "does not"), ("isn't", "is not"), ("aren't", "are not"),
            ("can't", "cannot"), ("i've", "i have"), ("we've", "we have"), ("they've", "they have"), ("you've", "you have"),
            ("it'll", "it will"), ("i'd", "i would"), ("haven't", "have not"),
        ]
        .iter()
        .map(|(w, r)| (Regex::new(&format!(r"(?<![A-Za-z0-9_]){}(?![A-Za-z0-9_])", fancy_regex::escape(w))).unwrap(), *r))
        .collect()
    })
}

/// Lowercase, no punctuation, single spaces; English also expands contractions and
/// applies the course's synonyms.
pub fn norm(s: &str, lang: &str, syn: Option<&IndexMap<String, String>>) -> String {
    let t: String = s.nfc().collect::<String>().to_lowercase();
    let mut t: String = t
        .chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{02BC}' | '`' | '\u{00B4}' => '\'',
            '\u{201C}' | '\u{201D}' | '"' => ' ',
            c => c,
        })
        .collect();
    if lang == "en" {
        for (re, r) in contractions() {
            t = re.replace_all(&t, *r).into_owned();
        }
    }
    // [^\p{L}\p{N}' ]+ -> ' '
    let mut u = String::with_capacity(t.len());
    let mut in_bad = false;
    for c in t.chars() {
        if is_letter(c) || is_number(c) || c == '\'' || c == ' ' {
            u.push(c);
            in_bad = false;
        } else if !in_bad {
            u.push(' ');
            in_bad = true;
        }
    }
    static LEAD: OnceLock<Regex> = OnceLock::new();
    static TRAIL: OnceLock<Regex> = OnceLock::new();
    let u = LEAD.get_or_init(|| Regex::new(r"(^|\s)'+").unwrap()).replace_all(&u, "$1").into_owned();
    let u = TRAIL.get_or_init(|| Regex::new(r"'+(?=\s|$)").unwrap()).replace_all(&u, "").into_owned();
    let u = marking::collapse_ws(&u).trim().to_string();
    match (lang, syn) {
        ("en", Some(syn)) if !syn.is_empty() => u.split(' ').map(|w| syn.get(w).map(String::as_str).unwrap_or(w)).collect::<Vec<_>>().join(" "),
        _ => u,
    }
}

fn is_letter(c: char) -> bool {
    c.is_alphabetic() && !c.is_numeric() && !is_combining_mark(c)
}
fn is_number(c: char) -> bool {
    c.is_numeric()
}

/// Without accents.
pub fn plain(t: &str) -> String {
    t.nfd().filter(|c| !is_combining_mark(*c)).nfc().collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct CheckResult {
    pub correct: bool,
    pub empty: bool,
    pub skipped: bool,
    /// The accepted answer it matched.
    pub matched: Option<String>,
    /// "accent" | "typo"
    pub note: Option<&'static str>,
}

impl CheckResult {
    pub fn wrong() -> Self {
        CheckResult { correct: false, empty: false, skipped: false, matched: None, note: None }
    }
    pub fn right() -> Self {
        CheckResult { correct: true, ..Self::wrong() }
    }
}

pub fn check(given: &str, accepted: &[String], lang: &str, syn: Option<&IndexMap<String, String>>, known: Option<&HashSet<String>>) -> CheckResult {
    let g = norm(given, lang, syn);
    if g.is_empty() {
        return CheckResult { empty: true, ..CheckResult::wrong() };
    }
    let acc: Vec<(String, String)> = accepted.iter().map(|raw| (raw.clone(), norm(raw, lang, syn))).collect();
    if let Some((raw, _)) = acc.iter().find(|(_, n)| *n == g) {
        return CheckResult { matched: Some(raw.clone()), ..CheckResult::right() };
    }
    let gp = plain(&g);
    if let Some((raw, _)) = acc.iter().find(|(_, n)| plain(n) == gp) {
        return CheckResult { matched: Some(raw.clone()), note: Some("accent"), ..CheckResult::right() };
    }
    let count = |t: &str| t.split(' ').count();
    if let Some((raw, _)) = acc.iter().find(|(_, n)| count(n) == count(&g) && marking::typo_of(&gp, &plain(n), known)) {
        return CheckResult { matched: Some(raw.clone()), note: Some("typo"), ..CheckResult::right() };
    }
    CheckResult::wrong()
}

/// Word tiles for a sentence: its words without the punctuation around them.
pub fn tiles_of(sentence: &str) -> Vec<String> {
    let keep = |c: char| is_letter(c) || is_number(c) || c == '\'';
    sentence
        .split_whitespace()
        .map(|w| w.trim_start_matches(|c: char| !keep(c)).trim_end_matches(|c: char| !keep(c)).to_string())
        .filter(|w| !w.is_empty())
        .collect()
}

// ---------------------------------------------------------------- the course model

#[derive(Clone)]
pub struct At {
    pub unit_index: usize,
    pub index: usize,
    pub index_in_unit: usize,
    pub lesson: Lesson,
}

pub struct Model {
    pub lessons: Vec<At>,
    pub meanings_of: HashMap<String, HashSet<String>>,
    pub known: HashSet<String>,
    pub syn: IndexMap<String, String>,
}

impl Model {
    pub fn new(course: &Course) -> Model {
        let syn = course.english_synonyms.clone();
        let mut lessons = Vec::new();
        let mut meanings_of: HashMap<String, HashSet<String>> = HashMap::new();
        for (ui, u) in course.units.iter().enumerate() {
            for (li, l) in u.lessons.iter().enumerate() {
                lessons.push(At { unit_index: ui, index: lessons.len(), index_in_unit: li, lesson: l.clone() });
                for w in &l.words {
                    let key = norm(&w.af, "af", None);
                    let set = meanings_of.entry(key).or_default();
                    for e in &w.en {
                        set.insert(norm(e, "en", Some(&syn)));
                    }
                }
            }
        }
        let mut known = HashSet::new();
        let mut add_all = |s: &str, lang: &str| {
            for w in norm(s, lang, Some(&syn)).split(' ') {
                if !w.is_empty() {
                    known.insert(plain(w));
                }
            }
        };
        for at in &lessons {
            let l = &at.lesson;
            for w in &l.words {
                add_all(&w.af, "af");
                w.en.iter().for_each(|e| add_all(e, "en"));
            }
            for s in &l.sentences {
                s.af.iter().for_each(|a| add_all(a, "af"));
                s.en.iter().for_each(|e| add_all(e, "en"));
            }
            for d in &l.drills {
                add_all(&d.from, "af");
                d.to.iter().for_each(|a| add_all(a, "af"));
            }
        }
        Model { lessons, meanings_of, known, syn }
    }

    pub fn unit<'a>(&self, course: &'a Course, at: &At) -> &'a Unit {
        &course.units[at.unit_index]
    }
}

// ---------------------------------------------------------------- generation

/// mulberry32, as in the JavaScript.
pub struct Rng(u32);
impl Rng {
    pub fn new(seed: &str) -> Rng {
        let a = u32::from_str_radix(&hash(seed), 16).unwrap_or(0);
        Rng(if a == 0 { 1 } else { a })
    }
    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6D2B79F5);
        let a = self.0;
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = (t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t))) ^ t;
        ((t ^ (t >> 14)) as f64) / 4294967296.0
    }
}

pub fn shuffle<T: Clone>(arr: &[T], rand: &mut Rng) -> Vec<T> {
    let mut a = arr.to_vec();
    for i in (1..a.len()).rev() {
        let j = (rand.next() * (i as f64 + 1.0)).floor() as usize;
        a.swap(i, j);
    }
    a
}

fn take_distinct<T: Clone>(pool: &[T], n: usize, avoid: &[T], rand: &mut Rng, key: &dyn Fn(&T) -> String) -> Vec<T> {
    let mut seen: HashSet<String> = avoid.iter().map(key).collect();
    let mut out = Vec::new();
    for x in shuffle(pool, rand) {
        let k = key(&x);
        if seen.contains(&k) {
            continue;
        }
        seen.insert(k);
        out.push(x);
        if out.len() >= n {
            break;
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct PairItem {
    pub af: String,
    pub en: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Exercise {
    #[serde(rename_all = "camelCase")]
    Meaning { is_new: bool, af: String, options: Vec<String>, answer: i64, audio: String },
    #[serde(rename_all = "camelCase")]
    Pairs { pairs: Vec<PairItem>, left: Vec<usize>, right: Vec<usize> },
    #[serde(rename_all = "camelCase")]
    Build { dir: String, prompt: String, accepted: Vec<String>, answer: String, tiles: Vec<String>, audio: Option<String>, answer_audio: Option<String> },
    #[serde(rename_all = "camelCase")]
    Type { dir: String, prompt: String, accepted: Vec<String>, answer: String, audio: Option<String>, answer_audio: Option<String> },
    #[serde(rename_all = "camelCase")]
    Listen { audio: String, accepted: Vec<String>, answer: String, tiles: Vec<String>, en: String },
    #[serde(rename_all = "camelCase")]
    Blank { sentence: String, options: Vec<String>, answer: i64, en: String, full: String, audio: String },
    #[serde(rename_all = "camelCase")]
    Transform { input: String, task: String, en: String, prompt: String, accepted: Vec<String>, answer: String, tiles: Vec<String>, audio: String, answer_audio: String },
    #[serde(rename_all = "camelCase")]
    Pick { prompt: String, options: Vec<String>, answer: i64, audio: String },
}

impl Exercise {
    pub fn kind(&self) -> &'static str {
        match self {
            Exercise::Meaning { .. } => "meaning",
            Exercise::Pairs { .. } => "pairs",
            Exercise::Build { .. } => "build",
            Exercise::Type { .. } => "type",
            Exercise::Listen { .. } => "listen",
            Exercise::Blank { .. } => "blank",
            Exercise::Transform { .. } => "transform",
            Exercise::Pick { .. } => "pick",
        }
    }
}

struct Pool {
    words: Vec<Word>,
    sentences: Vec<(usize, Sentence)>,
}

fn pool_up_to(m: &Model, index: usize) -> Pool {
    let ls: Vec<&Lesson> = m.lessons.iter().take(index + 1).map(|x| &x.lesson).collect();
    let mut sentences = Vec::new();
    for (li, l) in ls.iter().enumerate() {
        for (si, s) in l.sentences.iter().enumerate() {
            sentences.push((li * 1000 + si, s.clone()));
        }
    }
    Pool { words: ls.iter().flat_map(|l| l.words.iter().cloned()).collect(), sentences }
}

fn index_of(options: &[String], v: &str) -> i64 {
    options.iter().position(|o| o == v).map_or(-1, |i| i as i64)
}

fn meaning_exercise(m: &Model, w: &Word, rand: &mut Rng, is_new: bool, pool: &Pool) -> Exercise {
    let empty = HashSet::new();
    let own = m.meanings_of.get(&norm(&w.af, "af", None)).unwrap_or(&empty);
    let en0 = |x: &Word| x.en.first().cloned().unwrap_or_default();
    let candidates: Vec<Word> = pool.words.iter().filter(|x| !own.contains(&norm(&en0(x), "en", Some(&m.syn)))).cloned().collect();
    let others = take_distinct(&candidates, 3, std::slice::from_ref(w), rand, &|x: &Word| norm(&en0(x), "en", Some(&m.syn)));
    let mut opts = vec![en0(w)];
    opts.extend(others.iter().map(en0));
    let options = shuffle(&opts, rand);
    let answer = index_of(&options, &en0(w));
    Exercise::Meaning { is_new, af: w.af.clone(), options, answer, audio: audio_id(&w.af) }
}

fn pairs_exercise(m: &Model, words: &[Word], rand: &mut Rng) -> Exercise {
    let pairs = take_distinct(words, 5, &[], rand, &|x: &Word| norm(&x.af, "af", None));
    let mut used = HashSet::new();
    let ok: Vec<Word> = pairs
        .into_iter()
        .filter(|p| used.insert(norm(p.en.first().map(String::as_str).unwrap_or(""), "en", Some(&m.syn))))
        .collect();
    let idx: Vec<usize> = (0..ok.len()).collect();
    let left = shuffle(&idx, rand);
    let right = shuffle(&idx, rand);
    Exercise::Pairs { pairs: ok.iter().map(|p| PairItem { af: p.af.clone(), en: p.en.first().cloned().unwrap_or_default() }).collect(), left, right }
}

fn side<'a>(s: &'a Sentence, lang: &str) -> &'a Vec<String> {
    if lang == "af" { &s.af } else { &s.en }
}
fn first(v: &[String]) -> String {
    v.first().cloned().unwrap_or_default()
}

fn build_exercise(s: &(usize, Sentence), dir: &str, rand: &mut Rng, pool: &Pool) -> Exercise {
    let (from, to) = if dir == "af-en" { ("af", "en") } else { ("en", "af") };
    let sen = &s.1;
    let answer = first(side(sen, to));
    let tiles = tiles_of(&answer);
    let lower: HashSet<String> = tiles.iter().map(|t| t.to_lowercase()).collect();
    let extra_pool: Vec<String> = pool
        .sentences
        .iter()
        .filter(|x| x.0 != s.0)
        .flat_map(|x| tiles_of(&first(side(&x.1, to))))
        .filter(|t| !lower.contains(&t.to_lowercase()))
        .collect();
    let n = 4.min(2.max(tiles.len().div_ceil(2)));
    let extra = take_distinct(&extra_pool, n, &[], rand, &|t: &String| t.to_lowercase());
    let mut all = tiles.clone();
    all.extend(extra);
    let af0 = first(&sen.af);
    Exercise::Build {
        dir: dir.into(),
        prompt: first(side(sen, from)),
        accepted: side(sen, to).clone(),
        answer,
        tiles: shuffle(&all, rand),
        audio: (from == "af").then(|| audio_id(&af0)),
        answer_audio: (to == "af").then(|| audio_id(&af0)),
    }
}

fn type_exercise(s: &Sentence, dir: &str) -> Exercise {
    let (from, to) = if dir == "af-en" { ("af", "en") } else { ("en", "af") };
    let af0 = first(&s.af);
    Exercise::Type {
        dir: dir.into(),
        prompt: first(side(s, from)),
        accepted: side(s, to).clone(),
        answer: first(side(s, to)),
        audio: (from == "af").then(|| audio_id(&af0)),
        answer_audio: (to == "af").then(|| audio_id(&af0)),
    }
}

fn listen_exercise(s: &(usize, Sentence), rand: &mut Rng, pool: &Pool) -> Exercise {
    let Exercise::Build { tiles, .. } = build_exercise(s, "en-af", rand, pool) else { unreachable!() };
    Exercise::Listen { audio: audio_id(&first(&s.1.af)), accepted: s.1.af.clone(), answer: first(&s.1.af), tiles, en: first(&s.1.en) }
}

fn upper_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Blank out one vocabulary word of the sentence (preferring this lesson's words).
fn blank_exercise(s: &Sentence, lesson_words: &[Word], rand: &mut Rng, pool: &Pool) -> Option<Exercise> {
    let af0 = first(&s.af);
    let tiles = tiles_of(&af0);
    if tiles.len() < 2 {
        return None;
    }
    let mut vocab: IndexMap<String, Word> = IndexMap::new();
    for w in pool.words.iter().filter(|w| !w.af.chars().any(char::is_whitespace)) {
        vocab.insert(norm(&w.af, "af", None), w.clone());
    }
    let lesson_set: HashSet<String> = lesson_words.iter().map(|w| norm(&w.af, "af", None)).collect();
    let candidates: Vec<(String, usize, String)> = tiles.iter().enumerate().map(|(i, t)| (t.clone(), i, norm(t, "af", None))).filter(|x| vocab.contains_key(&x.2)).collect();
    if candidates.is_empty() {
        return None;
    }
    let preferred: Vec<(String, usize, String)> = candidates.iter().filter(|x| lesson_set.contains(&x.2)).cloned().collect();
    let pick = shuffle(if preferred.is_empty() { &candidates } else { &preferred }, rand)[0].clone();
    let others: Vec<Word> = vocab.values().filter(|w| !tiles.iter().any(|t| norm(t, "af", None) == norm(&w.af, "af", None))).cloned().collect();
    let wrong = take_distinct(&others, 2, &[], rand, &|w: &Word| norm(&w.af, "af", None));
    if wrong.len() < 2 {
        return None;
    }
    let mut opts = vec![pick.0.clone()];
    opts.extend(wrong.iter().map(|w| if pick.1 == 0 { upper_first(&w.af) } else { w.af.clone() }));
    let options = shuffle(&opts, rand);
    let words: Vec<&str> = af0.split_whitespace().collect();
    let shown = words.iter().enumerate().map(|(i, w)| if i == pick.1 { w.replacen(&pick.0, "\u{2009}___\u{2009}", 1) } else { w.to_string() }).collect::<Vec<_>>().join(" ");
    let answer = index_of(&options, &pick.0);
    Some(Exercise::Blank { sentence: shown, options, answer, en: first(&s.en), full: af0.clone(), audio: audio_id(&af0) })
}

fn transform_exercise(d: &Drill, input: &str, rand: &mut Rng, pool: &Pool) -> Exercise {
    let answer = first(&d.to);
    let tiles = tiles_of(&answer);
    let lower: HashSet<String> = tiles.iter().map(|t| t.to_lowercase()).collect();
    let mut extra_pool: Vec<String> = tiles_of(&d.from).into_iter().filter(|t| !lower.contains(&t.to_lowercase())).collect();
    extra_pool.extend(pool.sentences.iter().flat_map(|x| tiles_of(&first(&x.1.af))).filter(|t| !lower.contains(&t.to_lowercase())));
    let n = 4.min(2.max(tiles.len().div_ceil(3)));
    let extra = take_distinct(&extra_pool, n, &[], rand, &|t: &String| t.to_lowercase());
    let mut all = tiles.clone();
    all.extend(extra);
    Exercise::Transform {
        input: input.into(),
        task: d.task.clone(),
        en: d.en.clone().unwrap_or_default(),
        prompt: d.from.clone(),
        accepted: d.to.clone(),
        answer: answer.clone(),
        tiles: shuffle(&all, rand),
        audio: audio_id(&d.from),
        answer_audio: audio_id(&answer),
    }
}

fn pick_exercise(m: &Model, s: &(usize, Sentence), rand: &mut Rng, pool: &Pool) -> Exercise {
    let others_pool: Vec<(usize, Sentence)> = pool.sentences.iter().filter(|x| x.0 != s.0).cloned().collect();
    let others = take_distinct(&others_pool, 2, &[], rand, &|x: &(usize, Sentence)| norm(&first(&x.1.en), "en", Some(&m.syn)));
    let mut opts = vec![first(&s.1.en)];
    opts.extend(others.iter().map(|x| first(&x.1.en)));
    let options = shuffle(&opts, rand);
    let answer = index_of(&options, &first(&s.1.en));
    Exercise::Pick { prompt: first(&s.1.af), options, answer, audio: audio_id(&first(&s.1.af)) }
}

/// The exercises for one lesson.
pub fn lesson_exercises(m: &Model, lesson_id: &str, attempt: usize, seen: &HashSet<String>, has_audio: &dyn Fn(&str) -> bool) -> Vec<Exercise> {
    let Some(at) = m.lessons.iter().find(|x| x.lesson.id == lesson_id) else { return vec![] };
    let l = &at.lesson;
    let mut rand = Rng::new(&format!("{}#{attempt}", l.id));
    let pool = pool_up_to(m, at.index);
    let fresh: Vec<Word> = l.words.iter().filter(|w| !seen.contains(&norm(&w.af, "af", None))).cloned().collect();
    let intro_words: Vec<Word> = if !fresh.is_empty() { fresh.clone() } else { shuffle(&l.words, &mut rand).into_iter().take(2).collect() };
    let intro: Vec<Exercise> = intro_words
        .iter()
        .take(6)
        .map(|w| {
            let is_new = fresh.iter().any(|f| f.af == w.af && f.en == w.en);
            meaning_exercise(m, w, &mut rand, is_new, &pool)
        })
        .collect();
    // The lesson's own sentences, with their identity in the pool.
    let own: Vec<(usize, Sentence)> = l.sentences.iter().enumerate().map(|(si, s)| (at.index * 1000 + si, s.clone())).collect();
    let sentences = shuffle(&own, &mut rand);
    let mut easy = Vec::new();
    let mut hard = Vec::new();
    for (i, s) in sentences.iter().enumerate() {
        let long = tiles_of(&first(&s.1.af)).len() > 1 && tiles_of(&first(&s.1.en)).len() > 1;
        let audio = has_audio(&audio_id(&first(&s.1.af)));
        let first_kinds: Vec<&str> = if long { vec!["build-af-en", if audio { "listen" } else { "pick" }, "pick"] } else { vec!["pick"] };
        let k1 = first_kinds[(i + attempt) % first_kinds.len()];
        easy.push(match k1 {
            "build-af-en" => build_exercise(s, "af-en", &mut rand, &pool),
            "listen" => listen_exercise(s, &mut rand, &pool),
            _ => pick_exercise(m, s, &mut rand, &pool),
        });
        let second_kinds: Vec<&str> = if long { vec!["build-en-af", "blank", "type-en-af", "type-af-en"] } else { vec!["type-af-en", "blank"] };
        let k2 = second_kinds[(i + attempt) % second_kinds.len()];
        let ex2 = match k2 {
            "build-en-af" => Some(build_exercise(s, "en-af", &mut rand, &pool)),
            "blank" => blank_exercise(&s.1, &l.words, &mut rand, &pool),
            "type-en-af" => Some(type_exercise(&s.1, "en-af")),
            _ => Some(type_exercise(&s.1, "af-en")),
        };
        hard.push(ex2.unwrap_or_else(|| type_exercise(&s.1, "af-en")));
    }
    let drills = shuffle(&l.drills, &mut rand);
    for (i, d) in drills.iter().enumerate() {
        hard.push(transform_exercise(d, if (i + attempt) % 2 == 1 { "type" } else { "tiles" }, &mut rand, &pool));
    }
    let pairs_words: Vec<Word> = if l.words.len() >= 4 {
        l.words.clone()
    } else {
        let mut v = l.words.clone();
        v.extend(pool.words.iter().filter(|w| !l.words.iter().any(|x| x.af == w.af && x.en == w.en)).cloned());
        v.truncate(5);
        v
    };
    let mut first_part = Vec::new();
    let mut easy_q: std::collections::VecDeque<Exercise> = easy.into();
    for (i, x) in intro.into_iter().enumerate() {
        first_part.push(x);
        if i % 2 == 1 {
            if let Some(e) = easy_q.pop_front() {
                first_part.push(e);
            }
        }
    }
    let mut out = first_part;
    for _ in 0..2 {
        if let Some(e) = easy_q.pop_front() {
            out.push(e);
        }
    }
    out.push(pairs_exercise(m, &pairs_words, &mut rand));
    out.extend(easy_q);
    out.extend(hard);
    out.truncate(19);
    out
}

/// A practice session mixing what has been learnt so far.
pub fn practice_exercises(m: &Model, done_ids: &[String], attempt: usize, has_audio: &dyn Fn(&str) -> bool) -> Vec<Exercise> {
    let done: Vec<&At> = m.lessons.iter().filter(|x| done_ids.contains(&x.lesson.id)).collect();
    let Some(last) = done.last() else { return vec![] };
    let mut rand = Rng::new(&format!("practice#{attempt}"));
    let pool = pool_up_to(m, last.index);
    let words: Vec<Word> = done.iter().flat_map(|x| x.lesson.words.iter().cloned()).collect();
    let all_sent: Vec<(usize, Sentence)> = done.iter().flat_map(|x| x.lesson.sentences.iter().enumerate().map(move |(si, s)| (x.index * 1000 + si, s.clone()))).collect();
    let sentences: Vec<(usize, Sentence)> = shuffle(&all_sent, &mut rand).into_iter().take(8).collect();
    let shuffled_words = shuffle(&words, &mut rand);
    let mut out = vec![pairs_exercise(m, &shuffled_words, &mut rand)];
    for (i, s) in sentences.iter().enumerate() {
        let long = tiles_of(&first(&s.1.af)).len() > 1;
        let kinds: Vec<&str> = if long {
            vec!["build-en-af", "type-af-en", if has_audio(&audio_id(&first(&s.1.af))) { "listen" } else { "build-af-en" }, "blank", "type-en-af"]
        } else {
            vec!["pick", "type-af-en"]
        };
        let k = kinds[(i + attempt) % kinds.len()];
        let ex = match k {
            "build-en-af" => Some(build_exercise(s, "en-af", &mut rand, &pool)),
            "build-af-en" => Some(build_exercise(s, "af-en", &mut rand, &pool)),
            "listen" => Some(listen_exercise(s, &mut rand, &pool)),
            "blank" => blank_exercise(&s.1, &words, &mut rand, &pool),
            "pick" => Some(pick_exercise(m, s, &mut rand, &pool)),
            "type-en-af" => Some(type_exercise(&s.1, "en-af")),
            _ => Some(type_exercise(&s.1, "af-en")),
        };
        out.push(ex.unwrap_or_else(|| type_exercise(&s.1, "af-en")));
    }
    for w in shuffle(&words, &mut rand).into_iter().take(2) {
        let pos = 1 + (rand.next() * out.len() as f64).floor() as usize;
        let ex = meaning_exercise(m, &w, &mut rand, false, &pool);
        out.insert(pos.min(out.len()), ex);
    }
    let drills: Vec<Drill> = done.iter().flat_map(|x| x.lesson.drills.iter().cloned()).collect();
    for (i, d) in shuffle(&drills, &mut rand).into_iter().take(3).enumerate() {
        out.push(transform_exercise(&d, if (i + attempt) % 2 == 1 { "type" } else { "tiles" }, &mut rand, &pool));
    }
    out
}

/// What the learner gave: an option index, or text / joined tiles.
#[derive(Clone, Debug, PartialEq)]
pub enum Response {
    None,
    Option(i64),
    Text(String),
}

pub fn check_exercise(m: &Model, ex: &Exercise, response: &Response) -> CheckResult {
    let text = match response {
        Response::Text(t) => t.as_str(),
        _ => "",
    };
    let opt = match response {
        Response::Option(i) => Some(*i),
        _ => None,
    };
    match ex {
        Exercise::Meaning { answer, .. } | Exercise::Blank { answer, .. } | Exercise::Pick { answer, .. } => {
            if opt == Some(*answer) { CheckResult::right() } else { CheckResult::wrong() }
        }
        Exercise::Build { accepted, dir, .. } | Exercise::Type { accepted, dir, .. } => check(text, accepted, if dir == "af-en" { "en" } else { "af" }, Some(&m.syn), Some(&m.known)),
        Exercise::Listen { accepted, .. } | Exercise::Transform { accepted, .. } => check(text, accepted, "af", Some(&m.syn), Some(&m.known)),
        Exercise::Pairs { .. } => CheckResult::right(),
    }
}

/// XP for finishing a lesson.
pub fn xp_for(practice: bool, mistakes: usize) -> u32 {
    (if practice { 5 } else { 10 }) + if mistakes == 0 { 5 } else { 0 }
}
