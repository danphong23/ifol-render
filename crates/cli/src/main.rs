//! ifol-render CLI — headless rendering and export.
//!
//! A third-party consumer that only depends on ifol-render-core.
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

    /// Export scene to video file using FFmpeg.
    Export {
        /// Path to scene JSON file.
        #[arg(short, long)]
        scene: PathBuf,
        /// Output video file path.
        #[arg(short, long, default_value = "output.mp4")]
        output: String,
        /// Video codec: h264, h265, vp9, prores, png.
        #[arg(short, long, default_value = "h264")]
        codec: String,
        /// Constant Rate Factor (quality). Lower = better quality. Range: 0–51.
        #[arg(long, default_value = "23")]
        crf: u32,
        /// Override FPS.
        #[arg(long)]
        fps: Option<f64>,
        /// Override width.
        #[arg(long)]
        width: Option<u32>,
        /// Override height.
        #[arg(long)]
        height: Option<u32>,
        /// Path to FFmpeg binary (default: searches PATH).
        #[arg(long)]
        ffmpeg: Option<String>,
    },

    /// Test render directly (no ECS). Draws test patterns to verify GPU output.
    RenderTest {
        /// Test name: basic, blend, effects
        #[arg(short, long, default_value = "basic")]
        test: String,
        /// Output image path.
        #[arg(short, long, default_value = "render_test.png")]
        output: PathBuf,
        /// Output width.
        #[arg(long, default_value = "800")]
        width: u32,
        /// Output height.
        #[arg(long, default_value = "600")]
        height: u32,
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
            // Use core's re-exported Renderer (no direct GPU dependency)
            let mut renderer = ifol_render_core::Renderer::new(settings.width, settings.height);

            let total_frames = (settings.duration * fps) as u64;

            for frame in 0..total_frames {
                time.seek(frame as f64 / fps);
                let pixels = ifol_render_core::ecs::pipeline::render_frame(
                    &mut world,
                    &time,
                    &settings,
                    &mut renderer,
                );

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
                println!(
                    "  - {} (components: {})",
                    entity.id,
                    count_components(entity)
                );
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

            // Use core's re-exported Renderer
            let mut renderer = ifol_render_core::Renderer::new(settings.width, settings.height);

            // Load image sources through core's re-exported API
            for entity in &world.entities {
                if let Some(ref img) = entity.components.image_source
                    && let Err(e) = renderer.load_image(&entity.id, &img.path)
                {
                    log::warn!("{}", e);
                }
            }

            let pixels = ifol_render_core::ecs::pipeline::render_frame(
                &mut world,
                &time,
                &settings,
                &mut renderer,
            );

            let out_path = output.to_str().unwrap();
            if out_path.ends_with(".png") {
                ifol_render_core::Renderer::save_png(
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

        Commands::Export {
            scene,
            output,
            codec,
            crf,
            fps,
            width,
            height,
            ffmpeg,
        } => {
            let json = std::fs::read_to_string(&scene).expect("Failed to read scene file");
            let scene_desc = ifol_render_core::scene::SceneDescription::from_json(&json)
                .expect("Failed to parse scene JSON");

            let settings = scene_desc.settings.clone();
            let mut world = scene_desc.into_world();

            let effective_fps = fps.unwrap_or(settings.fps);
            let effective_w = width.unwrap_or(settings.width);
            let effective_h = height.unwrap_or(settings.height);

            let video_codec = ifol_render_core::export::VideoCodec::parse_codec(&codec)
                .unwrap_or_else(|| {
                    eprintln!("Unknown codec '{}', defaulting to h264.", codec);
                    ifol_render_core::export::VideoCodec::H264
                });

            let config = ifol_render_core::export::ExportConfig {
                output_path: output.clone(),
                codec: video_codec,
                pixel_format: "yuv420p".into(),
                crf,
                fps: Some(effective_fps),
                width: Some(effective_w),
                height: Some(effective_h),
                ffmpeg_path: ffmpeg,
            };

            let mut renderer = ifol_render_core::Renderer::new(effective_w, effective_h);

            // Load image sources
            for entity in &world.entities {
                if let Some(ref img) = entity.components.image_source
                    && let Err(e) = renderer.load_image(&entity.id, &img.path)
                {
                    log::warn!("{}", e);
                }
            }

            println!(
                "Exporting: {}x{} @ {}fps → {} ({:?})",
                effective_w, effective_h, effective_fps, output, video_codec
            );

            let start = std::time::Instant::now();

            let result = ifol_render_core::export::export_video(
                &mut world,
                &settings,
                &config,
                &mut renderer,
                |progress| {
                    // Print progress bar
                    let pct = progress.percent();
                    let bar_len = 40;
                    let filled = (pct / 100.0 * bar_len as f64) as usize;
                    let bar: String = "█".repeat(filled) + &"░".repeat(bar_len - filled);

                    eprint!(
                        "\r  [{bar}] {pct:5.1}%  {}/{}  ETA: {:.0}s  ({:.1}fps)  ",
                        progress.current_frame + 1,
                        progress.total_frames,
                        progress.eta_seconds,
                        progress.export_fps,
                    );
                },
            );

            eprintln!(); // newline after progress bar
            match result {
                Ok(()) => {
                    let elapsed = start.elapsed().as_secs_f64();
                    println!("Export complete: {} ({:.1}s)", output, elapsed);
                }
                Err(e) => {
                    eprintln!("Export failed: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::RenderTest {
            test,
            output,
            width,
            height,
        } => {
            println!(
                "Render test: '{}' → {:?} ({}x{})",
                test, output, width, height
            );

            let mut renderer = ifol_render_core::Renderer::new(width, height);

            // Register composite pipeline from core
            ifol_render_core::ecs::pipeline::setup_renderer(&mut renderer);

            let caps = renderer.capabilities();
            println!("GPU: {} ({})", caps.gpu_name, caps.backend);
            println!("Max texture: {}", caps.max_texture_size);

            let commands = match test.as_str() {
                "basic" => build_test_basic(width, height),
                "blend" => build_test_blend(width, height),
                "effects" => {
                    let cmds = build_test_basic(width, height);
                    let effects = vec![ifol_render_core::EffectConfig {
                        effect_type: "vignette".into(),
                        params: std::collections::HashMap::from([("intensity".into(), 0.8)]),
                    }];
                    let pixels = renderer.render_frame_with_effects(&cmds, &effects);
                    let out_path = output.to_str().unwrap();
                    ifol_render_core::Renderer::save_png(&pixels, width, height, out_path)
                        .expect("Failed to save PNG");
                    println!("Saved: {}", out_path);
                    return;
                }
                _ => {
                    eprintln!("Unknown test: '{}'. Available: basic, blend, effects", test);
                    std::process::exit(1);
                }
            };

            let pixels = renderer.render_frame(&commands);
            let out_path = output.to_str().unwrap();
            ifol_render_core::Renderer::save_png(&pixels, width, height, out_path)
                .expect("Failed to save PNG");
            println!("Saved: {}", out_path);
        }
    }
}

fn count_components(entity: &ifol_render_core::ecs::Entity) -> usize {
    let c = &entity.components;
    let mut n = 0;
    if c.video_source.is_some() {
        n += 1;
    }
    if c.image_source.is_some() {
        n += 1;
    }
    if c.text_source.is_some() {
        n += 1;
    }
    if c.color_source.is_some() {
        n += 1;
    }
    if c.timeline.is_some() {
        n += 1;
    }
    if c.transform.is_some() {
        n += 1;
    }
    if c.opacity.is_some() {
        n += 1;
    }
    if c.color.is_some() {
        n += 1;
    }
    if c.animation.is_some() {
        n += 1;
    }
    if c.effects.is_some() {
        n += 1;
    }
    n
}

/// Build a composite DrawCommand with the standard uniform layout.
/// Layout: [transform: f32x16, color: f32x4, opacity: f32, use_texture: f32, blend_mode: f32, _pad: f32]
fn make_draw_cmd(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [f32; 4],
    opacity: f32,
    blend_mode: f32,
    canvas_w: u32,
    canvas_h: u32,
) -> ifol_render_core::DrawCommand {
    // Convert pixel coords to clip space (-1..1)
    let sx = w / canvas_w as f32 * 2.0;
    let sy = h / canvas_h as f32 * 2.0;
    let tx = (x + w / 2.0) / canvas_w as f32 * 2.0 - 1.0;
    let ty = 1.0 - (y + h / 2.0) / canvas_h as f32 * 2.0;

    #[rustfmt::skip]
    let transform = [
        sx,  0.0, 0.0, 0.0,
        0.0, sy,  0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        tx,  ty,  0.0, 1.0,
    ];

    let mut uniforms = Vec::with_capacity(24);
    uniforms.extend_from_slice(&transform);
    uniforms.extend_from_slice(&color);
    uniforms.push(opacity);
    uniforms.push(0.0); // use_texture = false
    uniforms.push(blend_mode);
    uniforms.push(0.0); // _pad

    ifol_render_core::DrawCommand {
        pipeline: "composite".into(),
        uniforms,
        textures: vec![],
    }
}

/// Test: basic colored quads.
fn build_test_basic(w: u32, h: u32) -> Vec<ifol_render_core::DrawCommand> {
    vec![
        // Dark background
        make_draw_cmd(
            0.0,
            0.0,
            w as f32,
            h as f32,
            [0.1, 0.1, 0.15, 1.0],
            1.0,
            0.0,
            w,
            h,
        ),
        // Red quad (top-left)
        make_draw_cmd(
            50.0,
            50.0,
            200.0,
            200.0,
            [0.9, 0.2, 0.2, 1.0],
            1.0,
            0.0,
            w,
            h,
        ),
        // Green quad (center, semi-transparent)
        make_draw_cmd(
            150.0,
            150.0,
            200.0,
            200.0,
            [0.2, 0.9, 0.3, 1.0],
            0.7,
            0.0,
            w,
            h,
        ),
        // Blue quad (right)
        make_draw_cmd(
            350.0,
            100.0,
            200.0,
            200.0,
            [0.2, 0.3, 0.9, 1.0],
            1.0,
            0.0,
            w,
            h,
        ),
        // Yellow quad (bottom)
        make_draw_cmd(
            200.0,
            350.0,
            300.0,
            150.0,
            [0.9, 0.9, 0.2, 1.0],
            0.85,
            0.0,
            w,
            h,
        ),
        // White small quad (overlay)
        make_draw_cmd(
            300.0,
            200.0,
            100.0,
            100.0,
            [1.0, 1.0, 1.0, 1.0],
            0.5,
            0.0,
            w,
            h,
        ),
    ]
}

/// Test: blend modes (7 modes side by side).
fn build_test_blend(w: u32, h: u32) -> Vec<ifol_render_core::DrawCommand> {
    let mut cmds = vec![
        // White background
        make_draw_cmd(
            0.0,
            0.0,
            w as f32,
            h as f32,
            [0.8, 0.8, 0.85, 1.0],
            1.0,
            0.0,
            w,
            h,
        ),
    ];

    let modes = [
        "Normal",
        "Multiply",
        "Screen",
        "Overlay",
        "SoftLight",
        "Add",
        "Difference",
    ];
    let colors = [
        [0.9, 0.2, 0.2, 1.0],
        [0.2, 0.8, 0.2, 1.0],
        [0.2, 0.2, 0.9, 1.0],
        [0.9, 0.5, 0.1, 1.0],
        [0.7, 0.2, 0.8, 1.0],
        [0.2, 0.8, 0.8, 1.0],
        [0.9, 0.9, 0.2, 1.0],
    ];

    let count = modes.len();
    let quad_w = w as f32 / count as f32 - 10.0;
    let quad_h = h as f32 * 0.6;

    for (i, _mode) in modes.iter().enumerate() {
        let x = 5.0 + i as f32 * (quad_w + 10.0);
        let y = (h as f32 - quad_h) / 2.0;
        cmds.push(make_draw_cmd(
            x, y, quad_w, quad_h, colors[i], 0.9, i as f32, w, h,
        ));
    }

    cmds
}
