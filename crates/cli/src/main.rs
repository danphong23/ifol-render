//! ifol-render CLI — headless rendering and export.
//!
//! Usage:
//!   ifol-render render --scene scene.json --fps 30 --output output.raw
//!   ifol-render info --scene scene.json
//!   ifol-render preview --scene scene.json --timestamp 5.0

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
        /// Path to scene description JSON.
        #[arg(short, long)]
        scene: PathBuf,

        /// Frames per second.
        #[arg(long, default_value = "30")]
        fps: f64,

        /// Output width (overrides scene setting).
        #[arg(long)]
        width: Option<u32>,

        /// Output height (overrides scene setting).
        #[arg(long)]
        height: Option<u32>,

        /// Output file path (use "pipe:1" for stdout).
        #[arg(short, long, default_value = "pipe:1")]
        output: String,
    },

    /// Print scene information.
    Info {
        /// Path to scene description JSON.
        #[arg(short, long)]
        scene: PathBuf,
    },

    /// Render a single frame at a specific timestamp.
    Preview {
        /// Path to scene description JSON.
        #[arg(short, long)]
        scene: PathBuf,

        /// Timestamp in seconds.
        #[arg(short, long)]
        timestamp: f64,

        /// Output PNG file path.
        #[arg(short, long, default_value = "preview.rgba")]
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
                let _pixels = renderer.render_frame(&world, &time);

                // TODO: Write pixels to output (file or stdout pipe)
                if frame % 100 == 0 {
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

            let mut world = desc.into_world();
            let mut time = ifol_render_core::time::TimeState::new(30.0);
            time.seek(timestamp);
            ifol_render_core::ecs::pipeline::run(&mut world, &time);

            let mut renderer =
                ifol_render_gpu::Renderer::new_headless(&ifol_render_core::scene::RenderSettings {
                    width: 1920,
                    height: 1080,
                    fps: 30.0,
                    duration: 0.0,
                    color_space: Default::default(),
                    output_color_space: Default::default(),
                });
            let pixels = renderer.render_frame(&world, &time);
            std::fs::write(&output, &pixels).expect("Failed to write output");
            log::info!("Preview saved: {} bytes", pixels.len());
        }
    }
}
