# ifol-render — System Architecture

## Overview

ifol-render is a GPU-accelerated rendering engine for video composition and visual effects.
The system follows a **3-layer architecture** where each layer has a single responsibility:

```
┌─────────────────────────────────────────────────────────────────┐
│  Layer 3: Frontend (Web App / Studio / CLI)                    │
│  ─────────────────────────────────────────                     │
│  Owns: Scene editing, ECS, timeline, animation, camera,        │
│        keyframes, bone systems, particle systems, plugins      │
│  Output: FrameData (flat, pixel-based, pre-computed)           │
└────────────────────────────┬────────────────────────────────────┘
                             │ FrameData (JSON / Rust struct)
┌────────────────────────────▼────────────────────────────────────┐
│  Layer 2: Core Engine (Rust / WASM)                            │
│  ──────────────────────────────────                            │
│  Owns: Shader registry, texture cache, text rasterization,    │
│        pixel→clip conversion, render pass orchestration,      │
│        video export (FFmpeg)                                   │
│  Output: DrawCommands per pass                                │
└────────────────────────────┬────────────────────────────────────┘
                             │ DrawCommands
┌────────────────────────────▼────────────────────────────────────┐
│  Layer 1: Render Tool (GPU / wgpu)                             │
│  ─────────────────────────────────                             │
│  Owns: GPU pipeline execution, vertex/index buffers,           │
│        uniform ring buffer, texture upload, readback           │
│  Output: RGBA pixels                                          │
└─────────────────────────────────────────────────────────────────┘
```

---

## Technology Stack

| Layer | Technology | Version |
|-------|------------|---------|
| Language | Rust | Edition 2024 (1.85+) |
| GPU | wgpu | 24 (Vulkan / DX12 / Metal / WebGPU) |
| Desktop GUI | egui + eframe | 0.31 |
| Web Frontend | Vite + ES Modules | Vite 5 |
| WASM Bridge | wasm-bindgen + wasm-pack | 0.2 |
| Video I/O | FFmpeg (external binary) | 5+ |
| Text Rendering | ab_glyph (CPU rasterization) | 0.2 |
| Image Decoding | image crate | 0.25 |
| Audio (Studio) | rodio | 0.19 |
| CLI Parser | clap (derive) | 4 |
| Serialization | serde + serde_json | 1 |

### Workspace Crates

| Crate | Type | Purpose |
|-------|------|---------|
| `ifol-render` | lib | GPU render tool — pure draw command executor |
| `ifol-render-core` | lib | Core engine — textures, shaders, video decode, export |
| `ifol-render-studio` | bin | Desktop GUI editor (egui/Vulkan) |
| `ifol-render-cli` | bin | Headless CLI — render, export, test |
| `ifol-render-wasm` | cdylib | WASM target for browser via WebGPU |
| `server` | bin | (stub) Future server-side rendering |

## Design Principles

1. **Data flows down, pixels flow up**
   - Frontend produces data → Core processes → Render executes → pixels returned
   - No layer reaches upward; dependencies are strictly one-directional

2. **Frontend owns complexity, Core owns speed**
   - Complex logic (ECS, animation, physics) lives in frontend — easy to change
   - Hot path (sort, pack uniforms, GPU dispatch) lives in Core — compiled Rust

3. **Flat data contract**
   - Core does NOT know about timelines, keyframes, bone systems, or any domain logic
   - Core receives `FrameData`: a flat list of "what to draw, where, how"
   - All positions are in **pixels**, pre-computed by frontend

4. **Shader-agnostic execution**
   - Core ships with built-in shaders but frontend can register custom ones
   - Core doesn't interpret shader params — just packs them as uniforms

5. **Multi-pass render graph**
   - A frame is a sequence of render passes
   - Each pass either renders entities or applies a fullscreen shader effect
   - Output of one pass can be input to the next (texture chaining)

---

## Data Flow

### Preview (single frame, real-time)

```
User drags entity in editor
  → Frontend ECS updates position
  → Frontend resolves: hierarchy, camera, animation, visibility
  → Frontend builds FrameData (entities in pixels)
  → Core.render_frame(frame_data) 
  → Core: sort → pixel→clip → pack uniforms → DrawCommands
  → Render: GPU execute → RGBA pixels
  → Display in viewport
  
  Total: < 16ms (60fps target)
```

### Export (all frames, sequential)

```
User clicks "Export"
  → Frontend bakes ALL frames: for each frame time, resolve → FrameData
  → Core.export_video(frame_iterator, config)
  → Core: for each FrameData:
       process textures → sort → pack → render → pipe to FFmpeg
  → FFmpeg encodes → output.mp4
```

---

## FrameData Specification

`FrameData` is the **API contract** between Frontend and Core.
Frontend builds it, Core consumes it. Core never modifies it.

### Structure

```
FrameData
├── passes: Vec<RenderPass>        // ordered render passes
└── texture_updates: Vec<TextureUpdate>  // textures to load/update this frame
```

### RenderPass

Each pass produces a texture that can be used by later passes.

```
RenderPass
├── output: String                 // output texture key (e.g. "layer_0", "final")
└── pass_type: PassType
    ├── Entities                   // render a list of entities
    │   ├── entities: Vec<FlatEntity>
    │   └── clear_color: [f32; 4]  // background of this pass
    ├── Effect                     // apply shader on existing texture(s)
    │   ├── shader: String         // registered shader name
    │   ├── inputs: Vec<String>    // input texture keys
    │   └── params: Vec<f32>       // shader uniforms
    └── Output                     // mark which texture is the final output
        └── input: String          // texture key to read back as pixels
```

### FlatEntity

A single drawable element. All values are **final** — no further computation needed.

```
FlatEntity
├── id: u64                // unique ID for dirty tracking / caching
├── x: f32                 // top-left X in pixels
├── y: f32                 // top-left Y in pixels
├── width: f32             // rendered width in pixels
├── height: f32            // rendered height in pixels
├── rotation: f32          // radians (around entity center)
├── opacity: f32           // 0.0 (transparent) to 1.0 (opaque)
├── blend_mode: u32        // 0=Normal 1=Multiply 2=Screen 3=Overlay ...
├── color: [f32; 4]        // RGBA tint (default: [1,1,1,1] = no tint)
├── shader: String         // pipeline name (e.g. "composite")
├── textures: Vec<String>  // texture cache keys
├── params: Vec<f32>       // additional shader uniforms
├── layer: i32             // sorting priority 1 (ascending)
└── z_index: f32           // sorting priority 2 within same layer (ascending)
```

### TextureUpdate

Instructions for Core to load/update textures before rendering.

```
TextureUpdate
├── LoadImage   { key, path }                          // from file (cached)
├── UploadRgba  { key, data: Vec<u8>, width, height }  // raw pixels (video frame)
├── RasterizeText { key, content, font_size, color }    // Core handles ab_glyph
└── Evict       { key }                                // remove from cache
```

### Sorting Rules

```
Primary:   layer (i32, ascending)     — layer 0 draws first (behind)
Secondary: z_index (f32, ascending)   — within same layer, lower z draws first
```

---

## Core Engine

### Responsibilities

| Responsibility | Details |
|---------------|---------|
| **Shader registry** | Register WGSL shaders (built-in + custom). Compile once, cache |
| **Texture cache** | Load images, upload RGBA data, rasterize text. LRU eviction |
| **Render pass orchestration** | Execute passes in order, manage intermediate textures |
| **Pixel→clip conversion** | Convert pixel coordinates to GPU clip space (-1..1) |
| **Uniform packing** | Pack entity fields into shader-specific uniform buffers |
| **Dirty tracking** | Cache DrawCommands by entity ID. Reuse if unchanged |
| **Video export** | FFmpeg pipe: iterate frames → render → encode |
| **Text rasterization** | ab_glyph: string → RGBA texture (CPU side) |

### NOT Core's Responsibility

| Not Core | Who owns it |
|----------|-------------|
| Timeline / visibility | Frontend |
| Animation / keyframes | Frontend |
| Camera transform | Frontend |
| Entity hierarchy | Frontend |
| Bone / skeleton | Frontend |
| Particle systems | Frontend |
| Undo / redo | Frontend |
| Scene serialization | Frontend |
| UI / editor | Frontend |

### Built-in Shaders

Core ships with these shaders pre-registered:

| Shader | Type | Purpose |
|--------|------|---------|
| `composite` | Per-entity | Texture/color rendering with blend modes, UV crop |
| `shapes` | Per-entity | SDF shapes (circle, rectangle, rounded rect) |
| `gradient` | Per-entity | Linear/radial/conic gradient fill |
| `mask` | Per-entity | Alpha masking / clipping |
| `blur` | Effect | Gaussian blur (horizontal + vertical, 2 passes) |
| `color_grade` | Effect | Brightness, contrast, saturation |
| `vignette` | Effect | Edge darkening |
| `chromatic_aberration` | Effect | RGB channel offset |

### Pixel→Clip Conversion

Core converts pixel positions to GPU clip space:

```
Given: entity at (x=100, y=50, w=200, h=150) in a 1920×1080 output

Step 1: Normalize to 0..1
  nx = x / 1920 = 0.052
  ny = y / 1080 = 0.046

Step 2: Convert to clip space (-1..1)
  clip_x = nx * 2 - 1 = -0.896
  clip_y = 1 - ny * 2 = 0.907    (Y flipped for GPU)

Step 3: Scale quad to entity size
  scale_x = w / 1920 = 0.104
  scale_y = h / 1080 = 0.139

Step 4: Build transform matrix with rotation
  → 4×4 matrix sent as uniform to shader
```

---

## Render Tool

The lowest layer. A pure GPU executor.

### What it does
- Compile WGSL → GPU pipeline
- Upload vertex/index/uniform data
- Execute draw calls
- Read back pixels from GPU → CPU

### What it does NOT do
- No scene logic
- No coordinate systems
- No caching decisions (Core decides)

### Key optimizations
- **Uniform ring buffer**: 2MB pre-allocated, zero-alloc per draw
- **Dynamic bind group offsets**: 1 bind group per pipeline, offset per draw
- **Pipeline switch tracking**: Only `set_pipeline()` when shader changes
- **Single command encoder**: All draws in 1 `queue.submit()`
- **Texture LRU**: Automatic eviction when VRAM budget exceeded
- **Ping-pong textures**: Reuse 2 textures for multi-pass effects

---

## Multi-Pass Rendering

### Simple case (no effects)

```json
"passes": [
  { "output": "main", "pass_type": { "Entities": { "entities": [...], "clear_color": [0,0,0,1] } } },
  { "output": "screen", "pass_type": { "Output": { "input": "main" } } }
]
```

### Entity + Post-effects

```json
"passes": [
  { "output": "main", "pass_type": { "Entities": { "entities": [...] } } },
  { "output": "bloomed", "pass_type": { "Effect": { "shader": "bloom", "inputs": ["main"], "params": [...] } } },
  { "output": "graded", "pass_type": { "Effect": { "shader": "color_grade", "inputs": ["bloomed"], "params": [...] } } },
  { "output": "screen", "pass_type": { "Output": { "input": "graded" } } }
]
```

### Multi-layer with per-layer effects

```
passes: [
  // Layer 0: background with blur
  { Entities: [bg entities], output: "layer_0" },
  { Effect: { shader: "blur", inputs: ["layer_0"], params: [4.0] }, output: "layer_0_blur" },

  // Layer 1: foreground
  { Entities: [fg entities], output: "layer_1" },

  // Composite layers
  { Effect: { shader: "composite", inputs: ["layer_0_blur", "layer_1"] }, output: "scene" },

  // Frame-level post-processing
  { Effect: { shader: "vignette", inputs: ["scene"], params: [0.5] }, output: "final" },
  { Output: { input: "final" }, output: "screen" }
]
```

### Scope mapping

| Scope | How frontend builds passes |
|-------|---------------------------|
| **Per-entity** | entity.shader = custom shader within Entities pass |
| **Per-layer** | Separate Entities pass + Effect pass for that layer |
| **Per-scene** | Effect pass that composites all layer outputs |
| **Per-frame** | Effect pass on final composited scene |

---

## Performance Architecture

### Dirty Tracking

```
Frame N-1: entities [A, B, C, D, E]
Frame N  : entities [A, B*, C, D, F]   (* = changed, F = new)

Core detects:
  A → cache hit (reuse DrawCommand)
  B → changed (rebuild DrawCommand)
  C → cache hit
  D → cache hit
  E → removed (evict from cache)
  F → new (build DrawCommand)

Result: 3 cache hits, 1 rebuild, 1 new = 80% reuse
```

### Texture Management

```
Textures loaded once, reused across frames:
  LoadImage("bg", "photo.png")     → cached until Evict
  UploadRgba("vid_0", data, w, h)  → replaced each frame (video)
  RasterizeText("txt_0", ...)      → cached until text changes

LRU eviction when VRAM exceeds budget:
  Least recently used textures evicted first
  Active textures (used this frame) never evicted
```

### Export Optimization

```
Sequential frame rendering:
  Frame 0: process textures + render (cold start)
  Frame 1: only changed textures + render (warm)
  Frame 2: only video frame update + render (minimal CPU)
  ...
  
  Static textures loaded once, video frames streamed per-frame
```

### CPU-GPU Zero-Copy Pass Strategy

To avoid suffocating the PCIe lanes (sending 8MB images back and forth between RAM and VRAM for every layer), intermediate passes (`PassType::Entities` and `PassType::Effect`) write directly to **offscreen GPU Texture Attachments** mapping to `wgpu::TextureUsages::RENDER_ATTACHMENT`. 
These intermediate render streams never touch the CPU.

Only upon encountering a `PassType::Output` node does the engine bind the synchronous `readback_output` wgpu staging buffer command to map the **final** frame array directly to `Vec<u8>`.

### Multi-threaded `mpsc::sync_channel` Async Exporter

Video encoding (`libx264` / `h264_qsv`) requires heavy, sustained CPU effort. Waiting for FFmpeg to finish encoding Frame `N` before letting the GPU render Frame `N+1` starves the GPU.

Instead, the export pipeline initializes an `FfmpegMediaBackend` that acts on a dedicated thread. Completed `Vec<u8>` payloads are sent from the `CoreEngine` loop via a bounded `mpsc::sync_channel(3)`. This concurrency effectively hides FFmpeg's heavy write-stalls, ensuring the GPU is fed new draw commands constantly and doubling export throughput.

---

## Extending the System

### Add a new shader (no Core change)

```
1. Frontend: write custom.wgsl
2. Frontend: core.register_shader("my_effect", wgsl_code, config)
3. Frontend: entity.shader = "my_effect", entity.params = [...]
4. Core renders it — zero code changes
```

### Add a new system (e.g. bone/skeleton)

```
1. Frontend: implement bone solver
2. Frontend: resolve bone transforms → flat positions
3. Frontend: emit FlatEntities with resolved positions
4. Core renders them — zero code changes
```

### Add a new render pass type (Core change)

```
1. Add new PassType variant in frame.rs
2. Add handler in engine.rs
3. Frontend sends new pass type in FrameData
4. Backward compatible — old FrameData still works
```

---

## MediaBackend — Multi-Environment Architecture

The `MediaBackend` trait abstracts platform-specific operations:

```
MediaBackend trait
├── read_file_bytes(path)          // asset loading
├── get_video_frame(path, time)    // encoded frame (JPEG/PNG)
├── get_video_frame_rgba(path, t)  // raw RGBA pixels
├── get_video_info(path)           // probe duration, dimensions
└── start_export(w, h, fps, cfg)   // begin video encoding
```

### Platform Implementations

| Platform | Backend | Video Decode | Video Encode |
|----------|---------|-------------|-------------|
| **Native** | `FfmpegMediaBackend` | VideoStream (FFmpeg pipe, raw RGBA) | FFmpeg pipe (H264/H265/VP9/ProRes) |
| **WASM** | `WebMediaBackend` | JS → Canvas2D → getImageData → WASM cache | N/A (uses server CLI for export) |

### cfg-gated Optimization

On native, `decode_video_frame` skips the MediaBackend entirely and uses `VideoStream` directly.
On WASM, it delegates to the JS-provided frame cache.
This avoids unnecessary `Arc<RwLock>` + HashMap lookups on native (2× per frame).

```rust
#[cfg(target_arch = "wasm32")]  → backend.get_video_frame_rgba()
#[cfg(not(wasm32))]             → VideoStream::frame_at() (direct FFmpeg pipe)
```

### Adding a New Environment

1. Implement `MediaBackend` trait for your platform
2. Pass to `CoreEngine::new_async(settings, Box::new(MyBackend))`
3. All rendering works automatically — no Core changes needed

Example targets: Android (MediaCodec), iOS (AVFoundation), Server (headless FFmpeg).

---

## Deployment

### Windows / macOS / Linux (Desktop)

**Prerequisites**: Rust 1.85+, GPU with Vulkan/DX12/Metal, FFmpeg in PATH or `tool/` directory.

```bash
# Build all
cargo build --release

# Run Studio GUI
cargo run -p ifol-render-studio --release

# CLI export
cargo run -p ifol-render-cli --release -- export \
  --scene scene.json --output video.mp4 --ffmpeg path/to/ffmpeg

# CLI single-frame render
cargo run -p ifol-render-cli --release -- frame-render \
  --frame frame.json --output frame.png
```

**FFmpeg path resolution** (Studio):
1. `tool/ffmpeg.exe` relative to working directory
2. `tool/ffmpeg.exe` relative to executable
3. System PATH (`ffmpeg`)
4. User-specified path in Studio settings

### Web (Browser — WebGPU)

**Prerequisites**: wasm-pack, Node.js 18+, Chrome/Edge 113+ (WebGPU support).

**Step 1: Build WASM module**
```bash
cd crates/wasm
wasm-pack build --target web --release
```

**Step 2: Install web dependencies**
```bash
cd web
npm install
```

**Step 3: Start asset server** (serves video/image files)
```bash
python web/server.py
# Listens on http://localhost:8000
```

**Step 4: Start Vite dev server**
```bash
cd web
npm run dev
# Opens http://localhost:5173
```

**Web Architecture**:
```
Browser (Vite + JS)
  ├── main.js           → UI, playback loop (rAF + wall-clock timing)
  ├── ifol-render-wasm  → WASM module (WebGPU rendering)
  └── server.py         → Asset proxy + CLI export dispatcher
      └── ifol-render.exe export  (headless native export)
```

**Web Playback Pipeline**:
1. `video.play()` — browser hardware-decodes video continuously
2. `Canvas2D.drawImage(video)` → `getImageData()` — capture frame pixels
3. `renderer.cache_video_frame()` — copy RGBA to WASM HashMap
4. `renderer.render_frame_scaled()` — WebGPU composite + display

**Web Export** delegates to native CLI for full-speed FFmpeg encoding.

---

## Video Frame Performance

### Native — `update_rgba` texture reuse

Video frames use `update_rgba` instead of `load_rgba` to write new pixel data into an existing GPU texture. This eliminates 8MB alloc/dealloc per frame at 30fps (240MB/s VRAM churn → 0).

First frame: fallback to `load_rgba` (creates texture).
Subsequent frames: `queue.write_texture` directly into existing texture.

### Web — Known Limitation

`getImageData()` CPU readback is ~15-30ms/frame at 1280×720. Combined with WASM boundary crossing and GPU re-upload, total per-frame cost is ~40-80ms (vs 33ms budget at 30fps). This causes frame drops during playback.

**Future fix**: `WebGPU.copyExternalImageToTexture()` can import `HTMLVideoElement` directly to GPU texture without CPU readback, potentially reducing per-frame cost to <5ms.
