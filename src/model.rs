use crate::texture::Texture;
use bevy::prelude::Component;
use std::ops::Range;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub normal: [f32; 3],
}

impl ModelVertex {
    pub fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ModelVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

#[derive(Component)]
pub struct Model {
    pub meshes: Vec<ModelMesh>,
    pub materials: Vec<Material>,
}

impl Model {
    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    ) {
        self.draw_instanced(render_pass, 0..1, camera_bind_group, light_bind_group);
    }

    pub fn draw_instanced<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    ) {
        for mesh in &self.meshes {
            let material = &self.materials[mesh.material_id];
            mesh.draw_instanced(
                render_pass,
                instances.clone(),
                material,
                camera_bind_group,
                light_bind_group,
            );
        }
    }
}

#[derive(Debug)]
pub struct Material {
    pub name: String,
    pub diffuse_texture: Texture,
    // pub normal_texture: Texture,
    pub bind_group: wgpu::BindGroup,
}

#[derive(Debug)]
pub struct ModelMesh {
    pub name: String,
    // TODO don't store buffer on mesh
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material_id: usize,
}

impl ModelMesh {
    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        material: &'a Material,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    ) {
        self.draw_instanced(
            render_pass,
            0..1,
            material,
            camera_bind_group,
            light_bind_group,
        );
    }

    pub fn draw_instanced<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        instances: Range<u32>,
        material: &'a Material,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.set_bind_group(0, &material.bind_group, &[]);
        render_pass.set_bind_group(1, camera_bind_group, &[]);
        render_pass.set_bind_group(2, light_bind_group, &[]);
        render_pass.draw_indexed(0..self.num_elements, 0, instances);
    }
}
