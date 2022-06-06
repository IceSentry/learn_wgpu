use std::ops::Range;

use crate::model::{Mesh, Model};

// main.rs
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    pub position: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding: u32,
    pub color: [f32; 3],
    // Due to uniforms requiring 16 byte (4 float) spacing, we need to use a padding field here
    _padding2: u32,
}

impl LightUniform {
    pub fn new(position: [f32; 3], color: [f32; 3]) -> Self {
        Self {
            position,
            _padding: 0,
            color,
            _padding2: 0,
        }
    }
}

fn draw_light_mesh<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    mesh: &'a Mesh,
    camera_bind_group: &'a wgpu::BindGroup,
    light_bind_group: &'a wgpu::BindGroup,
) {
    draw_light_mesh_instanced(render_pass, mesh, 0..1, camera_bind_group, light_bind_group);
}

fn draw_light_mesh_instanced<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    mesh: &'a Mesh,
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
