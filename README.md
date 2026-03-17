# ifol-render

A standalone, extensible **GPU rendering engine** for video compositing, animation, and real-time graphics. Built with Rust + [wgpu](https://wgpu.rs/).

> **ifol-render** is an independent engine. It can be used by any consumer — a workflow tool, a standalone editor, a CLI, or your own application.

## Features

- **Cross-platform GPU**: wgpu auto-selects Vulkan, DX12, Metal, or WebGPU
- **ECS architecture**: Entity-Component-System for scene management
- **Render Graph DAG**: Dependency-tracked GPU pass execution
- **Color management**: sRGB, Linear, ACEScg, Rec.709, Rec.2020, Display P3
- **Time-aware shaders**: Built-in time uniforms auto-injected every frame
- **Extensible**: Add effects, passes, and components by adding files — no core changes
- **Dual target**: Compile to WASM (browser preview) or native binary (headless export)
- **Standalone editor**: Built-in GUI with entity inspector, timeline, and viewport

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (1.85+)
- GPU with Vulkan, DX12, or Metal support

### Run the Editor

```bash
cargo run -p ifol-render-editor
```

### Run CLI

```bash
# Show scene info
cargo run -p ifol-render-cli -- info --scene examples/simple_scene.json

# Render a single frame
cargo run -p ifol-render-cli -- preview --scene examples/simple_scene.json --timestamp 2.0

# Render all frames
cargo run -p ifol-render-cli -- render --scene examples/simple_scene.json --fps 30
```

## Architecture

```
ifol-render/
├── crates/
│   ├── core/     ECS, components, systems, scene API, color, datatypes
│   ├── gpu/      wgpu engine, render graph, resource manager, passes
│   ├── wasm/     WebAssembly target for browser integration
│   └── cli/      Command-line rendering tool
├── editor/       Standalone GUI editor (egui)
├── shaders/      WGSL shader files (runtime loaded)
└── examples/     Example scenes
```

See [ARCHITECTURE.md](docs/ARCHITECTURE.md) for a detailed technical overview.

## Scene Format

Consumers interact with the engine via `SceneDescription` (JSON):

```json
{
  "version": "1.0",
  "settings": { "width": 1920, "height": 1080, "fps": 30, "duration": 10.0 },
  "entities": [
    {
      "id": "my_clip",
      "components": {
        "videoSource": { "path": "video.mp4" },
        "timeline": { "startTime": 0.0, "duration": 10.0, "layer": 0 },
        "transform": { "position": [960, 540], "scale": [1.0, 1.0] },
        "opacity": 1.0
      }
    }
  ]
}
```

## Adding Effects

Add a new effect by creating two files:

1. **Shader**: `shaders/my_effect.wgsl`
2. **Pass**: `crates/gpu/src/passes/my_effect.rs`

No core code changes needed. See [docs/creating-effects.md](docs/creating-effects.md).

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines. All contributions are welcome!
