// Color adjustment fragment shader.
// Applies brightness, contrast, saturation, and hue adjustments.

@group(0) @binding(0) var t_texture: texture_2d<f32>;
@group(0) @binding(1) var t_sampler: sampler;

struct ColorUniforms {
    brightness: f32,
    contrast: f32,
    saturation: f32,
    hue: f32,
}
@group(0) @binding(2) var<uniform> color: ColorUniforms;

struct TimeUniforms {
    frame_time: f32,
    global_time: f32,
    normalized_time: f32,
    delta_time: f32,
}
@group(0) @binding(3) var<uniform> time: TimeUniforms;

@fragment
fn fs_main(@location(0) uv: vec2f) -> @location(0) vec4f {
    var c = textureSample(t_texture, t_sampler, uv);

    // Brightness
    c = vec4f(c.rgb + vec3f(color.brightness), c.a);

    // Contrast
    c = vec4f((c.rgb - 0.5) * color.contrast + 0.5, c.a);

    // Saturation
    let luminance = dot(c.rgb, vec3f(0.2126, 0.7152, 0.0722));
    c = vec4f(mix(vec3f(luminance), c.rgb, color.saturation), c.a);

    // Clamp
    c = clamp(c, vec4f(0.0), vec4f(1.0));

    return c;
}
