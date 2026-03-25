// SDF Shapes Shader — draws smooth primitives via Signed Distance Fields.
//
// Uniforms layout (24 floats):
//   transform: mat4x4f (16 floats)
//   color: vec4f (4 floats)
//   opacity: f32
//   shape_type: f32 (0=rect, 1=rounded_rect, 2=circle, 3=ellipse, 4=line)
//   param1: f32 (corner_radius for rounded_rect, line width for line)
//   param2: f32 (border_width, 0 = filled)

struct Uniforms {
    transform: mat4x4f,
    color: vec4f,
    opacity: f32,
    shape_type: f32,
    param1: f32,
    param2: f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var t_input: texture_2d<f32>;
@group(0) @binding(2) var t_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) uv: vec2f,
}

// Vertex shader — uses quad vertices from vertex buffer
@vertex
fn vs_main(@location(0) position: vec2f, @location(1) uv: vec2f) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = u.transform * vec4f(position, 0.0, 1.0);
    out.uv = uv;
    return out;
}

// SDF functions (all operate in -0.5..0.5 space)

fn sdf_rect(p: vec2f, half_size: vec2f) -> f32 {
    let d = abs(p) - half_size;
    return length(max(d, vec2f(0.0))) + min(max(d.x, d.y), 0.0);
}

fn sdf_rounded_rect(p: vec2f, half_size: vec2f, radius: f32) -> f32 {
    let r = min(radius, min(half_size.x, half_size.y));
    let d = abs(p) - half_size + vec2f(r);
    return length(max(d, vec2f(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

fn sdf_circle(p: vec2f, radius: f32) -> f32 {
    return length(p) - radius;
}

fn sdf_ellipse(p: vec2f, radii: vec2f) -> f32 {
    // Approximate SDF for ellipse
    let p_norm = p / radii;
    let d = length(p_norm) - 1.0;
    return d * min(radii.x, radii.y);
}

fn sdf_line(p: vec2f, half_len: f32, width: f32) -> f32 {
    let d = abs(p) - vec2f(half_len, width * 0.5);
    return length(max(d, vec2f(0.0))) + min(max(d.x, d.y), 0.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    // Map UV (0..1) to centered coords (-0.5..0.5)
    let p = in.uv - vec2f(0.5);
    let half = vec2f(0.5);

    var dist: f32;
    let shape = i32(u.shape_type);

    // Compute SDF based on shape type
    switch shape {
        case 0 { // rect
            dist = sdf_rect(p, half);
        }
        case 1 { // rounded_rect
            dist = sdf_rounded_rect(p, half, u.param1);
        }
        case 2 { // circle
            dist = sdf_circle(p, 0.5);
        }
        case 3 { // ellipse
            dist = sdf_ellipse(p, half);
        }
        case 4 { // line
            dist = sdf_line(p, 0.5, u.param1);
        }
        default {
            dist = sdf_rect(p, half);
        }
    }

    // Anti-aliased edge (smooth 1px transition)
    let pixel_size = fwidth(dist);
    var alpha: f32;

    if u.param2 > 0.0 {
        // Border/stroke mode: hollow shape
        let border = u.param2;
        alpha = 1.0 - smoothstep(-pixel_size, pixel_size, abs(dist) - border * 0.5);
    } else {
        // Filled mode
        alpha = 1.0 - smoothstep(-pixel_size, pixel_size, dist);
    }

    let final_alpha = alpha * u.opacity * u.color.a;
    if final_alpha < 0.001 {
        discard;
    }

    return vec4f(u.color.rgb, final_alpha);
}
