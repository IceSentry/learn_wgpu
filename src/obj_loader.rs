use anyhow::Context;
use bevy::{
    asset::{AssetLoader, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    tasks::IoTaskPool,
    utils::Instant,
};
use image::RgbaImage;
use std::{
    io::{BufReader, Cursor},
    path::{Path, PathBuf},
};

use crate::{
    instances::Instances, renderer::WgpuRenderer, resources, transform::Transform, Wave,
    INSTANCED_MODEL_NAME, INSTANCED_SCALE, MODEL_NAME, NUM_INSTANCES_PER_ROW, SCALE, SPACE_BETWEEN,
};

const ROOT_DIR: &str = "assets\\";

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

#[derive(Debug)]
pub struct ObjMaterial {
    pub name: String,
    pub base_color: Vec4,
    pub alpha: f32,
    pub gloss: f32,
    pub diffuse_texture_data: RgbaImage,
    pub normal_texture_data: Option<RgbaImage>,
}

#[derive(Debug, TypeUuid)]
#[uuid = "39cadc56-aa9c-4543-8640-a018b74b5052"]
pub struct LoadedObj {
    pub models: Vec<tobj::Model>,
    pub materials: Vec<ObjMaterial>,
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
    load_context: &'a mut bevy::asset::LoadContext<'b>,
) -> anyhow::Result<LoadedObj> {
    let path = load_context.path();
    let asset_io = load_context.asset_io();

    let (obj_models, obj_materials) = tobj::load_obj_buf_async(
        &mut BufReader::new(Cursor::new(bytes)),
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |mtl_path| async move {
            log::info!("Loading {mtl_path:?}");
            let mut path = path.parent().expect("no parent").to_path_buf();
            path.push(mtl_path);
            let material_file_data = asset_io
                .load_path(&path)
                .await
                .unwrap_or_else(|_| panic!("Failed to load {path:?}"));
            let mtl = tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(material_file_data)));
            log::info!("Finished loading {path:?}");
            mtl
        },
    )
    .await
    .with_context(|| format!("Failed to load obj {path:?}"))?;

    let mut tasks = Vec::new();
    let pool = IoTaskPool::get();
    let parent_path = load_context
        .path()
        .parent()
        .expect("No parent found for load_context path")
        .to_path_buf();

    for obj_material in obj_materials.clone().expect("Failed to load materials") {
        let parent_path = parent_path.clone();
        let task = pool.spawn(async move {
            let texture_path = if obj_material.diffuse_texture.is_empty() {
                // default texture
                Path::new("white.png").to_path_buf()
            } else {
                let mut texture_path = parent_path.clone();
                texture_path.push(obj_material.diffuse_texture.clone());
                texture_path
            };
            let texture = load_texture(texture_path.clone())
                .with_context(|| format!("Failed to load texture {texture_path:?}"))
                .unwrap();
            log::info!("Finished loading {texture_path:?}");

            let normal_texture = if !obj_material.normal_texture.is_empty() {
                let mut texture_path = parent_path.clone();
                texture_path.push(obj_material.normal_texture.clone());
                let texture = load_texture(texture_path.clone())
                    .with_context(|| format!("Failed to load texture {texture_path:?}"))
                    .unwrap();
                log::info!("Finished loading {texture_path:?}");
                Some(texture)
            } else {
                None
            };
            (obj_material, texture, normal_texture)
        });
        tasks.push(task);
    }

    let mut materials: Vec<ObjMaterial> = Vec::new();
    for task in tasks {
        let (obj_material, texture, normal_texture) = task.await;
        materials.push(ObjMaterial {
            name: obj_material.name.clone(),
            base_color: Vec3::from(obj_material.diffuse).extend(obj_material.dissolve),
            diffuse_texture_data: texture,
            alpha: obj_material.dissolve,
            gloss: obj_material.shininess,
            normal_texture_data: normal_texture,
        });
        log::info!(
            "Finished loading {} {:?}",
            obj_material.name,
            obj_material.dissolve
        );
    }

    Ok(LoadedObj {
        models: obj_models,
        materials,
    })
}

fn load_texture(path: PathBuf) -> anyhow::Result<RgbaImage> {
    let mut asset_path = Path::new(ROOT_DIR).to_path_buf();
    asset_path.push(path);
    let data = std::fs::read(asset_path)?;
    let rgba = image::load_from_memory(&data)?.to_rgba8();
    Ok(rgba)
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

    let model = resources::load_model(
        INSTANCED_MODEL_NAME,
        Path::new(&INSTANCED_MODEL_NAME),
        models,
        materials,
        &renderer.device,
        &renderer.queue,
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

    let model = resources::load_model(
        INSTANCED_MODEL_NAME,
        Path::new(&INSTANCED_MODEL_NAME),
        models,
        materials,
        &renderer.device,
        &renderer.queue,
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
