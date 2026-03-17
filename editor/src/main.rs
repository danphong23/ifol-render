//! ifol-render Editor — standalone GUI for compositing and animation.

mod app;
mod ui;

fn main() -> eframe::Result<()> {
    // Only show warnings from our crates, suppress noisy wgpu Vulkan layer errors
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("ifol=info,eframe=warn,wgpu=warn,wgpu_hal=warn,naga=warn"),
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
