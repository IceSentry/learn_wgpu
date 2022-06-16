use bevy::{
    math::{Mat3, Mat4, Vec2, Vec3},
    render::render_resource::{encase, ShaderType},
};
use wgpu::util::DeviceExt;

use crate::{model::ModelMesh, transform::Transform};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
}

// TODO use Map for attributes
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Option<Vec<u32>>,
}

impl Mesh {
    pub fn get_vertex_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&self.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        })
    }

    pub fn get_index_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(
                self.indices
                    .as_ref()
                    .expect("tried to get index buffer without indices"),
            ),
            usage: wgpu::BufferUsages::INDEX,
        })
    }

    pub fn compute_normals(&mut self) {
        fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
            let (a, b, c) = (Vec3::from(a), Vec3::from(b), Vec3::from(c));
            (b - a).cross(c - a).normalize().into()
        }

        if let Some(indices) = self.indices.as_ref() {
            for v in self.vertices.iter_mut() {
                v.normal = Vec3::ZERO;
            }

            for i in indices.chunks_exact(3) {
                if let [i1, i2, i3] = i {
                    let v_a = self.vertices[*i1 as usize];
                    let v_b = self.vertices[*i2 as usize];
                    let v_c = self.vertices[*i3 as usize];

                    let edge_ab = v_b.position - v_a.position;
                    let edge_ac = v_c.position - v_a.position;

                    let normal = edge_ab.cross(edge_ac);

                    self.vertices[*i1 as usize].normal += normal;
                    self.vertices[*i2 as usize].normal += normal;
                    self.vertices[*i3 as usize].normal += normal;
                }
            }

            for v in self.vertices.iter_mut() {
                v.normal = v.normal.normalize();
            }
        } else {
            let mut normals = vec![];
            for v in self.vertices.chunks_exact_mut(3) {
                if let [v1, v2, v3] = v {
                    let normal = face_normal(
                        v1.position.to_array(),
                        v2.position.to_array(),
                        v3.position.to_array(),
                    );
                    normals.push(normal);
                }
            }
        }
    }
}

#[derive(ShaderType)]
pub struct MeshUniform {
    transform: Mat4,
    normal: Mat3,
}

impl MeshUniform {
    fn from_mesh(transform: Transform) -> Self {
        Self {
            transform: Mat4::from_scale_rotation_translation(
                transform.scale,
                transform.rotation,
                transform.translation,
            ),
            normal: Mat3::from_quat(transform.rotation),
        }
    }
}

pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mesh_bind_group_layout"),
        entries: &[
            // transform
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

pub fn create_bind_group(device: &wgpu::Device, mesh: &MeshUniform) -> wgpu::BindGroup {
    let mut buffer = encase::UniformBuffer::new(Vec::new());
    buffer.write(&mesh).unwrap();

    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        contents: buffer.as_ref(),
        label: None,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("mesh_bind_group"),
        layout: &bind_group_layout(device),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}
