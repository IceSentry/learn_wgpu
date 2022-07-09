use bevy::{
    asset::{AssetLoader, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    utils::Instant,
};

use crate::{
    instances::Instances,
    mesh::Vertex,
    model::{Material, Model, ModelMesh},
    obj_loader::loader::load_obj,
    renderer::WgpuRenderer,
    transform::Transform,
    Wave, INSTANCED_MODEL_NAME, INSTANCED_SCALE, MODEL_NAME, NUM_INSTANCES_PER_ROW, SCALE,
    SPACE_BETWEEN,
};

mod loader;

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

fn handle_obj_loaded(
    mut commands: Commands,
    obj_assets: Res<Assets<LoadedObj>>,
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

            ModelMesh::from_mesh(name, device, &mesh)
        })
        .collect();

    log::info!(
        "Finished creating mesh buffers {}ms",
        (Instant::now() - start).as_millis()
    );

    Ok(Model { meshes, materials })
}
