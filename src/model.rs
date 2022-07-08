use crate::{mesh::Mesh, renderer::bind_groups::material::GpuModelMaterials, texture::Texture};
use bevy::{
    math::{Vec3, Vec4},
    prelude::Component,
};
use std::ops::Range;

#[derive(Component)]
pub struct Model {
    pub meshes: Vec<ModelMesh>,
    pub materials: Vec<Material>,
}

impl Model {
    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        gpu_materials: &'a GpuModelMaterials,
        mesh_view_bind_group: &'a wgpu::BindGroup,
        transparent: bool,
    ) {
        self.draw_instanced(
            render_pass,
            0..1,
            gpu_materials,
            mesh_view_bind_group,
            transparent,
        );
    }

    pub fn draw_instanced<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        instances: Range<u32>,
        gpu_materials: &'a GpuModelMaterials,
        mesh_view_bind_group: &'a wgpu::BindGroup,
        transparent: bool,
    ) {
        for mesh in &self.meshes {
            // TODO get data from Handle
            let material = &gpu_materials.data[mesh.material_id];

            if transparent && material.0.alpha < 1.0 {
                mesh.draw_instanced(
                    render_pass,
                    instances.clone(),
                    &material.2,
                    mesh_view_bind_group,
                );
            }

            if !transparent && material.0.alpha == 1.0 {
                mesh.draw_instanced(
                    render_pass,
                    instances.clone(),
                    &material.2,
                    mesh_view_bind_group,
                );
            }
        }
    }
}

#[derive(Debug)]
pub struct Material {
    pub name: String,
    pub base_color: Vec4,
    pub alpha: f32,
    pub gloss: f32,
    pub specular: Vec3,
    pub diffuse_texture: Texture,
    pub normal_texture: Option<Texture>,
    pub specular_texture: Option<Texture>,
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
    pub fn from_mesh(label: &str, device: &wgpu::Device, mesh: Mesh, material_id: usize) -> Self {
        let mut mesh = mesh;
        mesh.compute_tangents();

        ModelMesh {
            name: label.to_string(),
            vertex_buffer: mesh.get_vertex_buffer(device),
            index_buffer: mesh.get_index_buffer(device),
            num_elements: mesh.indices.map(|i| i.len() as u32).unwrap_or(1),
            material_id,
        }
    }

    #[allow(unused)]
    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        material_bind_group: &'a wgpu::BindGroup,
        mesh_view_bind_group: &'a wgpu::BindGroup,
    ) {
        self.draw_instanced(render_pass, 0..1, material_bind_group, mesh_view_bind_group);
    }

    pub fn draw_instanced<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        instances: Range<u32>,
        material_bind_group: &'a wgpu::BindGroup,
        mesh_view_bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.set_bind_group(0, mesh_view_bind_group, &[]);
        render_pass.set_bind_group(1, material_bind_group, &[]);
        render_pass.draw_indexed(0..self.num_elements, 0, instances);
    }
}
