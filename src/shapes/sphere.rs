use std::f32::consts::PI;

use wgpu::util::DeviceExt;

use crate::model::{ModelMesh, ModelVertex};

/// A sphere made of sectors and stacks.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy)]
pub struct UVSphere {
    /// The radius of the sphere.
    pub radius: f32,
    /// Longitudinal sectors
    pub sectors: usize,
    /// Latitudinal stacks
    pub stacks: usize,
}

impl Default for UVSphere {
    fn default() -> Self {
        Self {
            radius: 0.5,
            sectors: 36,
            stacks: 18,
        }
    }
}

impl UVSphere {
    pub fn mesh(&self, device: &wgpu::Device) -> ModelMesh {
        // Largely inspired from http://www.songho.ca/opengl/gl_self.html

        let sectors = self.sectors as f32;
        let stacks = self.stacks as f32;
        let length_inv = 1. / self.radius;
        let sector_step = 2. * PI / sectors;
        let stack_step = PI / stacks;

        let mut positions: Vec<[f32; 3]> = Vec::with_capacity(self.stacks * self.sectors);
        let mut normals: Vec<[f32; 3]> = Vec::with_capacity(self.stacks * self.sectors);
        let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(self.stacks * self.sectors);
        let mut indices: Vec<u32> = Vec::with_capacity(self.stacks * self.sectors * 2 * 3);

        for i in 0..self.stacks + 1 {
            let stack_angle = PI / 2. - (i as f32) * stack_step;
            let xy = self.radius * stack_angle.cos();
            let z = self.radius * stack_angle.sin();

            for j in 0..self.sectors + 1 {
                let sector_angle = (j as f32) * sector_step;
                let x = xy * sector_angle.cos();
                let y = xy * sector_angle.sin();

                positions.push([x, y, z]);
                normals.push([x * length_inv, y * length_inv, z * length_inv]);
                uvs.push([(j as f32) / sectors, (i as f32) / stacks]);
            }
        }

        // indices
        //  k1--k1+1
        //  |  / |
        //  | /  |
        //  k2--k2+1
        for i in 0..self.stacks {
            let mut k1 = i * (self.sectors + 1);
            let mut k2 = k1 + self.sectors + 1;
            for _j in 0..self.sectors {
                if i != 0 {
                    indices.push(k1 as u32);
                    indices.push(k2 as u32);
                    indices.push((k1 + 1) as u32);
                }
                if i != self.stacks - 1 {
                    indices.push((k1 + 1) as u32);
                    indices.push(k2 as u32);
                    indices.push((k2 + 1) as u32);
                }
                k1 += 1;
                k2 += 1;
            }
        }

        let mut vertices = Vec::new();
        for (i, position) in positions.iter().enumerate() {
            vertices.push(ModelVertex {
                position: *position,
                normal: normals[i],
                uv: uvs[i],
            });
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        ModelMesh {
            name: "uv_sphere".to_string(),
            vertex_buffer,
            index_buffer,
            num_elements: indices.len() as u32,
            material_id: 0,
        }
    }
}
