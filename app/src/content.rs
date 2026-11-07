//! Notes, practice questions, exams and language courses.
//!
//! The whole content/ folder (a few MB of JSON) is built into the program, so every
//! platform reads it the same way and nothing can go missing. Layout (see
//! docs/CONTENT_FORMAT.md):
//!   content/grade-<n>/<subject>/subject.json
//!   content/grade-<n>/<subject>/topics/<topic>.json
//!   content/grade-<n>/<subject>/exams/<exam>.json
//!   content/courses/<course>/course.json

use crate::model::*;
use include_dir::{include_dir, Dir, DirEntry};
use regex::Regex;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};
use unicode_normalization::UnicodeNormalization;

static CONTENT: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../content");

/// Ids used in file names: lower-case words joined by single hyphens.
pub fn valid_id(id: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$").unwrap()).is_match(id)
}

fn parse<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, String> {
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())
}

fn read<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let file = CONTENT.get_file(path).ok_or_else(|| format!("Not found: {path}"))?;
    parse(file.contents_utf8().ok_or_else(|| format!("Not text: {path}"))?)
}

fn build_catalog() -> Catalog {
    let mut grades = Vec::new();
    for dir in CONTENT.dirs() {
        let name = dir.path().file_name().and_then(|n| n.to_str()).unwrap_or("");
        let Some(grade) = name.strip_prefix("grade-").and_then(|n| n.parse::<i64>().ok()) else { continue };
        let mut subjects: Vec<SubjectMeta> = Vec::new();
        for sub in dir.dirs() {
            let path = format!("{}/subject.json", sub.path().to_string_lossy().replace('\\', "/"));
            if CONTENT.get_file(&path).is_none() {
                continue;
            }
            match read::<SubjectMeta>(&path) {
                Ok(s) => subjects.push(s),
                Err(e) => log::error!("Could not read {path}: {e}"),
            }
        }
        subjects.sort_by_key(|s| s.name.to_lowercase());
        grades.push(GradeData { grade, subjects });
    }
    grades.sort_by_key(|g| g.grade);
    Catalog { grades, courses: list_courses() }
}

/// Language courses are shown as a subject in every grade.
fn list_courses() -> Vec<CourseMeta> {
    let mut out = Vec::new();
    let Some(courses) = CONTENT.get_dir("courses") else { return out };
    let mut dirs: Vec<&Dir> = courses.dirs().collect();
    dirs.sort_by_key(|d| d.path().to_path_buf());
    for d in dirs {
        let id = d.path().file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let path = format!("courses/{id}/course.json");
        if !valid_id(&id) || CONTENT.get_file(&path).is_none() {
            continue;
        }
        match read::<Course>(&path) {
            Ok(c) => out.push(CourseMeta {
                lessons: c.units.iter().map(|u| u.lessons.len()).sum(),
                units: c.units.len(),
                id,
                name: c.name,
                about: c.about.unwrap_or_default(),
                grade: c.grade,
                level: c.level.unwrap_or_default(),
            }),
            Err(e) => log::error!("Bad course file {path}: {e}"),
        }
    }
    out
}

pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(build_catalog)
}

/// Every word that appears in the content, for telling spelling slips from real
/// different answers when marking (see marking::typo_of).
pub fn words() -> Arc<HashSet<String>> {
    static WORDS: OnceLock<Arc<HashSet<String>>> = OnceLock::new();
    WORDS
        .get_or_init(|| {
            let re = Regex::new(r"\p{L}[\p{L}']*").unwrap();
            let mut seen = HashSet::new();
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
            let mut add = |text: &str| {
                let norm: String = text.nfkc().collect::<String>().to_lowercase();
                for m in re.find_iter(&norm) {
                    seen.insert(m.as_str().trim_end_matches('\'').to_string());
                }
            };
            for f in all {
                if f.path().extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                if let Some(Ok(v)) = f.contents_utf8().map(parse::<Value>) {
                    walk(&v, &mut add);
                }
            }
            Arc::new(seen)
        })
        .clone()
}

pub fn course(id: &str) -> Result<Course, String> {
    if !valid_id(id) {
        return Err("Invalid course".into());
    }
    let mut c: Course = read(&format!("courses/{id}/course.json"))?;
    c.id = id.to_string();
    Ok(c)
}

pub fn topic(grade: i64, subject: &str, id: &str) -> Result<Topic, String> {
    if !(1..=12).contains(&grade) || !valid_id(subject) || !valid_id(id) {
        return Err("Invalid topic".into());
    }
    read(&format!("grade-{grade}/{subject}/topics/{id}.json"))
}

pub fn exam(grade: i64, subject: &str, id: &str) -> Result<Exam, String> {
    if !(1..=12).contains(&grade) || !valid_id(subject) || !valid_id(id) {
        return Err("Invalid exam".into());
    }
    read(&format!("grade-{grade}/{subject}/exams/{id}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_grades_and_courses() {
        let c = catalog();
        assert!(c.grades.len() >= 5);
        assert!(c.grades.iter().all(|g| !g.subjects.is_empty()));
        assert!(c.courses.len() >= 5);
    }

    #[test]
    fn every_listed_topic_exam_and_course_loads() {
        for g in &catalog().grades {
            for s in &g.subjects {
                for t in &s.topics {
                    let topic = topic(g.grade, &s.id, &t.id).unwrap_or_else(|e| panic!("{}/{}/{}: {e}", g.grade, s.id, t.id));
                    assert!(!topic.notes.is_empty());
                }
                for x in &s.exams {
                    exam(g.grade, &s.id, &x.id).unwrap_or_else(|e| panic!("{}/{}/{}: {e}", g.grade, s.id, x.id));
                }
            }
        }
        for c in &catalog().courses {
            course(&c.id).unwrap();
        }
    }

    #[test]
    fn word_list_and_ids() {
        assert!(words().len() > 10000);
        assert!(valid_id("grade-10-maths"));
        assert!(!valid_id("../x") && !valid_id("A") && !valid_id("a--b"));
        assert!(topic(13, "maths", "x").is_err());
    }
}
