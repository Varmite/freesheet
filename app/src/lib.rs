//! FreeSheet: offline CAPS study notes, practice questions and exams, written in Rust.
//!
//! Logic (ported from FreeSheet 1.1's JavaScript and checked against it by tests):
//! content, markup, marking, hints, sections, streak, course, quiz. Screens: ui (egui).
//! Sound: audio. Computers and phones: platform.

pub mod ai;
pub mod audio;
pub mod content;
pub mod course;
pub mod hints;
pub mod markup;
pub mod marking;
pub mod math;
pub mod model;
pub mod platform;
pub mod quiz;
pub mod sections;
pub mod store;
pub mod streak;
pub mod ui;
pub mod update;

/// Window options shared by the desktop builds.
#[cfg(not(target_os = "android"))]
pub fn run_desktop() -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title("FreeSheet")
        .with_app_id("freesheet")
        .with_inner_size([1200.0, 820.0])
        .with_min_inner_size([760.0, 560.0])
        .with_icon(std::sync::Arc::new(ui::icon()));
    let options = eframe::NativeOptions { viewport, renderer: eframe::Renderer::Glow, centered: true, ..Default::default() };
    eframe::run_native("FreeSheet", options, Box::new(|cc| Ok(Box::new(ui::FreeSheet::new(cc)))))
}

/// The Android entry point (GameActivity).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("FreeSheet"));
    platform::android::init(app.clone());
    let options = eframe::NativeOptions { android_app: Some(app), renderer: eframe::Renderer::Glow, ..Default::default() };
    if let Err(e) = eframe::run_native("FreeSheet", options, Box::new(|cc| Ok(Box::new(ui::FreeSheet::new(cc))))) {
        log::error!("FreeSheet could not start: {e}");
    }
}
