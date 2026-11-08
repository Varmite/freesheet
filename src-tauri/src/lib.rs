//! FreeSheet: offline CAPS study notes, practice questions and exams.
//!
//! The app's screens are the HTML/JS in ../src, shown in the system's own web view
//! (WebView2 on Windows, WebKit on macOS and Linux, Android's WebView on phones), so no
//! browser engine is bundled. This Rust side opens the window, serves the content and
//! read-aloud audio, and keeps the learner's progress. src/js/api.js is the other end.

mod audio;
mod content;
mod progress;

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs::{FsExt, OpenOptions};

struct App {
    store: progress::Store,
    audio: audio::Audio,
    /// Greyscale ("smooth") text was switched on at start-up (Windows: no ClearType).
    smooth_text: bool,
    /// The app has saved and may now close.
    closing: AtomicBool,
    /// A close was asked for and the page is saving.
    close_started: AtomicBool,
}

type Res<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ---------------------------------------------------------------- content

#[tauri::command]
async fn catalog() -> &'static Value {
    content::catalog()
}

#[tauri::command]
async fn content_words() -> &'static Vec<String> {
    content::words()
}

#[tauri::command]
async fn course(id: String) -> Res<Value> {
    content::course(&id)
}

#[tauri::command]
async fn topic(grade: i64, subject_id: String, topic_id: String) -> Res<Value> {
    content::item(grade, &subject_id, "topics", &topic_id)
}

#[tauri::command]
async fn exam(grade: i64, subject_id: String, exam_id: String) -> Res<Value> {
    content::item(grade, &subject_id, "exams", &exam_id)
}

// ---------------------------------------------------------------- audio

fn topic_audio_folder(grade: i64, subject: &str, topic: &str) -> Res<String> {
    let g = content::check_grade(grade)?;
    content::check_id(subject, "subject")?;
    content::check_id(topic, "topic")?;
    Ok(format!("grade-{g}/{subject}/{topic}"))
}

fn course_audio_folder(id: &str) -> Res<String> {
    content::check_id(id, "course")?;
    Ok(format!("courses/{id}"))
}

fn check_audio(id: &str) -> Res<()> {
    if audio::valid_audio_id(id) { Ok(()) } else { Err("Invalid audio id".into()) }
}

#[tauri::command]
async fn audio_list(app: State<'_, App>, grade: i64, subject_id: String, topic_id: String) -> Res<Vec<String>> {
    Ok(app.audio.list(&topic_audio_folder(grade, &subject_id, &topic_id)?))
}

/// The .ogg file's bytes, sent as an ArrayBuffer (not JSON).
#[tauri::command]
async fn audio(app: State<'_, App>, grade: i64, subject_id: String, topic_id: String, id: String) -> Res<Response> {
    check_audio(&id)?;
    Ok(Response::new(app.audio.read(&topic_audio_folder(grade, &subject_id, &topic_id)?, &id)?))
}

#[tauri::command]
async fn course_audio_list(app: State<'_, App>, id: String) -> Res<Vec<String>> {
    Ok(app.audio.list(&course_audio_folder(&id)?))
}

#[tauri::command]
async fn course_audio(app: State<'_, App>, id: String, audio: String) -> Res<Response> {
    check_audio(&audio)?;
    Ok(Response::new(app.audio.read(&course_audio_folder(&id)?, &audio)?))
}

// ---------------------------------------------------------------- progress

#[tauri::command]
async fn progress_load(app: State<'_, App>) -> Res<Value> {
    Ok(app.store.load())
}

#[tauri::command]
async fn progress_save(app: State<'_, App>, data: Value) -> Res<bool> {
    app.store.save(&data).map_err(|e| {
        eprintln!("Saving progress failed: {e}");
        err(e)
    })?;
    Ok(true)
}

#[tauri::command]
async fn progress_location(app: State<'_, App>) -> Res<String> {
    if cfg!(target_os = "android") {
        return Ok("the FreeSheet app's private storage".into());
    }
    Ok(app.store.file.parent().map(|p| p.display().to_string()).unwrap_or_default())
}

/// Today as YYYY-MM-DD (UTC), for the export file name.
fn today() -> String {
    let days = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() / 86400).unwrap_or(0) as i64;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

#[tauri::command]
async fn progress_export(handle: AppHandle, data: Value) -> Res<Value> {
    let json = serde_json::to_string_pretty(&data).map_err(err)?;
    #[allow(unused_mut)]
    let mut dialog = handle
        .dialog()
        .file()
        .set_title("Export progress")
        .set_file_name(format!("freesheet-progress-{}.json", today()))
        .add_filter("FreeSheet progress", &["json"]);
    #[cfg(desktop)]
    if let Ok(docs) = handle.path().document_dir() {
        dialog = dialog.set_directory(docs);
    }
    let Some(path) = dialog.blocking_save_file() else { return Ok(json!({ "ok": false })) };
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    let mut file = handle.fs().open(path.clone(), opts).map_err(err)?;
    file.write_all(json.as_bytes()).map_err(err)?;
    Ok(json!({ "ok": true, "path": path.to_string() }))
}

#[tauri::command]
async fn progress_import(handle: AppHandle) -> Res<Value> {
    let picked = handle
        .dialog()
        .file()
        .set_title("Import progress")
        .add_filter("FreeSheet progress", &["json"])
        .blocking_pick_file();
    let Some(path) = picked else { return Ok(json!({ "ok": false })) };
    let read = || -> Res<Value> {
        let mut opts = OpenOptions::new();
        opts.read(true);
        let mut text = String::new();
        handle.fs().open(path, opts).map_err(err)?.read_to_string(&mut text).map_err(err)?;
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(err)
    };
    match read() {
        Ok(Value::Object(m)) if m.contains_key("topics") && m.contains_key("exams") => {
            Ok(json!({ "ok": true, "data": progress::with_defaults(m) }))
        }
        Ok(_) => Ok(json!({ "ok": false, "error": "That file is not a FreeSheet progress file." })),
        Err(_) => Ok(json!({ "ok": false, "error": "That file could not be read." })),
    }
}

// ---------------------------------------------------------------- app

#[tauri::command]
async fn open_data_folder(_handle: AppHandle, _app: State<'_, App>) -> Res<()> {
    #[cfg(desktop)]
    {
        use tauri_plugin_opener::OpenerExt;
        if let Some(dir) = _app.store.file.parent() {
            let _ = std::fs::create_dir_all(dir);
            _handle.opener().open_path(dir.display().to_string(), None::<&str>).map_err(err)?;
        }
    }
    Ok(())
}

#[tauri::command]
fn app_version(handle: AppHandle) -> String {
    handle.package_info().version.to_string()
}

/// Opens a FreeSheet release page in the browser; nothing else may be opened from here.
#[tauri::command]
fn open_release_page(handle: AppHandle, url: String) -> Res<()> {
    if !url.starts_with("https://github.com/Varmite/freesheet/releases/") {
        return Err("Not a FreeSheet release".into());
    }
    use tauri_plugin_opener::OpenerExt;
    handle.opener().open_url(url, None::<&str>).map_err(err)
}

#[tauri::command]
fn smooth_text_active(app: State<'_, App>) -> bool {
    app.smooth_text
}

#[tauri::command]
fn relaunch(handle: AppHandle, app: State<'_, App>) {
    app.closing.store(true, Ordering::SeqCst);
    handle.restart();
}

/// Called by the page once it has saved, after "freesheet:closing".
#[tauri::command]
fn close_window(handle: AppHandle, app: State<'_, App>) {
    app.closing.store(true, Ordering::SeqCst);
    handle.exit(0);
}

/// Android: the back button on the home screen sends the app to the background.
#[tauri::command]
fn minimize_app() -> Res<()> {
    #[cfg(target_os = "android")]
    audio::move_task_to_back()?;
    Ok(())
}

/// Windows: while minimised, WebView2 is asked to give back as much memory as it can.
#[cfg(windows)]
fn track_minimised(handle: &AppHandle, label: &str) {
    use std::sync::atomic::AtomicU8;
    static LAST: AtomicU8 = AtomicU8::new(0); // 0 unknown, 1 normal, 2 low
    let Some(w) = handle.get_webview_window(label) else { return };
    let low = w.is_minimized().unwrap_or(false);
    if LAST.swap(if low { 2 } else { 1 }, Ordering::SeqCst) == if low { 2 } else { 1 } {
        return;
    }
    let _ = w.with_webview(move |wv| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::*;
        use windows::core::Interface;
        if let Ok(core) = wv.controller().CoreWebView2() {
            if let Ok(c) = core.cast::<ICoreWebView2_19>() {
                let level = if low { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW } else { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL };
                let _ = c.SetMemoryUsageTargetLevel(level);
            }
        }
    });
}

/// Asks the page to save, then closes (or closes anyway after a few seconds).
fn begin_close(handle: &AppHandle) {
    let app = handle.state::<App>();
    if app.close_started.swap(true, Ordering::SeqCst) {
        return;
    }
    if handle.emit("freesheet:closing", ()).is_err() {
        app.closing.store(true, Ordering::SeqCst);
        handle.exit(0);
        return;
    }
    let h = handle.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(4));
        h.state::<App>().closing.store(true, Ordering::SeqCst);
        h.exit(0);
    });
}

fn platform() -> &'static str {
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

/// Only the app's own pages may load in the window.
fn allowed(url: &tauri::Url) -> bool {
    match (url.scheme(), url.host_str()) {
        ("tauri", _) => true,
        ("http" | "https", Some("tauri.localhost")) => true,
        ("http", Some("localhost" | "127.0.0.1")) => cfg!(debug_assertions),
        _ => false,
    }
}

/// Testing: keep everything in this folder instead of the real data folder.
fn test_data_dir() -> Option<PathBuf> {
    std::env::var_os("FREESHEET_DATA").map(PathBuf::from)
}

fn data_dirs(handle: &AppHandle) -> (PathBuf, Vec<PathBuf>) {
    if let Some(d) = test_data_dir() {
        return (d, Vec::new());
    }
    let path = handle.path();
    if cfg!(target_os = "android") {
        let dir = path.app_data_dir().unwrap_or_else(|_| PathBuf::from("/data/data/za.freesheet.app/files"));
        // Android Edition 1.0 (Capacitor) kept a copy of the progress in getFilesDir().
        let mut legacy = vec![dir.join("files").join("progress.json")];
        if let Some(parent) = dir.parent() {
            legacy.push(parent.join("files").join("progress.json"));
        }
        legacy.push(PathBuf::from("/data/user/0/za.freesheet.app/files/progress.json"));
        legacy.push(PathBuf::from("/data/data/za.freesheet.app/files/progress.json"));
        (dir, legacy)
    } else {
        // The same folder Electron used (FreeSheet 1.0), so progress carries over.
        let base = path.config_dir().unwrap_or_else(|_| PathBuf::from("."));
        (base.join("FreeSheet"), Vec::new())
    }
}

fn audio_root(handle: &AppHandle) -> PathBuf {
    if cfg!(debug_assertions) && !cfg!(mobile) {
        return PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("audio");
    }
    handle.path().resource_dir().map(|d| d.join("audio")).unwrap_or_else(|_| PathBuf::from("audio"))
}

/// The window's colour before the page has drawn, so a dark theme never flashes white.
#[cfg_attr(not(desktop), allow(dead_code))]
fn theme_background(theme: &str) -> Option<tauri::window::Color> {
    use tauri::window::Color;
    Some(match theme {
        "light" | "contrast" => Color(255, 255, 255, 255),
        "dark" => Color(22, 22, 22, 255),
        "sepia" => Color(248, 241, 227, 255),
        "midnight" => Color(15, 22, 38, 255),
        "forest" => Color(18, 26, 21, 255),
        "black" => Color(0, 0, 0, 255),
        _ => return None, // Automatic: known once the window can ask the system.
    })
}

fn create_window(handle: &AppHandle, prefs: &progress::Startup, smooth_text: bool) -> tauri::Result<()> {
    let appearance = json!({
        "theme": prefs.theme, "accent": prefs.accent, "lineSpacing": prefs.line_spacing,
        "readingWidth": prefs.reading_width, "textRendering": prefs.text_rendering,
    });
    let init = format!("window.__FREESHEET_PLATFORM__ = {}; window.__FREESHEET_APPEARANCE__ = {appearance};", serde_json::to_string(platform()).unwrap());
    #[allow(unused_mut)]
    let mut b = WebviewWindowBuilder::new(handle, "main", WebviewUrl::App("index.html".into()))
        .initialization_script(&init)
        .on_navigation(allowed)
        .background_color(tauri::window::Color(255, 255, 255, 255));
    #[cfg(desktop)]
    {
        b = b.title("FreeSheet").inner_size(1200.0, 820.0).min_inner_size(760.0, 560.0).center().visible(false);
        if let Some(c) = theme_background(&prefs.theme) {
            b = b.background_color(c);
        }
    }
    #[cfg(windows)]
    {
        // "Smooth" (the default) draws text the way macOS does: greyscale anti-aliasing
        // instead of ClearType's coloured fringes. WebView2 only reads this at start-up,
        // so changing the setting needs a restart (Settings offers one).
        // --in-process-gpu: no separate GPU process (about 25 MB less memory, no visible change).
        let mut args = String::from("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --in-process-gpu");
        if smooth_text {
            args.push_str(" --disable-lcd-text");
        }
        b = b.additional_browser_args(&args);
    }
    let _ = smooth_text;
    if let Some(d) = test_data_dir() {
        b = b.data_directory(d.join("webview"));
    }
    let window = b.build()?;
    #[cfg(desktop)]
    {
        if prefs.theme == "system" && matches!(window.theme(), Ok(tauri::Theme::Dark)) {
            let _ = window.set_background_color(Some(tauri::window::Color(22, 22, 22, 255)));
        }
        window.show()?;
    }
    let _ = window;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    #[cfg(desktop)]
    {
        // A second launch brings the open window to the front instead (a test copy with
        // its own data folder runs alongside).
        if test_data_dir().is_none() {
            builder = builder.plugin(tauri_plugin_single_instance::init(|handle, _args, _cwd| {
                if let Some(w) = handle.get_webview_window("main") {
                    let _ = w.unminimize();
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }));
        }
    }
    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            catalog,
            content_words,
            course,
            topic,
            exam,
            audio_list,
            audio,
            course_audio_list,
            course_audio,
            progress_load,
            progress_save,
            progress_location,
            progress_export,
            progress_import,
            open_data_folder,
            app_version,
            smooth_text_active,
            relaunch,
            open_release_page,
            close_window,
            minimize_app,
        ])
        .setup(|app| {
            let handle = app.handle();
            let (dir, legacy) = data_dirs(handle);
            let store = progress::Store::new(dir, legacy);
            // Ready by the time the page asks for it.
            std::thread::spawn(|| {
                content::catalog();
            });
            let prefs = store.startup();
            let smooth_text = cfg!(windows) && prefs.text_rendering == "smooth";
            app.manage(App {
                store,
                audio: audio::Audio::new(audio_root(handle)),
                smooth_text,
                closing: AtomicBool::new(false),
                close_started: AtomicBool::new(false),
            });
            create_window(handle, &prefs, smooth_text)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while starting FreeSheet")
        .run(|handle, event| match event {
            RunEvent::WindowEvent { event: WindowEvent::CloseRequested { api, .. }, .. } => {
                if handle.try_state::<App>().is_some_and(|a| !a.closing.load(Ordering::SeqCst)) {
                    api.prevent_close();
                    begin_close(handle);
                }
            }
            #[cfg(windows)]
            RunEvent::WindowEvent { label, event: WindowEvent::Resized(_), .. } => track_minimised(handle, &label),
            RunEvent::ExitRequested { api, .. } => {
                if handle.try_state::<App>().is_some_and(|a| !a.closing.load(Ordering::SeqCst)) {
                    api.prevent_exit();
                    begin_close(handle);
                }
            }
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    #[test]
    fn today_is_a_date() {
        let t = super::today();
        assert_eq!(t.len(), 10);
        assert!(t.starts_with("20"));
    }

    #[test]
    fn navigation_stays_in_the_app() {
        let ok = |u: &str| super::allowed(&tauri::Url::parse(u).unwrap());
        assert!(ok("tauri://localhost/index.html"));
        assert!(ok("http://tauri.localhost/index.html"));
        assert!(!ok("https://example.com/"));
        assert!(!ok("file:///C:/Windows/"));
    }
}
