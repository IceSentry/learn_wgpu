use std::path::{Path, PathBuf};

use crate::{
    mesh::{Mesh, Vertex},
    model::{Material, Model, ModelMesh},
    obj_loader::ObjMaterial,
    texture::{self, Texture},
};
use anyhow::Context;
use bevy::{
    math::{Vec2, Vec3},
    utils::Instant,
};

pub fn load_bytes(file_name: &PathBuf) -> anyhow::Result<Vec<u8>> {
    let path = std::env::current_dir()?.join("assets").join(file_name);
    let data = std::fs::read(path.clone()).with_context(|| format!("Failed to read {path:?}"))?;
    Ok(data)
}

pub fn load_texture(
    file_name: &PathBuf,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<Texture> {
    let data =
        load_bytes(file_name).with_context(|| format!("Failed to load texture {file_name:?}"))?;
    Texture::from_bytes(
        device,
        queue,
        &data,
        &file_name.file_name().unwrap().to_string_lossy(),
    )
    .with_context(|| "Failed to create Texture from bytes".to_string())
}

// TODO consider loading materials in a separate frame to avoid blocking for too long
pub fn load_model(
    name: &str,
    root_path: &Path,
    obj_models: &[tobj::Model],
    obj_materials: &[ObjMaterial],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<Model> {
    let start = Instant::now();

    log::info!("Creating gpu textures from obj materials");

    let mut materials = Vec::new();
    for m in obj_materials {
        let diffuse_texture =
            Texture::from_image(device, queue, &m.diffuse_texture_data, Some(&m.name))?;
        let bind_group = texture::bind_group(device, layout, &diffuse_texture);
        materials.push(Material {
            name: m.name.clone(),
            diffuse_texture,
            bind_group,
        });
    }
    if materials.is_empty() {
        let mut path = root_path.to_path_buf();
        path.pop();
        path.push("pink.png");

        let diffuse_texture = load_texture(&path, device, queue)?;
        let bind_group = texture::bind_group(device, layout, &diffuse_texture);
        materials.push(Material {
            name: "default texture".to_string(),
            diffuse_texture,
            bind_group,
        });
    }

    log::info!(
        "Finished creating gpu textures from obj materials {}ms",
        (Instant::now() - start).as_millis()
    );

    let start = Instant::now();
    log::info!("Creating Mesh buffers");

    let meshes: Vec<_> = obj_models
        .iter()
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
                indices: Some(m.mesh.indices.clone()),
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

    log::info!(
        "Finished creating mesh buffers {}ms",
        (Instant::now() - start).as_millis()
    );

    Ok(Model { meshes, materials })
}
