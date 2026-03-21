# Contributing to ifol-render

Thank you for your interest in contributing! We welcome all contributions — bug reports, feature requests, documentation improvements, and code contributions.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/ifol-render.git`
3. Create a branch: `git checkout -b feature/my-feature`
4. Make your changes
5. Run checks: `cargo check --workspace && cargo test --workspace`
6. Commit: `git commit -m "feat: add my feature"`
7. Push: `git push origin feature/my-feature`
8. Open a Pull Request

## Development Setup

### Prerequisites

- [Rust](https://rustup.rs/) 1.85+ (stable)
- GPU with Vulkan/DX12/Metal driver
- [FFmpeg](https://ffmpeg.org/) (for video export)

### Build

```bash
cargo build --workspace        # Build all crates
cargo run -p ifol-render-studio  # Run the Studio GUI
cargo test --workspace         # Run all tests
```

### Shader Development

Shaders in `shaders/` are WGSL files loaded at compile time via `include_str!()`. Edit → save → rebuild.

## Project Structure

| Crate | Purpose |
|---|---|
| `render/` | wgpu GPU executor — pure draw command execution |
| `core/` | Core engine — shaders, textures, video decode, export pipeline |
| `audio/` | Audio mixing and muxing (standalone crate) |
| `studio/` | Desktop GUI editor (egui + wgpu) |
| `crates/cli/` | Headless CLI — render, export, GPU tests |
| `crates/wasm/` | WASM target for browser (WebGPU) |
| `crates/server/` | HTTP server for web asset serving |
| `sdk/` | TypeScript SDK — produces Frame JSON for Core |

## Coding Guidelines

### Rust Style

- Follow `rustfmt` defaults (`cargo fmt`)
- Use `clippy` (`cargo clippy --workspace`)
- Doc comments on all public items
- Use `thiserror` for error types

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` — new feature
- `fix:` — bug fix
- `docs:` — documentation
- `refactor:` — code changes that don't add/fix
- `perf:` — performance improvement
- `test:` — adding tests
- `chore:` — maintenance

### Adding a New Shader

1. Create `shaders/my_shader.wgsl` with vertex + fragment shader
2. Register in `core/src/lib.rs` → `setup_builtins()` using `register_pipeline()` or `register_effect()`
3. Use in Frame JSON: `entity.shader = "my_shader"`
4. No render crate changes needed — render is shader-agnostic

### Adding a New Effect

1. Create `shaders/effects/my_effect.wgsl` with fullscreen fragment shader
2. Register via `renderer.register_effect("my_effect", wgsl, params, pass_count)`
3. Use via `EffectConfig { effect_type: "my_effect", params: {...} }`

## Pull Request Process

1. Ensure `cargo check --workspace` passes
2. Ensure `cargo test --workspace` passes
3. Ensure `cargo clippy --workspace` has no warnings
4. Ensure `cargo fmt --all -- --check` passes
5. Update documentation if needed

## Code of Conduct

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Be respectful and constructive.

## License

By contributing, you agree that your contributions will be licensed under the MIT OR Apache-2.0 license.
