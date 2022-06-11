use bevy::math::{Vec2, Vec3};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: Vec3,
    pub uv: Vec2,
    pub normal: Vec3,
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
