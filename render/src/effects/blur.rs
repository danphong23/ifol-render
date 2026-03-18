//! Gaussian blur effect — 2-pass separable (horizontal + vertical).

use super::EffectPass;
use super::context::EffectContext;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Gaussian blur effect.
pub struct BlurEffect;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct BlurUniforms {
    direction: [f32; 2], // (1,0) for horizontal, (0,1) for vertical
    radius: f32,
    texel_size: f32, // 1.0 / dimension
}

impl EffectPass for BlurEffect {
    fn name(&self) -> &str {
        "blur"
    }

    fn execute(&self, device: &wgpu::Device, queue: &wgpu::Queue, ctx: &mut EffectContext) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blur shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../shaders/blur.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blur bgl"),
            entries: &[
                // Uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Input texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blur pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blur pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_blur"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let radius = 4.0; // TODO: parameterize from EffectConfig

        // Pass 1: Horizontal blur
        run_blur_pass(
            device,
            queue,
            &pipeline,
            &bind_group_layout,
            ctx,
            [1.0, 0.0],
            radius,
            1.0 / ctx.width as f32,
        );
        ctx.swap();

        // Pass 2: Vertical blur
        run_blur_pass(
            device,
            queue,
            &pipeline,
            &bind_group_layout,
            ctx,
            [0.0, 1.0],
            radius,
            1.0 / ctx.height as f32,
        );
        ctx.swap();
    }
}

#[allow(clippy::too_many_arguments)]
fn run_blur_pass(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipeline: &wgpu::RenderPipeline,
    bind_group_layout: &wgpu::BindGroupLayout,
    ctx: &EffectContext,
    direction: [f32; 2],
    radius: f32,
    texel_size: f32,
) {
    let uniforms = BlurUniforms {
        direction,
        radius,
        texel_size,
    };

    let ub = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("blur uniform"),
        contents: bytemuck::cast_slice(&[uniforms]),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("blur bg"),
        layout: bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: ub.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(ctx.input_view()),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&ctx.sampler),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("blur pass"),
    });

    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("blur render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ctx.output_view(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_pipeline(pipeline);
        rpass.set_bind_group(0, &bg, &[]);
        rpass.draw(0..3, 0..1); // fullscreen triangle
    }

    queue.submit(std::iter::once(encoder.finish()));
}
