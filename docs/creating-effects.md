# Creating Custom Effects

## Tổng quan

Render tool hỗ trợ thêm effect mới bằng cách **chỉ viết 1 file .wgsl** — không cần code Rust.

## Shader Convention

Mọi effect phải tuân theo layout bindings:

```wgsl
// binding 0: uniform buffer — params float của bạn
struct Params {
    intensity: f32,
    radius: f32,
    _pad0: f32,        // padding tới bội 16 bytes
    _pad1: f32,
}

// binding 1: texture input (ping-pong)
// binding 2: sampler (linear, clamp-to-edge)
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var t_input: texture_2d<f32>;
@group(0) @binding(2) var t_sampler: sampler;

// Vertex shader: fullscreen triangle (3 vertices, no VBO)
struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) uv: vec2f,
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(vi) / 2) * 4.0 - 1.0;
    let y = f32(i32(vi) % 2) * 4.0 - 1.0;
    out.clip_position = vec4f(x, y, 0.0, 1.0);
    out.uv = vec2f((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// Fragment shader: your effect logic
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let color = textureSample(t_input, t_sampler, in.uv);
    // ... apply effect ...
    return color;
}
```

## Quy tắc quan trọng

| Quy tắc | Lý do |
|---------|-------|
| Vertex entry = `vs_fullscreen` | Pipeline cache tìm theo tên |
| Fragment entry = `fs_main` | Pipeline cache tìm theo tên |
| Params struct = chỉ `f32` | Generic engine pack float array |
| Struct size = bội 16 bytes | GPU uniform alignment |
| binding 0/1/2 = fixed layout | Bind group layout chung |

## Thêm effect built-in

### Bước 1: Tạo shader

```
shaders/effects/glow.wgsl
```

### Bước 2: Register trong EffectRegistry

```rust
// render/src/effects/mod.rs — register_builtins()
self.register(EffectEntry {
    name: "glow".into(),
    shader_source: include_str!("../../../shaders/effects/glow.wgsl").into(),
    default_params: vec![
        ("intensity".into(), 0.5),
        ("threshold".into(), 0.8),
        ("_pad0".into(), 0.0),
        ("_pad1".into(), 0.0),
    ],
    pass_count: 1,
});
```

**Xong.** Không cần viết thêm bất kỳ file Rust nào.

## Thêm effect custom (runtime)

```rust
let wgsl = std::fs::read_to_string("my_custom_effect.wgsl").unwrap();
renderer.register_effect(
    "my_custom_effect",
    wgsl,
    vec![
        ("param1".into(), 0.5),
        ("param2".into(), 1.0),
        ("_pad0".into(), 0.0),
        ("_pad1".into(), 0.0),
    ],
    1,  // number of passes
);
```

## Multi-pass effect (ví dụ: blur)

Blur cần 2 pass (horizontal + vertical). Set `pass_count: 2` và render engine tự quản lý direction per pass.

Nếu effect custom cần multi-pass, render engine sẽ:
1. Pass 0: render vào ping-pong output
2. Swap ping-pong (output → input)
3. Pass 1: render vào ping-pong output
4. Swap...

## Sử dụng từ Core (ECS)

```json
{
  "effectStack": {
    "effects": [
      { "type": "blur", "params": { "radius": 5.0 } },
      { "type": "vignette", "params": { "intensity": 0.3 } }
    ]
  }
}
```

Core chuyển thành `EffectConfig[]` → gửi cho `render_frame_with_effects()`.

## Effects sẵn có

| Effect | Params | Passes |
|--------|--------|--------|
| `blur` | direction_x, direction_y, radius, texel_size | 2 |
| `color_grade` | brightness, contrast, saturation | 1 |
| `vignette` | intensity, smoothness | 1 |
| `chromatic_aberration` | intensity | 1 |
