//! The learner's progress: one JSON file, progress.json, in the app's data folder.
//!
//! The folder is the one every earlier version used, so updating keeps everyone's progress:
//!   Windows  %APPDATA%\FreeSheet
//!   macOS    ~/Library/Application Support/FreeSheet
//!   Linux    ~/.config/FreeSheet
//!   Android  the app's private storage
//! Saves go to a temporary file first and the previous version is kept as progress.json.bak,
//! so a crash or power cut mid-save never loses everything.

use crate::model::Progress;
use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

pub fn read_json(file: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(file).map_err(|e| e.to_string())?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())
}

#[derive(Clone)]
pub struct Store {
    pub file: PathBuf,
    /// Older places the progress may be in (earlier phone versions kept it elsewhere).
    legacy: Vec<PathBuf>,
}

fn with_ext(p: &Path, ext: &str) -> PathBuf {
    let mut s = p.to_path_buf().into_os_string();
    s.push(ext);
    PathBuf::from(s)
}

impl Store {
    pub fn new(dir: PathBuf, legacy: Vec<PathBuf>) -> Self {
        Store { file: dir.join("progress.json"), legacy }
    }

    pub fn dir(&self) -> PathBuf {
        self.file.parent().map(Path::to_path_buf).unwrap_or_default()
    }

    pub fn load(&self) -> Progress {
        let mut candidates = vec![self.file.clone(), with_ext(&self.file, ".bak")];
        candidates.extend(self.legacy.iter().cloned());
        for c in candidates {
            if !c.exists() {
                continue;
            }
            match read_json(&c).map(Progress::from_value) {
                Ok(Some(p)) => return p,
                Ok(None) => {}
                Err(e) => log::error!("Progress file {} unreadable: {e}", c.display()),
            }
        }
        Progress::default()
    }

    pub fn save_json(&self, json: &str) -> io::Result<()> {
        if let Some(dir) = self.file.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = with_ext(&self.file, ".tmp");
        fs::write(&tmp, json)?;
        if self.file.exists() {
            let _ = fs::copy(&self.file, with_ext(&self.file, ".bak"));
        }
        fs::rename(&tmp, &self.file)
    }
}

/// Writes progress on a background thread, one save at a time, newest wins.
pub struct Saver {
    tx: mpsc::Sender<SaveMsg>,
    done: mpsc::Receiver<Result<(), String>>,
}

enum SaveMsg {
    Save(String),
    Flush(mpsc::Sender<()>),
}

impl Saver {
    pub fn new(store: Store) -> Self {
        let (tx, rx) = mpsc::channel::<SaveMsg>();
        let (dtx, drx) = mpsc::channel();
        std::thread::Builder::new()
            .name("progress-saver".into())
            .spawn(move || {
                while let Ok(first) = rx.recv() {
                    // Take everything queued: only the newest save needs writing.
                    let mut newest = None;
                    let mut waiters = Vec::new();
                    let mut take = |m: SaveMsg| match m {
                        SaveMsg::Save(j) => newest = Some(j),
                        SaveMsg::Flush(w) => waiters.push(w),
                    };
                    take(first);
                    while let Ok(m) = rx.try_recv() {
                        take(m);
                    }
                    if let Some(json) = newest {
                        let r = store.save_json(&json).map_err(|e| e.to_string());
                        if let Err(e) = &r {
                            log::error!("Saving progress failed: {e}");
                        }
                        let _ = dtx.send(r);
                    }
                    for w in waiters {
                        let _ = w.send(());
                    }
                }
            })
            .expect("saver thread");
        Saver { tx, done: drx }
    }

    pub fn save(&self, p: &Progress) {
        if let Ok(json) = serde_json::to_string(p) {
            let _ = self.tx.send(SaveMsg::Save(json));
        }
    }

    /// Waits (up to 3 s) until everything queued is written.
    pub fn flush(&self) {
        let (wtx, wrx) = mpsc::channel();
        if self.tx.send(SaveMsg::Flush(wtx)).is_ok() {
            let _ = wrx.recv_timeout(std::time::Duration::from_secs(3));
        }
    }

    /// The result of the last finished save, if one finished since the last call.
    pub fn last_error(&self) -> Option<String> {
        let mut err = None;
        while let Ok(r) = self.done.try_recv() {
            err = r.err();
        }
        err
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_backup_and_fallback() {
        let dir = std::env::temp_dir().join(format!("freesheet-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::new(dir.clone(), vec![]);
        assert_eq!(store.load().topics.len(), 0);
        let saver = Saver::new(store.clone());
        let mut p = Progress::default();
        p.settings.grade = Some(9);
        saver.save(&p);
        saver.flush();
        p.settings.grade = Some(10);
        saver.save(&p);
        saver.flush();
        assert_eq!(store.load().settings.grade, Some(10));
        fs::write(&store.file, "{oops").unwrap();
        assert_eq!(store.load().settings.grade, Some(9)); // the .bak
        fs::remove_dir_all(&dir).unwrap();
    }
}
