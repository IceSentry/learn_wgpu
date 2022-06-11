use bevy::{
    input::InputPlugin,
    math::{const_vec3, vec3},
    prelude::*,
    window::{WindowPlugin, WindowResized},
    winit::{WinitPlugin, WinitWindows},
};
use camera::{Camera, CameraController, CameraUniform};
use depth_pass::DepthPass;
use light::Light;
use model::Model;
use render_phase::{
    ClearColor, DepthTexture, InstanceBuffer, InstanceCount, LightBindGroup, RenderPhase3d,
};
use renderer::{Instance, Pipeline, WgpuRenderer};
use texture::Texture;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;

use crate::{model::ModelVertex, renderer::InstanceRaw};

mod camera;
mod depth_pass;
mod light;
mod model;
mod render_phase;
mod renderer;
mod resources;
mod shapes;
mod texture;

const NUM_INSTANCES_PER_ROW: u32 = 1;
const SPACE_BETWEEN: f32 = 3.0;
const LIGHT_POSITION: Vec3 = const_vec3!([5.0, 3.0, 0.0]);
const CAMERRA_EYE: Vec3 = const_vec3!([0.0, 5.0, 8.0]);

const MODEL_NAME: &str = "teapot.obj";
const SCALE: Vec3 = const_vec3!([0.05, 0.05, 0.05]);
// const MODEL_NAME: &str = "bunny.obj";
// const SCALE: Vec3 = const_vec3!([1.5, 1.5, 1.5]);

// TODO figure out how to draw lines
// TODO draw normals
// TODO better camera

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .filter_module("wgpu_hal", log::LevelFilter::Error)
        .filter_module("wgpu_core", log::LevelFilter::Error)
        .init();

    App::new()
        .insert_resource(WindowDescriptor {
            width: 800.0,
            height: 600.0,
            ..default()
        })
        .add_plugins(MinimalPlugins)
        .add_plugin(WindowPlugin::default())
        .add_plugin(WinitPlugin)
        .add_plugin(InputPlugin::default())
        .add_startup_system(setup.exclusive_system())
        .add_startup_system_to_stage(StartupStage::PostStartup, spawn_instances)
        .add_startup_system_to_stage(StartupStage::PostStartup, spawn_light)
        .add_startup_system_to_stage(StartupStage::PostStartup, init_depth_pass)
        .add_system(render.exclusive_system())
        .add_system(resize)
        // .add_system(cursor_moved)
        .add_system(update_window_title)
        .add_system(update_camera)
        .add_system(move_instances)
        .add_system(update_show_depth)
        .add_system(update_light)
        .add_system(update_camera_buffer)
        .add_system(bevy::input::system::exit_on_esc_system)
        .run();
}

struct CameraBuffer(wgpu::Buffer);
#[derive(Component)]
struct LightBuffer(wgpu::Buffer);
struct Instances(Vec<Instance>);
pub struct ShowDepthBuffer(bool);

fn setup(world: &mut World) {
    let window_id = {
        let windows = world.resource::<Windows>();
        windows.get_primary().expect("bevy window not found").id()
    };

    let winit_windows = world.non_send_resource_mut::<WinitWindows>();
    let winit_window = winit_windows
        .get_window(window_id)
        .expect("winit window not found");

    let renderer = futures::executor::block_on(WgpuRenderer::new(winit_window));

    let width = renderer.config.width as f32;
    let height = renderer.config.height as f32;
    let camera = Camera {
        eye: CAMERRA_EYE,
        target: vec3(0.0, 0.0, 0.0),
        up: Vec3::Y,
        aspect: width / height,
        fov_y: 45.0,
        z_near: 0.1,
        z_far: 100.0,
    };

    let mut camera_uniform = CameraUniform::new();
    camera_uniform.update_view_proj(&camera);

    let camera_buffer = renderer
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

    let camera_bind_group = camera::create_camera_bind_group(&renderer.device, &camera_buffer);

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
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        },
        &render_pipeline_layout,
        &[model::ModelVertex::layout(), InstanceRaw::layout()],
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
            source: wgpu::ShaderSource::Wgsl(include_str!("light.wgsl").into()),
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
        camera_bind_group,
        light_pipeline: light_render_pipeline,
    };

    let render_phase_3d = RenderPhase3d::from_world(world);

    world.insert_resource(renderer);
    world.insert_resource(pipeline);
    world.insert_resource(camera);
    world.insert_resource(CameraController::new(0.05));
    world.insert_resource(camera_uniform);
    world.insert_resource(CameraBuffer(camera_buffer));
    world.insert_resource(ShowDepthBuffer(false));
    world.insert_resource(render_phase_3d);
    world.insert_resource(ClearColor(Color::rgba(0.1, 0.2, 0.3, 1.0)));
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

fn spawn_instances(mut commands: Commands, renderer: Res<WgpuRenderer>) {
    // TODO consider using bevy assets to load stuff
    let obj_model = futures::executor::block_on(resources::load_model(
        MODEL_NAME,
        &renderer.device,
        &renderer.queue,
        &texture::bind_group_layout(&renderer.device),
    ))
    .expect("failed to load obj");

    let mut instances: Vec<_> = Vec::new();
    for z in 0..NUM_INSTANCES_PER_ROW {
        for x in 0..NUM_INSTANCES_PER_ROW {
            // let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
            // let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

            let translation = vec3(x as f32, 0.0, z as f32);
            let rotation = if translation == Vec3::ZERO {
                Quat::from_axis_angle(Vec3::Y, 0.0)
            } else {
                Quat::from_axis_angle(translation.normalize(), std::f32::consts::FRAC_PI_4)
            };

            instances.push(Instance {
                rotation,
                translation,
                scale: SCALE,
            });
        }
    }

    let instance_data: Vec<_> = instances.iter().map(Instance::to_raw).collect();

    let instance_buffer = renderer
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

    commands
        .spawn()
        .insert(obj_model)
        .insert(InstanceCount(instances.len()))
        .insert(InstanceBuffer(instance_buffer));
    commands.insert_resource(Instances(instances));
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

fn update_window_title(time: Res<Time>, mut windows: ResMut<Windows>) {
    let window = windows.get_primary_mut().unwrap();
    window.set_title(format!("dt: {}ms", time.delta().as_millis()));
}

fn update_show_depth(keyboard_input: Res<Input<KeyCode>>, mut draw_depth: ResMut<ShowDepthBuffer>) {
    if keyboard_input.just_pressed(KeyCode::X) {
        draw_depth.0 = !draw_depth.0;
    }
}

fn update_camera(
    mut camera_controller: ResMut<CameraController>,
    keyboard_input: Res<Input<KeyCode>>,
    mut camera: ResMut<Camera>,
    mut camera_uniform: ResMut<CameraUniform>,
) {
    camera_controller.is_forward_pressed = keyboard_input.pressed(KeyCode::W);
    camera_controller.is_left_pressed = keyboard_input.pressed(KeyCode::A);
    camera_controller.is_backward_pressed = keyboard_input.pressed(KeyCode::S);
    camera_controller.is_right_pressed = keyboard_input.pressed(KeyCode::D);

    camera_controller.update_camera(&mut camera);

    camera_uniform.update_view_proj(&camera);
}

fn update_camera_buffer(
    renderer: Res<WgpuRenderer>,
    camera_buffer: Res<CameraBuffer>,
    camera_uniform: Res<CameraUniform>,
) {
    if camera_uniform.is_changed() {
        renderer.queue.write_buffer(
            &camera_buffer.0,
            0,
            bytemuck::cast_slice(&[*camera_uniform]),
        );
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

fn move_instances(
    renderer: Res<WgpuRenderer>,
    mut instances: ResMut<Instances>,
    time: Res<Time>,
    mut wave: Local<Wave>,
    instance_buffer: Query<&InstanceBuffer>,
) {
    wave.offset += time.delta_seconds() * wave.frequency;

    for instance in instances.0.iter_mut() {
        instance.translation.y = wave.wave_height(instance.translation.x, instance.translation.z);
    }

    let data: Vec<_> = instances.0.iter().map(Instance::to_raw).collect();
    renderer.queue.write_buffer(
        &instance_buffer.single().0,
        0,
        bytemuck::cast_slice(&data[..]),
    );
}

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
