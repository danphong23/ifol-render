// Chromatic Aberration — RGB channel offset for a lens distortion look.
//
// Convention: vs_fullscreen + fs_main, bindings 0=uniform, 1=texture, 2=sampler

struct Params {
    intensity: f32,     // offset amount in UV space (0.005 = subtle)
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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    // Direction from center (radial offset)
    let center = vec2f(0.5, 0.5);
    let dir = in.uv - center;

    // Sample each channel with different offset
    let r = textureSample(t_input, t_sampler, in.uv + dir * params.intensity).r;
    let g = textureSample(t_input, t_sampler, in.uv).g;
    let b = textureSample(t_input, t_sampler, in.uv - dir * params.intensity).b;
    let a = textureSample(t_input, t_sampler, in.uv).a;

    return vec4f(r, g, b, a);
}
