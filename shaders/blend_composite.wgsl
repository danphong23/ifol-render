// blend_composite.wgsl — 2-pass accurate blend mode compositor
//
// Inputs:
//   binding 1: t_source  — the entity rendered in isolation (src layer)
//   binding 2: s_source  — sampler
//   binding 3: t_dest    — snapshot of the render target BEFORE this entity (dst layer)
//   binding 4: s_dest    — sampler
//
// Uniforms:
//   blend_mode : u32  (1=Multiply, 2=Screen, 3=Overlay, 4=Add, 5=Subtract, 6=Darken, 7=Lighten, 8=SoftLight, 9=HardLight, 10=Difference)
//   opacity    : f32  (entity opacity, already multiplied in if nested effects exist)
//   _pad0, _pad1: f32
//
// Output: the correctly blended RGBA, ready to composite over the accumulation buffer
//         with Normal (source-over) blend — blend math is baked into the result.

struct Uniforms {
    blend_mode: u32,
    opacity: f32,
    _pad0: f32,
    _pad1: f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var t_source: texture_2d<f32>;
@group(0) @binding(2) var s_source: sampler;
@group(0) @binding(3) var t_dest: texture_2d<f32>;
@group(0) @binding(4) var s_dest: sampler;

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

// ── Blend functions (Photoshop-accurate, linear premultiplied space) ──

fn blend_multiply(dst: vec3f, src: vec3f) -> vec3f {
    return dst * src;
}

fn blend_screen(dst: vec3f, src: vec3f) -> vec3f {
    return dst + src - dst * src;
}

fn overlay_ch(d: f32, s: f32) -> f32 {
    if d < 0.5 {
        return 2.0 * d * s;
    }
    return 1.0 - 2.0 * (1.0 - d) * (1.0 - s);
}

fn blend_overlay(dst: vec3f, src: vec3f) -> vec3f {
    return vec3f(overlay_ch(dst.r, src.r), overlay_ch(dst.g, src.g), overlay_ch(dst.b, src.b));
}

fn soft_light_ch(d: f32, s: f32) -> f32 {
    if s <= 0.5 {
        return d - (1.0 - 2.0 * s) * d * (1.0 - d);
    }
    let q = select(sqrt(d), ((16.0 * d - 12.0) * d + 4.0) * d, d <= 0.25);
    return d + (2.0 * s - 1.0) * (q - d);
}

fn blend_soft_light(dst: vec3f, src: vec3f) -> vec3f {
    return vec3f(soft_light_ch(dst.r, src.r), soft_light_ch(dst.g, src.g), soft_light_ch(dst.b, src.b));
}

fn blend_add(dst: vec3f, src: vec3f) -> vec3f {
    return min(dst + src, vec3f(1.0));
}

fn blend_subtract(dst: vec3f, src: vec3f) -> vec3f {
    return max(dst - src, vec3f(0.0));
}

fn blend_darken(dst: vec3f, src: vec3f) -> vec3f {
    return min(dst, src);
}

fn blend_lighten(dst: vec3f, src: vec3f) -> vec3f {
    return max(dst, src);
}

fn hard_light_ch(d: f32, s: f32) -> f32 {
    if s < 0.5 {
        return 2.0 * d * s;
    }
    return 1.0 - 2.0 * (1.0 - d) * (1.0 - s);
}

fn blend_hard_light(dst: vec3f, src: vec3f) -> vec3f {
    return vec3f(hard_light_ch(dst.r, src.r), hard_light_ch(dst.g, src.g), hard_light_ch(dst.b, src.b));
}

fn blend_difference(dst: vec3f, src: vec3f) -> vec3f {
    return abs(dst - src);
}

fn apply_blend(dst: vec3f, src: vec3f, mode: u32) -> vec3f {
    switch mode {
        case 1u: { return blend_multiply(dst, src); }
        case 2u: { return blend_screen(dst, src); }
        case 3u: { return blend_overlay(dst, src); }
        case 4u: { return blend_add(dst, src); }
        case 5u: { return blend_subtract(dst, src); }
        case 6u: { return blend_darken(dst, src); }
        case 7u: { return blend_lighten(dst, src); }
        case 8u: { return blend_soft_light(dst, src); }
        case 9u: { return blend_hard_light(dst, src); }
        case 10u: { return blend_difference(dst, src); }
        default: { return src; }
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let src = textureSample(t_source, s_source, in.uv);
    let dst = textureSample(t_dest, s_dest, in.uv);

    // src.a is the entity's natural alpha (from texture/shape alpha).
    // u.opacity scales it (0.0 = invisible, 1.0 = full).
    let alpha = src.a * u.opacity;

    if alpha <= 0.0 {
        // Fully transparent — pass dst through untouched.
        return dst;
    }

    // Blend the RGB channels using the chosen mode.
    let blended_rgb = apply_blend(dst.rgb, src.rgb, u.blend_mode);

    // Porter-Duff source-over with blended RGB:
    //   out.rgb = blended_rgb * alpha + dst.rgb * (1 - alpha)
    //   out.a   = alpha + dst.a * (1 - alpha)
    let out_rgb = blended_rgb * alpha + dst.rgb * (1.0 - alpha);
    let out_a   = alpha + dst.a * (1.0 - alpha);

    return vec4f(out_rgb, out_a);
}
