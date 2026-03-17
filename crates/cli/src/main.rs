//! ifol-render CLI — headless rendering and export.
//!
//! Usage:
//!   ifol-render render --scene scene.json --fps 30 --output output.raw
//!   ifol-render info --scene scene.json
//!   ifol-render preview --scene scene.json --timestamp 5.0 --output preview.png

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ifol-render")]
#[command(about = "Standalone GPU rendering engine for video compositing and animation")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Render all frames to raw RGBA output.
    Render {
        #[arg(short, long)]
        scene: PathBuf,
        #[arg(long, default_value = "30")]
        fps: f64,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(short, long, default_value = "pipe:1")]
        output: String,
    },

    /// Print scene information.
    Info {
        #[arg(short, long)]
        scene: PathBuf,
    },

    /// Render a single frame at a specific timestamp.
    Preview {
        #[arg(short, long)]
        scene: PathBuf,
        #[arg(short, long)]
        timestamp: f64,
        #[arg(short, long, default_value = "preview.png")]
        output: PathBuf,
    },
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Render {
            scene,
            fps,
            width,
            height,
            output,
        } => {
            log::info!("Rendering scene: {:?} at {fps}fps", scene);
            let json = std::fs::read_to_string(&scene).expect("Failed to read scene file");

            let scene_desc = ifol_render_core::scene::SceneDescription::from_json(&json)
                .expect("Failed to parse scene JSON");

            let settings = ifol_render_core::scene::RenderSettings {
                width: width.unwrap_or(scene_desc.settings.width),
                height: height.unwrap_or(scene_desc.settings.height),
                fps,
                ..scene_desc.settings.clone()
            };

            let mut world = scene_desc.into_world();
            let mut time = ifol_render_core::time::TimeState::new(fps);
            let mut renderer = ifol_render_gpu::Renderer::new_headless(&settings);

            let total_frames = (settings.duration * fps) as u64;

            for frame in 0..total_frames {
                time.seek(frame as f64 / fps);
                ifol_render_core::ecs::pipeline::run(&mut world, &time);
                let pixels = renderer.render_frame(&world, &time);

                if output == "pipe:1" {
                    use std::io::Write;
                    std::io::stdout().write_all(&pixels).unwrap();
                }

                if frame % 30 == 0 {
                    log::info!("Frame {}/{}", frame, total_frames);
                }
            }

            let _ = output;
            log::info!("Render complete: {} frames", total_frames);
        }

        Commands::Info { scene } => {
            let json = std::fs::read_to_string(&scene).expect("Failed to read scene file");
            let desc = ifol_render_core::scene::SceneDescription::from_json(&json)
                .expect("Failed to parse scene JSON");

            println!("Scene: v{}", desc.version);
            println!(
                "Resolution: {}x{}",
                desc.settings.width, desc.settings.height
            );
            println!("FPS: {}", desc.settings.fps);
            println!("Duration: {}s", desc.settings.duration);
            println!("Entities: {}", desc.entities.len());
            for entity in &desc.entities {
                println!("  - {} (components: {})", entity.id, count_components(entity));
            }
            println!("Custom shaders: {}", desc.shaders.len());
        }

        Commands::Preview {
            scene,
            timestamp,
            output,
        } => {
            log::info!("Preview at {timestamp}s → {:?}", output);
            let json = std::fs::read_to_string(&scene).expect("Failed to read scene file");
            let desc = ifol_render_core::scene::SceneDescription::from_json(&json)
                .expect("Failed to parse scene JSON");

            let settings = desc.settings.clone();
            let mut world = desc.into_world();
            let mut time = ifol_render_core::time::TimeState::new(settings.fps);
            time.seek(timestamp);
            ifol_render_core::ecs::pipeline::run(&mut world, &time);

            let mut renderer = ifol_render_gpu::Renderer::new_headless(&settings);

            // Load image sources
            for entity in &world.entities {
                if let Some(ref img) = entity.components.image_source {
                    if let Err(e) = renderer.load_image(&entity.id, &img.path) {
                        log::warn!("{}", e);
                    }
                }
            }

            let pixels = renderer.render_frame(&world, &time);

            let out_path = output.to_str().unwrap();
            if out_path.ends_with(".png") {
                ifol_render_gpu::Renderer::save_png(
                    &pixels,
                    settings.width,
                    settings.height,
                    out_path,
                )
                .expect("Failed to save PNG");
            } else {
                std::fs::write(&output, &pixels).expect("Failed to write output");
            }
            println!(
                "Preview saved: {} ({}x{}, {:.2}s)",
                out_path, settings.width, settings.height, timestamp
            );
        }
    }
}

fn count_components(entity: &ifol_render_core::ecs::Entity) -> usize {
    let c = &entity.components;
    let mut n = 0;
    if c.video_source.is_some() { n += 1; }
    if c.image_source.is_some() { n += 1; }
    if c.text_source.is_some() { n += 1; }
    if c.color_source.is_some() { n += 1; }
    if c.timeline.is_some() { n += 1; }
    if c.transform.is_some() { n += 1; }
    if c.opacity.is_some() { n += 1; }
    if c.color.is_some() { n += 1; }
    if c.animation.is_some() { n += 1; }
    if c.effects.is_some() { n += 1; }
    n
}
