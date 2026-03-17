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

### Build

```bash
cargo build --workspace        # Build all crates
cargo run -p ifol-render-editor  # Run the editor
cargo test --workspace         # Run all tests
```

### Shader Development

Shaders in `shaders/` are loaded at runtime. Edit → save → restart (or hot-reload when implemented).

## Project Structure

| Crate | Purpose |
|---|---|
| `crates/core` | ECS, components, systems, datatypes, color management |
| `crates/gpu` | wgpu engine, render graph, resource manager |
| `crates/wasm` | WASM target for browser integration |
| `crates/cli` | CLI rendering tool |
| `editor` | Standalone GUI editor (egui) |

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

### Adding a New Effect

1. Create `shaders/my_effect.wgsl` with the fragment shader
2. Create `crates/gpu/src/passes/my_effect.rs` implementing `RenderPass`
3. Register in `crates/gpu/src/passes/mod.rs`
4. Add tests

### Adding a New Component

1. Add component struct in `crates/core/src/ecs/components.rs`
2. Add field to `Components` struct in `crates/core/src/ecs/mod.rs`
3. Add system in `crates/core/src/ecs/systems.rs` if needed
4. Register in pipeline in `crates/core/src/ecs/pipeline.rs`

## Pull Request Process

1. Ensure `cargo check --workspace` passes
2. Ensure `cargo test --workspace` passes
3. Ensure `cargo clippy --workspace` has no warnings
4. Update documentation if needed
5. Add yourself to contributors (optional)

## Code of Conduct

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Be respectful and constructive.

## License

By contributing, you agree that your contributions will be licensed under the MIT OR Apache-2.0 license.
