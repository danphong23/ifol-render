// Alpha Mask Shader — composites source texture masked by a mask texture's alpha.
//
// Uniforms layout (24 floats):
//   transform: mat4x4f (16 floats)
//   color: vec4f (4 floats) — tint color (usually white)
//   opacity: f32
//   mask_mode: f32 (0=alpha, 1=luminance, 2=inverted_alpha)
//   softness: f32 (edge softness for mask)
//   _pad: f32
//
// Textures:
//   binding 1: source texture (the content to show)
//   binding 2: sampler
//
// NOTE: For the masking approach, we use a 2-pass system:
//   Pass 1: Render the content normally (composite pipeline)
//   Pass 2: Apply mask shader that multiplies by mask shape
//
// This shader draws a mask SHAPE (like the shapes shader) and
// uses it to modulate the underlying content.
//
// For simplicity, this shader draws a mask quad where:
//   - Inside the mask region → alpha = 1 (visible)
//   - Outside → alpha = 0 (hidden)
// The mask shape is controlled by mask_mode:
//   0 = rectangular (simple clip)
//   1 = circular (round clip)
//   2 = rounded rect clip
//   3 = feathered edge (soft mask)

struct Uniforms {
    transform: mat4x4f,
    color: vec4f,
    opacity: f32,
    mask_shape: f32,   // 0=rect, 1=circle, 2=rounded_rect, 3=feathered
    param1: f32,       // corner_radius for rounded_rect, feather amount
    _pad: f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var t_source: texture_2d<f32>;
@group(0) @binding(2) var t_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) uv: vec2f,
}

@vertex
fn vs_main(@location(0) position: vec2f, @location(1) uv: vec2f) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = u.transform * vec4f(position, 0.0, 1.0);
    out.uv = uv;
    return out;
}

// SDF functions for mask shapes
fn sdf_rect(p: vec2f, half_size: vec2f) -> f32 {
    let d = abs(p) - half_size;
    return length(max(d, vec2f(0.0))) + min(max(d.x, d.y), 0.0);
}

fn sdf_circle(p: vec2f, radius: f32) -> f32 {
    return length(p) - radius;
}

fn sdf_rounded_rect(p: vec2f, half_size: vec2f, radius: f32) -> f32 {
    let r = min(radius, min(half_size.x, half_size.y));
    let d = abs(p) - half_size + vec2f(r);
    return length(max(d, vec2f(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    // Sample the source texture
    let source_color = textureSample(t_source, t_sampler, in.uv);

    // Compute mask SDF
    let p = in.uv - vec2f(0.5);
    let half = vec2f(0.5);

    var dist: f32;
    let shape = i32(u.mask_shape);

    switch shape {
        case 0 { // rect clip
            dist = sdf_rect(p, half);
        }
        case 1 { // circle clip
            dist = sdf_circle(p, 0.5);
        }
        case 2 { // rounded rect clip
            dist = sdf_rounded_rect(p, half, u.param1);
        }
        case 3 { // feathered rect (soft edges)
            dist = sdf_rect(p, half * (1.0 - u.param1));
        }
        default {
            dist = sdf_rect(p, half);
        }
    }

    // Anti-aliased mask edge
    let pixel_size = fwidth(dist);
    let feather = select(pixel_size, u.param1 * 0.5, u.mask_shape == 3.0);

    let mask_alpha = 1.0 - smoothstep(-feather, feather, dist);

    // Apply mask to source
    let final_alpha = source_color.a * mask_alpha * u.opacity * u.color.a;

    if final_alpha < 0.001 {
        discard;
    }

    return vec4f(source_color.rgb * u.color.rgb, final_alpha);
}
