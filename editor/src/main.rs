//! ifol-render Editor — standalone GUI for scene editing and preview.
//!
//! Run with: `cargo run -p ifol-render-editor`

mod app;
mod ui;

fn main() -> eframe::Result {
    env_logger::init();
    log::info!("Starting ifol-render editor");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("ifol-render Editor"),
        ..Default::default()
    };

    eframe::run_native(
        "ifol-render Editor",
        options,
        Box::new(|cc| Ok(Box::new(app::EditorApp::new(cc)))),
    )
}
