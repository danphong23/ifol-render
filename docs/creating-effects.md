# Custom Render Effects / Materials

## Tổng quan (Khung kiến trúc V4)

Kể từ IFOL Render V4, Engine sử dụng hệ thống ECS và cơ chế Pass-based Rendering. Các Effect (ví dụ: Glow, Blur, Drop Shadow) được thiết kế theo dạng pipeline **Material Shaders**.
- **Render Backend không sở hữu logic shader.** Nó chỉ nhận các lệnh DrawCommand qua `render_frame_to` hoặc `render_frame`.
- Core Engine (`core/src/shaders.rs`) đóng vai trò biên dịch (compile) và đăng ký các Effect Shader vào Renderer dưới dạng Ping-Pong texturing pass.
- Các Shaders bắt buộc tuân thủ toán học **Premultiplied Alpha** vì `wgpu::BlendState::ALPHA_BLENDING` được cấu hình ngầm định là `SrcFactor::One`.

---

## Tiêu chuẩn Material Shader (WGSL)

Mọi Material Shader phải:
1. Nhận uniform params là block float (bội số của 16 bytes).
2. Viết dạng Fullscreen Triangle Pass.
3. Nhận đầu vào là **Premultiplied Alpha** từ Base Render Pass.
4. Xử lý blend gốc (OVER) và **BUỘC TRẢ VỀ PREMULTIPLIED ALPHA** (`vec4f(rgb * a, a)`). Không áp dụng Straight Alpha.

### Cấu trúc cơ bản

```wgsl
// binding 0: uniform params 
struct Params {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
    // phải có padding để chia hết 16 bytes
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var t_input: texture_2d<f32>; // Tạm chứa ảnh gốc đã premultiplied alpha
@group(0) @binding(2) var t_sampler: sampler;

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let original = textureSample(t_input, t_sampler, in.uv);
    let orig_premult = original.rgb;
    let orig_a = original.a;
    
    // Thuật toán ví dụ: tính shadow alpha
    let shadow_alpha = 0.5 * params.a;
    let shadow_premult = vec3f(params.r, params.g, params.b) * shadow_alpha;

    // A OVER B bằng công thức Premultiplied
    let out_a = orig_a + shadow_alpha * (1.0 - orig_a);
    let out_rgb_premult = orig_premult + shadow_premult * (1.0 - orig_a);
    
    return vec4f(out_rgb_premult, out_a);
}
```

---

## Tích hợp vào ECS V4 (JSON -> Engine)

Bạn khai báo shader từ bên ngoài (JSON scene). Core tự động load và render.

```json
{
    "id": "my_text",
    "materials": [
        {
            "shader_id": "glow",
            "scope": "padded",
            "float_uniforms": {
                // Đủ 8 key theo đúng thứ tự struct Params trong shaders.rs
                "u0_r": { "keyframes": [{"time": 0, "value": 1.0}] },
                "u1_g": { "keyframes": [{"time": 0, "value": 0.0}] },
                "u2_b": { "keyframes": [{"time": 0, "value": 1.0}] },
                "u3_a": { "keyframes": [{"time": 0, "value": 1.0}] },
                "u4_size": { "keyframes": [{"time": 0, "value": 10.0}] },
                "u5_intent": { "keyframes": [{"time": 0, "value": 1.0}] },
                "u6_pad": { "keyframes": [{"time": 0, "value": 0}] },
                "u7_pad": { "keyframes": [{"time": 0, "value": 0}] }
            }
        }
    ]
}
```

### Shader Scope
Có hai kiểu scope khi áp dụng materials:
1. `"padded"`: Render Box được mở rộng (padding). Cần thiết cho Blur, Drop Shadow, Glow vì phần mờ sẽ lan tỏa ra ngoài kích thước entity gốc. Chạy file compositing `composite.wgsl`.
2. `"masked"` / `"clipped"`: Effect bị cắt cứng khuôn trong phạm vi Alpha ban đầu của Entity gốc. Chạy file compositing `mask_composite.wgsl` - sử dụng PREMULTIPLIED ALPHA nhân với mask alpha gốc.

---

## Core Engine: Đăng ký Shader

Tất cả các Effect mặc định hiện được đăng ký cứng (Built-in) tại `crates/core/src/shaders.rs` `register_builtin_shaders()`. Nếu cần thêm builtin effect, bạn cần thêm file .wgsl vào `core/src/shaders/effects/` và register tại đây. 

```rust
pub fn register_builtin_shaders(engine: &mut CoreEngine) {
    let wgsl = include_str!("../shaders/effects/my_effect.wgsl");
    engine.renderer_mut().register_pipeline("my_effect", PipelineConfig {
        shader_src: wgsl.to_string(),
        // Các config khác...
    });
}
```

Danh sách Effects sẵn có (đã test kỹ chuẩn alpha):
- `blur` (Độ mờ lan / Gaussian theo góc)
- `glow` (Phát sáng neon - Premultiplied Alpha)
- `drop_shadow` (Đổ bóng theo offset - Premultiplied Alpha)
- `composite`, `mask_composite` (Hệ thống compositing hệ màu v4)
