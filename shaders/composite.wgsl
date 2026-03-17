// Composite shader — vertex + fragment.
// Draws a textured quad with transform and opacity.

struct Uniforms {
    transform: mat4x4f,
    color: vec4f,
    opacity: f32,
    use_texture: f32, // 1.0 = sample texture, 0.0 = use solid color
    _pad: vec2f,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var t_texture: texture_2d<f32>;
@group(0) @binding(2) var t_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2f,
    @location(1) uv: vec2f,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) uv: vec2f,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.transform * vec4f(in.position, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    var color: vec4f;
    if (uniforms.use_texture > 0.5) {
        color = textureSample(t_texture, t_sampler, in.uv);
    } else {
        color = uniforms.color;
    }
    color.a = color.a * uniforms.opacity;
    return color;
}
