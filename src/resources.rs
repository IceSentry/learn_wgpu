use bevy::math::{Vec2, Vec3};

// TODO add support for wasm
use crate::{
    mesh::{Mesh, Vertex},
    model::{Material, Model, ModelMesh},
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

pub async fn load_obj(
    file_name: &str,
) -> anyhow::Result<(
    Vec<tobj::Model>,
    Result<Vec<tobj::Material>, tobj::LoadError>,
)> {
    let obj_data = load_string(file_name)?;
    let obj_cursor = Cursor::new(obj_data);
    let mut obj_reader = BufReader::new(obj_cursor);

    Ok(tobj::load_obj_buf_async(
        &mut obj_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |p| async move {
            // FIXME this assumes everything is at the root of res/
            log::info!("Loading {p}");
            let mat_text = load_string(&p).unwrap();
            tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
        },
    )
    .await?)
}

pub fn load_model(
    name: &str,
    (models, obj_materials): (
        Vec<tobj::Model>,
        Result<Vec<tobj::Material>, tobj::LoadError>,
    ),
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<Model> {
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
            let vertices: Vec<_> = (0..m.mesh.positions.len() / 3)
                .map(|i| Vertex {
                    position: Vec3::new(
                        m.mesh.positions[i * 3],
                        m.mesh.positions[i * 3 + 1],
                        m.mesh.positions[i * 3 + 2],
                    ),
                    uv: if m.mesh.texcoords.is_empty() {
                        Vec2::new(1.0, 1.0)
                    } else {
                        Vec2::new(m.mesh.texcoords[i * 2], m.mesh.texcoords[i * 2 + 1])
                    },
                    normal: if m.mesh.normals.is_empty() {
                        Vec3::new(0.0, 0.0, 0.0)
                    } else {
                        Vec3::new(
                            m.mesh.normals[i * 3],
                            m.mesh.normals[i * 3 + 1],
                            m.mesh.normals[i * 3 + 2],
                        )
                    },
                })
                .collect();

            let mut mesh = Mesh {
                vertices,
                indices: Some(m.mesh.indices),
            };

            if m.mesh.normals.is_empty() {
                mesh.compute_normals();
            }

            ModelMesh {
                name: name.to_string(),
                vertex_buffer: mesh.get_vertex_buffer(device),
                index_buffer: mesh.get_index_buffer(device),
                num_elements: mesh.indices.unwrap().len() as u32,
                material_id: m.mesh.material_id.unwrap_or(0),
            }
        })
        .collect();

    Ok(Model { meshes, materials })
}
