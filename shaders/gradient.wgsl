// Gradient Shader — linear, radial, and conic gradients.
//
// Uniforms layout (24 floats):
//   transform: mat4x4f (16 floats)
//   color_start: vec4f (4 floats) — first gradient color
//   grad_type: f32 (0=linear, 1=radial, 2=conic)
//   angle: f32 (rotation in radians, for linear/conic)
//   param1: f32 (aspect ratio correction or extra param)
//   param2: f32 (reserved)
//
// Second uniform buffer for gradient end color (binding 3 does not exist in standard layout,
// so we encode the end color in a second texture or workaround).
//
// WORKAROUND: We encode end color in the alpha-premultied "param" slots:
//   Actually, let's use a smarter encoding. The standard uniform buffer is 24 floats.
//   We can pack both colors in:
//   [0..15]  = transform (16 floats)
//   [16..19] = color_start RGBA (4 floats)
//   [20]     = grad_type
//   [21]     = angle
//   [22]     = param1
//   [23]     = param2
//
// But we need color_end too! Solution: use a LARGER uniform buffer (32 floats):
//   [0..15]  = transform (16 floats)
//   [16..19] = color_start RGBA
//   [20..23] = color_end RGBA
//   [24]     = grad_type
//   [25]     = angle
//   [26]     = param1 (center_x for radial)
//   [27]     = param2 (center_y for radial)
//
// Hmm, but struct must be 16-byte aligned. Let's structure it properly:

struct Uniforms {
    transform: mat4x4f,  // 16 floats (64 bytes)
    color_start: vec4f,  // 4 floats (16 bytes)
    color_end: vec4f,    // 4 floats (16 bytes)
    grad_type: f32,      // 1 float
    angle: f32,          // 1 float
    center_x: f32,       // 1 float (for radial center offset)
    center_y: f32,       // 1 float (for radial center offset)
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var t_input: texture_2d<f32>;
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

// Linear gradient: compute t along a direction
fn gradient_linear(uv: vec2f, angle: f32) -> f32 {
    let dir = vec2f(cos(angle), sin(angle));
    let centered = uv - vec2f(0.5);
    // Project onto direction, remap from [-0.5..0.5] to [0..1]
    return dot(centered, dir) + 0.5;
}

// Radial gradient: distance from center
fn gradient_radial(uv: vec2f, center: vec2f) -> f32 {
    let delta = uv - center;
    return length(delta) * 2.0; // 0 at center, 1 at edge
}

// Conic (angular) gradient: angle around center
fn gradient_conic(uv: vec2f, center: vec2f, start_angle: f32) -> f32 {
    let delta = uv - center;
    var a = atan2(delta.y, delta.x) - start_angle;
    // Normalize to [0..1]
    a = a / (2.0 * 3.14159265359);
    return fract(a);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    var t: f32;
    let grad = i32(u.grad_type);

    let center = vec2f(0.5 + u.center_x, 0.5 + u.center_y);

    switch grad {
        case 0 { // linear
            t = gradient_linear(in.uv, u.angle);
        }
        case 1 { // radial
            t = gradient_radial(in.uv, center);
        }
        case 2 { // conic
            t = gradient_conic(in.uv, center, u.angle);
        }
        default {
            t = gradient_linear(in.uv, u.angle);
        }
    }

    t = clamp(t, 0.0, 1.0);

    let color = mix(u.color_start, u.color_end, t);
    return color;
}
