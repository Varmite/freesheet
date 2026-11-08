//! Connects AI marking (crate::ai) to the screens: which cards can use it, putting its
//! marks where the learner's own marks go, and the switch in Settings.

use super::app::{App, DialogAction};
use super::questions::Ai;
use super::widgets::{self, btn, Kind};
use crate::ai::{self, DownloadEvent, Job, Review, Scheme, Target};
use crate::marking;
use crate::model::{Num, Question};
use egui::{vec2, Context, Ui};
use serde_json::Value;
use std::sync::Arc;

fn answer_text(r: Option<&Value>) -> String {
    r.and_then(|r| r.get("text")).and_then(Value::as_str).unwrap_or("").trim().to_string()
}

fn megabytes(bytes: u64) -> u64 {
    (bytes + 500_000) / 1_000_000
}

impl App {
    /// The learner has allowed it on this computer and the model is here.
    pub fn ai_on(&self) -> bool {
        ai::SUPPORTED && self.p.settings.ai_marking == Some(true) && self.ai.installed()
    }

    fn ai_scheme(&mut self, key: &str, q: &Question) -> Option<Arc<Scheme>> {
        if q.kind != "open" {
            return None;
        }
        self.ai_schemes.entry(key.to_string()).or_insert_with(|| ai::scheme::parse(q.memo.as_deref().unwrap_or(""), marking::marks_of(q)).map(Arc::new)).clone()
    }

    /// What this question card should offer.
    pub fn ai_card(&mut self, key: &str, q: &Question, response: Option<&Value>) -> Ai {
        if !self.ai_on() || q.kind != "open" {
            return Ai::Off;
        }
        if self.ai.is_pending(key) {
            return Ai::Marking;
        }
        let text = answer_text(response);
        if let Some(r) = self.ai_reviews.get(key).filter(|r| r.answer == text) {
            return Ai::Done(r.clone());
        }
        if let Some(m) = self.ai_failed.get(key) {
            return Ai::Failed(m.clone());
        }
        if text.is_empty() || self.ai_scheme(key, q).is_none() {
            return Ai::Off;
        }
        Ai::Ready
    }

    pub fn ai_start(&mut self, key: &str, target: Target, q: &Question, response: Option<&Value>) {
        let Some(scheme) = self.ai_scheme(key, q) else { return };
        let answer = answer_text(response);
        if answer.is_empty() {
            return;
        }
        self.ai_failed.remove(key);
        self.flush(); // marking is heavy work: keep everything safe first
        self.ai.enqueue(Job { key: key.to_string(), target, prompt: q.prompt.clone(), answer, scheme }, &self.ctx);
    }

    /// The learner chose to mark this one themselves.
    pub fn ai_forget(&mut self, key: &str) {
        self.ai.forget(key);
        self.ai_reviews.remove(key);
    }

    pub fn tick_ai(&mut self, ctx: &Context) {
        match self.ai.poll_download() {
            Some(DownloadEvent::Finished) => {
                self.p.settings.ai_marking = Some(true);
                self.save();
                self.toast("AI marking is ready.");
            }
            Some(DownloadEvent::Failed(e)) => self.toast(e),
            Some(DownloadEvent::Cancelled) | None => {}
        }
        for f in self.ai.poll() {
            let job = f.job;
            match f.result {
                Ok(met) => self.ai_apply(job, met),
                Err(e) => {
                    self.ai_failed.insert(job.key, e);
                }
            }
        }
        if self.ai.busy() {
            // Keeps the "marking" dots moving; results also wake the app themselves.
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    /// Put a finished marking where a self-mark goes, unless things moved on meanwhile.
    fn ai_apply(&mut self, job: Job, met: Vec<bool>) {
        let score = job.scheme.score(&met);
        let review = Arc::new(Review { answer: job.answer.clone(), scheme: job.scheme.clone(), met });
        match &job.target {
            Target::Topic { subject, topic, key, question } => {
                let tp = self.topic_prog(key);
                let still = tp.is_some_and(|tp| tp.checked.get(question).copied().unwrap_or(false) && answer_text(tp.responses.get(question)) == job.answer && tp.self_marks.get(question).copied().flatten().is_none());
                if !still {
                    return;
                }
                self.topic_prog_mut(key).self_marks.insert(question.clone(), Some(Num(score)));
                if let Some(td) = self.load_topic(subject, topic) {
                    if let Some(q) = td.topic.questions.iter().find(|q| q.id == *question) {
                        let (_, voices) = crate::audio::sfx::mark(score, marking::marks_of(q));
                        self.sound.play_voices("mark", voices);
                    }
                    self.record_if_complete(&td.topic, key);
                }
                self.save();
            }
            Target::Exam { key, question } => {
                let Some(e) = self.session.as_mut().filter(|e| e.key == *key && e.marking) else { return };
                if answer_text(e.responses.get(question)) != job.answer || e.self_marks.get(question).copied().flatten().is_some() {
                    return;
                }
                e.self_marks.insert(question.clone(), Some(Num(score)));
                self.write_draft();
                self.save();
            }
        }
        self.ai_reviews.insert(job.key, review);
    }

    pub fn ai_ask_consent(&mut self) {
        let size = megabytes(ai::download_size());
        self.dialog(
            "Turn on AI marking?",
            vec![
                format!("<p>FreeSheet needs to download an AI model once. It is {size} MB, so you may want to use Wi-Fi. After that it works offline.</p>"),
                "<p>The model runs on this computer only. Your answers are not sent anywhere.</p>".into(),
                "<p>While it marks, it uses nearly 5 GB of memory, so it needs a computer with at least 8 GB (16 GB is better). The memory is freed about 30 seconds after it finishes.</p>".into(),
                "<p>It can make mistakes, so check each mark against the memo. You can always change it.</p>".into(),
            ],
            &format!("Download {size} MB"),
            Some("Not now"),
            false,
            DialogAction::AiDownload,
        );
    }

    /// The "AI marking" part of Settings (computers only).
    pub fn ai_settings(&mut self, ui: &mut Ui) {
        if !ai::SUPPORTED {
            return;
        }
        let t = self.t;
        let p = t.p;
        let size = megabytes(ai::download_size());
        widgets::h2(ui, &t, "AI marking");
        widgets::muted(ui, &t, "Marks your written answers instead of only showing you the memo. It is off until you turn it on, it runs only when you ask it to mark an answer, and your answers never leave this computer.", 0.9);
        ui.add_space(t.px(0.6));
        if let Some(got) = self.ai.download_progress() {
            ui.horizontal_wrapped(|ui| {
                widgets::plain(ui, &t, &format!("Downloading: {} of {size} MB", megabytes(got)), 1.0, super::theme::W::Regular, p.text);
                if btn("Cancel").small().show(ui, &t).clicked() {
                    self.ai.cancel_download();
                }
            });
            return;
        }
        let on = self.ai_on();
        if let Some(i) = widgets::segmented(ui, &t, egui::Id::new("aimark"), &["On", "Off"], if on { 0 } else { 1 }, true) {
            let turn_on = i == 0;
            if turn_on && !self.ai.installed() {
                self.ai_ask_consent();
            } else {
                self.sound.play(if turn_on { "toggleOn" } else { "toggleOff" });
                self.p.settings.ai_marking = Some(turn_on);
                if !turn_on {
                    self.ai.stop();
                }
                self.save();
            }
        }
        if self.ai.installed() {
            ui.add_space(t.px(0.5));
            widgets::wrap(ui, vec2(t.px(0.6), t.px(0.6)), |ui| {
                if btn(&format!("Delete the AI model ({size} MB)")).kind(Kind::DangerOutline).small().show(ui, &t).clicked() {
                    self.p.settings.ai_marking = Some(false);
                    self.save();
                    let msg = if self.ai.delete_model().is_ok() { "AI model deleted." } else { "The AI model could not be deleted." };
                    self.toast(msg);
                }
            });
        }
    }
}
