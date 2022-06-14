use bevy::{
    asset::{AssetLoader, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    utils::Instant,
};
use std::{
    io::{BufReader, Cursor},
    path::Path,
};

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
    pub diffuse_texture_data: Vec<u8>,
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
        let start = Instant::now();
        Box::pin(async move {
            let path = load_context.path();
            log::info!("Loading {path:?}");
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
                    tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(material_file_data)))
                },
            )
            .await
            .unwrap_or_else(|_| panic!("Failed to load {path:?}"));

            let mut materials = Vec::new();
            for mat in obj_materials.expect("Failed to load materials") {
                log::info!("loading {} {}", mat.name, mat.diffuse_texture);
                if mat.diffuse_texture.is_empty() {
                    // TODO only load handles
                    // let path = AssetPath::new_ref(load_context.path(), Some(&label));
                    // load_context.get_handle(id)
                    // load_context.set_labeled_asset(LoadedAsset::new(material));
                    let data = load_context
                        .asset_io()
                        .load_path(Path::new("pink.png"))
                        .await
                        .unwrap_or_else(|_| panic!("Failed to load {path:?}"));

                    materials.push(ObjMaterial {
                        name: mat.name,
                        diffuse_texture_data: data,
                    });
                    continue;
                }

                let mut path = path.parent().expect("no parent").to_path_buf();
                path.push(mat.diffuse_texture.clone());

                let data = asset_io
                    .load_path(&path)
                    .await
                    .unwrap_or_else(|_| panic!("Failed to load {path:?}"));

                log::info!("finished loading {} {}", mat.name, mat.diffuse_texture);

                materials.push(ObjMaterial {
                    name: mat.name,
                    diffuse_texture_data: data,
                });
            }

            let obj = LoadedObj {
                models: obj_models,
                materials,
            };
            log::info!(
                "Loading {:?} took {}ms",
                path.to_path_buf(),
                (Instant::now() - start).as_millis(),
            );
            load_context.set_default_asset(LoadedAsset::new(obj));

            Ok(())
        })
    }
}
