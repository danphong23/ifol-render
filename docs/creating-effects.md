# Creating Custom Effects

## Tổng quan

Thêm effect = viết 1 file `.wgsl` + register vào render từ bên ngoài.
**Render không sở hữu shader.** Bên ngoài quản lý và truyền vào.

---

## Shader Convention

```wgsl
// binding 0: uniform params (float only, bội 16 bytes)
struct Params {
    intensity: f32,
    radius: f32,
    _pad0: f32,
    _pad1: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var t_input: texture_2d<f32>;
@group(0) @binding(2) var t_sampler: sampler;

// Fullscreen triangle (no VBO)
@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> VertexOutput { ... }

// Your effect logic
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f { ... }
```

---

## Register effect (bên ngoài → render)

### Từ code Rust

```rust
let wgsl = include_str!("../shaders/effects/my_effect.wgsl");
renderer.register_effect("my_effect", wgsl, vec![
    ("intensity".into(), 0.5),
    ("radius".into(), 3.0),
    ("_pad0".into(), 0.0),
    ("_pad1".into(), 0.0),
], 1);
```

### Từ file runtime

```rust
let wgsl = std::fs::read_to_string("custom_effect.wgsl").unwrap();
renderer.register_effect("custom", &wgsl, params, 1);
```

### Sử dụng

```rust
let effects = vec![EffectConfig {
    effect_type: "my_effect".into(),
    params: HashMap::from([("intensity".into(), 0.8)]),
}];
let pixels = renderer.render_frame_with_effects(&commands, &effects);
```

---

## Effects sẵn có (do core register)

| Effect | Params | Passes |
|--------|--------|--------|
| `blur` | direction_x, direction_y, radius, texel_size | 2 |
| `color_grade` | brightness, contrast, saturation | 1 |
| `vignette` | intensity, smoothness | 1 |
| `chromatic_aberration` | intensity | 1 |

**Tất cả do core/CLI register vào render khi init.** Render không biết chúng tồn tại cho tới khi register.
