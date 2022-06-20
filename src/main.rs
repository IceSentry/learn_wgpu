use std::path::Path;

use bevy::{
    app::AppExit,
    asset::AssetPlugin,
    input::{Input, InputPlugin},
    math::{const_vec3, vec3, Quat, Vec3},
    prelude::*,
    window::{CursorMoved, WindowDescriptor, WindowPlugin, Windows},
    winit::WinitPlugin,
    MinimalPlugins,
};
use texture::Texture;

use crate::{
    egui_plugin::EguiPlugin,
    instances::Instances,
    light::Light,
    model::Model,
    obj_loader::{LoadedObj, ObjLoaderPlugin},
    renderer::plugin::WgpuRendererPlugin,
    renderer::render_phase_3d::RenderPhase3dDescriptor,
    renderer::WgpuRenderer,
    transform::Transform,
};

mod camera;
mod egui_plugin;
mod instances;
mod light;
mod mesh;
mod model;
mod obj_loader;
mod renderer;
mod resources;
mod shapes;
mod texture;
mod transform;

const NUM_INSTANCES_PER_ROW: u32 = 6;
const SPACE_BETWEEN: f32 = 3.0;
const LIGHT_POSITION: Vec3 = const_vec3!([5.0, 3.0, 0.0]);

// const MODEL_NAME: &str = "teapot/teapot.obj";
const MODEL_NAME: &str = "large_obj/sponza_obj/sponza.obj";
// const MODEL_NAME: &str = "large_obj/bistro/Exterior/exterior.obj";
const SCALE: Vec3 = const_vec3!([0.05, 0.05, 0.05]);
// const MODEL_NAME: &str = "bunny.obj";
// const SCALE: Vec3 = const_vec3!([1.5, 1.5, 1.5]);
const INSTANCED_MODEL_NAME: &str = "cube/cube.obj";
const INSTANCED_SCALE: Vec3 = const_vec3!([1.0, 1.0, 1.0]);

// TODO figure out MSAA
// TODO figure out how to draw lines and use it to draw wireframes
// TODO use LogPlugin
// TODO setup traces for renderer

struct LightSettings {
    rotate: bool,
    color: [f32; 3],
    speed: f32,
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .filter_module("wgpu_hal", log::LevelFilter::Error)
        .filter_module("wgpu_core", log::LevelFilter::Error)
        .init();

    App::new()
        .insert_resource(WindowDescriptor {
            // width: 800.0,
            // height: 600.0,
            // mode: WindowMode::Fullscreen,
            ..default()
        })
        .insert_resource(RenderPhase3dDescriptor {
            clear_color: Color::rgba(0.1, 0.1, 0.1, 1.0),
            ..default()
        })
        .insert_resource(LightSettings {
            rotate: true,
            color: [1.0, 1.0, 1.0],
            speed: 0.25,
        })
        .add_plugins(MinimalPlugins)
        .add_plugin(WindowPlugin::default())
        .add_plugin(WinitPlugin)
        .add_plugin(WgpuRendererPlugin)
        .add_plugin(InputPlugin::default())
        .add_plugin(AssetPlugin)
        .add_plugin(ObjLoaderPlugin)
        .add_plugin(EguiPlugin)
        .add_startup_system(spawn_light)
        .add_startup_system(spawn_shapes)
        .add_startup_system(load_obj_asset)
        .add_system(update_window_title)
        .add_system(update_show_depth)
        .add_system(handle_instanced_obj_loaded)
        .add_system(handle_obj_loaded)
        // .add_system(cursor_moved)
        .add_system(move_instances)
        .add_system(update_light)
        .add_system(exit_on_esc)
        .add_system(settings_ui)
        .run();
}

fn spawn_light(mut commands: Commands, renderer: Res<WgpuRenderer>) {
    let cube = shapes::cube::Cube::new(1.0, 1.0, 1.0);
    let mesh = cube.mesh(&renderer.device);
    let model = Model {
        meshes: vec![mesh],
        materials: vec![],
    };

    let light = Light {
        position: LIGHT_POSITION,
        color: Color::WHITE.as_rgba_f32().into(),
    };

    commands.spawn().insert(light).insert(model);
}

fn spawn_shapes(mut commands: Commands, renderer: Res<WgpuRenderer>) {
    let plane = Model {
        meshes: vec![shapes::plane::Plane {
            resolution: 1,
            size: 5.0,
        }
        .mesh(&renderer.device)],
        materials: vec![get_default_material(&renderer, Color::GREEN)],
    };
    commands.spawn_bundle((
        plane,
        Transform {
            translation: Vec3::new(-2.5, -1.0, -2.5),
            ..default()
        },
    ));

    let cube = Model {
        meshes: vec![shapes::cube::Cube::new(1.0, 1.0, 1.0).mesh(&renderer.device)],
        materials: vec![get_default_material(&renderer, Color::WHITE)],
    };
    commands.spawn_bundle((
        cube,
        Transform {
            translation: Vec3::ZERO - (Vec3::X * 1.5),
            ..default()
        },
    ));

    let sphere = Model {
        meshes: vec![shapes::sphere::UVSphere::default().mesh(&renderer.device)],
        materials: vec![get_default_material(&renderer, Color::WHITE)],
    };
    commands.spawn_bundle((
        sphere,
        Transform {
            translation: Vec3::ZERO,
            ..default()
        },
    ));

    let capsule = Model {
        meshes: vec![shapes::capsule::Capsule::default().mesh(&renderer.device)],
        materials: vec![get_default_material(&renderer, Color::WHITE)],
    };
    commands.spawn_bundle((
        capsule,
        Transform {
            translation: Vec3::ZERO + (Vec3::X * 1.5),
            ..default()
        },
    ));
}

fn get_default_material(renderer: &WgpuRenderer, base_color: Color) -> model::Material {
    let default_texture = Texture::from_bytes(
        &renderer.device,
        &renderer.queue,
        &std::fs::read("assets/white.png").expect("failed to load white.png"),
        "default_texture",
    )
    .expect("Failed to load texture");
    model::Material::new(
        renderer,
        "default_material",
        default_texture,
        base_color.as_rgba_f32().into(),
        1.0,
    )
}

fn load_obj_asset(asset_server: Res<AssetServer>) {
    let _: Handle<LoadedObj> = asset_server.load(INSTANCED_MODEL_NAME);
    let _: Handle<LoadedObj> = asset_server.load(MODEL_NAME);
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

            let translation = vec3(x as f32, 0.0, z as f32);
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

fn update_window_title(time: Res<Time>, mut windows: ResMut<Windows>) {
    if let Some(window) = windows.get_primary_mut() {
        window.set_title(format!("dt: {}ms", time.delta().as_millis()));
    }
}

fn update_show_depth(
    keyboard_input: Res<Input<KeyCode>>,
    mut descriptor: ResMut<RenderPhase3dDescriptor>,
) {
    if keyboard_input.just_pressed(KeyCode::X) {
        descriptor.show_depth_buffer = !descriptor.show_depth_buffer;
    }
}

#[allow(unused)]
fn cursor_moved(
    renderer: Res<WgpuRenderer>,
    mut events: EventReader<CursorMoved>,
    mut descriptor: ResMut<RenderPhase3dDescriptor>,
) {
    for event in events.iter() {
        descriptor.clear_color = Color::rgb(
            event.position.x as f32 / renderer.size.width as f32,
            event.position.y as f32 / renderer.size.height as f32,
            descriptor.clear_color.b(),
        );
    }
}

fn move_instances(time: Res<Time>, mut query: Query<(&mut Instances, &mut Wave)>) {
    for (mut instances, mut wave) in query.iter_mut() {
        wave.offset += time.delta_seconds() * wave.frequency;
        for instance in instances.0.iter_mut() {
            instance.translation.y =
                wave.wave_height(instance.translation.x, instance.translation.z);
        }
    }
}

#[derive(Component)]
pub struct Wave {
    pub amplitude: f32,
    pub wavelength: f32,
    pub frequency: f32,
    pub offset: f32,
}

impl Default for Wave {
    fn default() -> Self {
        Self {
            amplitude: 1.0,
            wavelength: 10.0,
            frequency: 2.0,
            offset: 0.0,
        }
    }
}

impl Wave {
    pub fn wave_height(&self, x: f32, z: f32) -> f32 {
        // Wave number
        let k = std::f32::consts::TAU / self.wavelength;
        let r = (x * x + z * z).sqrt();
        self.amplitude * (k * (r - self.offset)).sin()
    }
}

fn exit_on_esc(key_input: Res<Input<KeyCode>>, mut exit_events: EventWriter<AppExit>) {
    if key_input.just_pressed(KeyCode::Escape) {
        exit_events.send_default();
    }
}

fn update_light(mut query: Query<&mut Light>, time: Res<Time>, settings: Res<LightSettings>) {
    if !settings.rotate {
        return;
    }
    for mut light in query.iter_mut() {
        let old_position = light.position;
        light.position = Quat::from_axis_angle(
            Vec3::Y,
            std::f32::consts::TAU * time.delta_seconds() * settings.speed,
        )
        .mul_vec3(old_position);
        light.color = settings.color.into();
    }
}

fn settings_ui(ctx: Res<egui::Context>, mut light_settings: ResMut<LightSettings>) {
    egui::Window::new("Settings")
        .resizable(true)
        .collapsible(true)
        .show(&ctx, |ui| {
            ui.heading("Light Settings");
            ui.checkbox(&mut light_settings.rotate, "Rotate");
            ui.label("Speed");
            ui.add(egui::Slider::new(&mut light_settings.speed, 0.0..=2.0).step_by(0.05));
            ui.label("Color");
            ui.color_edit_button_rgb(&mut light_settings.color);
        });
}
