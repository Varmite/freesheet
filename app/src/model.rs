//! The content (notes, questions, exams, courses) and the learner's saved progress.
//!
//! Content types follow docs/CONTENT_FORMAT.md. Progress types follow the progress.json
//! that FreeSheet has always written: every field keeps its name, unknown fields are kept
//! as they are (`extra`), and whole numbers are written without ".0", so files move freely
//! between this version, older versions and the phone.

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

// ---------------------------------------------------------------- helpers

/// Any JSON scalar as text (table cells are sometimes numbers).
fn de_text<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(text_of(&Value::deserialize(d)?))
}
fn de_opt_text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(if v.is_null() { None } else { Some(text_of(&v)) })
}
fn de_text_list<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(match v {
        Value::Array(a) => a.iter().map(text_of).collect(),
        Value::Null => Vec::new(),
        other => vec![text_of(&other)],
    })
}
fn de_rows<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Vec<String>>, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(match v {
        Value::Array(rows) => rows
            .iter()
            .map(|r| match r {
                Value::Array(c) => c.iter().map(text_of).collect(),
                _ => Vec::new(),
            })
            .collect(),
        _ => Vec::new(),
    })
}

/// JavaScript's String(v) for JSON values.
pub fn text_of(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::String(s) => s.clone(),
        Value::Number(n) => js_num(n.as_f64().unwrap_or(0.0)),
        Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// JavaScript's String(number).
pub fn js_num(n: f64) -> String {
    if n.is_nan() {
        "NaN".into()
    } else if n.is_infinite() {
        if n > 0.0 { "Infinity".into() } else { "-Infinity".into() }
    } else if n == n.trunc() && n.abs() < 1e21 {
        format!("{}", n as i128)
    } else {
        format!("{}", n)
    }
}

/// A number that is written as a whole number when it is one (as JavaScript does).
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Num(pub f64);
impl Serialize for Num {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        num_value(self.0).serialize(s)
    }
}
impl<'de> Deserialize<'de> for Num {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        Ok(Num(value_f64(&v).unwrap_or(0.0)))
    }
}
pub fn num_value(n: f64) -> Value {
    if n.is_finite() && n == n.trunc() && n.abs() < 9e15 {
        Value::from(n as i64)
    } else if n.is_finite() {
        serde_json::Number::from_f64(n).map(Value::Number).unwrap_or(Value::Null)
    } else {
        Value::Null
    }
}
/// Number(v) for numbers and numeric strings.
pub fn value_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

// ---------------------------------------------------------------- content

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Block {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(deserialize_with = "de_opt_text")]
    pub text: Option<String>,
    #[serde(deserialize_with = "de_text_list")]
    pub items: Vec<String>,
    pub ordered: bool,
    #[serde(deserialize_with = "de_opt_text")]
    pub term: Option<String>,
    #[serde(deserialize_with = "de_opt_text")]
    pub title: Option<String>,
    #[serde(deserialize_with = "de_opt_text")]
    pub problem: Option<String>,
    #[serde(deserialize_with = "de_text_list")]
    pub steps: Vec<String>,
    #[serde(deserialize_with = "de_opt_text")]
    pub answer: Option<String>,
    #[serde(deserialize_with = "de_opt_text")]
    pub label: Option<String>,
    #[serde(deserialize_with = "de_text_list")]
    pub headers: Vec<String>,
    #[serde(deserialize_with = "de_rows")]
    pub rows: Vec<Vec<String>>,
    #[serde(deserialize_with = "de_opt_text")]
    pub caption: Option<String>,
    #[serde(deserialize_with = "de_opt_text")]
    pub latex: Option<String>,
    #[serde(deserialize_with = "de_opt_text")]
    pub svg: Option<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Pair {
    #[serde(deserialize_with = "de_text")]
    pub left: String,
    #[serde(deserialize_with = "de_text")]
    pub right: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Question {
    #[serde(deserialize_with = "de_text")]
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(deserialize_with = "de_text")]
    pub prompt: String,
    pub marks: Option<Value>,
    #[serde(deserialize_with = "de_text_list")]
    pub options: Vec<String>,
    pub answer: Value,
    #[serde(deserialize_with = "de_text_list")]
    pub answers: Vec<String>,
    #[serde(deserialize_with = "de_opt_text")]
    pub unit: Option<String>,
    pub tolerance: Option<Value>,
    pub pairs: Vec<Pair>,
    #[serde(deserialize_with = "de_opt_text")]
    pub memo: Option<String>,
    #[serde(deserialize_with = "de_opt_text")]
    pub explanation: Option<String>,
    /// Short answers outside language subjects forgive spelling slips (set by the app).
    #[serde(skip)]
    pub typo_words: Option<std::sync::Arc<std::collections::HashSet<String>>>,
}

impl Question {
    /// The correct option index of a multiple-choice question.
    pub fn answer_index(&self) -> Option<i64> {
        self.answer.as_i64().or_else(|| self.answer.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64))
    }
    pub fn answer_bool(&self) -> Option<bool> {
        self.answer.as_bool()
    }
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Topic {
    pub id: String,
    pub title: String,
    #[serde(deserialize_with = "de_text")]
    pub summary: String,
    pub notes: Vec<Block>,
    pub questions: Vec<Question>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ExamSection {
    #[serde(deserialize_with = "de_text")]
    pub title: String,
    #[serde(deserialize_with = "de_opt_text")]
    pub instructions: Option<String>,
    pub context: Option<Vec<Block>>,
    pub questions: Vec<Question>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Exam {
    pub id: String,
    pub title: String,
    pub duration_minutes: f64,
    pub total_marks: f64,
    #[serde(deserialize_with = "de_text_list")]
    pub instructions: Vec<String>,
    pub sections: Vec<ExamSection>,
}

// ---- catalogue

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
pub struct TopicMeta {
    pub id: String,
    pub title: String,
    pub term: i64,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct ExamMeta {
    pub id: String,
    pub title: String,
    pub duration_minutes: f64,
    pub total_marks: f64,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
pub struct SubjectMeta {
    pub id: String,
    pub name: String,
    pub description: String,
    pub topics: Vec<TopicMeta>,
    pub exams: Vec<ExamMeta>,
}

#[derive(Clone, Debug, Default)]
pub struct GradeData {
    pub grade: i64,
    pub subjects: Vec<SubjectMeta>,
}

#[derive(Clone, Debug, Default)]
pub struct CourseMeta {
    pub id: String,
    pub name: String,
    pub about: String,
    pub grade: Option<i64>,
    pub level: String,
    pub units: usize,
    pub lessons: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub grades: Vec<GradeData>,
    pub courses: Vec<CourseMeta>,
}

// ---- language courses

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Word {
    pub af: String,
    #[serde(deserialize_with = "de_text_list")]
    pub en: Vec<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Sentence {
    #[serde(deserialize_with = "de_text_list")]
    pub af: Vec<String>,
    #[serde(deserialize_with = "de_text_list")]
    pub en: Vec<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Drill {
    pub task: String,
    pub from: String,
    #[serde(deserialize_with = "de_text_list")]
    pub to: Vec<String>,
    pub en: Option<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Lesson {
    pub id: String,
    pub title: String,
    pub words: Vec<Word>,
    pub sentences: Vec<Sentence>,
    pub drills: Vec<Drill>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Unit {
    pub title: String,
    pub english: Option<String>,
    pub hue: Option<f64>,
    pub lessons: Vec<Lesson>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Course {
    #[serde(skip)]
    pub id: String,
    pub name: String,
    pub about: Option<String>,
    pub level: Option<String>,
    pub grade: Option<i64>,
    pub units: Vec<Unit>,
    pub english_synonyms: IndexMap<String, String>,
}

// ---------------------------------------------------------------- progress

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct LastTopic {
    pub grade: i64,
    pub subject_id: String,
    pub topic_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct BuddyPrefs {
    pub kind: String,
    pub color: String,
    pub accessory: String,
    pub name: String,
    pub on: bool,
    pub quiet: bool,
    pub min: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
impl Default for BuddyPrefs {
    fn default() -> Self {
        BuddyPrefs { kind: "blob".into(), color: "sky".into(), accessory: "none".into(), name: "Pip".into(), on: true, quiet: false, min: false, extra: Map::new() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grade: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sounds: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_rendering: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_speed: Option<Num>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_topic: Option<LastTopic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buddy: Option<BuddyPrefs>,
    /// The learner allowed AI marking on this computer (see crate::ai).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_marking: Option<bool>,
    /// A colour theme from ui::theme::THEMES ("system" when unset).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// An accent colour from ui::theme::ACCENTS ("default": the theme's own).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    /// Notes: "compact", "normal" or "relaxed".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_spacing: Option<String>,
    /// Notes: "narrow", "normal" or "wide".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reading_width: Option<String>,
    /// The tab FreeSheet opens on: "study", "exams" or "progress".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_tab: Option<String>,
    /// Look for a new version each time FreeSheet opens (on unless turned off).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_updates: Option<bool>,
    /// The version whose "update available" card was closed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_dismissed: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct TopicProg {
    pub notes_read: bool,
    pub responses: IndexMap<String, Value>,
    pub checked: IndexMap<String, bool>,
    pub self_marks: IndexMap<String, Option<Num>>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub hints: IndexMap<String, u32>,
    pub attempts: u32,
    #[serde(skip_serializing_if = "is_false")]
    pub recorded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<Num>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_max: Option<Num>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best: Option<Num>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_max: Option<Num>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sections_seen: Option<u32>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct SectionScore {
    pub title: String,
    pub score: Num,
    pub max: Num,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Attempt {
    pub date: String,
    pub score: Num,
    pub max: Num,
    pub percent: i64,
    pub time_used_sec: Num,
    pub timed: bool,
    pub sections: Vec<SectionScore>,
    pub responses: IndexMap<String, Value>,
    pub self_marks: IndexMap<String, Option<Num>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Draft {
    pub phase: String,
    pub responses: IndexMap<String, Value>,
    pub self_marks: IndexMap<String, Option<Num>>,
    pub remaining_sec: Num,
    pub elapsed_sec: Num,
    pub timer_on: bool,
    pub saved_at: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct DayEntry {
    pub q: Num,
    pub x: Num,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct StreakData {
    pub days: IndexMap<String, DayEntry>,
    pub longest: Num,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct LessonRecord {
    pub times: u32,
    pub best: Num,
    pub xp: Num,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct CourseProg {
    pub done: IndexMap<String, LessonRecord>,
    pub xp: Num,
    pub days: IndexMap<String, Num>,
    pub seen: Vec<String>,
    pub practice: u32,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn one() -> u32 {
    1
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Progress {
    #[serde(default = "one")]
    pub version: u32,
    pub settings: Settings,
    pub topics: IndexMap<String, TopicProg>,
    pub exams: IndexMap<String, Vec<Attempt>>,
    pub exam_drafts: IndexMap<String, Draft>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streak: Option<StreakData>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub courses: IndexMap<String, CourseProg>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Progress {
    fn default() -> Self {
        Progress {
            version: 1,
            settings: Settings::default(),
            topics: IndexMap::new(),
            exams: IndexMap::new(),
            exam_drafts: IndexMap::new(),
            streak: None,
            courses: IndexMap::new(),
            extra: Map::new(),
        }
    }
}

impl Progress {
    /// Reads progress JSON leniently: a broken part is reset rather than losing everything.
    pub fn from_value(v: Value) -> Option<Progress> {
        let Value::Object(mut m) = v else { return None };
        // Each top-level part on its own, so one bad field doesn't throw the rest away.
        let mut p = Progress::default();
        if let Some(x) = m.remove("version") {
            p.version = x.as_u64().unwrap_or(1) as u32;
        }
        macro_rules! part {
            ($key:literal, $field:ident) => {
                if let Some(x) = m.remove($key) {
                    match serde_json::from_value(x.clone()) {
                        Ok(v) => p.$field = v,
                        Err(e) => {
                            log::warn!("progress: {} unreadable ({e}), kept aside", $key);
                            p.extra.insert(concat!($key, "Unreadable").into(), x);
                        }
                    }
                }
            };
        }
        part!("settings", settings);
        part!("topics", topics);
        part!("exams", exams);
        part!("examDrafts", exam_drafts);
        part!("streak", streak);
        part!("courses", courses);
        p.extra.extend(m);
        Some(p)
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_round_trip_keeps_everything() {
        let src = serde_json::json!({
            "version": 1,
            "settings": { "grade": 9, "readSpeed": 1.25, "textSize": "normal", "future": "x",
                "buddy": { "kind": "robot", "color": "ocean", "accessory": "none", "name": "Botline", "on": true, "quiet": true, "min": false },
                "lastTopic": { "grade": 9, "subjectId": "mathematics", "topicId": "exponents", "mode": "notes" } },
            "topics": { "9/mathematics/exponents": { "notesRead": true, "responses": { "q1": 2, "q2": "x", "q3": [null, "a"], "q4": { "text": "hi" } },
                "checked": { "q1": true }, "selfMarks": { "q4": 3 }, "attempts": 1, "best": 7.5, "bestMax": 10, "sectionsSeen": 4 } },
            "exams": { "9/mathematics/november-exam": [ { "date": "2026-11-01T10:00:00.000Z", "score": 50.5, "max": 100, "percent": 51,
                "timeUsedSec": 3600, "timed": true, "sections": [ { "title": "Q1", "score": 10, "max": 20 } ], "responses": {}, "selfMarks": { "a": null } } ] },
            "examDrafts": {},
            "streak": { "days": { "2026-11-06": { "q": 3, "x": 0 } }, "longest": 2 },
            "somethingNew": [1, 2]
        });
        let p = Progress::from_value(src.clone()).unwrap();
        assert_eq!(p.settings.grade, Some(9));
        assert_eq!(p.topics["9/mathematics/exponents"].best, Some(Num(7.5)));
        let back = p.to_value();
        assert_eq!(back["somethingNew"], src["somethingNew"]);
        assert_eq!(back["settings"]["future"], "x");
        assert_eq!(back["topics"]["9/mathematics/exponents"]["bestMax"], serde_json::json!(10));
        assert_eq!(back["exams"]["9/mathematics/november-exam"][0]["score"], serde_json::json!(50.5));
        assert_eq!(back["exams"]["9/mathematics/november-exam"][0]["selfMarks"]["a"], Value::Null);
        assert_eq!(back["topics"]["9/mathematics/exponents"]["responses"], src["topics"]["9/mathematics/exponents"]["responses"]);
    }

    #[test]
    fn js_number_strings() {
        assert_eq!(js_num(3.0), "3");
        assert_eq!(js_num(3.5), "3.5");
        assert_eq!(js_num(-0.25), "-0.25");
        assert_eq!(text_of(&serde_json::json!(12)), "12");
    }
}

/// String(v) for an optional answer (null/undefined become "").
pub fn text_of_opt(v: &Value) -> String {
    if v.is_null() { String::new() } else { text_of(v) }
}
