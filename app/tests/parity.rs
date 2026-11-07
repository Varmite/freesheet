//! Compares the Rust logic with what FreeSheet 1.1's JavaScript produced for all content.
//! Needs a fixture made by the JavaScript (scripts/parity-fixture.js in the 1.1 sources);
//! skipped without one:
//!   FS_PARITY=<fixture.json> cargo test --test parity -- --nocapture

use freesheet::{content, course, hints, markup, marking, quiz, sections};
use serde_json::{json, Value};
use std::collections::HashSet;

struct Report {
    name: &'static str,
    total: usize,
    bad: usize,
    shown: usize,
}

impl Report {
    fn new(name: &'static str) -> Self {
        Report { name, total: 0, bad: 0, shown: 0 }
    }
    fn check(&mut self, ok: bool, what: impl FnOnce() -> String) {
        self.total += 1;
        if !ok {
            self.bad += 1;
            if self.shown < 4 {
                self.shown += 1;
                println!("  [{}] {}", self.name, what());
            }
        }
    }
    fn done(&self) -> bool {
        println!("{:<12} {} of {} differ", self.name, self.bad, self.total);
        self.bad == 0
    }
}

/// Both JSON texts around the first place they differ.
fn clip(v: &Value, other: &Value) -> String {
    let (x, y): (Vec<char>, Vec<char>) = (v.to_string().chars().collect(), other.to_string().chars().collect());
    let i = x.iter().zip(&y).position(|(p, q)| p != q).unwrap_or(x.len().min(y.len()));
    let from = i.saturating_sub(150);
    let show = |s: &Vec<char>| s[from.min(s.len())..(i + 150).min(s.len())].iter().collect::<String>();
    format!("at {i}\n      rs ...{}\n      js ...{}", show(&x), show(&y))
}

/// The two JSON texts from just before where they first differ.
fn diff(a: &Value, b: &Value) -> String {
    let (x, y): (Vec<char>, Vec<char>) = (a.to_string().chars().collect(), b.to_string().chars().collect());
    let i = x.iter().zip(&y).position(|(p, q)| p != q).unwrap_or(x.len().min(y.len()));
    let from = i.saturating_sub(120);
    let show = |v: &Vec<char>| v[from.min(v.len())..(i + 120).min(v.len())].iter().collect::<String>();
    format!("at {i}:
      rs ...{}
      js ...{}", show(&x), show(&y))
}

#[test]
fn matches_javascript() {
    let Ok(path) = std::env::var("FS_PARITY") else {
        eprintln!("FS_PARITY not set: skipped");
        return;
    };
    let fx: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut sec = Report::new("sections");
    let mut blk = Report::new("blocks");
    let mut hin = Report::new("hints");
    let mut rel = Report::new("related");
    let mut cor = Report::new("correct");
    let mut mk = Report::new("markup");
    let mut sh = Report::new("shuffle");
    let mut nrm = Report::new("normText");
    for t in fx["topics"].as_array().unwrap() {
        let key = t["key"].as_str().unwrap();
        let parts: Vec<&str> = key.split('/').collect();
        let grade: i64 = parts[0].trim_start_matches("grade-").parse().unwrap();
        let topic = content::topic(grade, parts[1], parts[2]).unwrap();
        let ss = sections::split(&topic.notes);
        let js = t["sections"].as_array().unwrap();
        sec.check(ss.len() == js.len(), || format!("{key}: {} vs {} sections", ss.len(), js.len()));
        for (a, b) in ss.iter().zip(js) {
            sec.check(a.text == b["text"].as_str().unwrap() && a.id == b["id"].as_str().unwrap(), || format!("{key}:\n    rs {:?}\n    js {:?}", a.text, b["text"]));
        }
        for (b, h) in topic.notes.iter().zip(t["blocks"].as_array().unwrap()) {
            let r = markup::block_html(b);
            blk.check(r == h.as_str().unwrap(), || format!("{key}:\n    rs {r}\n    js {}", h.as_str().unwrap()));
        }
        let idx = hints::NotesIndex::new(&topic);
        for (q, jq) in topic.questions.iter().zip(t["questions"].as_array().unwrap()) {
            let id = format!("{key}#{}", q.id);
            let hs: Vec<Value> = hints::build(q, &idx)
                .iter()
                .map(|h| json!({"kind": h.kind, "section": h.section, "eliminate": h.eliminate, "pair": h.pair, "html": h.html}))
                .collect();
            let hs = Value::Array(hs);
            hin.check(hs == jq["hints"], || format!("{id} {}", clip(&hs, &jq["hints"])));
            let r = hints::related_section(q, &idx);
            rel.check(json!(r) == jq["related"], || format!("{id}: {r:?} vs {}", jq["related"]));
            let c = quiz::correct_answer_html(q);
            cor.check(c == jq["correct"].as_str().unwrap(), || format!("{id}: {c} vs {}", jq["correct"]));
            let p = markup::paragraphs(&q.prompt);
            mk.check(p == jq["prompt"].as_str().unwrap(), || format!("{id}:\n    rs {p}\n    js {}", jq["prompt"]));
            if let Some(e) = &q.explanation {
                let p = markup::paragraphs(e);
                mk.check(p == jq["explanation"].as_str().unwrap_or(""), || format!("{id}: expl {p}"));
            }
            for (o, jo) in q.options.iter().zip(jq["options"].as_array().unwrap()) {
                let p = markup::inline(o);
                mk.check(p == jo.as_str().unwrap(), || format!("{id}: {p} vs {jo}"));
            }
            if q.kind == "match" {
                let rights: Vec<String> = q.pairs.iter().map(|p| p.right.clone()).collect();
                let s = quiz::seeded_shuffle(&rights, &q.id);
                sh.check(json!(s) == jq["rights"], || id.clone());
            }
            for (a, n) in q.answers.iter().zip(jq["norms"].as_array().unwrap()) {
                let r = marking::norm_text(a);
                nrm.check(r == n.as_str().unwrap(), || format!("{a:?}: {r:?} vs {n}"));
            }
        }
    }
    let mut num = Report::new("numbers");
    for n in fx["numbers"].as_array().unwrap() {
        let v = marking::parse_number(n["input"].as_str().unwrap());
        let ok = match n["value"].as_f64() {
            Some(x) => (v - x).abs() < 1e-12,
            None => v.is_nan(),
        };
        num.check(ok, || format!("{:?}: {v} vs {}", n["input"], n["value"]));
    }
    for n in fx["norms"].as_array().unwrap() {
        let r = marking::norm_text(n["input"].as_str().unwrap());
        nrm.check(r == n["out"].as_str().unwrap(), || format!("{:?}: {r:?} vs {}", n["input"], n["out"]));
    }
    let mut typo = Report::new("typos");
    for t in fx["typos"].as_array().unwrap() {
        let r = marking::typo_of(&marking::norm_text(t["g"].as_str().unwrap()), &marking::norm_text(t["a"].as_str().unwrap()), None);
        typo.check(json!(r) == t["ok"], || format!("{t}"));
    }
    let mut les = Report::new("lessons");
    let mut pra = Report::new("practice");
    let mut chk = Report::new("checks");
    let mut til = Report::new("tiles");
    for c in fx["courses"].as_array().unwrap() {
        let id = c["id"].as_str().unwrap();
        let co = content::course(id).unwrap();
        let m = course::Model::new(&co);
        let no_audio = |_: &str| false;
        let mut seen = HashSet::new();
        let mut i = 0;
        let ls = c["lessons"].as_array().unwrap();
        for at in &m.lessons {
            for attempt in 0..3 {
                for fresh in [true, false] {
                    let j = &ls[i];
                    i += 1;
                    let s = if fresh { HashSet::new() } else { seen.clone() };
                    let ex = serde_json::to_value(course::lesson_exercises(&m, &at.lesson.id, attempt, &s, &no_audio)).unwrap();
                    les.check(ex == j["exercises"], || {
                        format!("{id}/{} attempt {attempt} fresh {fresh} {}", at.lesson.id, clip(&ex, &j["exercises"]))
                    });
                }
            }
            for w in &at.lesson.words {
                seen.insert(course::norm(&w.af, "af", None));
            }
        }
        let done: Vec<String> = m.lessons.iter().map(|x| x.lesson.id.clone()).collect();
        for (attempt, j) in c["practice"].as_array().unwrap().iter().enumerate() {
            let n = (4 + attempt * 3).min(done.len());
            let ex = serde_json::to_value(course::practice_exercises(&m, &done[..n], attempt, &no_audio)).unwrap();
            pra.check(&ex == j, || format!("{id} practice {attempt} {}", clip(&ex, j)));
        }
        for k in c["checks"].as_array().unwrap() {
            let acc: Vec<String> = serde_json::from_value(k["accepted"].clone()).unwrap();
            let r = course::check(k["given"].as_str().unwrap(), &acc, k["lang"].as_str().unwrap(), Some(&m.syn), Some(&m.known));
            let jr = &k["r"];
            let ok = json!(r.correct) == jr["correct"]
                && json!(r.matched) == jr.get("match").cloned().unwrap_or(Value::Null)
                && json!(r.note) == jr.get("note").cloned().unwrap_or(Value::Null);
            chk.check(ok, || format!("{id} {:?}: {r:?} vs {jr}", k["given"]));
        }
        for t in c["tiles"].as_array().unwrap() {
            let r = course::tiles_of(t["s"].as_str().unwrap());
            til.check(json!(r) == t["t"], || format!("{t}: {r:?}"));
        }
    }
    let all = [&sec, &blk, &hin, &rel, &cor, &mk, &sh, &nrm, &num, &typo, &les, &pra, &chk, &til].iter().map(|r| r.done()).fold(true, |a, b| a & b);
    assert!(all, "the Rust logic differs from the JavaScript");
}
