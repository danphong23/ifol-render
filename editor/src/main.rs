//! ifol-render Editor — standalone GUI for compositing and animation.

mod app;
mod ui;

fn main() -> eframe::Result<()> {
    // Suppress ALL wgpu/vulkan noise — only show our app logs
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or("ifol=info,wgpu=off,wgpu_hal=off,wgpu_core=off,naga=off"),
    )
    .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_title("ifol-render Editor"),
        ..Default::default()
    };

    eframe::run_native(
        "ifol-render Editor",
        options,
        Box::new(|_cc| Ok(Box::new(app::EditorApp::new()))),
    )
}
