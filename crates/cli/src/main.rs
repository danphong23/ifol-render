#![allow(clippy::too_many_arguments)]
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
    /// Render a Frame JSON file to PNG using CoreEngine.
    FrameRender {
        /// Path to Frame JSON file.
        #[arg(short, long)]
        frame: PathBuf,
        /// Output PNG path.
        #[arg(short, long, default_value = "output.png")]
        output: PathBuf,
    },

    /// Test render directly. Draws test patterns to verify GPU output.
    RenderTest {
        /// Test name: basic, blend, shapes, gradients, resize, masking, text, effects, perf
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
        Commands::FrameRender { frame, output } => {
            println!("Frame render: {:?} → {:?}", frame, output);

            // Read JSON
            let json = std::fs::read_to_string(&frame).unwrap_or_else(|e| {
                eprintln!("Failed to read {:?}: {}", frame, e);
                std::process::exit(1);
            });

            // Parse Frame JSON format: { "settings": {...}, "frame": {...} }
            let doc: serde_json::Value = serde_json::from_str(&json).unwrap_or_else(|e| {
                eprintln!("Invalid JSON: {}", e);
                std::process::exit(1);
            });

            let settings: ifol_render_core::RenderSettings = if let Some(s) = doc.get("settings") {
                serde_json::from_value(s.clone()).unwrap_or_else(|e| {
                    eprintln!("Invalid settings: {}", e);
                    std::process::exit(1);
                })
            } else {
                ifol_render_core::RenderSettings::default()
            };

            let frame_data: ifol_render_core::Frame = if let Some(f) = doc.get("frame") {
                serde_json::from_value(f.clone()).unwrap_or_else(|e| {
                    eprintln!("Invalid frame data: {}", e);
                    std::process::exit(1);
                })
            } else {
                eprintln!("Missing 'frame' key in JSON");
                std::process::exit(1);
            };

            println!("Settings: {}x{}", settings.width, settings.height);
            let pass_count = frame_data.passes.len();
            let entity_count: usize = frame_data
                .passes
                .iter()
                .map(|p| match &p.pass_type {
                    ifol_render_core::PassType::Entities { entities, .. } => entities.len(),
                    _ => 0,
                })
                .sum();
            println!("Passes: {}, Entities: {}", pass_count, entity_count);

            // Create CoreEngine
            let mut engine = ifol_render_core::CoreEngine::new(settings);
            engine.setup_builtins();

            let caps = engine.capabilities();
            println!("GPU: {} ({})", caps.gpu_name, caps.backend);

            // Render
            let t = std::time::Instant::now();
            let pixels = engine.render_frame(&frame_data);
            let render_ms = t.elapsed().as_secs_f64() * 1000.0;
            println!("Render: {:.2}ms", render_ms);

            // Save
            let out_path = output.to_str().unwrap();
            ifol_render_core::CoreEngine::save_png(
                &pixels,
                engine.settings().width,
                engine.settings().height,
                out_path,
            )
            .expect("Failed to save PNG");
            println!("Saved: {}", out_path);
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

            // Register built-in shaders from core
            ifol_render_core::shaders::setup_builtins(&mut renderer);

            let caps = renderer.capabilities();
            println!("GPU: {} ({})", caps.gpu_name, caps.backend);
            println!("Max texture: {}", caps.max_texture_size);

            let commands = match test.as_str() {
                "basic" => build_test_basic(width, height),
                "blend" => build_test_blend(width, height),
                "shapes" => build_test_shapes(width, height),
                "gradients" => build_test_gradients(width, height),
                "resize" => {
                    // Test 1: render at initial size
                    let cmds = build_test_basic(width, height);
                    let pixels = renderer.render_frame(&cmds);
                    let small_path = output.with_file_name("render_test_resize_small.png");
                    ifol_render_core::Renderer::save_png(
                        &pixels,
                        width,
                        height,
                        small_path.to_str().unwrap(),
                    )
                    .unwrap();
                    println!("Saved small: {:?} ({}x{})", small_path, width, height);

                    // Test 2: resize to 1200x900 and render again
                    let new_w = 1200;
                    let new_h = 900;
                    renderer.resize(new_w, new_h);
                    let cmds2 = build_test_basic(new_w, new_h);
                    let pixels2 = renderer.render_frame(&cmds2);
                    let out_path = output.to_str().unwrap();
                    ifol_render_core::Renderer::save_png(&pixels2, new_w, new_h, out_path).unwrap();
                    println!("Saved resized: {} ({}x{})", out_path, new_w, new_h);
                    return;
                }
                "masking" => {
                    // First render a gradient background to use as source
                    let bg_cmds = vec![make_gradient_cmd(
                        0.0,
                        0.0,
                        width as f32,
                        height as f32,
                        [0.9, 0.3, 0.1, 1.0],
                        [0.1, 0.3, 0.9, 1.0],
                        0.0,
                        std::f32::consts::PI / 4.0,
                        0.0,
                        0.0,
                        width,
                        height,
                    )];
                    let bg_pixels = renderer.render_frame(&bg_cmds);
                    // Load the rendered gradient as a texture for masking
                    renderer.load_rgba("gradient_bg", &bg_pixels, width, height);

                    // Now draw mask shapes over the gradient
                    let cmds = build_test_masking(width, height);
                    let pixels = renderer.render_frame(&cmds);
                    let out_path = output.to_str().unwrap();
                    ifol_render_core::Renderer::save_png(&pixels, width, height, out_path).unwrap();
                    println!("Saved: {}", out_path);
                    return;
                }
                "text" => {
                    let font_data = ifol_render_core::text::default_font_data();

                    // Rasterize several text lines at different sizes
                    let texts = [
                        ("ifol-render GPU Engine", 48.0, [1.0, 1.0, 1.0, 1.0]),
                        (
                            "Pure GPU Executor — No Shader Ownership",
                            28.0,
                            [0.7, 0.9, 1.0, 1.0],
                        ),
                        (
                            "Shapes • Gradients • Masks • Text • Effects",
                            22.0,
                            [0.9, 0.8, 0.5, 1.0],
                        ),
                    ];

                    // Draw gradient background first
                    let mut cmds: Vec<ifol_render_core::DrawCommand> = vec![make_gradient_cmd(
                        0.0,
                        0.0,
                        width as f32,
                        height as f32,
                        [0.05, 0.05, 0.15, 1.0],
                        [0.15, 0.05, 0.2, 1.0],
                        0.0,
                        std::f32::consts::PI / 2.0,
                        0.0,
                        0.0,
                        width,
                        height,
                    )];

                    let mut y_offset = 80.0f32;
                    for (i, (text, size, color)) in texts.iter().enumerate() {
                        let key = format!("text_{}", i);
                        let opts = ifol_render_core::text::TextOptions {
                            font_size: *size,
                            color: *color,
                            ..Default::default()
                        };
                        match ifol_render_core::text::rasterize_text(text, font_data, &opts) {
                            Ok((pixels, tw, th)) => {
                                renderer.load_rgba(&key, &pixels, tw, th);
                                // Draw as textured quad centered horizontally
                                let x = (width as f32 - tw as f32) / 2.0;
                                cmds.push(make_textured_cmd(
                                    x, y_offset, tw as f32, th as f32, &key, 1.0, width, height,
                                ));
                                y_offset += th as f32 + 30.0;
                            }
                            Err(e) => eprintln!("Text rasterization failed: {}", e),
                        }
                    }

                    let pixels = renderer.render_frame(&cmds);
                    let out_path = output.to_str().unwrap();
                    ifol_render_core::Renderer::save_png(&pixels, width, height, out_path).unwrap();
                    println!("Saved: {}", out_path);
                    return;
                }
                "perf" => {
                    use std::time::Instant;
                    let n = 500u32;
                    println!("=== Performance Test: {} draw commands ===", n);

                    // Build 500 draw commands (mixed pipelines)
                    let t_build = Instant::now();
                    let mut cmds: Vec<ifol_render_core::DrawCommand> =
                        Vec::with_capacity(n as usize);
                    // Background
                    cmds.push(make_draw_cmd(
                        0.0,
                        0.0,
                        width as f32,
                        height as f32,
                        [0.1, 0.1, 0.15, 1.0],
                        1.0,
                        0.0,
                        width,
                        height,
                    ));
                    for i in 1..n {
                        let x = (i % 25) as f32 * 30.0;
                        let y = (i / 25) as f32 * 30.0;
                        let r = ((i * 7) % 256) as f32 / 255.0;
                        let g = ((i * 13) % 256) as f32 / 255.0;
                        let b = ((i * 23) % 256) as f32 / 255.0;
                        cmds.push(make_draw_cmd(
                            x,
                            y,
                            28.0,
                            28.0,
                            [r, g, b, 0.7],
                            0.7,
                            0.0,
                            width,
                            height,
                        ));
                    }
                    let build_ms = t_build.elapsed().as_secs_f64() * 1000.0;

                    // Render
                    let t_render = Instant::now();
                    let pixels = renderer.render_frame(&cmds);
                    let render_ms = t_render.elapsed().as_secs_f64() * 1000.0;

                    // Save
                    let t_save = Instant::now();
                    let out_path = output.to_str().unwrap();
                    ifol_render_core::Renderer::save_png(&pixels, width, height, out_path).unwrap();
                    let save_ms = t_save.elapsed().as_secs_f64() * 1000.0;

                    // VRAM stats
                    let vram = renderer.vram_usage();
                    println!("Build commands:  {:.2}ms", build_ms);
                    println!("Render frame:    {:.2}ms ({} draws)", render_ms, n);
                    println!("Save PNG:        {:.2}ms", save_ms);
                    println!("Total:           {:.2}ms", build_ms + render_ms + save_ms);
                    println!("--- VRAM ---");
                    println!(
                        "Texture cache:   {} textures, {} KB",
                        vram.texture_count,
                        vram.texture_cache_bytes / 1024
                    );
                    println!("Uniform ring:    {} KB", vram.uniform_buffer_bytes / 1024);
                    println!("Saved: {}", out_path);
                    return;
                }
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
                    eprintln!(
                        "Unknown test: '{}'. Available: basic, blend, shapes, gradients, resize, masking, text, effects",
                        test
                    );
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

/// Build a shape DrawCommand with SDF uniform layout.
/// Layout: [transform: f32x16, color: f32x4, opacity: f32, shape_type: f32, param1: f32, param2: f32]
fn make_shape_cmd(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [f32; 4],
    opacity: f32,
    shape_type: f32,
    param1: f32,
    param2: f32,
    canvas_w: u32,
    canvas_h: u32,
) -> ifol_render_core::DrawCommand {
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
    uniforms.push(shape_type);
    uniforms.push(param1);
    uniforms.push(param2);

    ifol_render_core::DrawCommand {
        pipeline: "shapes".into(),
        uniforms,
        textures: vec![],
    }
}

/// Test: SDF shapes.
fn build_test_shapes(w: u32, h: u32) -> Vec<ifol_render_core::DrawCommand> {
    let mut cmds = vec![
        // Dark background
        make_draw_cmd(
            0.0,
            0.0,
            w as f32,
            h as f32,
            [0.08, 0.08, 0.12, 1.0],
            1.0,
            0.0,
            w,
            h,
        ),
    ];

    // Row 1: Filled shapes
    // Rect (shape_type=0)
    cmds.push(make_shape_cmd(
        30.0,
        30.0,
        150.0,
        120.0,
        [0.9, 0.3, 0.3, 1.0],
        1.0,
        0.0,
        0.0,
        0.0,
        w,
        h,
    ));
    // Rounded rect (shape_type=1, param1=corner_radius)
    cmds.push(make_shape_cmd(
        210.0,
        30.0,
        150.0,
        120.0,
        [0.3, 0.9, 0.4, 1.0],
        1.0,
        1.0,
        0.08,
        0.0,
        w,
        h,
    ));
    // Circle (shape_type=2)
    cmds.push(make_shape_cmd(
        400.0,
        30.0,
        120.0,
        120.0,
        [0.3, 0.5, 0.95, 1.0],
        1.0,
        2.0,
        0.0,
        0.0,
        w,
        h,
    ));
    // Ellipse (shape_type=3)
    cmds.push(make_shape_cmd(
        560.0,
        30.0,
        200.0,
        120.0,
        [0.9, 0.7, 0.2, 1.0],
        1.0,
        3.0,
        0.0,
        0.0,
        w,
        h,
    ));

    // Row 2: Stroke/border shapes (param2=border_width)
    // Rect stroke
    cmds.push(make_shape_cmd(
        30.0,
        200.0,
        150.0,
        120.0,
        [0.9, 0.5, 0.5, 1.0],
        1.0,
        0.0,
        0.0,
        0.015,
        w,
        h,
    ));
    // Rounded rect stroke
    cmds.push(make_shape_cmd(
        210.0,
        200.0,
        150.0,
        120.0,
        [0.5, 0.9, 0.6, 1.0],
        1.0,
        1.0,
        0.08,
        0.015,
        w,
        h,
    ));
    // Circle stroke
    cmds.push(make_shape_cmd(
        400.0,
        200.0,
        120.0,
        120.0,
        [0.5, 0.7, 0.95, 1.0],
        1.0,
        2.0,
        0.0,
        0.015,
        w,
        h,
    ));
    // Ellipse stroke
    cmds.push(make_shape_cmd(
        560.0,
        200.0,
        200.0,
        120.0,
        [0.95, 0.85, 0.4, 1.0],
        1.0,
        3.0,
        0.0,
        0.015,
        w,
        h,
    ));

    // Row 3: Line + semi-transparent overlapping shapes
    // Line (shape_type=4, param1=line_width)
    cmds.push(make_shape_cmd(
        30.0,
        400.0,
        300.0,
        40.0,
        [1.0, 1.0, 1.0, 1.0],
        0.9,
        4.0,
        0.15,
        0.0,
        w,
        h,
    ));
    // Overlapping circles
    cmds.push(make_shape_cmd(
        400.0,
        370.0,
        150.0,
        150.0,
        [0.9, 0.2, 0.5, 1.0],
        0.6,
        2.0,
        0.0,
        0.0,
        w,
        h,
    ));
    cmds.push(make_shape_cmd(
        480.0,
        400.0,
        150.0,
        150.0,
        [0.2, 0.5, 0.9, 1.0],
        0.6,
        2.0,
        0.0,
        0.0,
        w,
        h,
    ));

    cmds
}

/// Build a gradient DrawCommand.
/// Layout: [transform: f32x16, color_start: f32x4, color_end: f32x4, grad_type: f32, angle: f32, center_x: f32, center_y: f32]
/// = 28 floats
fn make_gradient_cmd(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color_start: [f32; 4],
    color_end: [f32; 4],
    grad_type: f32,
    angle: f32,
    center_x: f32,
    center_y: f32,
    canvas_w: u32,
    canvas_h: u32,
) -> ifol_render_core::DrawCommand {
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

    let mut uniforms = Vec::with_capacity(28);
    uniforms.extend_from_slice(&transform); // 16
    uniforms.extend_from_slice(&color_start); // 4
    uniforms.extend_from_slice(&color_end); // 4
    uniforms.push(grad_type); // 1
    uniforms.push(angle); // 1
    uniforms.push(center_x); // 1
    uniforms.push(center_y); // 1

    ifol_render_core::DrawCommand {
        pipeline: "gradient".into(),
        uniforms,
        textures: vec![],
    }
}

/// Test: gradient fills.
fn build_test_gradients(w: u32, h: u32) -> Vec<ifol_render_core::DrawCommand> {
    let pi = std::f32::consts::PI;
    let mut cmds = vec![
        // Dark background
        make_draw_cmd(
            0.0,
            0.0,
            w as f32,
            h as f32,
            [0.08, 0.08, 0.12, 1.0],
            1.0,
            0.0,
            w,
            h,
        ),
    ];

    // Row 1: Linear gradients at different angles
    // Horizontal (angle=0)
    cmds.push(make_gradient_cmd(
        20.0,
        20.0,
        230.0,
        170.0,
        [0.9, 0.2, 0.3, 1.0],
        [0.2, 0.3, 0.9, 1.0],
        0.0,
        0.0,
        0.0,
        0.0,
        w,
        h,
    ));
    // Diagonal (angle=PI/4)
    cmds.push(make_gradient_cmd(
        280.0,
        20.0,
        230.0,
        170.0,
        [0.1, 0.8, 0.4, 1.0],
        [0.8, 0.9, 0.1, 1.0],
        0.0,
        pi / 4.0,
        0.0,
        0.0,
        w,
        h,
    ));
    // Vertical (angle=PI/2)
    cmds.push(make_gradient_cmd(
        540.0,
        20.0,
        230.0,
        170.0,
        [0.9, 0.6, 0.1, 1.0],
        [0.4, 0.1, 0.8, 1.0],
        0.0,
        pi / 2.0,
        0.0,
        0.0,
        w,
        h,
    ));

    // Row 2: Radial and conic
    // Radial (centered)
    cmds.push(make_gradient_cmd(
        20.0,
        220.0,
        230.0,
        170.0,
        [1.0, 1.0, 0.3, 1.0],
        [0.1, 0.1, 0.5, 1.0],
        1.0,
        0.0,
        0.0,
        0.0,
        w,
        h,
    ));
    // Radial (off-center)
    cmds.push(make_gradient_cmd(
        280.0,
        220.0,
        230.0,
        170.0,
        [0.3, 0.9, 0.9, 1.0],
        [0.9, 0.1, 0.3, 1.0],
        1.0,
        0.0,
        -0.2,
        -0.15,
        w,
        h,
    ));
    // Conic
    cmds.push(make_gradient_cmd(
        540.0,
        220.0,
        230.0,
        170.0,
        [0.9, 0.3, 0.5, 1.0],
        [0.3, 0.8, 0.9, 1.0],
        2.0,
        0.0,
        0.0,
        0.0,
        w,
        h,
    ));

    // Row 3: Full-width gradients
    // Sunset
    cmds.push(make_gradient_cmd(
        20.0,
        420.0,
        w as f32 - 40.0,
        80.0,
        [1.0, 0.4, 0.1, 1.0],
        [0.1, 0.0, 0.3, 1.0],
        0.0,
        0.0,
        0.0,
        0.0,
        w,
        h,
    ));
    // Ocean
    cmds.push(make_gradient_cmd(
        20.0,
        510.0,
        w as f32 - 40.0,
        70.0,
        [0.0, 0.8, 0.9, 1.0],
        [0.0, 0.2, 0.5, 1.0],
        0.0,
        0.0,
        0.0,
        0.0,
        w,
        h,
    ));

    cmds
}

/// Build a mask DrawCommand.
/// Layout: [transform: f32x16, color: f32x4, opacity: f32, mask_shape: f32, param1: f32, _pad: f32]
fn make_mask_cmd(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    mask_shape: f32,
    param1: f32,
    texture_key: &str,
    canvas_w: u32,
    canvas_h: u32,
) -> ifol_render_core::DrawCommand {
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
    uniforms.extend_from_slice(&[1.0, 1.0, 1.0, 1.0]); // color (white = no tint)
    uniforms.push(1.0); // opacity
    uniforms.push(mask_shape); // mask_shape
    uniforms.push(param1); // param1
    uniforms.push(0.0); // _pad

    ifol_render_core::DrawCommand {
        pipeline: "mask".into(),
        uniforms,
        textures: vec![texture_key.to_string()],
    }
}

/// Test: masking / clipping.
fn build_test_masking(w: u32, h: u32) -> Vec<ifol_render_core::DrawCommand> {
    let mut cmds = vec![
        // Dark background
        make_draw_cmd(
            0.0,
            0.0,
            w as f32,
            h as f32,
            [0.15, 0.15, 0.2, 1.0],
            1.0,
            0.0,
            w,
            h,
        ),
    ];

    // Row 1: rect clip and circle clip
    // Rect clip (mask_shape=0)
    cmds.push(make_mask_cmd(
        30.0,
        30.0,
        200.0,
        200.0,
        0.0,
        0.0,
        "gradient_bg",
        w,
        h,
    ));
    // Circle clip (mask_shape=1)
    cmds.push(make_mask_cmd(
        260.0,
        30.0,
        200.0,
        200.0,
        1.0,
        0.0,
        "gradient_bg",
        w,
        h,
    ));
    // Rounded rect clip (mask_shape=2, param1=corner_radius)
    cmds.push(make_mask_cmd(
        490.0,
        30.0,
        250.0,
        200.0,
        2.0,
        0.08,
        "gradient_bg",
        w,
        h,
    ));

    // Row 2: Feathered soft masks
    // Feathered rect (mask_shape=3, param1=feather_amount)
    cmds.push(make_mask_cmd(
        30.0,
        280.0,
        200.0,
        200.0,
        3.0,
        0.1,
        "gradient_bg",
        w,
        h,
    ));
    // Feathered larger
    cmds.push(make_mask_cmd(
        260.0,
        280.0,
        200.0,
        200.0,
        3.0,
        0.25,
        "gradient_bg",
        w,
        h,
    ));
    // Circle soft
    cmds.push(make_mask_cmd(
        490.0,
        280.0,
        250.0,
        200.0,
        1.0,
        0.0,
        "gradient_bg",
        w,
        h,
    ));

    cmds
}

/// Build a textured composite DrawCommand — draws a pre-loaded texture at position.
fn make_textured_cmd(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    texture_key: &str,
    opacity: f32,
    canvas_w: u32,
    canvas_h: u32,
) -> ifol_render_core::DrawCommand {
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
    uniforms.extend_from_slice(&[1.0, 1.0, 1.0, 1.0]); // color (white = no tint)
    uniforms.push(opacity); // opacity
    uniforms.push(1.0); // use_texture = true
    uniforms.push(0.0); // blend_mode = Normal
    uniforms.push(0.0); // _pad

    ifol_render_core::DrawCommand {
        pipeline: "composite".into(),
        uniforms,
        textures: vec![texture_key.to_string()],
    }
}
