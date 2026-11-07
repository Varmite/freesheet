//! Notes, practice questions, exams and language courses.
//!
//! The whole content/ folder (a few MB of JSON) is built into the program, so every
//! platform, Android included, reads it the same way and nothing can go missing.
//! Layout (see docs/CONTENT_FORMAT.md):
//!   content/grade-<n>/<subject>/subject.json
//!   content/grade-<n>/<subject>/topics/<topic>.json
//!   content/grade-<n>/<subject>/exams/<exam>.json
//!   content/courses/<course>/course.json

use include_dir::{include_dir, Dir, DirEntry};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

static CONTENT: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../content");

/// Ids used in file names: lower-case words joined by single hyphens.
pub fn valid_id(id: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$").unwrap()).is_match(id)
}

pub fn check_id(id: &str, what: &str) -> Result<(), String> {
    if valid_id(id) { Ok(()) } else { Err(format!("Invalid {what}")) }
}

pub fn check_grade(grade: i64) -> Result<i64, String> {
    if (1..=12).contains(&grade) { Ok(grade) } else { Err("Invalid grade".into()) }
}

fn parse(text: &str) -> Result<Value, String> {
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())
}

fn read_json(path: &str) -> Result<Value, String> {
    let file = CONTENT.get_file(path).ok_or_else(|| format!("Not found: {path}"))?;
    let text = file.contents_utf8().ok_or_else(|| format!("Not text: {path}"))?;
    parse(text)
}

fn s(v: &Value, key: &str) -> Value {
    v.get(key).cloned().unwrap_or(Value::Null)
}

fn len(v: &Value) -> usize {
    v.as_array().map_or(0, |a| a.len())
}

fn subject_summary(s_: &Value) -> Value {
    let list = |key: &str| s_.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
    json!({
        "id": s(s_, "id"),
        "name": s(s_, "name"),
        "description": s_.get("description").and_then(Value::as_str).unwrap_or(""),
        "topics": list("topics").iter().map(|t| json!({ "id": s(t, "id"), "title": s(t, "title"), "term": s(t, "term") })).collect::<Vec<_>>(),
        "exams": list("exams").iter().map(|e| json!({
            "id": s(e, "id"), "title": s(e, "title"),
            "durationMinutes": s(e, "durationMinutes"), "totalMarks": s(e, "totalMarks"),
        })).collect::<Vec<_>>(),
    })
}

fn build_catalog() -> Value {
    let mut grades: Vec<(i64, Value)> = Vec::new();
    for dir in CONTENT.dirs() {
        let name = dir.path().file_name().and_then(|n| n.to_str()).unwrap_or("");
        let Some(grade) = name.strip_prefix("grade-").and_then(|n| n.parse::<i64>().ok()) else { continue };
        let mut subjects: Vec<Value> = Vec::new();
        for sub in dir.dirs() {
            let file = sub.path().join("subject.json");
            let Some(path) = file.to_str().map(|p| p.replace('\\', "/")) else { continue };
            if CONTENT.get_file(&path).is_none() { continue; }
            match read_json(&path) {
                Ok(v) => subjects.push(subject_summary(&v)),
                Err(e) => eprintln!("Could not read {path}: {e}"),
            }
        }
        let key = |v: &Value| v["name"].as_str().unwrap_or("").to_lowercase();
        subjects.sort_by(|a, b| key(a).cmp(&key(b)));
        grades.push((grade, json!({ "grade": grade, "subjects": subjects })));
    }
    grades.sort_by_key(|(g, _)| *g);
    json!({ "grades": grades.into_iter().map(|(_, v)| v).collect::<Vec<_>>(), "courses": list_courses() })
}

/// Language courses are shown as a subject in every grade.
fn list_courses() -> Vec<Value> {
    let mut out = Vec::new();
    let Some(courses) = CONTENT.get_dir("courses") else { return out };
    let mut dirs: Vec<&Dir> = courses.dirs().collect();
    dirs.sort_by_key(|d| d.path().to_path_buf());
    for d in dirs {
        let id = d.path().file_name().and_then(|n| n.to_str()).unwrap_or("");
        let path = format!("courses/{id}/course.json");
        if !valid_id(id) || CONTENT.get_file(&path).is_none() { continue; }
        match read_json(&path) {
            Ok(c) => {
                let units = c.get("units").and_then(Value::as_array).cloned().unwrap_or_default();
                let lessons: usize = units.iter().map(|u| len(&u["lessons"])).sum();
                out.push(json!({
                    "id": id, "name": s(&c, "name"), "about": s(&c, "about"),
                    "grade": s(&c, "grade"), "level": s(&c, "level"),
                    "units": units.len(), "lessons": lessons,
                }));
            }
            Err(e) => eprintln!("Bad course file {path}: {e}"),
        }
    }
    out
}

pub fn catalog() -> &'static Value {
    static CATALOG: OnceLock<Value> = OnceLock::new();
    CATALOG.get_or_init(build_catalog)
}

/// Every word that appears in the content, for telling spelling slips from real
/// different answers when marking (see typoOf in src/js/marking.js).
pub fn words() -> &'static Vec<String> {
    static WORDS: OnceLock<Vec<String>> = OnceLock::new();
    WORDS.get_or_init(|| {
        let re = Regex::new(r"\p{L}[\p{L}']*").unwrap();
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        let mut add = |text: &str| {
            let norm: String = text.nfkc().collect::<String>().to_lowercase();
            for m in re.find_iter(&norm) {
                let w = m.as_str().trim_end_matches('\'');
                if seen.insert(w.to_string()) { out.push(w.to_string()); }
            }
        };
        fn walk(v: &Value, add: &mut dyn FnMut(&str)) {
            match v {
                Value::String(t) => add(t),
                Value::Array(a) => a.iter().for_each(|x| walk(x, add)),
                Value::Object(o) => o.values().for_each(|x| walk(x, add)),
                _ => {}
            }
        }
        fn files<'a>(d: &'a Dir<'a>, out: &mut Vec<&'a include_dir::File<'a>>) {
            for e in d.entries() {
                match e {
                    DirEntry::Dir(sub) => files(sub, out),
                    DirEntry::File(f) => out.push(f),
                }
            }
        }
        let mut all = Vec::new();
        files(&CONTENT, &mut all);
        for f in all {
            if f.path().extension().and_then(|e| e.to_str()) != Some("json") { continue; }
            if let Some(Ok(v)) = f.contents_utf8().map(parse) { walk(&v, &mut add); }
        }
        out
    })
}

pub fn course(id: &str) -> Result<Value, String> {
    check_id(id, "course")?;
    read_json(&format!("courses/{id}/course.json"))
}

/// kind is "topics" or "exams".
pub fn item(grade: i64, subject: &str, kind: &str, id: &str) -> Result<Value, String> {
    let g = check_grade(grade)?;
    check_id(subject, "subject")?;
    check_id(id, if kind == "topics" { "topic" } else { "exam" })?;
    read_json(&format!("grade-{g}/{subject}/{kind}/{id}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_grades_and_courses() {
        let c = catalog();
        let grades = c["grades"].as_array().unwrap();
        assert!(!grades.is_empty());
        assert!(grades.iter().all(|g| !g["subjects"].as_array().unwrap().is_empty()));
        assert!(!c["courses"].as_array().unwrap().is_empty());
    }

    #[test]
    fn every_listed_topic_and_exam_loads() {
        for g in catalog()["grades"].as_array().unwrap() {
            let grade = g["grade"].as_i64().unwrap();
            for s_ in g["subjects"].as_array().unwrap() {
                let sid = s_["id"].as_str().unwrap();
                for t in s_["topics"].as_array().unwrap() {
                    item(grade, sid, "topics", t["id"].as_str().unwrap()).unwrap();
                }
                for e in s_["exams"].as_array().unwrap() {
                    item(grade, sid, "exams", e["id"].as_str().unwrap()).unwrap();
                }
            }
        }
        for c in catalog()["courses"].as_array().unwrap() {
            course(c["id"].as_str().unwrap()).unwrap();
        }
    }

    #[test]
    fn words_and_ids() {
        assert!(words().len() > 1000);
        assert!(words().iter().all(|w| !w.ends_with('\'') && w.to_lowercase() == *w));
        assert!(valid_id("grade-10-maths"));
        assert!(!valid_id("../x") && !valid_id("A") && !valid_id("a--b"));
        assert!(item(13, "maths", "topics", "x").is_err());
    }
}
