use crate::texture::{self, Texture};
use bevy::{
    math::Vec4,
    render::render_resource::{encase, ShaderType},
};
use wgpu::util::DeviceExt;

#[derive(ShaderType)]
pub struct MaterialUniform {
    pub base_color: Vec4,
    pub alpha: f32,
}

pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("material_bind_group_layout"),
        entries: &[
            // material
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
            // diffuse_texture
            texture::bind_group_layout_entry(0)[0],
            texture::bind_group_layout_entry(0)[1],
        ],
    })
}

pub fn create_bind_group(
    device: &wgpu::Device,
    material: &MaterialUniform,
    diffuse_texture: &Texture,
) -> wgpu::BindGroup {
    let byte_buffer = Vec::new();
    let mut buffer = encase::UniformBuffer::new(byte_buffer);
    buffer.write(&material).unwrap();

    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        contents: buffer.as_ref(),
        label: None,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("material_bind_group"),
        layout: &bind_group_layout(device),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
            },
        ],
    })
}
