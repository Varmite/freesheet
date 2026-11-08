//! AI marking of written answers, on this computer only.
//!
//! Nothing here runs unless the learner has turned it on in Settings: the model is only
//! downloaded after they agree, it is only loaded when they ask for an answer to be
//! marked, and it is unloaded again shortly afterwards so it holds no memory while idle.
//! Answers never leave the computer; the only network use is the one download.
//!
//! The model does not award marks. It answers yes or no for each point in the memo (see
//! scheme.rs), and the marks are added up from those.

#[cfg(not(target_os = "android"))]
mod download;
#[cfg(not(target_os = "android"))]
mod engine;
pub mod scheme;

pub use scheme::Scheme;
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
#[cfg(not(target_os = "android"))]
use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, Mutex};

/// Phones are left out for now: the model needs more memory than many of them have.
pub const SUPPORTED: bool = cfg!(not(target_os = "android"));

pub struct ModelFile {
    pub name: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

/// Qwen3-1.7B (Apache-2.0), pinned to exact revisions. The Qwen3 sizes share a tokenizer.
/// The 0.6B model was tried first and gave marks to too many wrong answers.
pub const FILES: [ModelFile; 2] = [
    ModelFile {
        name: "tokenizer.json",
        url: "https://huggingface.co/Qwen/Qwen3-0.6B/resolve/c1899de289a04d12100db370d81485cdf75e47ca/tokenizer.json",
        sha256: "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4",
        size: 11_422_654,
    },
    ModelFile {
        name: "qwen3-1.7b-q4.gguf",
        url: "https://huggingface.co/unsloth/Qwen3-1.7B-GGUF/resolve/d7f544eead698dbd1f15126ef60b45a1e1933222/Qwen3-1.7B-Q4_K_M.gguf",
        sha256: "b139949c5bd74937ad8ed8c8cf3d9ffb1e99c866c823204dc42c0d91fa181897",
        size: 1_107_409_472,
    },
];

pub fn download_size() -> u64 {
    FILES.iter().map(|f| f.size).sum()
}

/// Where a mark goes once it is ready.
#[derive(Clone, Debug)]
pub enum Target {
    Topic { subject: String, topic: String, key: String, question: String },
    Exam { key: String, question: String },
}

#[derive(Clone, Debug)]
pub struct Job {
    /// The question card's key.
    pub key: String,
    pub target: Target,
    pub prompt: String,
    pub answer: String,
    pub scheme: Arc<Scheme>,
}

/// A finished marking, shown on the card.
#[derive(Clone, Debug)]
pub struct Review {
    pub answer: String,
    pub scheme: Arc<Scheme>,
    pub met: Vec<bool>,
}

pub struct Finished {
    pub job: Job,
    pub result: Result<Vec<bool>, String>,
}

pub enum DownloadEvent {
    Finished,
    Failed(String),
    Cancelled,
}

#[derive(Default)]
pub(crate) struct Queue {
    pub jobs: VecDeque<Job>,
    pub running: bool,
    pub stop: bool,
    pub done: Vec<Finished>,
}

pub(crate) struct Shared {
    pub q: Mutex<Queue>,
    pub wake: Condvar,
}

pub struct Marker {
    dir: PathBuf,
    shared: Arc<Shared>,
    /// Cards waiting for a mark. A result for anything else is dropped.
    pending: HashSet<String>,
    installed: bool,
    #[cfg(not(target_os = "android"))]
    download: Option<Arc<download::Progress>>,
}

impl Marker {
    pub fn new(dir: PathBuf) -> Marker {
        let installed = Self::check_installed(&dir);
        Marker {
            dir,
            shared: Arc::new(Shared { q: Mutex::new(Queue::default()), wake: Condvar::new() }),
            pending: HashSet::new(),
            installed,
            #[cfg(not(target_os = "android"))]
            download: None,
        }
    }

    fn check_installed(dir: &std::path::Path) -> bool {
        SUPPORTED && FILES.iter().all(|f| std::fs::metadata(dir.join(f.name)).is_ok_and(|m| m.len() == f.size))
    }

    pub fn installed(&self) -> bool {
        self.installed
    }

    pub fn is_pending(&self, key: &str) -> bool {
        self.pending.contains(key)
    }

    pub fn busy(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Queue an answer for marking, starting the worker if it isn't running.
    pub fn enqueue(&mut self, job: Job, ctx: &egui::Context) {
        if !self.installed || !self.pending.insert(job.key.clone()) {
            return;
        }
        let mut q = self.shared.q.lock().unwrap();
        q.jobs.push_back(job);
        q.stop = false;
        if q.running {
            self.shared.wake.notify_all();
        } else {
            q.running = true;
            drop(q);
            self.spawn_worker(ctx);
        }
    }

    #[cfg(not(target_os = "android"))]
    fn spawn_worker(&self, ctx: &egui::Context) {
        let (shared, dir, ctx2) = (self.shared.clone(), self.dir.clone(), ctx.clone());
        let spawned = std::thread::Builder::new().name("ai-marker".into()).spawn(move || engine::worker(shared, dir, ctx2));
        if let Err(e) = spawned {
            let mut q = self.shared.q.lock().unwrap();
            q.running = false;
            let jobs: Vec<Job> = q.jobs.drain(..).collect();
            q.done.extend(jobs.into_iter().map(|job| Finished { job, result: Err(format!("The marker could not start: {e}")) }));
        }
    }

    #[cfg(target_os = "android")]
    fn spawn_worker(&self, _ctx: &egui::Context) {}

    /// The learner will mark this one themselves.
    pub fn forget(&mut self, key: &str) {
        self.pending.remove(key);
        self.shared.q.lock().unwrap().jobs.retain(|j| j.key != key);
    }

    /// Stop now and unload the model.
    pub fn stop(&mut self) {
        self.pending.clear();
        let mut q = self.shared.q.lock().unwrap();
        q.jobs.clear();
        q.stop = true;
        self.shared.wake.notify_all();
    }

    /// Results that arrived since the last call and are still wanted.
    pub fn poll(&mut self) -> Vec<Finished> {
        let done: Vec<Finished> = std::mem::take(&mut self.shared.q.lock().unwrap().done);
        done.into_iter().filter(|f| self.pending.remove(&f.job.key)).collect()
    }

    /// Bytes downloaded so far, while a download runs.
    pub fn download_progress(&self) -> Option<u64> {
        #[cfg(not(target_os = "android"))]
        {
            self.download.as_ref().map(|p| p.got.load(Ordering::Relaxed))
        }
        #[cfg(target_os = "android")]
        {
            None
        }
    }

    pub fn start_download(&mut self, _ctx: &egui::Context) {
        #[cfg(not(target_os = "android"))]
        if self.download.is_none() {
            self.download = Some(download::start(self.dir.clone(), _ctx.clone()));
        }
    }

    pub fn cancel_download(&mut self) {
        #[cfg(not(target_os = "android"))]
        if let Some(p) = &self.download {
            p.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// How a running download ended, once it has.
    pub fn poll_download(&mut self) -> Option<DownloadEvent> {
        #[cfg(not(target_os = "android"))]
        {
            let outcome = self.download.as_ref()?.outcome.lock().unwrap().take()?;
            self.download = None;
            self.installed = Self::check_installed(&self.dir);
            Some(match outcome {
                Ok(()) => DownloadEvent::Finished,
                Err(e) if download::was_cancelled(&e) => DownloadEvent::Cancelled,
                Err(e) => DownloadEvent::Failed(e),
            })
        }
        #[cfg(target_os = "android")]
        {
            None
        }
    }

    /// Remove the downloaded model from this computer.
    pub fn delete_model(&mut self) -> std::io::Result<()> {
        self.stop();
        let r = if self.dir.exists() { std::fs::remove_dir_all(&self.dir) } else { Ok(()) };
        self.installed = Self::check_installed(&self.dir);
        r
    }
}
