use crate::{
    mesh::Vertex,
    model::{Material, Model, ModelMesh},
};
use anyhow::Context;
use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    tasks::IoTaskPool,
    utils::Instant,
};
use image::RgbaImage;
use std::io::{BufReader, Cursor};

use crate::{
    image_utils::image_from_color, instances::Instances, renderer::WgpuRenderer,
    transform::Transform, Wave, INSTANCED_MODEL_NAME, INSTANCED_SCALE, MODEL_NAME,
    NUM_INSTANCES_PER_ROW, SCALE, SPACE_BETWEEN,
};

// References:
// <https://andrewnoske.com/wiki/OBJ_file_format>
// <http://paulbourke.net/dataformats/mtl/>
// <https://en.wikipedia.org/wiki/Wavefront_.obj_file>

pub struct ObjLoaderPlugin;

impl Plugin for ObjLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_asset::<LoadedObj>()
            .init_asset_loader::<ObjLoader>()
            // TODO improve loaded detection
            .add_system(handle_obj_loaded)
            .add_system(handle_instanced_obj_loaded);
    }
}

#[derive(Default)]
pub struct ObjLoader;

#[derive(Debug, TypeUuid)]
#[uuid = "39cadc56-aa9c-4543-8640-a018b74b5052"]
pub struct LoadedObj {
    pub models: Vec<tobj::Model>,
    pub materials: Vec<Material>,
}

impl AssetLoader for ObjLoader {
    fn extensions(&self) -> &[&str] {
        &["obj"]
    }

    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut bevy::asset::LoadContext,
    ) -> bevy::asset::BoxedFuture<'a, anyhow::Result<(), anyhow::Error>> {
        Box::pin(async move {
            let start = Instant::now();

            log::info!("Loading {:?}", load_context.path());

            let obj = load_obj(bytes, load_context).await?;
            load_context.set_default_asset(LoadedAsset::new(obj));

            log::info!(
                "Finished loading {:?} {}ms",
                load_context.path(),
                (Instant::now() - start).as_millis(),
            );

            Ok(())
        })
    }
}

async fn load_obj<'a, 'b>(
    bytes: &'a [u8],
    load_context: &'a LoadContext<'b>,
) -> anyhow::Result<LoadedObj> {
    let (obj_models, obj_materials) = tobj::load_obj_buf_async(
        &mut BufReader::new(Cursor::new(bytes)),
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |mtl_path| async move {
            let path = load_context.path().parent().unwrap().join(mtl_path);
            let mtl_bytes = load_context.read_asset_bytes(&path).await.unwrap();
            let mtl = tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mtl_bytes)));
            log::info!("Finished loading {path:?}");
            mtl
        },
    )
    .await
    .with_context(|| format!("Failed to load obj {:?}", load_context.path()))?;

    let obj_materials = obj_materials.expect("Failed to load materials");
    let materials: Vec<Material> = IoTaskPool::get()
        .scope(|scope: &mut bevy::tasks::Scope<'_, anyhow::Result<_>>| {
            obj_materials.iter().for_each(|obj_material| {
                log::info!("Loading {}", obj_material.name);
                scope.spawn(async move { load_material(load_context, obj_material).await });
            });
        })
        .into_iter()
        .filter_map(|res| {
            if let Err(err) = res.as_ref() {
                log::error!("Error while loading obj: {err}");
            }
            log::info!("Finished loading material: {}", res.as_ref().unwrap().name);
            res.ok()
        })
        .collect();

    Ok(LoadedObj {
        models: obj_models,
        materials,
    })
}

async fn load_material<'a>(
    load_context: &LoadContext<'a>,
    obj_material: &tobj::Material,
) -> anyhow::Result<Material> {
    let diffuse_texture = load_texture(load_context, &obj_material.diffuse_texture)
        .await?
        .unwrap_or_else(|| image_from_color(Color::WHITE));
    let normal_texture = load_texture(load_context, &obj_material.normal_texture).await?;
    let specular_texture = load_texture(load_context, &obj_material.specular_texture).await?;

    Ok(Material {
        name: obj_material.name.clone(),
        base_color: Vec3::from(obj_material.diffuse).extend(obj_material.dissolve),
        diffuse_texture,
        alpha: obj_material.dissolve,
        gloss: obj_material.shininess,
        specular: Vec3::from(obj_material.specular),
        normal_texture,
        specular_texture,
    })
}

async fn load_texture<'a>(
    load_context: &LoadContext<'a>,
    texture_path: &str,
) -> anyhow::Result<Option<RgbaImage>> {
    Ok(if !texture_path.is_empty() {
        let bytes = load_context
            .read_asset_bytes(load_context.path().parent().unwrap().join(&texture_path))
            .await?;
        log::info!("Finished loading texture: {texture_path:?}");
        let rgba = image::load_from_memory(&bytes)?.to_rgba8();
        Some(rgba)
    } else {
        None
    })
}

fn handle_obj_loaded(
    mut commands: Commands,
    obj_assets: ResMut<Assets<LoadedObj>>,
    asset_server: Res<AssetServer>,
    renderer: Res<WgpuRenderer>,
    mut mesh_spawned: Local<bool>,
) {
    let loaded_obj = obj_assets.get(&asset_server.get_handle(MODEL_NAME));
    if *mesh_spawned || loaded_obj.is_none() {
        return;
    }

    let LoadedObj { models, materials } = loaded_obj.unwrap();

    let model = generate_mesh(
        INSTANCED_MODEL_NAME,
        models,
        materials.clone(),
        &renderer.device,
    )
    .expect("failed to load model from obj");

    commands.spawn_bundle((
        model,
        Transform {
            scale: SCALE,
            ..default()
        },
    ));
    *mesh_spawned = true;
}

fn handle_instanced_obj_loaded(
    mut commands: Commands,
    obj_assets: ResMut<Assets<LoadedObj>>,
    asset_server: Res<AssetServer>,
    renderer: Res<WgpuRenderer>,
    mut mesh_spawned: Local<bool>,
) {
    let loaded_obj = obj_assets.get(&asset_server.get_handle(INSTANCED_MODEL_NAME));
    if *mesh_spawned || loaded_obj.is_none() {
        return;
    }

    let LoadedObj { models, materials } = loaded_obj.unwrap();

    let model = generate_mesh(
        INSTANCED_MODEL_NAME,
        models,
        materials.clone(),
        &renderer.device,
    )
    .expect("failed to load model from obj");

    let mut instances = Vec::new();
    for z in 0..=NUM_INSTANCES_PER_ROW {
        for x in 0..=NUM_INSTANCES_PER_ROW {
            let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
            let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

            let translation = Vec3::new(x as f32, 0.0, z as f32);
            let rotation = if translation == Vec3::ZERO {
                Quat::from_axis_angle(Vec3::Y, 0.0)
            } else {
                Quat::from_axis_angle(translation.normalize(), std::f32::consts::FRAC_PI_4)
            };

            instances.push(Transform {
                rotation,
                translation,
                scale: INSTANCED_SCALE,
            });
        }
    }

    commands
        .spawn()
        .insert(model)
        .insert(Instances(instances))
        .insert(Wave::default());

    *mesh_spawned = true;
}

fn generate_mesh(
    name: &str,
    obj_models: &[tobj::Model],
    materials: Vec<Material>,
    device: &wgpu::Device,
) -> anyhow::Result<Model> {
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
                        Vec2::ZERO
                    } else {
                        // UVs are flipped
                        Vec2::new(m.mesh.texcoords[i * 2], 1.0 - m.mesh.texcoords[i * 2 + 1])
                    },
                    normal: if m.mesh.normals.is_empty() {
                        Vec3::ZERO
                    } else {
                        Vec3::new(
                            m.mesh.normals[i * 3],
                            m.mesh.normals[i * 3 + 1],
                            m.mesh.normals[i * 3 + 2],
                        )
                    },
                    tangent: Vec3::ZERO,
                    bitangent: Vec3::ZERO,
                })
                .collect();

            let mut mesh = crate::mesh::Mesh {
                vertices,
                indices: Some(m.mesh.indices.clone()),
                material_id: m.mesh.material_id,
            };

            if m.mesh.normals.is_empty() {
                mesh.compute_normals();
            }

            mesh.compute_tangents();

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
