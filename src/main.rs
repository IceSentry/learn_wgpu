use bevy::{
    input::InputPlugin,
    math::vec3,
    prelude::*,
    window::{WindowPlugin, WindowResized},
    winit::{WinitPlugin, WinitWindows},
};
use camera::{Camera, CameraController, CameraUniform};
use depth_pass::DepthPass;
use light::LightUniform;
use model::Model;
use renderer::{Instance, Pipeline, WgpuRenderer};
use texture::Texture;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;

use crate::{
    model::{ModelVertex, Vertex},
    renderer::InstanceRaw,
};

mod camera;
mod depth_pass;
mod light;
mod model;
mod renderer;
mod resources;
mod texture;

const NUM_INSTANCES_PER_ROW: u32 = 10;
const SPACE_BETWEEN: f32 = 3.0;

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .filter_module("wgpu_hal", log::LevelFilter::Error)
        .filter_module("wgpu_core", log::LevelFilter::Error)
        .init();

    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugin(WindowPlugin::default())
        .add_plugin(WinitPlugin)
        .add_plugin(InputPlugin::default())
        .add_startup_system(setup)
        .add_system(resize)
        .add_system(render)
        .add_system(cursor_moved)
        .add_system(update_window_title)
        .add_system(update_camera)
        .add_system(move_instances)
        .add_system(update_show_depth)
        .add_system(update_light)
        .add_system(bevy::input::system::exit_on_esc_system)
        .run();
}

struct CameraBuffer(wgpu::Buffer);
struct LightBuffer(wgpu::Buffer);
struct Instances(Vec<Instance>);
struct ShowDepthBuffer(bool);

fn setup(mut commands: Commands, winit_windows: NonSendMut<WinitWindows>, windows: Res<Windows>) {
    let bevy_window = windows.get_primary().expect("bevy window not found");
    let winit_window = winit_windows
        .get_window(bevy_window.id())
        .expect("winit window not found");

    let renderer = futures::executor::block_on(WgpuRenderer::new(winit_window));

    let texture = Texture::from_bytes(
        &renderer.device,
        &renderer.queue,
        &resources::load_bytes("happy-tree.png").expect("failed to load texture"),
        "happy-tree.png",
    )
    .expect("failed to create texture");

    let width = renderer.config.width as f32;
    let height = renderer.config.height as f32;
    let camera = Camera {
        eye: vec3(0.0, 1.0, 2.0),
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

    let mut instances: Vec<_> = Vec::new();
    for z in 0..NUM_INSTANCES_PER_ROW {
        for x in 0..NUM_INSTANCES_PER_ROW {
            let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
            let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

            let translation = vec3(x as f32, 0.0, z as f32);
            let rotation = if translation == Vec3::ZERO {
                Quat::from_axis_angle(Vec3::Z, 0.0)
            } else {
                Quat::from_axis_angle(translation.normalize(), std::f32::consts::FRAC_PI_4)
            };

            instances.push(Instance {
                rotation,
                translation,
            });
        }
    }

    let instance_data: Vec<_> = instances.iter().map(Instance::to_raw).collect();

    let light_uniform = LightUniform::new(vec3(2.0, 4.0, 2.0), Color::WHITE);

    let light_buffer = renderer
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light VB"),
            contents: bytemuck::cast_slice(&[light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

    let light_bind_group_layout =
        renderer
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: None,
            });

    let light_bind_group = renderer
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &light_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            }],
            label: None,
        });

    let (texture_bind_group_layout, texture_bind_group) =
        renderer.create_texture_bind_group(&texture, 0, "diffuse_bind_group");

    let (camera_bind_group_layout, camera_bind_group) =
        renderer.create_camera_bind_group(&camera_buffer);

    let render_pipeline_layout =
        renderer
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &camera_bind_group_layout,
                    &light_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

    let obj_model = futures::executor::block_on(resources::load_model(
        "cube.obj",
        &renderer.device,
        &renderer.queue,
        &texture_bind_group_layout,
    ))
    .expect("failed to load obj");

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

    let instance_buffer = renderer
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

    let light_render_pipeline = {
        let layout = renderer
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Light Pipeline Layout"),
                bind_group_layouts: &[&camera_bind_group_layout, &light_bind_group_layout],
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
        instance_buffer,
        texture_bind_group,
        texture_bind_group_layout,
        camera_bind_group,
        light_bind_group,
        light_pipeline: light_render_pipeline,
    };

    let depth_pass = DepthPass::new(&renderer);

    commands.insert_resource(renderer);
    commands.insert_resource(pipeline);
    commands.insert_resource(camera);
    commands.insert_resource(CameraController::new(0.05));
    commands.insert_resource(camera_uniform);
    commands.insert_resource(CameraBuffer(camera_buffer));
    commands.insert_resource(Instances(instances));
    commands.insert_resource(depth_pass);
    commands.insert_resource(ShowDepthBuffer(false));
    commands.insert_resource(obj_model);
    commands.insert_resource(light_uniform);
    commands.insert_resource(LightBuffer(light_buffer));
}

fn resize(
    mut renderer: ResMut<WgpuRenderer>,
    mut events: EventReader<WindowResized>,
    windows: Res<Windows>,
    mut depth_pass: ResMut<DepthPass>,
) {
    for event in events.iter() {
        let window = windows.get(event.id).expect("window not found");
        renderer.resize(PhysicalSize {
            width: window.physical_width(),
            height: window.physical_height(),
        });
        depth_pass.resize(&renderer.device, &renderer.config);
    }
}

fn render(
    mut renderer: ResMut<WgpuRenderer>,
    pipeline: Res<Pipeline>,
    instances: Res<Instances>,
    depth_pass: Res<DepthPass>,
    show_depth_buffer: Res<ShowDepthBuffer>,
    obj_model: Res<Model>,
) {
    match renderer.render(
        &pipeline,
        instances.0.len() as u32,
        &depth_pass,
        show_depth_buffer.0,
        &obj_model,
    ) {
        Ok(_) => {}
        Err(e) => log::error!("{:?}", e),
    }
}

fn cursor_moved(mut renderer: ResMut<WgpuRenderer>, mut events: EventReader<CursorMoved>) {
    for event in events.iter() {
        renderer.clear_color = wgpu::Color {
            r: event.position.x as f64 / renderer.size.width as f64,
            g: event.position.y as f64 / renderer.size.height as f64,
            ..renderer.clear_color
        };
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
    renderer: Res<WgpuRenderer>,
    mut camera: ResMut<Camera>,
    mut camera_uniform: ResMut<CameraUniform>,
    camera_buffer: Res<CameraBuffer>,
) {
    camera_controller.is_forward_pressed = keyboard_input.pressed(KeyCode::W);
    camera_controller.is_left_pressed = keyboard_input.pressed(KeyCode::A);
    camera_controller.is_backward_pressed = keyboard_input.pressed(KeyCode::S);
    camera_controller.is_right_pressed = keyboard_input.pressed(KeyCode::D);

    camera_controller.update_camera(&mut camera);

    camera_uniform.update_view_proj(&camera);

    renderer.queue.write_buffer(
        &camera_buffer.0,
        0,
        bytemuck::cast_slice(&[*camera_uniform]),
    );
}

fn update_light(
    renderer: Res<WgpuRenderer>,
    mut light_uniform: ResMut<LightUniform>,
    light_buffer: Res<LightBuffer>,
) {
    let old_position = light_uniform.position;
    light_uniform.position = Quat::from_axis_angle(Vec3::Y, 1.0_f32.to_radians())
        .mul_vec3(old_position.into())
        .to_array();
    renderer
        .queue
        .write_buffer(&light_buffer.0, 0, bytemuck::cast_slice(&[*light_uniform]));
}

fn move_instances(
    renderer: Res<WgpuRenderer>,
    pipeline: Res<Pipeline>,
    mut instances: ResMut<Instances>,
    time: Res<Time>,
    mut wave: Local<Wave>,
) {
    wave.offset += time.delta_seconds() * wave.frequency;

    for instance in instances.0.iter_mut() {
        instance.translation.y = wave.wave_height(instance.translation.x, instance.translation.z);
    }

    let data: Vec<_> = instances.0.iter().map(Instance::to_raw).collect();
    renderer.queue.write_buffer(
        &pipeline.instance_buffer,
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
