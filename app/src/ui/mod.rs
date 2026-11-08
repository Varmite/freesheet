//! The screens, drawn with egui (no web view): a native, GPU-drawn interface.

mod ai_marking;
mod appearance;
pub mod anim;
pub mod app;
mod automation;
mod app_buddy;
pub mod blocks;
pub mod buddy;
pub mod buddy_art;
pub mod course_ui;
mod exams;
pub mod images;
mod progress_page;
mod questions;
pub mod rich;
mod study;
pub mod theme;
pub mod widgets;

/// The eframe application.
pub struct FreeSheet {
    pub app: app::App,
    focused: bool,
}

impl FreeSheet {
    pub fn new(cc: &eframe::CreationContext<'_>) -> FreeSheet {
        FreeSheet { app: app::App::new(&cc.egui_ctx), focused: true }
    }
}

impl eframe::App for FreeSheet {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Going to the background (another app, the home screen): save now, keeping an
        // exam in progress so it can be resumed.
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true) && !i.viewport().minimized.unwrap_or(false));
        if self.focused && !focused {
            self.app.on_leave();
        }
        self.focused = focused;
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let started = std::time::Instant::now();
        self.app.ui(ui);
        self.app.note_frame(started.elapsed());
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.app.on_leave();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.app.t.p.bg.to_normalized_gamma_f32()
    }
}

pub fn icon() -> egui::IconData {
    let img = image_png(include_bytes!("../../../build-resources/icon.png"));
    img.unwrap_or_default()
}

fn image_png(bytes: &[u8]) -> Option<egui::IconData> {
    let tree = resvg::tiny_skia::Pixmap::decode_png(bytes).ok()?;
    let (w, h) = (tree.width(), tree.height());
    // Demultiply for the window icon.
    let mut rgba = tree.data().to_vec();
    for px in rgba.chunks_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    Some(egui::IconData { rgba, width: w, height: h })
}
