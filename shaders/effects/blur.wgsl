// Gaussian Blur — separable 2-pass (horizontal + vertical).
// Uses a 9-tap kernel with hardcoded weights for quality.
//
// Convention: vs_fullscreen + fs_main, bindings 0=uniform, 1=texture, 2=sampler

struct Params {
    direction_x: f32,
    direction_y: f32,
    radius: f32,
    texel_size: f32,
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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let direction = vec2f(params.direction_x, params.direction_y);
    let step = direction * params.texel_size * params.radius;

    // 9-tap Gaussian weights (sigma ≈ 2.0)
    let w0 = 0.2270270270;
    let w1 = 0.1945945946;
    let w2 = 0.1216216216;
    let w3 = 0.0540540541;
    let w4 = 0.0162162162;

    var result = textureSample(t_input, t_sampler, in.uv) * w0;
    result += textureSample(t_input, t_sampler, in.uv + step * 1.0) * w1;
    result += textureSample(t_input, t_sampler, in.uv - step * 1.0) * w1;
    result += textureSample(t_input, t_sampler, in.uv + step * 2.0) * w2;
    result += textureSample(t_input, t_sampler, in.uv - step * 2.0) * w2;
    result += textureSample(t_input, t_sampler, in.uv + step * 3.0) * w3;
    result += textureSample(t_input, t_sampler, in.uv - step * 3.0) * w3;
    result += textureSample(t_input, t_sampler, in.uv + step * 4.0) * w4;
    result += textureSample(t_input, t_sampler, in.uv - step * 4.0) * w4;

    return result;
}
