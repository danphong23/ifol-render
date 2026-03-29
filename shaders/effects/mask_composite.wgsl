// Mask Composite — clips effect output to the original entity's alpha mask.
// Used for "Masked" ShaderScope: the effect (blur, glow, etc.) is applied but only
// visible within the original entity's alpha silhouette. Prevents bleeding outside.
//
// Binding layout (fullscreen_two_textures):
//   binding 0: uniforms (unused, required by bind group layout)
//   binding 1: effect_output — the post-effect texture (STRAIGHT alpha)
//   binding 2: sampler for effect_output
//   binding 3: original_source — the pre-effect entity render (PREMULTIPLIED alpha)
//   binding 4: sampler for original_source
//
// Pipeline: fullscreen_two_textures (no alpha blend)
// Output: STRAIGHT alpha — composite shader + hardware blend will premultiply

struct Uniforms {
    _pad: vec4f,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) uv: vec2f,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var effect_tex: texture_2d<f32>;
@group(0) @binding(2) var effect_sampler: sampler;
@group(0) @binding(3) var mask_tex: texture_2d<f32>;
@group(0) @binding(4) var mask_sampler: sampler;

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
    // Both Effect output and Mask are PREMULTIPLIED Alpha
    let effect = textureSample(effect_tex, effect_sampler, in.uv);
    let mask = textureSample(mask_tex, mask_sampler, in.uv);

    // Clip effect to mask's alpha silhouette.
    // Because effect is premultiplied, scaling the opacity means scaling BOTH rgb and a.
    return vec4f(effect.rgb * mask.a, effect.a * mask.a);
}
