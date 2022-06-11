use bevy::math::Vec3;
use wgpu::util::DeviceExt;

// TODO add support for wasm
use crate::{
    model::{Material, Mesh, Model, ModelVertex},
    texture::{self, Texture},
};
use std::io::{BufReader, Cursor};

pub fn load_string(file_name: &str) -> anyhow::Result<String> {
    let path = std::path::Path::new(env!("OUT_DIR"))
        .join("res")
        .join(file_name);
    let txt = std::fs::read_to_string(path)?;

    Ok(txt)
}

pub fn load_bytes(file_name: &str) -> anyhow::Result<Vec<u8>> {
    let path = std::path::Path::new(env!("OUT_DIR"))
        .join("res")
        .join(file_name);
    let data = std::fs::read(path)?;

    Ok(data)
}

pub fn load_texture(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<Texture> {
    let data = load_bytes(file_name)?;
    Texture::from_bytes(device, queue, &data, file_name)
}

fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let (a, b, c) = (Vec3::from(a), Vec3::from(b), Vec3::from(c));
    (b - a).cross(c - a).normalize().into()
}

pub async fn load_model(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<Model> {
    let obj_data = load_string(file_name)?;
    let obj_cursor = Cursor::new(obj_data);
    let mut obj_reader = BufReader::new(obj_cursor);

    let (models, obj_materials) = tobj::load_obj_buf_async(
        &mut obj_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |p| async move {
            // FIXME this assumes everything is at the root of res/
            let mat_text = load_string(&p).unwrap();
            tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
        },
    )
    .await?;

    let mut materials = Vec::new();
    for m in obj_materials? {
        let diffuse_texture = load_texture(&m.diffuse_texture, device, queue)?;
        let bind_group = texture::bind_group(device, layout, &diffuse_texture);
        materials.push(Material {
            name: m.name,
            diffuse_texture,
            bind_group,
        });
    }
    if materials.is_empty() {
        let diffuse_texture = load_texture("pink.png", device, queue)?;
        let bind_group = texture::bind_group(device, layout, &diffuse_texture);
        materials.push(Material {
            name: "default texture".to_string(),
            diffuse_texture,
            bind_group,
        });
    }

    let meshes: Vec<_> = models
        .into_iter()
        .map(|m| {
            let mut vertices: Vec<_> = (0..m.mesh.positions.len() / 3)
                .map(|i| ModelVertex {
                    position: [
                        m.mesh.positions[i * 3],
                        m.mesh.positions[i * 3 + 1],
                        m.mesh.positions[i * 3 + 2],
                    ],
                    uv: if m.mesh.texcoords.is_empty() {
                        [1.0, 1.0]
                    } else {
                        [m.mesh.texcoords[i * 2], m.mesh.texcoords[i * 2 + 1]]
                    },
                    normal: if m.mesh.normals.is_empty() {
                        [0.0, 0.0, 0.0]
                    } else {
                        [
                            m.mesh.normals[i * 3],
                            m.mesh.normals[i * 3 + 1],
                            m.mesh.normals[i * 3 + 2],
                        ]
                    },
                })
                .collect();

            // compute_flat_normals
            if m.mesh.texcoords.is_empty() && m.mesh.indices.is_empty() {
                log::info!("flat normals");
                for v in vertices.chunks_exact_mut(3) {
                    if let [v1, v2, v3] = v {
                        let normal = face_normal(v1.position, v2.position, v3.position);
                        v1.normal = normal;
                        v2.normal = normal;
                        v3.normal = normal;
                    }
                }
            }

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{file_name:?} Vertex Buffer")),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{file_name:?} Index Buffer")),
                contents: bytemuck::cast_slice(&m.mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            Mesh {
                name: file_name.to_string(),
                vertex_buffer,
                index_buffer,
                num_elements: m.mesh.indices.len() as u32,
                material_id: m.mesh.material_id.unwrap_or(0),
            }
        })
        .collect();

    Ok(Model { meshes, materials })
}
