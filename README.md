# ifol-render

A standalone, extensible **GPU rendering engine** for video compositing, animation, and real-time graphics. Built with Rust + [wgpu](https://wgpu.rs/).

> **ifol-render** is an independent engine. It can be used by any consumer — a workflow tool, a standalone editor, a CLI, or your own application.

![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)
![Rust](https://img.shields.io/badge/rust-1.85%2B-orange)

## Features

| Feature | Description |
|---------|-------------|
| **GPU Rendering** | wgpu auto-selects Vulkan, DX12, Metal, or WebGPU |
| **ECS Architecture** | Entity-Component-System with parent-child hierarchy |
| **Compositing** | Layer-based compositing with opacity, blending, and z-ordering |
| **Animation** | Keyframe animation with CubicBezier, EaseIn/Out easing |
| **Color Management** | sRGB, Linear sRGB, ACEScg, Rec.709, Rec.2020, Display P3 |
| **Export Pipeline** | FFmpeg integration — H264, H265, VP9, ProRes, PNG sequence |
| **Studio Editor** | Professional GUI with viewport, timeline, entity list, properties |
| **CLI Tool** | Headless rendering, preview, export, and scene info |
| **Scene I/O** | JSON-based scene format with full round-trip save/load |

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) 1.85+
- GPU with Vulkan, DX12, or Metal support
- [FFmpeg](https://ffmpeg.org/) (optional — for video export)

### Run the Studio

```bash
cargo run -p ifol-render-studio
```

### CLI Usage

```bash
# Show scene info
cargo run -p ifol-render-cli -- info -s examples/test_render.json

# Preview a frame at timestamp 2.5s
cargo run -p ifol-render-cli -- preview -s examples/test_render.json -t 2.5 -o preview.png

# Export video (H264)
cargo run -p ifol-render-cli -- export -s examples/test_render.json -o output.mp4 -c h264

# Export with custom FFmpeg path
cargo run -p ifol-render-cli -- export -s examples/test_render.json -o output.mp4 --ffmpeg path/to/ffmpeg
```

## Architecture

```
ifol-render/
├── core/           ECS, components, systems, scene API, color, export pipeline
├── render/         wgpu GPU engine, render graph, resource manager, passes
├── studio/         Professional GUI editor (egui + egui_tiles)
├── crates/
│   ├── cli/        Command-line rendering and export tool
│   └── wasm/       WebAssembly target for browser integration
├── shaders/        WGSL shader files
├── examples/       Example scene files (.json)
└── docs/           Architecture and guides
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full technical overview.

## Scene Format

Scenes are described in JSON via `SceneDescription`:

```json
{
  "version": "1.0",
  "settings": {
    "width": 1920, "height": 1080,
    "fps": 30, "duration": 10.0,
    "colorSpace": "linearSrgb",
    "outputColorSpace": "srgb"
  },
  "entities": [
    {
      "id": "background",
      "components": {
        "colorSource": { "color": { "r": 0.1, "g": 0.1, "b": 0.15, "a": 1.0 } },
        "timeline": { "startTime": 0.0, "duration": 10.0, "layer": 0 },
        "transform": { "position": { "x": 0.0, "y": 0.0 }, "scale": { "x": 1.0, "y": 1.0 } }
      }
    },
    {
      "id": "animated_box",
      "components": {
        "colorSource": { "color": { "r": 0.9, "g": 0.2, "b": 0.3, "a": 1.0 } },
        "timeline": { "startTime": 0.0, "duration": 10.0, "layer": 1 },
        "transform": { "position": { "x": 0.0, "y": 0.0 }, "scale": { "x": 0.3, "y": 0.3 } },
        "parent": "background",
        "animation": {
          "keyframes": [
            { "time": 0.0, "property": "opacity", "value": 0.0, "easing": "easeOut" },
            { "time": 2.0, "property": "opacity", "value": 1.0, "easing": "linear" }
          ]
        }
      }
    }
  ]
}
```

## Studio Features

| Panel | Capabilities |
|-------|-------------|
| **Viewport** | Real-time preview, grid overlay, safe zones, resolution badge |
| **Entity List** | Add/delete entities, multi-select (Ctrl/Shift+Click), batch delete |
| **Properties** | Transform, color, timeline, opacity editing with undo/redo |
| **Timeline** | NLE-style tracks, playhead scrub, click/drag ruler seek, zoom |
| **Top Bar** | File menu, save/open, FFmpeg settings, workspace controls |

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Space` | Play / Pause |
| `Ctrl+Z` | Undo |
| `Ctrl+Y` | Redo |
| `Ctrl+S` | Save |
| `Delete` | Delete selected entity |

## Export

Export supports multiple codecs via FFmpeg:

| Format | Codec | Extension |
|--------|-------|-----------|
| H.264 | libx264 | `.mp4` |
| H.265 | libx265 | `.mp4` |
| VP9 | libvpx-vp9 | `.webm` |
| ProRes | prores_ks | `.mov` |
| PNG Sequence | png | directory |

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines. All contributions are welcome!
