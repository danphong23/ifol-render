// Selection Outline Effect
// Reads alpha edge and renders a cyan stroke.

struct Params {
    thickness: f32, // Pixel distance to sample outward
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
    let color = textureSample(t_input, t_sampler, in.uv);
    
    // Sample all 8 directions unconditionally (uniform control flow)
    let dimensions = textureDimensions(t_input);
    let texel_size = vec2f(1.0 / f32(dimensions.x), 1.0 / f32(dimensions.y));
    
    var outline_alpha = 0.0;
    let steps = 8.0;
    
    for (var i = 0.0; i < steps; i += 1.0) {
        let angle = (i / steps) * 6.2831853;
        let offset = vec2f(cos(angle), sin(angle)) * texel_size * params.thickness;
        let neighbor = textureSample(t_input, t_sampler, in.uv + offset);
        outline_alpha = max(outline_alpha, neighbor.a);
    }
    
    // OUTER EDGE ONLY: if pixel is inside the entity, output transparent
    if (color.a > 0.05) {
        return vec4f(0.0, 0.0, 0.0, 0.0);
    }
    
    // If neighbor has content but this pixel doesn't, draw cyan outline
    if (outline_alpha > 0.05) {
        return vec4f(0.0, 0.898, 1.0, outline_alpha * 0.9);
    }
    
    return vec4f(0.0, 0.0, 0.0, 0.0);
}
