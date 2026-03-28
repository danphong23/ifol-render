// Dashed Rectangle Shader — draws a dashed border around an entity.
//
// Uses the same uniform layout as shapes.wgsl for seamless integration:
//   transform: mat4x4f (16 floats)
//   color: vec4f (4 floats)
//   opacity: f32
//   dash_length: f32 (param1 — length of visible dash segments, normalized 0..1)
//   gap_length: f32  (param2 — length of gaps between dashes, normalized 0..1)
//   border_width: f32 (param3 — border thickness, normalized 0..1)

struct Uniforms {
    transform: mat4x4f,
    color: vec4f,
    opacity: f32,
    dash_length: f32,
    gap_length: f32,
    border_width: f32,
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

// SDF for rectangle border
fn sdf_rect(p: vec2f, half_size: vec2f) -> f32 {
    let d = abs(p) - half_size;
    return length(max(d, vec2f(0.0))) + min(max(d.x, d.y), 0.0);
}

// Compute arc-length position along the rectangle perimeter.
// Returns a value in [0, perimeter] representing how far along the edge we are.
fn rect_arc_length(p: vec2f, half_size: vec2f) -> f32 {
    let w = half_size.x;
    let h = half_size.y;
    
    // Classify which edge the point is closest to and compute arc position.
    // Perimeter order: top → right → bottom → left (clockwise from top-left)
    let top_dist    = abs(p.y - h);
    let bottom_dist = abs(p.y + h);
    let right_dist  = abs(p.x - w);
    let left_dist   = abs(p.x + w);
    
    let min_dist = min(min(top_dist, bottom_dist), min(right_dist, left_dist));
    
    // Top edge: left to right
    if min_dist == top_dist {
        return (p.x + w);  // 0 at left corner, 2w at right corner
    }
    // Right edge: top to bottom
    if min_dist == right_dist {
        return 2.0 * w + (h - p.y);  // continues from top-right
    }
    // Bottom edge: right to left
    if min_dist == bottom_dist {
        return 2.0 * w + 2.0 * h + (w - p.x);  // continues from bottom-right
    }
    // Left edge: bottom to top
    return 2.0 * w + 2.0 * h + 2.0 * w + (p.y + h);  // continues from bottom-left
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let p = in.uv - vec2f(0.5);
    let half = vec2f(0.5);
    
    // Distance to rectangle edge
    let dist = sdf_rect(p, half);
    
    // Anti-aliased border mask
    let pixel_size = fwidth(dist);
    let border = u.border_width;
    let border_mask = 1.0 - smoothstep(-pixel_size, pixel_size, abs(dist) - border * 0.5);
    
    if border_mask < 0.001 {
        discard;
    }
    
    // Compute dash pattern along perimeter
    let arc = rect_arc_length(p, half);
    let period = u.dash_length + u.gap_length;
    let t = arc % period;
    
    // Anti-aliased dash edge
    let dash_aa = fwidth(arc) * 1.5;
    let dash_mask = smoothstep(u.dash_length - dash_aa, u.dash_length, t);
    
    // Final: border × dash × opacity
    let final_alpha = border_mask * (1.0 - dash_mask) * u.opacity * u.color.a;
    
    if final_alpha < 0.001 {
        discard;
    }
    
    return vec4f(u.color.rgb, final_alpha);
}
