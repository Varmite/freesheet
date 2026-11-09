//! Settings for how FreeSheet looks, and the "new version" card.

use super::anim::Ease;
use super::app::{App, Tab, View};
use super::rich::{self, Style};
use super::theme::{self, Palette, W};
use super::widgets::{self, btn, radius};
use crate::update::{self, Job, State};
use egui::{pos2, vec2, Align2, Color32, Id, Order, Rect, Sense, Stroke, StrokeKind, Ui};

/// A small drawing of a theme: a page, a card, a line of text and a button.
fn paint_preview(ui: &Ui, r: Rect, p: &Palette) {
    let painter = ui.painter();
    painter.rect_filled(r, radius(9.0), p.bg);
    let card = Rect::from_min_size(r.min + vec2(9.0, 9.0), vec2(r.width() - 18.0, 13.0));
    painter.rect_filled(card, radius(4.0), p.surface2);
    let line = Rect::from_min_size(pos2(card.left(), card.bottom() + 7.0), vec2(r.width() * 0.48, 4.0));
    painter.rect_filled(line, radius(2.0), p.text);
    let pill = Rect::from_min_size(pos2(card.left(), line.bottom() + 6.0), vec2(26.0, 9.0));
    painter.rect_filled(pill, radius(4.0), p.btn);
}

impl App {
    fn theme_tile(&mut self, ui: &mut Ui, id: &str, label: &str, selected: bool, system_dark: bool) -> bool {
        let t = self.t;
        let p = t.p;
        let size = vec2(t.px(5.3), t.px(3.9) + t.px(1.4));
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered() && t.motion, 0.12);
        let press = if resp.is_pointer_button_down_on() { 0.97 } else { 1.0 };
        let preview = Rect::from_min_size(rect.min, vec2(rect.width(), t.px(3.9)));
        let preview = Rect::from_center_size(preview.center() - vec2(0.0, 1.5 * hover), preview.size() * press);
        if id == "system" {
            // Automatic: the light and the dark look, side by side.
            let left = Rect::from_min_max(preview.min, pos2(preview.center().x, preview.bottom()));
            let right = Rect::from_min_max(pos2(preview.center().x, preview.top()), preview.max);
            let (light, _) = theme::named("light", false);
            let (dark, _) = theme::named("dark", true);
            ui.painter().rect_filled(left, egui::CornerRadius { nw: 9, sw: 9, ne: 0, se: 0 }, light.bg);
            ui.painter().rect_filled(right, egui::CornerRadius { nw: 0, sw: 0, ne: 9, se: 9 }, dark.bg);
            let bar = |x: Rect, c: Color32| ui.painter().rect_filled(Rect::from_min_size(x.min + vec2(9.0, 9.0), vec2(x.width() - 18.0, 13.0)), radius(4.0), c);
            bar(left, light.surface2);
            bar(right, dark.surface2);
        } else {
            let (palette, _) = theme::named(id, system_dark);
            paint_preview(ui, preview, &palette);
        }
        let stroke = if selected { Stroke::new(2.0, p.accent) } else { Stroke::new(1.0, theme::mix(p.border_strong, p.border, hover)) };
        ui.painter().rect_stroke(preview, radius(9.0), stroke, StrokeKind::Inside);
        let st = Style::new(t.px(0.82), if selected { p.text } else { p.muted }, p.muted).weight(if selected { W::Semi } else { W::Regular }).center().line(1.2);
        let laid = rich::layout(ui.ctx(), label, &st, rect.width());
        rich::paint(ui.painter(), &laid, pos2(rect.left(), preview.bottom() + t.px(0.3)), &st);
        resp.clicked()
    }

    pub fn appearance_settings(&mut self, ui: &mut Ui) {
        let t = self.t;
        let system_dark = match ui.ctx().system_theme() {
            Some(egui::Theme::Dark) => true,
            Some(egui::Theme::Light) => false,
            None => t.dark,
        };
        widgets::h2(ui, &t, "Appearance");
        widgets::label(ui, &t, "Theme");
        ui.add_space(t.px(0.2));
        let cur = self.p.settings.theme.clone().unwrap_or_else(|| "system".into());
        let mut picked = None;
        widgets::wrap(ui, vec2(t.px(0.7), t.px(0.7)), |ui| {
            for (id, label) in theme::THEMES {
                if self.theme_tile(ui, id, label, cur == id, system_dark) && cur != id {
                    picked = Some(id);
                }
            }
        });
        if let Some(id) = picked {
            self.sound.play("select");
            self.p.settings.theme = Some(id.into());
            self.save();
        }
        ui.add_space(t.px(0.8));
        widgets::label(ui, &t, "Accent colour");
        ui.add_space(t.px(0.2));
        let locked = cur == "contrast";
        let accent = self.p.settings.accent.clone().unwrap_or_else(|| "default".into());
        let mut chosen: Option<String> = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(t.px(0.5), t.px(0.5));
            let own = theme::named(&cur, system_dark).0.accent;
            let list = std::iter::once(("default", "Theme colour", own)).chain(theme::ACCENTS.iter().map(|a| (a.0, a.1, theme::accent_colour(a.0, t.dark).unwrap_or(own))));
            for (id, label, color) in list {
                let (rect, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), if locked { Sense::hover() } else { Sense::click() });
                let on = accent == id;
                let alpha = if locked { 0.35 } else { 1.0 };
                let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered() && !locked && t.motion, 0.12);
                ui.painter().circle_filled(rect.center(), 12.0 + 1.5 * hover, color.gamma_multiply(alpha));
                if on {
                    ui.painter().circle_stroke(rect.center(), 15.0, Stroke::new(2.0, t.p.accent.gamma_multiply(alpha)));
                }
                if id == "default" {
                    ui.painter().circle_stroke(rect.center(), 5.0, Stroke::new(1.5, Color32::WHITE.gamma_multiply(alpha)));
                }
                if resp.on_hover_text(label).clicked() && !on && !locked {
                    chosen = Some(id.to_string());
                }
            }
        });
        if locked {
            widgets::muted(ui, &t, "High contrast keeps its own colours so everything stays easy to read.", 0.85);
        }
        if let Some(id) = chosen {
            self.sound.play("select");
            self.p.settings.accent = Some(id);
            self.save();
        }
    }

    pub fn check_updates_now(&mut self) {
        let ctx = self.ctx.clone();
        self.update.start(&ctx);
    }

    pub fn update_settings(&mut self, ui: &mut Ui) {
        let t = self.t;
        let p = t.p;
        if crate::platform::is_android() {
            ui.add_space(t.px(1.5));
            widgets::muted(ui, &t, &format!("FreeSheet {}", update::CURRENT), 0.85);
            return;
        }
        widgets::h2(ui, &t, "Updates");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(t.px(0.7), t.px(0.5));
            let on = self.p.settings.check_updates != Some(false);
            if let Some(i) = widgets::segmented(ui, &t, Id::new("upd"), &["On", "Off"], if on { 0 } else { 1 }, true) {
                self.sound.play(if i == 0 { "toggleOn" } else { "toggleOff" });
                self.p.settings.check_updates = Some(i == 0);
                self.save();
            }
            widgets::muted(ui, &t, "Look for a new version each time FreeSheet opens.", 0.9);
        });
        ui.add_space(t.px(0.3));
        let state = self.update.state();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(t.px(0.7), t.px(0.5));
            let status = match &state {
                State::Idle => format!("FreeSheet {} is installed.", update::CURRENT),
                State::Checking => "Checking for updates…".to_string(),
                State::UpToDate => format!("FreeSheet {} is up to date.", update::CURRENT),
                State::Available { version, .. } => format!("FreeSheet {version} is available. You have {}.", update::CURRENT),
                State::Failed => "Could not check for updates. Are you online?".to_string(),
            };
            widgets::plain(ui, &t, &status, 0.95, W::Regular, p.text);
            if let State::Available { version, url } = &state {
                if !self.update_job(ui, version, url) && btn("Update now").primary().small().icon("download").show(ui, &t).clicked() {
                    self.installer.start(ui.ctx(), version);
                }
            } else if btn("Check now").small().enabled(state != State::Checking).show(ui, &t).clicked() {
                self.check_updates_now();
            }
        });
    }

    /// Download progress and what can be done next. False when no update is under way.
    fn update_job(&mut self, ui: &mut Ui, version: &str, url: &str) -> bool {
        let t = self.t;
        let p = t.p;
        match self.installer.job() {
            Job::None => false,
            Job::Downloading { received, total } => {
                let mb = |n: u64| n / 1_048_576;
                let text = if total > 0 {
                    format!("Downloading the update: {}% ({} of {} MB)", received * 100 / total, mb(received), mb(total))
                } else {
                    "Downloading the update…".to_string()
                };
                widgets::muted(ui, &t, &text, 0.85);
                if btn("Cancel").small().show(ui, &t).clicked() {
                    self.installer.cancel();
                }
                true
            }
            Job::Ready => {
                widgets::plain(ui, &t, &format!("FreeSheet {version} is ready. FreeSheet will close and reopen."), 0.9, W::Regular, p.text);
                ui.horizontal(|ui| {
                    if btn("Restart and update").primary().small().show(ui, &t).clicked() {
                        self.save();
                        if self.installer.install(version) {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                    if btn("Later").small().show(ui, &t).clicked() {
                        self.installer.reset();
                        self.p.settings.update_dismissed = Some(version.to_string());
                        self.save();
                    }
                });
                true
            }
            Job::Failed => {
                widgets::plain(ui, &t, "The update could not be finished. Check your connection and try again.", 0.9, W::Regular, p.text);
                ui.horizontal(|ui| {
                    if btn("Try again").primary().small().show(ui, &t).clicked() {
                        self.installer.start(ui.ctx(), version);
                    }
                    if btn("Open download page").small().show(ui, &t).clicked() {
                        update::open_page(url);
                    }
                });
                true
            }
        }
    }

    /// A quiet card in the corner when a new version is out: never during an exam, and
    /// once closed it stays closed until an even newer version appears.
    pub fn update_card(&mut self, ctx: &egui::Context, gap: f32) {
        let State::Available { version, url } = self.update.state() else {
            self.update_card_at = None;
            return;
        };
        let writing = self.tab == Tab::Exams && matches!(self.view(), View::Run { .. } | View::Mark { .. });
        if writing || self.welcome || self.setup.is_some() || (self.installer.job() == Job::None && self.p.settings.update_dismissed.as_deref() == Some(version.as_str())) {
            self.update_card_at = None;
            return;
        }
        let t = self.t;
        let p = t.p;
        let at = *self.update_card_at.get_or_insert(self.now);
        let age = (self.now - at) as f32;
        let k = if t.motion { Ease::Spring.at((age / 0.42).min(1.0)) } else { 1.0 };
        if t.motion && age < 0.42 {
            ctx.request_repaint();
        }
        let screen = ctx.content_rect();
        let mut later = false;
        egui::Area::new(Id::new("update-card")).order(Order::Middle).pivot(Align2::LEFT_BOTTOM).fixed_pos(pos2(screen.left() + 16.0, screen.bottom() - 16.0 - gap + 18.0 * (1.0 - k))).show(ctx, |ui| {
            ui.set_opacity(if t.motion { (age / 0.2).min(1.0) } else { 1.0 });
            egui::Frame::new()
                .fill(p.bg)
                .stroke(Stroke::new(1.0, p.border_strong))
                .corner_radius(radius(18.0))
                .inner_margin(egui::Margin::symmetric(16, 13))
                .shadow(egui::epaint::Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(46) })
                .show(ui, |ui| {
                    ui.set_max_width((screen.width() - 64.0).min(300.0));
                    ui.spacing_mut().item_spacing.y = 2.0;
                    widgets::plain(ui, &t, &format!("FreeSheet {version} is available"), 1.0, W::Semi, p.text);
                    if !self.update_job(ui, &version, &url) {
                        widgets::muted(ui, &t, &format!("You have {}. Your progress stays when you update.", update::CURRENT), 0.85);
                        ui.add_space(t.px(0.45));
                        ui.horizontal(|ui| {
                            if btn("Update now").primary().small().icon("download").show(ui, &t).clicked() {
                                self.installer.start(ui.ctx(), &version);
                            }
                            if btn("Later").small().show(ui, &t).clicked() {
                                later = true;
                            }
                        });
                    }
                });
        });
        if later {
            self.p.settings.update_dismissed = Some(version);
            self.save();
        }
    }
}
