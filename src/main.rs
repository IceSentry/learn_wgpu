use crate::{model::ModelVertex, renderer::TransformRaw};
use bevy::{
    asset::AssetPlugin,
    input::InputPlugin,
    math::{const_vec3, vec3},
    prelude::*,
    window::{WindowPlugin, WindowResized},
    winit::{WinitPlugin, WinitWindows},
};
use camera::{Camera, CameraUniform};
use depth_pass::DepthPass;
use futures_lite::future;
use instances::Instances;
use light::Light;
use model::Model;
use obj_loader::{LoadedObj, ObjLoaderPlugin};
use render_phase_3d::{ClearColor, DepthTexture, LightBindGroup, RenderPhase3d};
use renderer::{Pipeline, Transform, WgpuRenderer};
use std::path::Path;
use texture::Texture;
use winit::dpi::PhysicalSize;

mod camera;
mod depth_pass;
mod instances;
mod light;
mod mesh;
mod model;
mod obj_loader;
mod render_phase_3d;
mod renderer;
mod resources;
mod shapes;
mod texture;

const NUM_INSTANCES_PER_ROW: u32 = 10;
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

// TODO figure out how to draw lines
// TODO extract to plugin
// TODO create buffers and bind groups when needed every frane

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
            ..default()
        })
        .add_plugins(MinimalPlugins)
        .add_plugin(WindowPlugin::default())
        .add_plugin(WinitPlugin)
        .add_plugin(InputPlugin::default())
        .add_plugin(AssetPlugin)
        .add_plugin(ObjLoaderPlugin)
        .add_plugin(camera::CameraPlugin)
        .add_startup_system_to_stage(StartupStage::PreStartup, init_renderer)
        .add_startup_system(setup)
        .add_startup_system(init_depth_pass)
        .add_startup_system(spawn_light)
        .add_startup_system(load_obj_asset)
        .add_startup_system_to_stage(
            StartupStage::PostStartup,
            init_render_phase.exclusive_system(),
        )
        .add_system(render.exclusive_system())
        .add_system(resize)
        .add_system(update_window_title)
        .add_system(update_show_depth)
        .add_system(update_light)
        .add_system(handle_instanced_obj_loaded)
        .add_system(handle_obj_loaded)
        // .add_system(cursor_moved)
        .add_system(move_instances)
        .add_system(instances::update_instance_buffer)
        .add_system(instances::create_instance_buffer)
        .run();
}

#[derive(Component)]
struct LightBuffer(wgpu::Buffer);

pub struct ShowDepthBuffer(bool);

fn init_renderer(
    mut commands: Commands,
    windows: Res<Windows>,
    winit_windows: NonSendMut<WinitWindows>,
) {
    let window_id = windows.get_primary().expect("bevy window not found").id();

    let winit_window = winit_windows
        .get_window(window_id)
        .expect("winit window not found");

    let renderer = future::block_on(WgpuRenderer::new(winit_window));
    commands.insert_resource(renderer);
}

fn setup(mut commands: Commands, renderer: Res<WgpuRenderer>) {
    let render_pipeline_layout =
        renderer
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &texture::bind_group_layout(&renderer.device),
                    &camera::bind_group_layout(&renderer.device),
                    &Light::bind_group_layout(&renderer.device),
                ],
                push_constant_ranges: &[],
            });

    let render_pipeline = renderer.create_render_pipeline(
        "Render Pipeline",
        wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
        },
        &render_pipeline_layout,
        &[model::ModelVertex::layout(), TransformRaw::layout()],
        Some(wgpu::DepthStencilState {
            format: Texture::DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
    );

    let light_render_pipeline = {
        let layout = renderer
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Light Pipeline Layout"),
                bind_group_layouts: &[
                    &camera::bind_group_layout(&renderer.device),
                    &Light::bind_group_layout(&renderer.device),
                ],
                push_constant_ranges: &[],
            });
        let shader = wgpu::ShaderModuleDescriptor {
            label: Some("Light Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light.wgsl").into()),
        };
        renderer.create_render_pipeline(
            "Light Render Pipeline",
            shader,
            &layout,
            &[ModelVertex::layout()],
            Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
        )
    };

    let pipeline = Pipeline {
        render_pipeline,
        light_pipeline: light_render_pipeline,
    };

    commands.insert_resource(pipeline);
    commands.insert_resource(ShowDepthBuffer(false));
    commands.insert_resource(ClearColor(Color::rgba(0.1, 0.2, 0.3, 1.0)));
}

fn init_render_phase(world: &mut World) {
    let render_phase_3d = RenderPhase3d::from_world(world);
    world.insert_resource(render_phase_3d);
}

fn init_depth_pass(mut commands: Commands, renderer: Res<WgpuRenderer>) {
    let depth_texture = Texture::create_depth_texture(&renderer.device, &renderer.config);
    let depth_pass = DepthPass::new(&renderer, &depth_texture);
    commands.insert_resource(DepthTexture(depth_texture));
    commands.insert_resource(depth_pass);
}

fn spawn_light(mut commands: Commands, renderer: Res<WgpuRenderer>) {
    let cube = shapes::Cube::new(1.0, 1.0, 1.0);
    let mesh = cube.mesh(&renderer.device);
    let model = Model {
        meshes: vec![mesh],
        materials: vec![],
    };

    let light = Light::new(LIGHT_POSITION, Color::WHITE);
    let (light_bind_group, light_buffer) = light.bind_group(&renderer.device);

    commands
        .spawn()
        .insert(light)
        .insert(model)
        .insert(LightBuffer(light_buffer));
    commands.insert_resource(LightBindGroup(light_bind_group));
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
        &texture::bind_group_layout(&renderer.device),
    )
    .expect("failed to load model from obj");

    commands.spawn().insert(model).insert(Transform {
        rotation: Quat::default(),
        translation: Vec3::ZERO,
        scale: SCALE,
    });
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
        &texture::bind_group_layout(&renderer.device),
    )
    .expect("failed to load model from obj");

    let mut instances = Vec::new();
    for z in 0..NUM_INSTANCES_PER_ROW {
        for x in 0..NUM_INSTANCES_PER_ROW {
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

pub fn render(world: &mut World) {
    world.resource_scope(|world, renderer: Mut<WgpuRenderer>| {
        if let Err(e) = renderer.render(world) {
            log::error!("{e:?}")
        };
    });
}

#[allow(clippy::too_many_arguments)]
fn resize(
    mut renderer: ResMut<WgpuRenderer>,
    mut events: EventReader<WindowResized>,
    windows: Res<Windows>,
    mut depth_pass: ResMut<DepthPass>,
    mut depth_texture: ResMut<DepthTexture>,
    mut camera_uniform: ResMut<CameraUniform>,
    mut camera: ResMut<Camera>,
) {
    for event in events.iter() {
        let window = windows.get(event.id).expect("window not found");
        let width = window.physical_width();
        let height = window.physical_height();

        camera.aspect = width as f32 / height as f32;
        camera_uniform.update_view_proj(&camera);

        renderer.resize(PhysicalSize { width, height });

        depth_texture.0 = Texture::create_depth_texture(&renderer.device, &renderer.config);
        depth_pass.resize(&renderer.device, &depth_texture.0);
    }
}

fn update_window_title(time: Res<Time>, mut windows: ResMut<Windows>) {
    if let Some(window) = windows.get_primary_mut() {
        window.set_title(format!("dt: {}ms", time.delta().as_millis()));
    }
}

fn update_show_depth(keyboard_input: Res<Input<KeyCode>>, mut draw_depth: ResMut<ShowDepthBuffer>) {
    if keyboard_input.just_pressed(KeyCode::X) {
        draw_depth.0 = !draw_depth.0;
    }
}

fn update_light(
    renderer: Res<WgpuRenderer>,
    mut query: Query<(&mut Light, &LightBuffer)>,
    time: Res<Time>,
) {
    for (mut light, light_buffer) in query.iter_mut() {
        let old_position = light.position;
        light.position =
            Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_2 * time.delta_seconds())
                .mul_vec3(old_position.into())
                .to_array();

        renderer
            .queue
            .write_buffer(&light_buffer.0, 0, bytemuck::cast_slice(&[*light]));
    }
}

#[allow(unused)]
fn cursor_moved(
    renderer: Res<WgpuRenderer>,
    mut events: EventReader<CursorMoved>,
    mut clear_color: ResMut<ClearColor>,
) {
    for event in events.iter() {
        clear_color.0 = Color::rgb(
            event.position.x as f32 / renderer.size.width as f32,
            event.position.y as f32 / renderer.size.height as f32,
            clear_color.0.b(),
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
