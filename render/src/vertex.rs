//! Vertex types and quad geometry for GPU rendering.

use bytemuck::{Pod, Zeroable};

/// A vertex with position and texture coordinates.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}

impl Vertex {
    /// Vertex buffer layout for wgpu.
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // uv
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// Fullscreen quad vertices (two triangles).
pub const QUAD_VERTICES: &[Vertex] = &[
    Vertex {
        position: [-1.0, -1.0],
        uv: [0.0, 1.0],
    }, // bottom-left
    Vertex {
        position: [1.0, -1.0],
        uv: [1.0, 1.0],
    }, // bottom-right
    Vertex {
        position: [1.0, 1.0],
        uv: [1.0, 0.0],
    }, // top-right
    Vertex {
        position: [-1.0, 1.0],
        uv: [0.0, 0.0],
    }, // top-left
];

/// Quad indices (two triangles).
pub const QUAD_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];
