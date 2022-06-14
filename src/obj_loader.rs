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

const ROOT_DIR: &str = "assets\\";

pub struct ObjLoaderPlugin;

impl Plugin for ObjLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_asset::<LoadedObj>()
            .init_asset_loader::<ObjLoader>();
    }
}

#[derive(Default)]
pub struct ObjLoader;

#[derive(Debug)]
pub struct ObjMaterial {
    pub name: String,
    pub diffuse_texture_data: RgbaImage,
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

    let obj_cursor = Cursor::new(bytes);
    let mut obj_reader = BufReader::new(obj_cursor);

    let (obj_models, obj_materials) = tobj::load_obj_buf_async(
        &mut obj_reader,
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
    let path = load_context.path().to_path_buf();

    for obj_material in obj_materials.clone().expect("Failed to load materials") {
        let path = path.clone();
        let task = pool.spawn(async move {
            let mut texture_path = path.parent().expect("no parent").to_path_buf();
            if obj_material.diffuse_texture.is_empty() {
                // default texture
                texture_path.push("pink.png");
            } else {
                texture_path.push(obj_material.diffuse_texture.clone());
            }
            let texture = load_texture(texture_path.clone()).expect("failed to load texture");
            log::info!("Finished loading {texture_path:?}");
            (obj_material, texture)
        });
        tasks.push(task);
    }

    let mut materials: Vec<ObjMaterial> = Vec::new();
    for task in tasks {
        let (obj_material, texture) = task.await;
        materials.push(ObjMaterial {
            name: obj_material.name.clone(),
            diffuse_texture_data: texture,
        });
        log::info!("Finished loading {}", obj_material.name);
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
