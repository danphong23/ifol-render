//! ifol-render Editor — standalone GUI for compositing and animation.

mod app;
mod ui;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("ifol-render Editor"),
        ..Default::default()
    };

    eframe::run_native(
        "ifol-render Editor",
        options,
        Box::new(|_cc| Ok(Box::new(app::EditorApp::new()))),
    )
}
