//! What differs between computers and phones: where progress lives, where the read-aloud
//! audio is installed, file dialogs for export/import, and opening the data folder.

use std::path::PathBuf;

pub fn name() -> &'static str {
    if cfg!(target_os = "windows") {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "android") {
        "android"
    } else {
        "linux"
    }
}

pub fn is_android() -> bool {
    cfg!(target_os = "android")
}

/// "this computer" or "this phone".
pub fn device() -> &'static str {
    if is_android() { "this phone" } else { "this computer" }
}

/// The progress folder, and older places progress may be in.
pub fn data_dirs() -> (PathBuf, Vec<PathBuf>) {
    // Testing: --data-dir (or FREESHEET_DATA) keeps progress somewhere else.
    if let Some(d) = std::env::var_os("FREESHEET_DATA") {
        return (PathBuf::from(d), Vec::new());
    }
    #[cfg(target_os = "android")]
    {
        android::data_dirs()
    }
    #[cfg(not(target_os = "android"))]
    {
        // The same folder every earlier version used, so progress carries over.
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        (base.join("FreeSheet"), Vec::new())
    }
}

// ---------------------------------------------------------------- audio files

#[cfg(not(target_os = "android"))]
fn audio_root() -> Option<PathBuf> {
    use std::sync::OnceLock;
    static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();
    ROOT.get_or_init(|| {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?.to_path_buf();
        let candidates = [
            dir.join("audio"),
            dir.join("resources").join("audio"),
            dir.join("..").join("Resources").join("audio"),
            dir.join("..").join("lib").join("freesheet").join("audio"),
            dir.join("..").join("lib").join("FreeSheet").join("audio"),
            dir.join("..").join("share").join("freesheet").join("audio"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("audio"),
        ];
        candidates.into_iter().find(|p| p.is_dir())
    })
    .clone()
}

/// The ids (file names without .ogg) of the audio in a folder of audio/ ("grade-10/x/y").
pub fn list_audio(folder: &str) -> Vec<String> {
    let names: Vec<String> = {
        #[cfg(target_os = "android")]
        {
            android::list_assets(&format!("audio/{folder}"))
        }
        #[cfg(not(target_os = "android"))]
        {
            match audio_root().and_then(|r| std::fs::read_dir(r.join(folder)).ok()) {
                Some(rd) => rd.flatten().filter_map(|e| e.file_name().into_string().ok()).collect(),
                None => Vec::new(),
            }
        }
    };
    let mut ids: Vec<String> = names
        .into_iter()
        .filter_map(|n| n.strip_suffix(".ogg").map(str::to_string))
        .filter(|id| id.len() == 8 && id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()))
        .collect();
    ids.sort();
    ids
}

pub fn read_audio(folder: &str, id: &str) -> Option<Vec<u8>> {
    #[cfg(target_os = "android")]
    {
        android::read_asset(&format!("audio/{folder}/{id}.ogg"))
    }
    #[cfg(not(target_os = "android"))]
    {
        std::fs::read(audio_root()?.join(folder).join(format!("{id}.ogg"))).ok()
    }
}

// ---------------------------------------------------------------- files

pub enum FileResult {
    /// Still waiting for the system dialog (phones answer later).
    Pending,
    Cancelled,
    Done(String),
    Failed(String),
}

/// Save `json` where the learner chooses. Done(path) when saved.
pub fn export(json: &str, file_name: &str) -> FileResult {
    #[cfg(target_os = "android")]
    {
        android::start_export(json, file_name);
        FileResult::Pending
    }
    #[cfg(not(target_os = "android"))]
    {
        let mut d = rfd::FileDialog::new().set_title("Export progress").set_file_name(file_name).add_filter("FreeSheet progress", &["json"]);
        if let Some(docs) = dirs::document_dir() {
            d = d.set_directory(docs);
        }
        match d.save_file() {
            None => FileResult::Cancelled,
            Some(path) => match std::fs::write(&path, json) {
                Ok(()) => FileResult::Done(path.display().to_string()),
                Err(e) => FileResult::Failed(e.to_string()),
            },
        }
    }
}

/// Pick a file and read it. Done(contents).
pub fn import() -> FileResult {
    #[cfg(target_os = "android")]
    {
        android::start_import();
        FileResult::Pending
    }
    #[cfg(not(target_os = "android"))]
    {
        match rfd::FileDialog::new().set_title("Import progress").add_filter("FreeSheet progress", &["json"]).pick_file() {
            None => FileResult::Cancelled,
            Some(path) => match std::fs::read_to_string(&path) {
                Ok(s) => FileResult::Done(s),
                Err(_) => FileResult::Failed("That file could not be read.".into()),
            },
        }
    }
}

/// The answer to an export/import started earlier (phones).
pub fn poll_file() -> Option<(bool, FileResult)> {
    #[cfg(target_os = "android")]
    {
        android::poll_file()
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

pub fn open_folder(path: &std::path::Path) {
    #[cfg(not(target_os = "android"))]
    {
        let _ = std::fs::create_dir_all(path);
        if let Err(e) = open::that(path) {
            log::warn!("could not open {}: {e}", path.display());
        }
    }
    #[cfg(target_os = "android")]
    {
        let _ = path;
    }
}

/// Phones: send the app to the background (the back button on the Study home).
pub fn minimise() {
    #[cfg(target_os = "android")]
    android::move_task_to_back();
}

#[cfg(target_os = "android")]
pub mod android;
