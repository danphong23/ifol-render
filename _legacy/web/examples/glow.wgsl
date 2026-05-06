// Outer Glow / Inner Glow effect
// Creates a soft luminous halo around alpha edges of the source entity.
//
// Params:
//   r, g, b, a  — glow color + opacity
//   size         — glow spread in pixels
//   intensity    — brightness multiplier (1.0 = normal, 2.0 = bright)
//   pad1, pad2   — unused padding
//
// Pipeline: fullscreen effect pass (no alpha blend, writes directly)
// Input: premultiplied-alpha entity texture from base pass
// Output: STRAIGHT alpha — composite shader + hardware SrcAlpha blend will premultiply

struct Params {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
    size: f32,
    intensity: f32,
    pad1: f32,
    pad2: f32,
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
    let dims = textureDimensions(t_input);
    let texel = vec2f(1.0 / f32(dims.x), 1.0 / f32(dims.y));
    
    // Read original pixel (premultiplied alpha from base pass)
    let original = textureSample(t_input, t_sampler, in.uv);
    // Keep original pixel AS IS (it's already premultiplied)
    let orig_premult = original.rgb;
    let orig_a = original.a;
    
    // ── Build glow from alpha channel ──
    let glow_color = vec3f(params.r, params.g, params.b);
    let spread = params.size; // in pixels
    
    var glow_sum = 0.0;
    var weight_sum = 0.0;
    let pi2 = 6.28318530718;
    
    // Center tap
    glow_sum += orig_a * 1.5;
    weight_sum += 1.5;
    
    // Ring 1: 8 taps at 33% radius — tight core for intensity
    for (var i = 0u; i < 8u; i = i + 1u) {
        let angle = f32(i) * pi2 / 8.0;
        let offset = vec2f(cos(angle), sin(angle)) * spread * 0.33 * texel;
        let a = textureSample(t_input, t_sampler, in.uv + offset).a;
        glow_sum += a * 1.0;
        weight_sum += 1.0;
    }
    
    // Ring 2: 12 taps at 66% radius — mid falloff
    for (var i = 0u; i < 12u; i = i + 1u) {
        let angle = f32(i) * pi2 / 12.0;
        let offset = vec2f(cos(angle), sin(angle)) * spread * 0.66 * texel;
        let a = textureSample(t_input, t_sampler, in.uv + offset).a;
        glow_sum += a * 0.6;
        weight_sum += 0.6;
    }
    
    // Ring 3: 16 taps at 100% radius — soft outer fringe
    for (var i = 0u; i < 16u; i = i + 1u) {
        let angle = f32(i) * pi2 / 16.0;
        let offset = vec2f(cos(angle), sin(angle)) * spread * 1.0 * texel;
        let a = textureSample(t_input, t_sampler, in.uv + offset).a;
        glow_sum += a * 0.25;
        weight_sum += 0.25;
    }
    
    // Normalize and apply intensity + glow opacity
    let glow_alpha = saturate((glow_sum / weight_sum) * params.intensity * params.a);
    let glow_premult = glow_color * glow_alpha;
    
    // ── Composite: Original OVER Glow (in Premultiplied-alpha space) ──
    // Standard source-over for premultiplied colors:
    let out_a = orig_a + glow_alpha * (1.0 - orig_a);
    let out_rgb_premult = orig_premult + glow_premult * (1.0 - orig_a);
    
    // Output PREMULTIPLIED alpha — wgpu BlendState::ALPHA_BLENDING uses SrcFactor::One
    return vec4f(out_rgb_premult, out_a);
}
