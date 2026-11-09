//! Downloads a new FreeSheet from its GitHub release, checks it, and installs it.
//! Shared by the web-view app (src-tauri) and the native app (app/), so it knows nothing about
//! either. Only ever talks to FreeSheet's own releases, and only moves to a newer version.
//!
//! Every release must keep these file names and include SHA256SUMS.txt:
//!   FreeSheet-Setup-X.Y.Z.exe, FreeSheet-X.Y.Z.msi, FreeSheet-X.Y.Z-mac-{arm64,x64}.zip,
//!   FreeSheet-X.Y.Z-linux-x64.{AppImage,deb}; Android (tag android-vX.Y.Z): FreeSheet-X.Y.Z-android.apk

use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

static ATTEMPT: AtomicU64 = AtomicU64::new(0);
const BASE: &str = "https://github.com/Varmite/freesheet/releases/download";
/// No bytes for this long means the connection has died.
const STALL: Duration = Duration::from_secs(60);
pub const CANCELLED: &str = "cancelled";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    WinNsis,
    WinMsi,
    MacArm,
    MacIntel,
    AppImage,
    Deb,
    Android,
}

/// What became of the install.
#[cfg_attr(windows, allow(dead_code))]
#[derive(Debug, PartialEq)]
pub enum Applied {
    /// An installer is taking over: the app must exit now.
    Exiting,
    /// The system's installer was opened; the app keeps running.
    Opened,
}

/// How this copy of FreeSheet was installed.
pub fn detect() -> Option<Target> {
    if cfg!(target_os = "android") {
        Some(Target::Android)
    } else if cfg!(windows) {
        // The NSIS installer leaves its uninstaller beside the app; the MSI does not.
        let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
        Some(if dir.join("uninstall.exe").is_file() { Target::WinNsis } else { Target::WinMsi })
    } else if cfg!(target_os = "macos") {
        Some(if cfg!(target_arch = "aarch64") { Target::MacArm } else { Target::MacIntel })
    } else if cfg!(target_os = "linux") {
        Some(if std::env::var_os("APPIMAGE").is_some() { Target::AppImage } else { Target::Deb })
    } else {
        None
    }
}

pub fn asset_name(t: Target, v: &str) -> String {
    match t {
        Target::WinNsis => format!("FreeSheet-Setup-{v}.exe"),
        Target::WinMsi => format!("FreeSheet-{v}.msi"),
        Target::MacArm => format!("FreeSheet-{v}-mac-arm64.zip"),
        Target::MacIntel => format!("FreeSheet-{v}-mac-x64.zip"),
        Target::AppImage => format!("FreeSheet-{v}-linux-x64.AppImage"),
        Target::Deb => format!("FreeSheet-{v}-linux-x64.deb"),
        Target::Android => format!("FreeSheet-{v}-android.apk"),
    }
}

fn parts(v: &str) -> Option<(u64, u64, u64)> {
    let mut it = v.split('.').map(|n| if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) { None } else { n.parse::<u64>().ok() });
    let out = (it.next()??, it.next()??, it.next()??);
    if it.next().is_some() { None } else { Some(out) }
}

/// `version` must be plain X.Y.Z and later than `current`.
pub fn acceptable(version: &str, current: &str) -> bool {
    matches!((parts(version), parts(current)), (Some(a), Some(b)) if a > b)
}

fn release_url(t: Target, v: &str, file: &str) -> String {
    let tag = if t == Target::Android { format!("android-v{v}") } else { format!("v{v}") };
    format!("{BASE}/{tag}/{file}")
}

/// The checksum listed for `name` in a SHA256SUMS.txt.
pub fn parse_sums(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (hash, file) = line.trim().split_once(char::is_whitespace)?;
        let matches = file.trim().trim_start_matches('*') == name && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit());
        matches.then(|| hash.to_ascii_lowercase())
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(hex(&h.finalize()));
        }
        h.update(&buf[..n]);
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_connect(Some(Duration::from_secs(30))).timeout_recv_response(Some(Duration::from_secs(60))).build().into()
}

fn get(a: &ureq::Agent, url: &str) -> Result<ureq::http::Response<ureq::Body>, String> {
    a.get(url).header("User-Agent", "FreeSheet-updater").call().map_err(|e| e.to_string())
}

fn clear_old(dir: &Path, keep: &str) {
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy() != keep {
                let p = e.path();
                let _ = if p.is_dir() { fs::remove_dir_all(&p) } else { fs::remove_file(&p) };
            }
        }
    }
}

fn download_verified(url: &str, expected: &str, dest: &Path, received: &AtomicU64, total: &AtomicU64, cancel: &AtomicBool) -> Result<(), String> {
    let part = dest.with_extension(format!("{}.part", ATTEMPT.fetch_add(1, Ordering::Relaxed)));
    let resp = get(&agent(), url)?;
    total.store(resp.body().content_length().unwrap_or(0), Ordering::Relaxed);
    let mut reader = resp.into_body().into_reader();
    let mut out = fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let finish = (|| loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.to_string());
        }
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break Ok(());
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        hash.update(&buf[..n]);
        received.fetch_add(n as u64, Ordering::Relaxed);
    })();
    drop(out);
    let verified = finish.and_then(|_| if hex(&hash.finalize()) == expected { Ok(()) } else { Err("The download was damaged. Please try again.".to_string()) });
    let done = verified.and_then(|_| fs::rename(&part, dest).map_err(|e| e.to_string()));
    if done.is_err() {
        let _ = fs::remove_file(&part);
    }
    done
}

/// Puts the installer for `version` in `dir`, checked against the release's checksums. A copy
/// that is already there and correct is used again, so a learner never pays for it twice.
/// `progress(received, total)` is called often, on this thread. Blocks until finished.
pub fn fetch(version: &str, current: &str, dir: &Path, cancel: Arc<AtomicBool>, mut progress: impl FnMut(u64, u64)) -> Result<(), String> {
    if !acceptable(version, current) {
        return Err("That is not a newer version".into());
    }
    let target = detect().ok_or("Updates are not available on this system")?;
    let name = asset_name(target, version);
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    clear_old(dir, &name);
    let sums = get(&agent(), &release_url(target, version, "SHA256SUMS.txt"))?.body_mut().read_to_string().map_err(|e| e.to_string())?;
    let expected = parse_sums(&sums, &name).ok_or("The release has no checksum for this file")?;
    let dest = dir.join(&name);
    if dest.is_file() && sha256_file(&dest).ok().as_deref() == Some(&expected) {
        return Ok(());
    }
    let _ = fs::remove_file(&dest);
    // The transfer has its own thread, so a dead connection or Cancel cannot hang the caller.
    let (received, total) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)));
    let (tx, rx) = mpsc::channel();
    {
        let url = release_url(target, version, &name);
        let (dest, received, total, cancel) = (dest.clone(), received.clone(), total.clone(), cancel.clone());
        std::thread::Builder::new()
            .name("update-download".into())
            .spawn(move || {
                let _ = tx.send(download_verified(&url, &expected, &dest, &received, &total, &cancel));
            })
            .map_err(|e| e.to_string())?;
    }
    let (mut last, mut moved) = (0, Instant::now());
    loop {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err("The download stopped".into()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.into());
        }
        let now = received.load(Ordering::Relaxed);
        if now != last {
            last = now;
            moved = Instant::now();
            progress(now, total.load(Ordering::Relaxed));
        } else if moved.elapsed() > STALL {
            cancel.store(true, Ordering::Relaxed);
            return Err("The connection was lost. Please try again.".into());
        }
    }
}

/// Starts installing the checked file left in `dir` by [`fetch`].
pub fn apply(version: &str, dir: &Path) -> Result<Applied, String> {
    let target = detect().ok_or("Updates are not available on this system")?;
    let file = dir.join(asset_name(target, version));
    if !file.is_file() {
        return Err("The update has not been downloaded".into());
    }
    install(target, &file, dir)
}

#[cfg(windows)]
fn install(target: Target, file: &Path, _dir: &Path) -> Result<Applied, String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    let spawned = if target == Target::WinNsis {
        // /P shows progress and asks nothing, /R reopens FreeSheet, /UPDATE keeps the shortcuts.
        // /D keeps a custom install folder; NSIS needs it last, and unquoted.
        let dir = std::env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("No install folder")?.display().to_string();
        Command::new(file).raw_arg(format!("/P /R /UPDATE /D={dir}")).spawn()
    } else {
        Command::new("msiexec").arg("/i").arg(file).args(["/passive", "/norestart", "AUTOLAUNCHAPP=1"]).spawn()
    };
    spawned.map_err(|e| e.to_string())?;
    Ok(Applied::Exiting)
}

/// Waits for this process to end, then runs `script` ($2... are `args`), detached.
#[cfg(all(unix, not(target_os = "android")))]
fn after_exit(script: &str, args: &[&Path]) -> Result<(), String> {
    use std::process::{Command, Stdio};
    let full = format!("while kill -0 \"$1\" 2>/dev/null; do sleep 0.3; done; {script}");
    Command::new("/bin/sh").args(["-c", &full, "sh", &std::process::id().to_string()]).args(args).env_remove("LD_LIBRARY_PATH").env_remove("APPDIR").env_remove("APPIMAGE").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "android")))]
fn check_writable(folder: &Path) -> Result<(), String> {
    let probe = folder.join(format!(".freesheet-update-{}", std::process::id()));
    fs::File::create(&probe).map_err(|_| format!("FreeSheet cannot write to {}", folder.display()))?;
    let _ = fs::remove_file(probe);
    Ok(())
}

#[cfg(target_os = "macos")]
fn install(_target: Target, file: &Path, dir: &Path) -> Result<Applied, String> {
    use std::process::Command;
    // .../FreeSheet.app/Contents/MacOS/FreeSheet -> the .app folder.
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let bundle = exe.ancestors().nth(3).filter(|p| p.extension().is_some_and(|e| e == "app")).ok_or("FreeSheet is not running from an app bundle")?.to_path_buf();
    if bundle.to_string_lossy().contains("/AppTranslocation/") {
        return Err("Move FreeSheet to Applications first".into());
    }
    check_writable(bundle.parent().ok_or("No Applications folder")?)?;
    let unpacked = dir.join("unpacked");
    let _ = fs::remove_dir_all(&unpacked);
    let ok = Command::new("/usr/bin/ditto").arg("-x").arg("-k").arg(file).arg(&unpacked).status().map_err(|e| e.to_string())?.success();
    let new_app = unpacked.join("FreeSheet.app");
    if !ok || !new_app.is_dir() {
        return Err("Could not unpack the update".into());
    }
    // Keeps the old app until the new one is in place.
    after_exit(
        r#"mv "$2" "$2.old" || { open "$2"; exit 1; }; if mv "$3" "$2"; then rm -rf "$2.old"; xattr -dr com.apple.quarantine "$2" 2>/dev/null; else mv "$2.old" "$2"; fi; open "$2""#,
        &[&bundle, &new_app],
    )?;
    Ok(Applied::Exiting)
}

#[cfg(all(target_os = "linux", not(target_os = "android")))]
fn install(target: Target, file: &Path, _dir: &Path) -> Result<Applied, String> {
    use std::os::unix::fs::PermissionsExt;
    if target == Target::Deb {
        // Installing a .deb needs an administrator, so the system's software installer does it.
        std::process::Command::new("xdg-open").arg(file).spawn().map_err(|e| e.to_string())?;
        return Ok(Applied::Opened);
    }
    let image = std::path::PathBuf::from(std::env::var_os("APPIMAGE").ok_or("Not running as an AppImage")?);
    check_writable(image.parent().ok_or("No folder")?)?;
    let fresh = image.with_extension("AppImage.new");
    fs::copy(file, &fresh).map_err(|e| e.to_string())?;
    fs::set_permissions(&fresh, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    fs::rename(&fresh, &image).map_err(|e| e.to_string())?;
    after_exit(r#""$2" & "#, &[&image])?;
    Ok(Applied::Exiting)
}

/// Android installs through the system's package installer: see MainActivity.kt.
#[cfg(not(any(windows, target_os = "macos", all(target_os = "linux", not(target_os = "android")))))]
fn install(_target: Target, _file: &Path, _dir: &Path) -> Result<Applied, String> {
    Err("Updates are not available on this system".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_newer_plain_versions() {
        assert!(acceptable("1.3.1", "1.3.0"));
        assert!(acceptable("2.0.0", "1.9.9"));
        assert!(!acceptable("1.3.0", "1.3.0"));
        assert!(!acceptable("1.2.9", "1.3.0"));
        for bad in ["1.3", "1.3.1.2", "1.3.x", "../1.3.1", "1.3.1/../../x", "+1.3.1", ""] {
            assert!(!acceptable(bad, "1.0.0"), "{bad}");
        }
    }

    #[test]
    fn urls_follow_the_release_tags() {
        assert_eq!(release_url(Target::WinNsis, "1.3.1", &asset_name(Target::WinNsis, "1.3.1")), format!("{BASE}/v1.3.1/FreeSheet-Setup-1.3.1.exe"));
        assert_eq!(release_url(Target::Android, "1.3.1", &asset_name(Target::Android, "1.3.1")), format!("{BASE}/android-v1.3.1/FreeSheet-1.3.1-android.apk"));
        assert_eq!(asset_name(Target::MacArm, "1.3.1"), "FreeSheet-1.3.1-mac-arm64.zip");
        assert_eq!(asset_name(Target::AppImage, "1.3.1"), "FreeSheet-1.3.1-linux-x64.AppImage");
    }

    #[test]
    fn reads_checksum_lines() {
        let a = "a".repeat(64);
        let b = "B".repeat(64);
        let text = format!("{a}  FreeSheet-1.3.1.msi\n{b} *FreeSheet-Setup-1.3.1.exe\nnot a checksum  x\n");
        assert_eq!(parse_sums(&text, "FreeSheet-1.3.1.msi"), Some(a));
        assert_eq!(parse_sums(&text, "FreeSheet-Setup-1.3.1.exe"), Some("b".repeat(64)));
        assert_eq!(parse_sums(&text, "x"), None);
        assert_eq!(parse_sums(&text, "missing.exe"), None);
    }

    #[test]
    fn hashes_a_file() {
        let p = std::env::temp_dir().join(format!("fs-hash-{}", std::process::id()));
        fs::write(&p, b"abc").unwrap();
        assert_eq!(sha256_file(&p).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        fs::remove_file(p).unwrap();
    }
}
