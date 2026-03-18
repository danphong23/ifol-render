// Vignette — darken edges of the frame.
//
// Convention: vs_fullscreen + fs_main, bindings 0=uniform, 1=texture, 2=sampler

struct Params {
    intensity: f32,     // 0.0 to 1.0 (0 = no vignette)
    smoothness: f32,    // 0.0 to 1.0 (higher = softer edge)
    _pad0: f32,
    _pad1: f32,
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
    let color = textureSample(t_input, t_sampler, in.uv);

    // Distance from center (0,0 at center, ~0.7 at corners)
    let center = vec2f(0.5, 0.5);
    let dist = distance(in.uv, center) * 1.414; // normalize to ~1.0 at corners

    // Smooth vignette falloff
    let vignette = smoothstep(1.0 - params.smoothness, 1.0, dist * (1.0 + params.intensity));
    let factor = 1.0 - vignette * params.intensity;

    return vec4f(color.rgb * factor, color.a);
}
