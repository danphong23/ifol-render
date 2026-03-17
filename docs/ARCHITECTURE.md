# Architecture

## Overview

ifol-render is a standalone GPU rendering engine with 5 layers:

```
Consumer (Workflow Builder, Editor, CLI, custom app)
    ↓ SceneDescription JSON
Layer 1: Scene API        Parse + validate input
    ↓
Layer 2: ECS              Resolve entities per frame (timeline, animation, transform)
    ↓
Layer 3: Render Graph     Build GPU execution DAG (passes, resources, sync)
    ↓
Layer 4: GPU Backend      wgpu execution (Vulkan/DX12/Metal/WebGPU)
    ↓
Layer 5: Output           Canvas (WASM) or RGBA bytes (native)
```

## Crate Dependencies

```
ifol-render-core   (no GPU dependency)
    ↑
ifol-render-gpu    (depends on core + wgpu)
    ↑
ifol-render-cli    (depends on core + gpu)
ifol-render-wasm   (depends on core + gpu + wasm-bindgen)
ifol-render-editor (depends on core + gpu + egui)
```

## Key Design Decisions

### 1. Core has no GPU dependency

`ifol-render-core` contains ECS, components, systems, datatypes, and color management. It has **zero GPU dependencies** — can be used standalone for scene processing, serialization, and testing.

### 2. SceneDescription as API contract

All consumers communicate through `SceneDescription` JSON. The engine does not know or care who the consumer is. This ensures:
- Workflow Builder can create scenes
- CLI can render scenes
- Editor can edit scenes
- Custom apps can generate scenes programmatically

### 3. Render Graph DAG

The render graph is separate from ECS. ECS resolves logical state → bridge builds render graph → GPU executes passes. This separation allows:
- Independent optimization of ECS and rendering
- Multi-pass effects (blur, bloom, shadow)
- Resource lifetime management
- GPU synchronization

### 4. Effect extensibility via plugin pattern

New effects = new files. The registry auto-discovers render passes and shaders:
- `shaders/*.wgsl` — loaded at runtime
- `crates/gpu/src/passes/*.rs` — compiled, implements `RenderPass` trait

### 5. Color management from day one

All internal processing in linear color space. Input/output automatically converted. This ensures:
- Correct blending (must be in linear space)
- Support for wide gamut (ACEScg, Rec.2020)
- Correct display on different monitor profiles
