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
    // High-quality single-pass Golden Spiral blur
    // This replaces the old 2-pass separable blur with a much more beautiful radial bokeh
    var result = vec4f(0.0);
    let golden_angle = 2.39996323; // ~137.5 degrees
    let taps = 32.0;
    var total_weight = 0.0;

    for (var i = 0.0; i < taps; i = i + 1.0) {
        // Square root of distance ensures uniform density of samples in the circle
        let r = sqrt(i / taps) * params.radius * params.texel_size;
        let theta = i * golden_angle;
        let offset = vec2f(cos(theta), sin(theta)) * r;
        
        result += textureSample(t_input, t_sampler, in.uv + offset);
        total_weight += 1.0;
    }

    return result / total_weight;
}
