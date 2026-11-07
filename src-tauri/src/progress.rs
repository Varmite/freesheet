//! The learner's progress: one JSON file, progress.json, in the app's data folder.
//!
//! The folder is the same one the earlier Electron version used, so updating keeps
//! everyone's progress:
//!   Windows  %APPDATA%\FreeSheet
//!   macOS    ~/Library/Application Support/FreeSheet
//!   Linux    ~/.config/FreeSheet
//!   Android  the app's private storage

use serde_json::{json, Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub fn empty() -> Value {
    json!({ "version": 1, "settings": {}, "topics": {}, "exams": {}, "examDrafts": {} })
}

/// Fills in any missing top-level parts, like Object.assign(empty(), data).
pub fn with_defaults(data: Map<String, Value>) -> Value {
    let mut out = empty();
    let obj = out.as_object_mut().unwrap();
    for (k, v) in data {
        obj.insert(k, v);
    }
    out
}

pub fn read_json(file: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(file).map_err(|e| e.to_string())?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())
}

pub struct Store {
    pub file: PathBuf,
    /// Older places the progress may be in (the Android Edition 1.0 kept it here).
    legacy: Vec<PathBuf>,
    /// Saves are written one at a time.
    lock: Mutex<()>,
}

impl Store {
    pub fn new(dir: PathBuf, legacy: Vec<PathBuf>) -> Self {
        Store { file: dir.join("progress.json"), legacy, lock: Mutex::new(()) }
    }

    fn with_ext(&self, ext: &str) -> PathBuf {
        let mut p = self.file.clone().into_os_string();
        p.push(ext);
        PathBuf::from(p)
    }

    pub fn load(&self) -> Value {
        let mut candidates = vec![self.file.clone(), self.with_ext(".bak")];
        candidates.extend(self.legacy.iter().cloned());
        for c in candidates {
            if !c.exists() { continue; }
            match read_json(&c) {
                Ok(Value::Object(m)) => return with_defaults(m),
                Ok(_) => {}
                Err(e) => eprintln!("Progress file {} unreadable: {e}", c.display()),
            }
        }
        empty()
    }

    /// Writes to a temporary file first and keeps the previous version as progress.json.bak,
    /// so a crash or power cut mid-save never loses everything.
    pub fn save(&self, data: &Value) -> io::Result<()> {
        let json = serde_json::to_string(data)?;
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(dir) = self.file.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.with_ext(".tmp");
        fs::write(&tmp, json)?;
        if self.file.exists() {
            let _ = fs::copy(&self.file, self.with_ext(".bak"));
        }
        fs::rename(&tmp, &self.file)
    }

    /// "smooth" (default) or "system".
    pub fn text_rendering(&self) -> String {
        let p = self.load();
        if p["settings"]["textRendering"] == "system" { "system".into() } else { "smooth".into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_round_trip_and_backup() {
        let dir = std::env::temp_dir().join(format!("freesheet-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::new(dir.clone(), vec![]);
        assert_eq!(store.load(), empty());
        store.save(&json!({ "settings": { "textRendering": "system" }, "topics": { "a": 1 } })).unwrap();
        store.save(&json!({ "settings": {}, "topics": { "a": 2 } })).unwrap();
        let p = store.load();
        assert_eq!(p["topics"]["a"], 2);
        assert_eq!(p["exams"], json!({}));
        assert_eq!(read_json(&store.with_ext(".bak")).unwrap()["topics"]["a"], 1);
        // A damaged file falls back to the backup.
        fs::write(&store.file, "{oops").unwrap();
        assert_eq!(store.load()["topics"]["a"], 1);
        assert_eq!(store.text_rendering(), "system");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_byte_order_mark() {
        let f = std::env::temp_dir().join(format!("freesheet-bom-{}.json", std::process::id()));
        fs::write(&f, "\u{feff}{\"topics\":{}}").unwrap();
        assert!(read_json(&f).unwrap().is_object());
        fs::remove_file(f).unwrap();
    }
}
