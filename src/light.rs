use std::ops::Range;

use bevy::{
    math::Vec3,
    prelude::{Color, Component},
};
use wgpu::util::DeviceExt;

use crate::model::{Model, ModelMesh};

// main.rs
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Component)]
pub struct Light {
    pub position: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding: u32,
    pub color: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding2: u32,
}

impl Light {
    pub fn new(position: Vec3, color: Color) -> Self {
        Self {
            position: position.to_array(),
            _padding: 0,
            color: [color.r(), color.g(), color.b()],
            _padding2: 0,
        }
    }

    pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: None,
        })
    }

    pub fn bind_group(&self, device: &wgpu::Device) -> (wgpu::BindGroup, wgpu::Buffer) {
        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light VB"),
            contents: bytemuck::cast_slice(&[*self]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &Light::bind_group_layout(device),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            }],
            label: None,
        });
        (bind_group, light_buffer)
    }
}

fn draw_light_mesh<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    mesh: &'a ModelMesh,
    camera_bind_group: &'a wgpu::BindGroup,
    light_bind_group: &'a wgpu::BindGroup,
) {
    draw_light_mesh_instanced(render_pass, mesh, 0..1, camera_bind_group, light_bind_group);
}

fn draw_light_mesh_instanced<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    mesh: &'a ModelMesh,
    instances: Range<u32>,
    camera_bind_group: &'a wgpu::BindGroup,
    light_bind_group: &'a wgpu::BindGroup,
) {
    render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
    render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    render_pass.set_bind_group(0, camera_bind_group, &[]);
    render_pass.set_bind_group(1, light_bind_group, &[]);
    render_pass.draw_indexed(0..mesh.num_elements, 0, instances);
}

pub fn draw_light_model<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    model: &'a Model,
    camera_bind_group: &'a wgpu::BindGroup,
    light_bind_group: &'a wgpu::BindGroup,
) {
    draw_light_model_instanced(
        render_pass,
        model,
        0..1,
        camera_bind_group,
        light_bind_group,
    );
}

fn draw_light_model_instanced<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    model: &'a Model,
    instances: Range<u32>,
    camera_bind_group: &'a wgpu::BindGroup,
    light_bind_group: &'a wgpu::BindGroup,
) {
    for mesh in &model.meshes {
        draw_light_mesh_instanced(
            render_pass,
            mesh,
            instances.clone(),
            camera_bind_group,
            light_bind_group,
        );
    }
}
