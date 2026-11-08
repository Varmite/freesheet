//! Testing aids (used by the developer and the CI smoke tests, harmless otherwise):
//!   --data-dir <dir>   keep progress there instead of the real data folder
//!   --goto <steps>     do these steps, separated by ';' (see `step`)
//!   --shot <file.png>  then save a screenshot there and quit
//!   --wait <secs>      seconds before the screenshot (default 1.5)
//!   --perf <file>      write how long each frame took to build (microseconds) there

use super::app::{App, Mode, Tab, View};
use egui::Context;

pub struct Script {
    steps: Vec<String>,
    shot: Option<String>,
    wait: f64,
    next_at: f64,
    shot_asked: bool,
    saved: std::sync::Arc<std::sync::atomic::AtomicBool>,
    perf: Option<(String, Vec<u32>)>,
}

impl Script {
    pub fn from_args() -> Option<Script> {
        let args: Vec<String> = std::env::args().collect();
        let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
        if let Some(d) = get("--data-dir") {
            std::env::set_var("FREESHEET_DATA", d);
        }
        let goto = get("--goto");
        let shot = get("--shot");
        if goto.is_none() && shot.is_none() {
            return None;
        }
        Some(Script {
            steps: goto.map(|g| g.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default(),
            shot,
            wait: get("--wait").and_then(|w| w.parse().ok()).unwrap_or(1.5),
            next_at: 0.8,
            shot_asked: false,
            saved: Default::default(),
            perf: get("--perf").map(|f| (f, Vec::new())),
        })
    }
}

impl App {
    pub fn run_script(&mut self, ctx: &Context) {
        let Some(s) = &mut self.auto else { return };
        // The screenshot was saved: quit.
        if s.saved.load(std::sync::atomic::Ordering::SeqCst) {
            if let Some((file, frames)) = &s.perf {
                let lines: Vec<String> = frames.iter().map(u32::to_string).collect();
                let _ = std::fs::write(file, lines.join("\n"));
            }
            self.on_leave();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        if self.now < s.next_at {
            return;
        }
        if !s.steps.is_empty() {
            let step = s.steps.remove(0);
            s.next_at = self.now + 0.6;
            self.step(&step);
            return;
        }
        if !s.shot_asked {
            if s.shot.is_some() {
                if self.now >= s.next_at + s.wait - 0.6 {
                    s.shot_asked = true;
                    let path = s.shot.clone().unwrap();
                    let saved = s.saved.clone();
                    let c2 = ctx.clone();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                        save_png(&img, &path);
                        saved.store(true, std::sync::atomic::Ordering::SeqCst);
                        c2.request_repaint();
                    })));
                }
            } else {
                self.auto = None;
            }
        }
    }

    /// Testing: remember how long a frame took to build.
    pub fn note_frame(&mut self, took: std::time::Duration) {
        if let Some((_, frames)) = self.auto.as_mut().and_then(|s| s.perf.as_mut()) {
            frames.push(took.as_micros() as u32);
        }
    }

    fn step(&mut self, step: &str) {
        let (cmd, arg) = step.split_once(':').unwrap_or((step, ""));
        let parts: Vec<String> = arg.split('/').map(str::to_string).collect();
        let a = |i: usize| parts.get(i).cloned().unwrap_or_default();
        match cmd {
            "grade" => {
                if let Ok(g) = arg.parse() {
                    self.welcome = false;
                    self.change_grade(g);
                }
            }
            "study" => self.go(Tab::Study, View::Home),
            "exams" => self.go(Tab::Exams, View::Home),
            "progress" => self.set_tab(Tab::Progress),
            "settings" => self.open_settings(),
            "subject" => self.go(Tab::Study, View::Subject { subject: a(0) }),
            "topic" => {
                let mode = if a(2) == "questions" { Mode::Questions } else { Mode::Notes };
                self.go(Tab::Study, View::Topic { subject: a(0), topic: a(1), mode });
            }
            "reveal" => self.notes.shown = arg.parse().unwrap_or(1),
            "read" => self.start_reading(arg.parse().unwrap_or(0)),
            "checkall" => {
                if let View::Topic { subject, topic, .. } = self.view() {
                    let k = self.tkey(&subject, &topic);
                    self.check_all(&k);
                }
            }
            "answer" => {
                // answer:<question index>/<json value>
                if let View::Topic { subject, topic, .. } = self.view() {
                    if let Some(td) = self.load_topic(&subject, &topic) {
                        let i: usize = a(0).parse().unwrap_or(0);
                        if let (Some(q), Ok(v)) = (td.topic.questions.get(i), serde_json::from_str(&parts[1..].join("/"))) {
                            let k = self.tkey(&subject, &topic);
                            self.topic_prog_mut(&k).responses.insert(q.id.clone(), v);
                        }
                    }
                }
            }
            "press" => self.auto_check = arg.parse().ok(),
            "showcard" => {
                // showcard:<question index>
                if let View::Topic { subject, topic, .. } = self.view() {
                    if let Some(q) = self.load_topic(&subject, &topic).and_then(|td| td.topic.questions.get(arg.parse::<usize>().unwrap_or(0)).map(|q| q.id.clone())) {
                        let ck = format!("{}#{q}", self.tkey(&subject, &topic));
                        self.scroll_rect = self.card_rects.get(&ck).map(|r| (*r, egui::Align::Center));
                    }
                }
            }
            "aiconsent" => self.ai_ask_consent(),
            "examstart" => {
                // examstart:<subject>/<exam>
                self.start_exam(&a(0), &a(1), None, false);
            }
            "examwrite" => {
                // examwrite:<text>: answers every written question in the running exam
                if let Some(e) = &mut self.session {
                    let exam = e.exam.clone();
                    for q in exam.sections.iter().flat_map(|s| &s.questions).filter(|q| q.kind == "open") {
                        e.responses.insert(q.id.clone(), serde_json::json!({ "text": arg }));
                    }
                }
            }
            "examsubmit" => self.submit_exam(false),
            "aidownload" => {
                let ctx = self.ctx.clone();
                self.ai.start_download(&ctx);
            }
            "hint" => {
                self.buddy_hint();
            }
            "course" => self.go(Tab::Study, View::Course { course: a(0) }),
            "lesson" => self.go(Tab::Study, View::Lesson { course: a(0), lesson: a(1), run: 1 }),
            "list" => self.go(Tab::Exams, View::List { subject: a(0) }),
            "intro" => self.go(Tab::Exams, View::Intro { subject: a(0), exam: a(1) }),
            "setup" => self.open_setup(true, ""),
            "streak" => self.show_streak(),
            "buddy" => self.buddy_menu(),
            "toast" => self.toast(arg.to_string()),
            "theme" => self.p.settings.theme = Some(arg.to_string()),
            "accent" => self.p.settings.accent = Some(arg.to_string()),
            "spacing" => self.p.settings.line_spacing = Some(arg.to_string()),
            "width" => self.p.settings.reading_width = Some(arg.to_string()),
            "updatecheck" => self.check_updates_now(),
            "updatefake" => self.update.pretend(arg, "https://github.com/Varmite/freesheet/releases/latest"),
            "scroll" => self.scroll_rect = Some((egui::Rect::from_min_size(egui::pos2(0.0, arg.parse().unwrap_or(0.0)), egui::vec2(1.0, 1.0)), egui::Align::TOP)),
            _ => log::warn!("unknown step {step}"),
        }
    }
}

fn save_png(img: &egui::ColorImage, path: &str) {
    let [w, h] = img.size;
    let mut data = Vec::with_capacity(w * h * 4);
    for c in &img.pixels {
        data.extend_from_slice(&c.to_array());
    }
    if let Some(size) = resvg::tiny_skia::IntSize::from_wh(w as u32, h as u32) {
        if let Some(pm) = resvg::tiny_skia::Pixmap::from_vec(data, size) {
            let _ = pm.save_png(path);
        }
    }
}
