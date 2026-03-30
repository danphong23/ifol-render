// Gradient Effect — fullscreen variant for material effect passes.
//
// Applies a gradient fill to the entity's visible area (alpha mask).
// Uses fullscreen triangle, no transform matrix.
//
// Params layout (alphabetical sort from material_sys):
//   For TC19: float_uniforms { u0_angle }, vec4_uniforms { u0_color1, u1_color2 }
//   Flattened with _0/_1/_2/_3 suffixes, sorted alphabetically:
//     u0_angle       → [0]
//     u0_color1_0    → [1]  (R)
//     u0_color1_1    → [2]  (G)
//     u0_color1_2    → [3]  (B)
//     u0_color1_3    → [4]  (A)
//     u1_color2_0    → [5]  (R)
//     u1_color2_1    → [6]  (G)
//     u1_color2_2    → [7]  (B)
//     u1_color2_3    → [8]  (A)
//   Padded to 12 floats (multiple of 4)

struct Params {
    angle: f32,
    color1_r: f32,
    color1_g: f32,
    color1_b: f32,
    color1_a: f32,
    color2_r: f32,
    color2_g: f32,
    color2_b: f32,
    color2_a: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var t_input: texture_2d<f32>;
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

fn gradient_linear(uv: vec2f, angle: f32) -> f32 {
    let dir = vec2f(cos(angle), sin(angle));
    let centered = uv - vec2f(0.5);
    return dot(centered, dir) + 0.5;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let original = textureSample(t_input, t_sampler, in.uv);

    let t = clamp(gradient_linear(in.uv, params.angle), 0.0, 1.0);

    let color1 = vec4f(params.color1_r, params.color1_g, params.color1_b, params.color1_a);
    let color2 = vec4f(params.color2_r, params.color2_g, params.color2_b, params.color2_a);
    let grad_color = mix(color1, color2, t);

    // Modulate: apply gradient fill within entity's alpha silhouette
    return vec4f(grad_color.rgb * original.a, grad_color.a * original.a);
}
