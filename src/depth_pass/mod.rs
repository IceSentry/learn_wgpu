use wgpu::util::DeviceExt;

use crate::{renderer::WgpuRenderer, texture::Texture};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex {
    pub fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as u64,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

// This is just a quad
const DEPTH_VERTICES: &[Vertex] = &[
    Vertex {
        position: [-1.0, -1.0, 0.0],
        uv: [0.0, 1.0],
    },
    Vertex {
        position: [1.0, -1.0, 0.0],
        uv: [1.0, 1.0],
    },
    Vertex {
        position: [1.0, 1.0, 0.0],
        uv: [1.0, 0.0],
    },
    Vertex {
        position: [-1.0, 1.0, 0.0],
        uv: [0.0, 0.0],
    },
];

const DEPTH_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

pub struct DepthPass {
    // TODO Extract depth texture to a separate component
    pub texture: Texture,
    layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_depth_indices: u32,
    render_pipeline: wgpu::RenderPipeline,
}

impl DepthPass {
    pub fn new(renderer: &WgpuRenderer) -> Self {
        let texture =
            Texture::create_depth_texture(&renderer.device, &renderer.config, "depth_texture");
        let layout = DepthPass::bind_group_layout(&renderer.device);
        let bind_group = DepthPass::bind_group(&renderer.device, &layout, &texture);

        let vertex_buffer = renderer
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Depth Pass VB"),
                contents: bytemuck::cast_slice(DEPTH_VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = renderer
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Depth Pass IB"),
                contents: bytemuck::cast_slice(DEPTH_INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Depth Pass Pipeline Layout"),
                    bind_group_layouts: &[&layout],
                    push_constant_ranges: &[],
                });

        let render_pipeline = renderer.create_render_pipeline(
            "Depth Pass Render Pipeline",
            wgpu::ShaderModuleDescriptor {
                label: Some("Depth Pass Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("depth.wgsl").into()),
            },
            &pipeline_layout,
            &[Vertex::layout()],
            None,
        );

        Self {
            texture,
            layout,
            bind_group,
            vertex_buffer,
            index_buffer,
            num_depth_indices: DEPTH_INDICES.len() as u32,
            render_pipeline,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) {
        self.texture = Texture::create_depth_texture(device, config, "depth_texture");
        self.bind_group = DepthPass::bind_group(device, &self.layout, &self.texture);
    }

    pub fn render(&self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Depth Visual Render Pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.num_depth_indices, 0, 0..1);
    }

    fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Depth Pass Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        })
    }

    fn bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture: &Texture,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("depth_pass.bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ],
        })
    }
}
