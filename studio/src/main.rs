//! ifol-render-studio — Standalone GUI editor.
//!
//! A third-party consumer that only depends on ifol-render-core.

pub mod app;
pub mod panels;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "ifol-render studio",
        options,
        Box::new(|_cc| Ok(Box::new(app::EditorApp::new()))),
    )
}
