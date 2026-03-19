//! ifol-render Studio — Minimal Frame JSON Viewer
//!
//! Load a flat scene JSON → seek → preview → render/export.

use eframe::egui;
use std::path::PathBuf;

mod app;

fn main() {
    env_logger::init();

    let scene_path = std::env::args().nth(1).map(PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("ifol-render Studio"),
        ..Default::default()
    };

    eframe::run_native(
        "ifol-render Studio",
        options,
        Box::new(move |cc| Ok(Box::new(app::StudioApp::new(cc, scene_path)))),
    )
    .expect("Failed to start Studio");
}
