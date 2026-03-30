// Color grading — tint, brightness, contrast, saturation adjustments.
//
// Convention: vs_fullscreen + fs_main, bindings 0=uniform, 1=texture, 2=sampler
//
// Params layout (alphabetical sort from material_sys):
//   vec4_uniforms { u0_tint } → u0_tint_0(R), u0_tint_1(G), u0_tint_2(B), u0_tint_3(A)
//   float_uniforms { u1_contrast, u2_saturation, u3_brightness }
//   Sorted: u0_tint_0, u0_tint_1, u0_tint_2, u0_tint_3, u1_contrast, u2_saturation, u3_brightness, pad
//
// When used WITHOUT vec4 tint (legacy mode with only float uniforms):
//   float_uniforms { brightness, contrast, saturation }
//   Sorted: brightness, contrast, saturation, pad
//   This still works because the struct layout starts at the beginning.

struct Params {
    tint_r: f32,
    tint_g: f32,
    tint_b: f32,
    tint_a: f32,
    contrast: f32,
    saturation: f32,
    brightness: f32,
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

    // Tint (multiply by tint color)
    let tint = vec3f(params.tint_r, params.tint_g, params.tint_b);
    // Only apply tint if it's non-zero (tint_a > 0 signals tint is active)
    if (params.tint_a > 0.001) {
        color = vec4f(color.rgb * tint, color.a);
    }

    // Brightness (multiplicative, 1.0 = neutral)
    color = vec4f(color.rgb * params.brightness, color.a);

    // Contrast (pivot at 0.5)
    color = vec4f((color.rgb - 0.5) * params.contrast + 0.5, color.a);

    // Saturation (BT.709 luminance)
    let lum = dot(color.rgb, vec3f(0.2126, 0.7152, 0.0722));
    color = vec4f(mix(vec3f(lum), color.rgb, params.saturation), color.a);

    return clamp(color, vec4f(0.0), vec4f(1.0));
}
