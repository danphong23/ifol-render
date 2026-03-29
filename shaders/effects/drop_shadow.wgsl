// Drop Shadow effect
// Creates a colored shadow offset from the source entity's alpha silhouette.
//
// Params:
//   r, g, b, a    — shadow color + opacity
//   offset_x      — horizontal offset in pixels
//   offset_y      — vertical offset in pixels
//   blur           — blur radius in pixels (softness of shadow edge)
//   pad1           — unused padding
//
// Pipeline: fullscreen effect pass (no alpha blend, writes directly)
// Input: premultiplied-alpha entity texture from base pass
// Output: STRAIGHT alpha — composite shader + hardware SrcAlpha blend will premultiply

struct Params {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
    offset_x: f32,
    offset_y: f32,
    blur: f32,
    pad1: f32,
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
    let shadow_color = vec3f(params.r, params.g, params.b);
    
    // Offset UV for shadow sampling (negative to place shadow behind)
    let shadow_uv = in.uv + vec2f(-params.offset_x, -params.offset_y) * texel;
    let blur_spread = params.blur; // in pixels
    
    // ── Sample shadow alpha ──
    var shadow_sum = 0.0;
    var weight_sum = 0.0;
    let pi2 = 6.28318530718;
    
    if (blur_spread < 0.5) {
        // No blur — sharp shadow
        shadow_sum = textureSample(t_input, t_sampler, shadow_uv).a;
        weight_sum = 1.0;
    } else {
        // Center tap
        shadow_sum += textureSample(t_input, t_sampler, shadow_uv).a * 1.5;
        weight_sum += 1.5;
        
        // Ring 1: 8 taps at 33% radius
        for (var i = 0u; i < 8u; i = i + 1u) {
            let angle = f32(i) * pi2 / 8.0;
            let offset = vec2f(cos(angle), sin(angle)) * blur_spread * 0.33 * texel;
            shadow_sum += textureSample(t_input, t_sampler, shadow_uv + offset).a * 1.0;
            weight_sum += 1.0;
        }
        
        // Ring 2: 12 taps at 66% radius
        for (var i = 0u; i < 12u; i = i + 1u) {
            let angle = f32(i) * pi2 / 12.0;
            let offset = vec2f(cos(angle), sin(angle)) * blur_spread * 0.66 * texel;
            shadow_sum += textureSample(t_input, t_sampler, shadow_uv + offset).a * 0.6;
            weight_sum += 0.6;
        }
        
        // Ring 3: 16 taps at 100% radius
        for (var i = 0u; i < 16u; i = i + 1u) {
            let angle = f32(i) * pi2 / 16.0;
            let offset = vec2f(cos(angle), sin(angle)) * blur_spread * texel;
            shadow_sum += textureSample(t_input, t_sampler, shadow_uv + offset).a * 0.25;
            weight_sum += 0.25;
        }
    }
    
    let shadow_alpha = saturate((shadow_sum / weight_sum) * params.a);
    
    // Read original pixel (premultiplied alpha)
    let original = textureSample(t_input, t_sampler, in.uv);
    
    // Un-premultiply original to get straight RGB
    let orig_rgb = original.rgb / max(original.a, 0.001);
    let orig_a = original.a;
    
    // ── Composite: Original OVER Shadow (in straight-alpha space) ──
    let out_a = orig_a + shadow_alpha * (1.0 - orig_a);
    
    var out_rgb = shadow_color;
    if (out_a > 0.001) {
        out_rgb = (orig_rgb * orig_a + shadow_color * shadow_alpha * (1.0 - orig_a)) / out_a;
    }
    
    // Output STRAIGHT alpha
    return vec4f(out_rgb, out_a);
}
