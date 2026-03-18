// Color grading — brightness, contrast, saturation adjustments.
//
// Convention: vs_fullscreen + fs_main, bindings 0=uniform, 1=texture, 2=sampler

struct Params {
    brightness: f32,    // -1.0 to 1.0 (0 = no change)
    contrast: f32,      // 0.0 to 2.0 (1.0 = no change)
    saturation: f32,    // 0.0 to 2.0 (1.0 = no change)
    _pad: f32,
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
    var color = textureSample(t_input, t_sampler, in.uv);

    // Brightness
    color = vec4f(color.rgb + vec3f(params.brightness), color.a);

    // Contrast (pivot at 0.5)
    color = vec4f((color.rgb - 0.5) * params.contrast + 0.5, color.a);

    // Saturation (BT.709 luminance)
    let lum = dot(color.rgb, vec3f(0.2126, 0.7152, 0.0722));
    color = vec4f(mix(vec3f(lum), color.rgb, params.saturation), color.a);

    return clamp(color, vec4f(0.0), vec4f(1.0));
}
