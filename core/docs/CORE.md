# Core Engine — API Reference

## Quick Start

```rust
use ifol_render_core::{CoreEngine, RenderSettings, FrameData, FlatEntity, RenderPass, PassType};

// 1. Create engine
let settings = RenderSettings { width: 1920, height: 1080, background: [0.0, 0.0, 0.0, 1.0] };
let mut engine = CoreEngine::new(settings).await;

// 2. Register built-in shaders
engine.setup_builtins();

// 3. Load textures
engine.load_image("bg", "assets/background.png")?;
engine.rasterize_text("title", "Hello World", 64.0, [1.0, 1.0, 1.0, 1.0]);

// 4. Build frame
let frame = FrameData {
    passes: vec![
        RenderPass {
            output: "main".into(),
            pass_type: PassType::Entities {
                entities: vec![
                    FlatEntity {
                        id: 1,
                        x: 0.0, y: 0.0,
                        width: 1920.0, height: 1080.0,
                        rotation: 0.0,
                        opacity: 1.0,
                        blend_mode: 0,
                        color: [1.0, 1.0, 1.0, 1.0],
                        shader: "composite".into(),
                        textures: vec!["bg".into()],
                        params: vec![],
                        layer: 0,
                        z_index: 0.0,
                    },
                    FlatEntity {
                        id: 2,
                        x: 810.0, y: 440.0,
                        width: 300.0, height: 50.0,
                        rotation: 0.0,
                        opacity: 0.9,
                        blend_mode: 0,
                        color: [1.0, 1.0, 1.0, 1.0],
                        shader: "composite".into(),
                        textures: vec!["title".into()],
                        params: vec![],
                        layer: 1,
                        z_index: 0.0,
                    },
                ],
                clear_color: [0.0, 0.0, 0.0, 1.0],
            },
        },
        RenderPass {
            output: "screen".into(),
            pass_type: PassType::Output { input: "main".into() },
        },
    ],
    texture_updates: vec![],
};

// 5. Render
let pixels = engine.render_frame(&frame);

// 6. Save
CoreEngine::save_png(&pixels, 1920, 1080, "output.png")?;
```

---

## CoreEngine

### `CoreEngine::new(settings) -> Self`

Create a new engine with the given render settings.
Initializes GPU context (headless, no window required).

| Param | Type | Description |
|-------|------|-------------|
| `settings` | `RenderSettings` | Output dimensions and background |

### `CoreEngine::resize(width, height)`

Change output resolution. Textures are preserved.

### `CoreEngine::capabilities() -> GpuCapabilities`

Returns detected GPU info: name, backend, max texture size, max buffer size.

---

## Shaders

### `setup_builtins()`

Registers all built-in shaders. Call once after creating the engine.

Built-in shaders:

| Name | Type | Description |
|------|------|-------------|
| `composite` | Entity | Textured/color quad with blend modes |
| `shapes` | Entity | SDF shapes (circle, rect, rounded rect) |
| `gradient` | Entity | Linear, radial, conic gradients |
| `mask` | Entity | Alpha mask / clipping |
| `blur` | Effect | Gaussian blur (2-pass) |
| `color_grade` | Effect | Brightness, contrast, saturation |
| `vignette` | Effect | Edge darkening |
| `chromatic_aberration` | Effect | RGB channel offset |

### `register_shader(name, wgsl_code, config)`

Register a custom WGSL shader.

| Param | Type | Description |
|-------|------|-------------|
| `name` | `&str` | Unique shader name |
| `wgsl_code` | `&str` | WGSL source code |
| `config` | `PipelineConfig` | `PipelineConfig::quad()` for entities, `PipelineConfig::fullscreen()` for effects |

### `has_shader(name) -> bool`

Check if a shader is registered.

---

## Textures

### `load_image(key, path) -> Result<[u32;2], String>`

Load an image file (PNG, JPEG, WebP) into texture cache.
Returns pixel dimensions `[width, height]`.
**Cached:** calling again with same key skips reload.

### `load_rgba(key, data, width, height)`

Upload raw RGBA pixels directly. Used for:
- Video frames (decoded externally)
- Procedurally generated content
- Any dynamic texture

**Replaces** existing texture with same key (no cache — always uploads).

### `rasterize_text(key, content, font_size, color) -> [u32;2]`

Rasterize text string to a texture using the built-in font (NotoSans).
Returns pixel dimensions `[width, height]`.

| Param | Type | Description |
|-------|------|-------------|
| `content` | `&str` | Text to render |
| `font_size` | `f32` | Font size in pixels |
| `color` | `[f32;4]` | RGBA color (0..1) |

### `has_texture(key) -> bool`

Check if a texture exists in cache.

### `evict_texture(key)`

Remove a texture from cache, freeing VRAM.

---

## Rendering

### `render_frame(frame) -> Vec<u8>`

Render a complete frame and return RGBA pixels.

**Pipeline:**
1. Process `texture_updates` (load/upload/rasterize/evict)
2. For each `RenderPass` in order:
   - `Entities`: sort by (layer, z_index) → pack uniforms → draw
   - `Effect`: bind input textures → apply shader fullscreen
   - `Output`: read pixels from specified texture → return

### `save_png(pixels, width, height, path) -> Result`

Static utility: save RGBA pixel buffer to PNG file.

---

## Export

### `export_video(frames, config, on_progress) -> Result`

Render sequential frames and encode to video via FFmpeg.

```rust
let config = ExportConfig {
    output_path: "output.mp4".into(),
    codec: VideoCodec::H264,
    pixel_format: "yuv420p".into(),
    crf: 23,
    fps: Some(30.0),
    ..Default::default()
};

engine.export_video(
    frame_iterator,     // impl Iterator<Item = FrameData>
    &config,
    |progress| {
        println!("Frame {}/{} ({:.1}%)",
            progress.current_frame,
            progress.total_frames,
            progress.percent());
    },
)?;
```

### Supported codecs

| Codec | Extension | Quality |
|-------|-----------|---------|
| H.264 | .mp4 | Good, universal |
| H.265 | .mp4 | Better, slower encode |
| VP9 | .webm | Good, web-native |
| ProRes | .mov | Professional, lossless |
| PNG Sequence | .png | Lossless frames |

### `export_frame(frame, path) -> Result`

Render a single frame and save as PNG.

---

## Data Types

### `RenderSettings`

```rust
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub background: [f32; 4],
}
```

### `FlatEntity`

```rust
pub struct FlatEntity {
    pub id: u64,            // unique ID for caching
    pub x: f32,             // top-left X (pixels)
    pub y: f32,             // top-left Y (pixels)
    pub width: f32,         // render width (pixels)
    pub height: f32,        // render height (pixels)
    pub rotation: f32,      // radians
    pub opacity: f32,       // 0..1
    pub blend_mode: u32,    // 0=Normal 1=Multiply 2=Screen ...
    pub color: [f32; 4],    // RGBA tint
    pub shader: String,     // pipeline name
    pub textures: Vec<String>,  // texture keys
    pub params: Vec<f32>,   // shader uniforms
    pub layer: i32,         // sort priority 1
    pub z_index: f32,       // sort priority 2
}
```

### `FrameData`

```rust
pub struct FrameData {
    pub passes: Vec<RenderPass>,
    pub texture_updates: Vec<TextureUpdate>,
}
```

### `RenderPass`

```rust
pub struct RenderPass {
    pub output: String,
    pub pass_type: PassType,
}

pub enum PassType {
    Entities { entities: Vec<FlatEntity>, clear_color: [f32; 4] },
    Effect { shader: String, inputs: Vec<String>, params: Vec<f32> },
    Output { input: String },
}
```

### `TextureUpdate`

```rust
pub enum TextureUpdate {
    LoadImage { key: String, path: String },
    UploadRgba { key: String, data: Vec<u8>, width: u32, height: u32 },
    RasterizeText { key: String, content: String, font_size: f32, color: [f32; 4] },
    Evict { key: String },
}
```

### Blend Modes

| Value | Mode | Description |
|-------|------|-------------|
| 0 | Normal | Standard alpha compositing |
| 1 | Multiply | Darken (a × b) |
| 2 | Screen | Lighten (1 - (1-a)(1-b)) |
| 3 | Overlay | Contrast enhance |
| 4 | SoftLight | Subtle light/dark |
| 5 | Add | Additive (glow) |
| 6 | Difference | Color inversion |
