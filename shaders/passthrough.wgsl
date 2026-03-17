// Passthrough fragment shader.
// Samples a texture and applies opacity.

@group(0) @binding(0) var t_texture: texture_2d<f32>;
@group(0) @binding(1) var t_sampler: sampler;

struct Uniforms {
    transform: mat4x4f,
    opacity: f32,
    _padding: vec3f,
}
@group(0) @binding(2) var<uniform> uniforms: Uniforms;

@fragment
fn fs_main(@location(0) uv: vec2f) -> @location(0) vec4f {
    let color = textureSample(t_texture, t_sampler, uv);
    return vec4f(color.rgb, color.a * uniforms.opacity);
}
