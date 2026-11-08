//! Fetches the model files, only after the learner has agreed to it. An interrupted
//! download carries on from where it stopped, and every file is checked against its
//! SHA-256 before it is used.

use super::{ModelFile, FILES};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CANCELLED: &str = "cancelled";

#[derive(Default)]
pub struct Progress {
    pub got: AtomicU64,
    pub cancel: AtomicBool,
    pub outcome: Mutex<Option<Result<(), String>>>,
}

pub fn start(dir: PathBuf, ctx: egui::Context) -> Arc<Progress> {
    let p = Arc::new(Progress::default());
    let p2 = p.clone();
    let spawned = std::thread::Builder::new().name("ai-download".into()).spawn(move || {
        let r = fetch_all(&dir, &p2, &ctx);
        *p2.outcome.lock().unwrap() = Some(r);
        ctx.request_repaint();
    });
    if spawned.is_err() {
        *p.outcome.lock().unwrap() = Some(Err("Could not start the download.".into()));
    }
    p
}

fn fetch_all(dir: &Path, p: &Progress, ctx: &egui::Context) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_connect(Some(Duration::from_secs(30))).build().into();
    for f in &FILES {
        fetch(&agent, f, dir, p, ctx)?;
    }
    Ok(())
}

fn fetch(agent: &ureq::Agent, f: &ModelFile, dir: &Path, p: &Progress, ctx: &egui::Context) -> Result<(), String> {
    let dest = dir.join(f.name);
    if fs::metadata(&dest).is_ok_and(|m| m.len() == f.size) {
        p.got.fetch_add(f.size, Ordering::Relaxed);
        return Ok(());
    }
    let part = dir.join(format!("{}.part", f.name));
    let mut have = fs::metadata(&part).map_or(0, |m| m.len());
    if have > f.size {
        fs::remove_file(&part).map_err(|e| e.to_string())?;
        have = 0;
    }
    if have < f.size {
        let mut req = agent.get(f.url);
        if have > 0 {
            req = req.header("Range", format!("bytes={have}-"));
        }
        let mut resp = req.call().map_err(|e| format!("Could not reach the download: {e}"))?;
        // A server that ignores the range sends the whole file again.
        if resp.status() != 206 {
            have = 0;
        }
        let mut out = OpenOptions::new().create(true).write(true).append(have > 0).truncate(have == 0).open(&part).map_err(|e| e.to_string())?;
        p.got.fetch_add(have, Ordering::Relaxed);
        let mut body = resp.body_mut().as_reader();
        let mut buf = vec![0u8; 128 * 1024];
        let mut painted = Instant::now();
        loop {
            if p.cancel.load(Ordering::Relaxed) {
                return Err(CANCELLED.into());
            }
            let n = body.read(&mut buf).map_err(|e| format!("The download stopped: {e}"))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| format!("Could not save the download: {e}"))?;
            p.got.fetch_add(n as u64, Ordering::Relaxed);
            if painted.elapsed() > Duration::from_millis(250) {
                painted = Instant::now();
                ctx.request_repaint();
            }
        }
        out.flush().map_err(|e| e.to_string())?;
    } else {
        p.got.fetch_add(have, Ordering::Relaxed);
    }
    if fs::metadata(&part).map_or(0, |m| m.len()) != f.size {
        return Err("The download stopped early. Try again to carry on from there.".into());
    }
    if sha256(&part).map_err(|e| e.to_string())? != f.sha256 {
        let _ = fs::remove_file(&part);
        return Err("The downloaded file was damaged, so it was deleted. Please try again.".into());
    }
    fs::rename(&part, &dest).map_err(|e| e.to_string())
}

fn sha256(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

pub fn was_cancelled(message: &str) -> bool {
    message == CANCELLED
}
